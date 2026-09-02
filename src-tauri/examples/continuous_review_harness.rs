use serde::Serialize;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use viewer_application::{
    review_evidence::HistorySelector,
    review_workspace::{
        ContinuousReviewRepositoryPort, ContinuousReviewRepositoryProviderPort,
        ContinuousReviewService, MigrationChoice, MigrationPlan, ReviewCommitError,
        ReviewWorkspaceCommand, ReviewWorkspaceContext, TargetEdit,
    },
};
use viewer_domain::review::{FeedbackAnchor, ProductionId, ProductionScope, continuous::*};
use viewer_domain::{
    AssetVersionId, FeedbackId, ReviewCommandId, ReviewStreamId, ReviewTargetId,
    ReviewTargetRevisionId, ReviewTextRevisionId,
};
use viewer_infrastructure::review::{
    ProjectReviewRepositoryProvider, ReviewCommitFaultInjector, ReviewCommitFaultPoint,
};

#[path = "support/continuous_review_fixture.rs"]
mod continuous_review_fixture;
use continuous_review_fixture::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Scenario {
    TwoIterations,
    PartialShared,
    SourceReplaced,
    UnknownBasis,
    RestoreConflict,
    LostReceipt,
    Migration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TwoIterationStep {
    SaveFirst,
    ArchiveFirst,
    SaveSecond,
    ArchiveSecond,
    SaveFinal,
}

impl TwoIterationStep {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "save-first" => Some(Self::SaveFirst),
            "archive-first" => Some(Self::ArchiveFirst),
            "save-second" => Some(Self::SaveSecond),
            "archive-second" => Some(Self::ArchiveSecond),
            "save-final" => Some(Self::SaveFinal),
            _ => None,
        }
    }
}

impl Scenario {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "two_iterations" => Some(Self::TwoIterations),
            "partial_shared" => Some(Self::PartialShared),
            "source_replaced" => Some(Self::SourceReplaced),
            "unknown_basis" => Some(Self::UnknownBasis),
            "restore_conflict" => Some(Self::RestoreConflict),
            "lost_receipt" => Some(Self::LostReceipt),
            "migration" => Some(Self::Migration),
            _ => None,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::TwoIterations => "two_iterations",
            Self::PartialShared => "partial_shared",
            Self::SourceReplaced => "source_replaced",
            Self::UnknownBasis => "unknown_basis",
            Self::RestoreConflict => "restore_conflict",
            Self::LostReceipt => "lost_receipt",
            Self::Migration => "migration",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HarnessChecks {
    current_preserved_later_edit: bool,
    history_preserved_basis_text: bool,
    source_replacement_detected: bool,
    partial_shared_retained: bool,
    snapshot_stayed_fixed: bool,
    unknown_basis_preserved: bool,
    restore_conflict_detected: bool,
    current_text_unchanged: bool,
    retried_receipt_stable: bool,
    current_did_not_roll_back: bool,
    legacy_index_bytes_preserved: bool,
    legacy_round_bytes_preserved: bool,
    legacy_draft_bytes_preserved: bool,
    active_draft_became_current: bool,
    completed_remained_history: bool,
    restore_applied: bool,
    stale_restore_rejected: bool,
    two_distinct_cycles: bool,
    two_source_replacements_detected: bool,
}

impl HarnessChecks {
    const fn empty() -> Self {
        Self {
            current_preserved_later_edit: false,
            history_preserved_basis_text: false,
            source_replacement_detected: false,
            partial_shared_retained: false,
            snapshot_stayed_fixed: false,
            unknown_basis_preserved: false,
            restore_conflict_detected: false,
            current_text_unchanged: false,
            retried_receipt_stable: false,
            current_did_not_roll_back: false,
            legacy_index_bytes_preserved: false,
            legacy_round_bytes_preserved: false,
            legacy_draft_bytes_preserved: false,
            active_draft_became_current: false,
            completed_remained_history: false,
            restore_applied: false,
            stale_restore_rejected: false,
            two_distinct_cycles: false,
            two_source_replacements_detected: false,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HarnessResult {
    scenario: &'static str,
    fixture: &'static str,
    project_id: String,
    stream_id: String,
    basis_snapshot_id: String,
    current_snapshot_id: String,
    archive_id: Option<String>,
    archive_ids: Vec<String>,
    edited_feedback_id: Option<String>,
    shared_feedback_id: Option<String>,
    archived_target_id: Option<String>,
    retained_target_id: Option<String>,
    usage_id: Option<String>,
    legacy_round_id: Option<String>,
    legacy_draft_feedback_id: Option<String>,
    restore_receipt_snapshot_id: Option<String>,
    restore_decision: Option<&'static str>,
    checks: HarnessChecks,
}

impl Composition {
    fn result(
        &self,
        scenario: Scenario,
        fixture: &'static str,
        basis_snapshot_id: viewer_domain::ReviewSnapshotId,
        current_snapshot_id: viewer_domain::ReviewSnapshotId,
        checks: HarnessChecks,
    ) -> HarnessResult {
        HarnessResult {
            scenario: scenario.name(),
            fixture,
            project_id: project_id().to_string(),
            stream_id: self.stream_id.to_string(),
            basis_snapshot_id: basis_snapshot_id.to_string(),
            current_snapshot_id: current_snapshot_id.to_string(),
            archive_id: None,
            archive_ids: vec![],
            edited_feedback_id: None,
            shared_feedback_id: None,
            archived_target_id: None,
            retained_target_id: None,
            usage_id: None,
            legacy_round_id: None,
            legacy_draft_feedback_id: None,
            restore_receipt_snapshot_id: None,
            restore_decision: None,
            checks,
        }
    }
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("continuous review harness failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let (project, scenario, step) = parse_arguments(std::env::args().skip(1))?;
    let project = fs::canonicalize(project)?;
    if !project.is_dir() {
        return Err("project must be a directory".into());
    }
    let result = match scenario {
        Scenario::TwoIterations => match step {
            Some(step) => run_two_iteration_step(&project, scenario, step).await?,
            None => run_two_iterations(&project, scenario).await?,
        },
        Scenario::PartialShared => run_partial_shared(&project, scenario).await?,
        Scenario::SourceReplaced => run_source_replaced(&project, scenario).await?,
        Scenario::UnknownBasis => run_unknown_basis(&project, scenario).await?,
        Scenario::RestoreConflict => run_restore_conflict(&project, scenario).await?,
        Scenario::LostReceipt => run_lost_receipt(&project, scenario).await?,
        Scenario::Migration => run_migration(&project, scenario).await?,
    };
    let encoded = serde_json::to_vec(&result)?;
    if encoded.len() > 64 * 1024 {
        return Err("harness result exceeds 64 KiB".into());
    }
    println!("{}", String::from_utf8(encoded)?);
    Ok(())
}

fn parse_arguments(
    mut arguments: impl Iterator<Item = String>,
) -> Result<(PathBuf, Scenario, Option<TwoIterationStep>), Box<dyn Error>> {
    let mut project = None;
    let mut scenario = None;
    let mut step = None;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--project" if project.is_none() => {
                project = Some(PathBuf::from(
                    arguments.next().ok_or("--project requires a value")?,
                ));
            }
            "--scenario" if scenario.is_none() => {
                let value = arguments.next().ok_or("--scenario requires a value")?;
                scenario = Scenario::parse(&value);
                if scenario.is_none() {
                    return Err("unknown scenario".into());
                }
            }
            "--step" if step.is_none() => {
                let value = arguments.next().ok_or("--step requires a value")?;
                step = TwoIterationStep::parse(&value);
                if step.is_none() {
                    return Err("unknown two-iteration step".into());
                }
            }
            _ => return Err("invalid harness arguments".into()),
        }
    }
    Ok((
        project.ok_or("--project is required")?,
        scenario.ok_or("--scenario is required")?,
        step,
    ))
}

async fn run_two_iteration_step(
    project: &Path,
    scenario: Scenario,
    step: TwoIterationStep,
) -> Result<HarnessResult, Box<dyn Error>> {
    let composition = build_composition(project)?;
    match step {
        TwoIterationStep::SaveFirst => {
            let saved = save_new(&composition, "袖口收紧").await?;
            let feedback_id = saved
                .view
                .current
                .as_ref()
                .and_then(|current| current.state.feedback.first())
                .ok_or("first cycle save omitted feedback")?
                .id;
            let mut result = composition.result(
                scenario,
                "tests/fixtures/images/srgb.jpg",
                saved.receipt.snapshot.snapshot_id,
                saved.receipt.snapshot.snapshot_id,
                HarnessChecks::empty(),
            );
            result.edited_feedback_id = Some(feedback_id.to_string());
            Ok(result)
        }
        TwoIterationStep::ArchiveFirst => {
            let before = composition.service.view(composition.stream_id).await?;
            let basis = before.current.ok_or("first cycle basis is missing")?;
            let feedback = basis
                .state
                .feedback
                .first()
                .ok_or("first cycle feedback is missing")?;
            let key = basis
                .state
                .target_key(feedback.targets[0].id)
                .ok_or("first cycle target key is missing")?;
            let edited = apply(
                &composition.service,
                Some(basis.reference.snapshot_id),
                ReviewWorkspaceCommand::SaveFeedback {
                    feedback_id: Some(feedback.id),
                    text: "袖口收紧，保留褶皱".into(),
                    targets: vec![],
                },
            )
            .await?;
            let selection = ArchiveSelection {
                expected_snapshot_id: edited.receipt.snapshot.snapshot_id,
                groups: vec![ArchiveGroup {
                    basis: ArchiveBasis::Known {
                        snapshot: basis.reference,
                        source: ArchiveBasisSource::UserSelected,
                    },
                    targets: vec![key],
                }],
            };
            let plan = composition
                .service
                .preview_archive(selection.clone())
                .await?;
            if plan.retained.len() != 1 || !plan.removed.is_empty() {
                return Err("first cycle did not retain the later edit".into());
            }
            let prepared = composition
                .service
                .prepare(
                    ReviewCommandId::new(),
                    Some(edited.receipt.snapshot.snapshot_id),
                    ReviewWorkspaceCommand::Archive(selection),
                )
                .await?;
            let archive_id = prepared.generated.archive_id;
            let archived = composition.service.apply(prepared).await?;
            let mut result = composition.result(
                scenario,
                "tests/fixtures/images/srgb.jpg",
                basis.reference.snapshot_id,
                archived.receipt.snapshot.snapshot_id,
                HarnessChecks {
                    current_preserved_later_edit: true,
                    history_preserved_basis_text: true,
                    source_replacement_detected: archived
                        .view
                        .source_checks
                        .iter()
                        .any(|check| check.status == SourceCheckStatus::Changed),
                    ..HarnessChecks::empty()
                },
            );
            result.archive_id = Some(archive_id.to_string());
            result.archive_ids = vec![archive_id.to_string()];
            result.edited_feedback_id = Some(feedback.id.to_string());
            Ok(result)
        }
        TwoIterationStep::SaveSecond => {
            let saved = save_new(&composition, "第二轮待归档意见").await?;
            Ok(composition.result(
                scenario,
                "tests/fixtures/images/p3.jpg",
                saved.receipt.snapshot.snapshot_id,
                saved.receipt.snapshot.snapshot_id,
                HarnessChecks::empty(),
            ))
        }
        TwoIterationStep::ArchiveSecond => {
            let before = composition.service.view(composition.stream_id).await?;
            let current = before.current.ok_or("second cycle current is missing")?;
            let feedback = current
                .state
                .feedback
                .iter()
                .find(|feedback| feedback.text == "第二轮待归档意见")
                .ok_or("second cycle feedback is missing")?;
            let key = current
                .state
                .target_key(feedback.targets[0].id)
                .ok_or("second cycle target key is missing")?;
            let selection = ArchiveSelection {
                expected_snapshot_id: current.reference.snapshot_id,
                groups: vec![ArchiveGroup {
                    basis: ArchiveBasis::Known {
                        snapshot: current.reference,
                        source: ArchiveBasisSource::UserSelected,
                    },
                    targets: vec![key],
                }],
            };
            let prepared = composition
                .service
                .prepare(
                    ReviewCommandId::new(),
                    Some(current.reference.snapshot_id),
                    ReviewWorkspaceCommand::Archive(selection),
                )
                .await?;
            let archive_id = prepared.generated.archive_id;
            let archived = composition.service.apply(prepared).await?;
            let source_replacement_detected = archived
                .view
                .source_checks
                .iter()
                .any(|check| check.status == SourceCheckStatus::Changed);
            let mut result = composition.result(
                scenario,
                "tests/fixtures/images/rotated-6.jpg",
                current.reference.snapshot_id,
                archived.receipt.snapshot.snapshot_id,
                HarnessChecks {
                    source_replacement_detected,
                    ..HarnessChecks::empty()
                },
            );
            result.archive_id = Some(archive_id.to_string());
            result.archive_ids = vec![archive_id.to_string()];
            Ok(result)
        }
        TwoIterationStep::SaveFinal => {
            let saved = save_new(&composition, "第三版的新意见").await?;
            Ok(composition.result(
                scenario,
                "tests/fixtures/images/rotated-6.jpg",
                saved.receipt.snapshot.snapshot_id,
                saved.receipt.snapshot.snapshot_id,
                HarnessChecks::empty(),
            ))
        }
    }
}

async fn run_two_iterations(
    project: &Path,
    scenario: Scenario,
) -> Result<HarnessResult, Box<dyn Error>> {
    let first = run_two_iteration_step(project, scenario, TwoIterationStep::SaveFirst).await?;
    fs::copy(project.join("source-2.jpg"), project.join("source-1.jpg"))?;
    let first_archive =
        run_two_iteration_step(project, scenario, TwoIterationStep::ArchiveFirst).await?;
    run_two_iteration_step(project, scenario, TwoIterationStep::SaveSecond).await?;
    fs::copy(project.join("source-3.jpg"), project.join("source-1.jpg"))?;
    let second_archive =
        run_two_iteration_step(project, scenario, TwoIterationStep::ArchiveSecond).await?;
    let mut result = run_two_iteration_step(project, scenario, TwoIterationStep::SaveFinal).await?;

    let first_archive_id = first_archive
        .archive_id
        .clone()
        .ok_or("first cycle omitted its archive identity")?;
    let second_archive_id = second_archive
        .archive_id
        .clone()
        .ok_or("second cycle omitted its archive identity")?;
    let two_distinct_cycles = first_archive_id != second_archive_id
        && first_archive.basis_snapshot_id != second_archive.basis_snapshot_id;
    let two_source_replacements_detected = first_archive.checks.source_replacement_detected
        && second_archive.checks.source_replacement_detected;
    if !two_distinct_cycles || !two_source_replacements_detected {
        return Err("bare two-iteration orchestration did not close two distinct cycles".into());
    }

    result.basis_snapshot_id = first.basis_snapshot_id;
    result.archive_id = Some(first_archive_id.clone());
    result.archive_ids = vec![first_archive_id, second_archive_id];
    result.edited_feedback_id = first_archive.edited_feedback_id;
    result.checks.current_preserved_later_edit = first_archive.checks.current_preserved_later_edit;
    result.checks.history_preserved_basis_text = first_archive.checks.history_preserved_basis_text;
    result.checks.source_replacement_detected = two_source_replacements_detected;
    result.checks.two_distinct_cycles = two_distinct_cycles;
    result.checks.two_source_replacements_detected = two_source_replacements_detected;
    Ok(result)
}

async fn run_partial_shared(
    project: &Path,
    scenario: Scenario,
) -> Result<HarnessResult, Box<dyn Error>> {
    let composition = build_composition(project)?;
    let assets = composition
        .prepare_named_assets(&["source-1.jpg", "source-2.jpg"])
        .await?;
    let first = apply(
        &composition.service,
        None,
        ReviewWorkspaceCommand::SaveFeedback {
            feedback_id: None,
            text: "两张都收紧".into(),
            targets: assets
                .iter()
                .map(|asset| TargetEdit::Add {
                    asset_version_id: asset.id,
                    anchor: FeedbackAnchor::Asset,
                })
                .collect(),
        },
    )
    .await?;
    let first_state = &first
        .view
        .current
        .as_ref()
        .ok_or("missing shared state")?
        .state;
    let feedback = first_state
        .feedback
        .first()
        .ok_or("missing shared feedback")?;
    let archived_target = feedback.targets[0].id;
    let retained_target = feedback.targets[1].id;
    let archived_key = first_state
        .target_key(archived_target)
        .ok_or("missing shared target key")?;
    let selection = ArchiveSelection {
        expected_snapshot_id: first.receipt.snapshot.snapshot_id,
        groups: vec![ArchiveGroup {
            basis: ArchiveBasis::Unknown,
            targets: vec![archived_key],
        }],
    };
    let prepared = composition
        .service
        .prepare(
            ReviewCommandId::new(),
            Some(first.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::Archive(selection),
        )
        .await?;
    let archive_id = prepared.generated.archive_id;
    let archived = composition.service.apply(prepared).await?;
    let current = &archived
        .view
        .current
        .as_ref()
        .ok_or("missing partial current")?
        .state;
    let history = composition
        .service
        .history(HistorySelector::Archive(archive_id))
        .await?;
    let partial_shared_retained = current.feedback.len() == 1
        && current.feedback[0].id == feedback.id
        && current.feedback[0].targets.len() == 1
        && current.feedback[0].targets[0].id == retained_target
        && history.entries.len() == 1
        && history.entries[0].selected == vec![archived_key];
    if !partial_shared_retained {
        return Err("partial shared archive did not retain the other target".into());
    }
    let mut result = composition.result(
        scenario,
        "tests/fixtures/images/srgb.jpg",
        first.receipt.snapshot.snapshot_id,
        archived.receipt.snapshot.snapshot_id,
        HarnessChecks {
            partial_shared_retained,
            ..HarnessChecks::empty()
        },
    );
    result.archive_id = Some(archive_id.to_string());
    result.shared_feedback_id = Some(feedback.id.to_string());
    result.archived_target_id = Some(archived_target.to_string());
    result.retained_target_id = Some(retained_target.to_string());
    Ok(result)
}

async fn run_source_replaced(
    project: &Path,
    scenario: Scenario,
) -> Result<HarnessResult, Box<dyn Error>> {
    let composition = build_composition(project)?;
    let asset = composition
        .prepare_named_assets(&["source-1.jpg"])
        .await?
        .remove(0);
    let saved = apply(
        &composition.service,
        None,
        ReviewWorkspaceCommand::SaveFeedback {
            feedback_id: None,
            text: "保留已保存的原始意见".into(),
            targets: vec![TargetEdit::Add {
                asset_version_id: asset.id,
                anchor: FeedbackAnchor::Asset,
            }],
        },
    )
    .await?;
    fs::copy(project.join("source-2.jpg"), project.join("source-1.jpg"))?;
    let stream = composition.stream_id;
    let view = composition.service.view(stream).await?;
    let current = view.current.as_ref().ok_or("missing replaced current")?;
    let source_replacement_detected = view
        .source_checks
        .iter()
        .any(|check| check.status == SourceCheckStatus::Changed);
    let snapshot_stayed_fixed = current.reference == saved.receipt.snapshot
        && current.state.feedback[0].text == "保留已保存的原始意见";
    if !source_replacement_detected || !snapshot_stayed_fixed {
        return Err("source replacement changed committed review facts".into());
    }
    let mut result = composition.result(
        scenario,
        "tests/fixtures/images/srgb.jpg",
        saved.receipt.snapshot.snapshot_id,
        current.reference.snapshot_id,
        HarnessChecks {
            source_replacement_detected,
            snapshot_stayed_fixed,
            ..HarnessChecks::empty()
        },
    );
    result.edited_feedback_id = Some(current.state.feedback[0].id.to_string());
    Ok(result)
}

async fn run_unknown_basis(
    project: &Path,
    scenario: Scenario,
) -> Result<HarnessResult, Box<dyn Error>> {
    let composition = build_composition(project)?;
    let saved = save_one(&composition, "手动存档，交接版本未确认").await?;
    let key = first_target_key(&saved)?;
    let selection = ArchiveSelection {
        expected_snapshot_id: saved.receipt.snapshot.snapshot_id,
        groups: vec![ArchiveGroup {
            basis: ArchiveBasis::Unknown,
            targets: vec![key],
        }],
    };
    let prepared = composition
        .service
        .prepare(
            ReviewCommandId::new(),
            Some(saved.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::Archive(selection),
        )
        .await?;
    let archive_id = prepared.generated.archive_id;
    let archived = composition.service.apply(prepared).await?;
    let checkpoint = composition
        .provider
        .continuous_reader()?
        .load_archive(composition.stream_id, archive_id)?;
    let unknown_basis_preserved = checkpoint
        .groups
        .iter()
        .all(|group| group.basis == ArchiveBasis::Unknown)
        && archived
            .view
            .current
            .as_ref()
            .is_some_and(|current| current.state.feedback.is_empty());
    if !unknown_basis_preserved {
        return Err("manual archive invented a usage basis".into());
    }
    let mut result = composition.result(
        scenario,
        "tests/fixtures/images/srgb.jpg",
        saved.receipt.snapshot.snapshot_id,
        archived.receipt.snapshot.snapshot_id,
        HarnessChecks {
            unknown_basis_preserved,
            ..HarnessChecks::empty()
        },
    );
    result.archive_id = Some(archive_id.to_string());
    result.archived_target_id = Some(key.target_id.to_string());
    Ok(result)
}

async fn run_restore_conflict(
    project: &Path,
    scenario: Scenario,
) -> Result<HarnessResult, Box<dyn Error>> {
    let composition = build_composition(project)?;
    let assets = composition
        .prepare_named_assets(&["source-1.jpg", "source-2.jpg"])
        .await?;
    let first = apply(
        &composition.service,
        None,
        ReviewWorkspaceCommand::SaveFeedback {
            feedback_id: None,
            text: "两张的旧意见".into(),
            targets: assets
                .iter()
                .map(|asset| TargetEdit::Add {
                    asset_version_id: asset.id,
                    anchor: FeedbackAnchor::Asset,
                })
                .collect(),
        },
    )
    .await?;
    let first_key = first_target_key(&first)?;
    let selection = ArchiveSelection {
        expected_snapshot_id: first.receipt.snapshot.snapshot_id,
        groups: vec![ArchiveGroup {
            basis: ArchiveBasis::Unknown,
            targets: vec![first_key],
        }],
    };
    let prepared = composition
        .service
        .prepare(
            ReviewCommandId::new(),
            Some(first.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::Archive(selection),
        )
        .await?;
    let archive_id = prepared.generated.archive_id;
    let archived = composition.service.apply(prepared).await?;
    let current_feedback = archived
        .view
        .current
        .as_ref()
        .and_then(|current| current.state.feedback.first())
        .ok_or("missing retained shared feedback")?;
    let edited = apply(
        &composition.service,
        Some(archived.receipt.snapshot.snapshot_id),
        ReviewWorkspaceCommand::SaveFeedback {
            feedback_id: Some(current_feedback.id),
            text: "图二的新意见，不可覆盖".into(),
            targets: vec![],
        },
    )
    .await?;
    let conflict_decisions = vec![RestoreDecision {
        historical_key: first_key,
        choice: RestoreChoice::UseHistorical,
    }];
    let plan = composition
        .service
        .preview_restore(archive_id, conflict_decisions)
        .await?;
    let decisions = vec![RestoreDecision {
        historical_key: first_key,
        choice: RestoreChoice::ContinueAsNew {
            feedback_id: FeedbackId::new(),
            text_revision_id: ReviewTextRevisionId::new(),
            target_id: ReviewTargetId::new(),
            target_revision_id: ReviewTargetRevisionId::new(),
            target_asset_version_id: AssetVersionId::from_str(&assets[0].id.to_string())?,
            confirmed_anchor: Some(FeedbackAnchor::Asset),
            created_at_ms: 1_900_000_000_000,
        },
    }];
    let prepared_restore = composition
        .service
        .prepare(
            ReviewCommandId::new(),
            Some(edited.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::Restore {
                archive_id,
                decisions: decisions.clone(),
            },
        )
        .await?;
    let restored = composition.service.apply(prepared_restore).await?;
    let stale_restore = composition
        .service
        .prepare(
            ReviewCommandId::new(),
            Some(archived.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::Restore {
                archive_id,
                decisions,
            },
        )
        .await?;
    let stale_restore_rejected = matches!(
        composition.service.apply(stale_restore).await,
        Err(
            viewer_application::review_workspace::ReviewWorkspaceError::Repository(
                ReviewCommitError::StaleSnapshot
            )
        )
    );
    let current = restored
        .view
        .current
        .as_ref()
        .ok_or("missing conflict current")?;
    let restore_conflict_detected = plan.conflicts == vec![first_key];
    let current_text_unchanged = current.state.feedback[0].text == "图二的新意见，不可覆盖";
    let restore_applied = restored.receipt.command_id != edited.receipt.command_id;
    if !restore_conflict_detected
        || !current_text_unchanged
        || !restore_applied
        || !stale_restore_rejected
    {
        return Err(format!(
            "restore checks failed: conflict={restore_conflict_detected} current={current_text_unchanged} applied={restore_applied} stale={stale_restore_rejected}"
        )
        .into());
    }
    let mut result = composition.result(
        scenario,
        "tests/fixtures/images/srgb.jpg",
        first.receipt.snapshot.snapshot_id,
        restored.receipt.snapshot.snapshot_id,
        HarnessChecks {
            restore_conflict_detected,
            current_text_unchanged,
            restore_applied,
            stale_restore_rejected,
            ..HarnessChecks::empty()
        },
    );
    result.archive_id = Some(archive_id.to_string());
    result.edited_feedback_id = Some(current_feedback.id.to_string());
    result.shared_feedback_id = Some(current_feedback.id.to_string());
    result.archived_target_id = Some(first_key.target_id.to_string());
    result.retained_target_id = Some(current.state.feedback[0].targets[0].id.to_string());
    result.restore_receipt_snapshot_id = Some(restored.receipt.snapshot.snapshot_id.to_string());
    result.restore_decision = Some("continue_as_new");
    Ok(result)
}

struct FailAfterIndex {
    pending: AtomicBool,
}

impl ReviewCommitFaultInjector for FailAfterIndex {
    fn check(&self, point: ReviewCommitFaultPoint) -> Result<(), ReviewCommitError> {
        if point == ReviewCommitFaultPoint::AfterIndex && self.pending.swap(false, Ordering::SeqCst)
        {
            Err(ReviewCommitError::Io)
        } else {
            Ok(())
        }
    }
}

struct LostReceiptProvider {
    real: Arc<ProjectReviewRepositoryProvider>,
    inject: AtomicBool,
}

impl ContinuousReviewRepositoryProviderPort for LostReceiptProvider {
    fn open_reader(&self) -> Result<Arc<dyn ContinuousReviewRepositoryPort>, ReviewCommitError> {
        self.real.continuous_reader()
    }

    fn open_writer(&self) -> Result<Arc<dyn ContinuousReviewRepositoryPort>, ReviewCommitError> {
        if self.inject.swap(false, Ordering::SeqCst) {
            self.real
                .continuous_writer_with_faults(Arc::new(FailAfterIndex {
                    pending: AtomicBool::new(true),
                }))
        } else {
            self.real.continuous_writer()
        }
    }
}

async fn run_lost_receipt(
    project: &Path,
    scenario: Scenario,
) -> Result<HarnessResult, Box<dyn Error>> {
    let provider = Arc::new(ProjectReviewRepositoryProvider::new(project, project_id()));
    let service_provider: Arc<dyn ContinuousReviewRepositoryProviderPort> =
        Arc::new(LostReceiptProvider {
            real: provider.clone(),
            inject: AtomicBool::new(true),
        });
    let composition = build_composition_with_provider(project, provider, service_provider)?;
    let asset = composition
        .prepare_named_assets(&["source-1.jpg"])
        .await?
        .remove(0);
    let envelope = composition
        .service
        .prepare(
            ReviewCommandId::new(),
            None,
            ReviewWorkspaceCommand::SaveFeedback {
                feedback_id: None,
                text: "首次提交，回执丢失".into(),
                targets: vec![TargetEdit::Add {
                    asset_version_id: asset.id,
                    anchor: FeedbackAnchor::Asset,
                }],
            },
        )
        .await?;
    if composition.service.apply(envelope.clone()).await
        != Err(
            viewer_application::review_workspace::ReviewWorkspaceError::Repository(
                ReviewCommitError::OutcomeUnknown,
            ),
        )
    {
        return Err("fault did not produce an unknown committed outcome".into());
    }
    let recovered = composition.service.apply(envelope.clone()).await?;
    let key = first_target_key(&recovered)?;
    let later = apply(
        &composition.service,
        Some(recovered.receipt.snapshot.snapshot_id),
        ReviewWorkspaceCommand::SaveFeedback {
            feedback_id: Some(key.feedback_id),
            text: "回执丢失后的后续意见".into(),
            targets: vec![],
        },
    )
    .await?;
    let retry = composition.service.apply(envelope).await?;
    let retried_receipt_stable = retry.receipt == recovered.receipt;
    let current_did_not_roll_back = retry.view.current.as_ref().is_some_and(|current| {
        current.reference == later.receipt.snapshot
            && current.state.feedback[0].text == "回执丢失后的后续意见"
    });
    if !retried_receipt_stable || !current_did_not_roll_back {
        return Err("lost receipt retry rolled the current head back".into());
    }
    let mut result = composition.result(
        scenario,
        "tests/fixtures/images/srgb.jpg",
        recovered.receipt.snapshot.snapshot_id,
        later.receipt.snapshot.snapshot_id,
        HarnessChecks {
            retried_receipt_stable,
            current_did_not_roll_back,
            ..HarnessChecks::empty()
        },
    );
    result.edited_feedback_id = Some(key.feedback_id.to_string());
    Ok(result)
}

async fn run_migration(
    project: &Path,
    scenario: Scenario,
) -> Result<HarnessResult, Box<dyn Error>> {
    let index_path = project.join(".viewer/reviews/index.json");
    let round_id = viewer_domain::ReviewRoundId::from_str("00000000-0000-4000-8000-000000000202")?;
    let round_path = project.join(format!(".viewer/reviews/rounds/{round_id}.json"));
    let draft_id = viewer_domain::ReviewRoundId::from_str("00000000-0000-4000-8000-000000000203")?;
    let draft_path = project.join(format!(".viewer/reviews/drafts/{draft_id}.json"));
    let legacy_index = fs::read(&index_path)
        .map_err(|error| format!("read legacy index {}: {error}", index_path.display()))?;
    let legacy_round = fs::read(&round_path)
        .map_err(|error| format!("read legacy round {}: {error}", round_path.display()))?;
    let legacy_draft = fs::read(&draft_path)
        .map_err(|error| format!("read legacy draft {}: {error}", draft_path.display()))?;
    let provider = Arc::new(ProjectReviewRepositoryProvider::new(project, project_id()));
    let service_provider: Arc<dyn ContinuousReviewRepositoryProviderPort> = provider.clone();
    let composition = build_composition_with_context(
        project,
        provider,
        service_provider,
        ReviewWorkspaceContext {
            project_id: project_id(),
            stream_id: ReviewStreamId::from_str("00000000-0000-4000-8000-000000000102")?,
            production: Some(ProductionScope {
                task_id: ProductionId::parse("task-b")?,
                batch_id: ProductionId::parse("batch-b")?,
            }),
        },
    )
    .map_err(|error| format!("build migration composition: {error}"))?;
    let inspection = composition
        .service
        .inspect_migration()
        .await
        .map_err(|error| format!("inspect migration: {error}"))?
        .ok_or("migration inspection is missing")?;
    let active_draft = inspection
        .active_draft
        .as_ref()
        .ok_or("migration inspection omitted active Draft")?;
    let legacy_draft_feedback_id = active_draft
        .draft
        .feedback
        .first()
        .ok_or("active Draft omitted feedback")?
        .id;
    let prepared = composition
        .service
        .prepare(
            ReviewCommandId::new(),
            None,
            ReviewWorkspaceCommand::Migrate(MigrationPlan {
                inspection_digest: inspection.inspection_digest,
                choice: MigrationChoice::KeepHistoryOnly,
            }),
        )
        .await
        .map_err(|error| format!("prepare migration: {error}"))?;
    let migrated = composition
        .service
        .apply(prepared)
        .await
        .map_err(|error| format!("apply migration: {error}"))?;
    let recovery = project.join(".viewer/reviews/recovery");
    let legacy_index_bytes_preserved = fs::read_dir(&recovery)
        .map_err(|error| format!("read recovery directory {}: {error}", recovery.display()))?
        .any(|entry| {
            entry
                .ok()
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with("legacy-index-")
                })
                .and_then(|entry| fs::read(entry.path()).ok())
                .is_some_and(|bytes| bytes == legacy_index)
        });
    let legacy_round_bytes_preserved = fs::read(&round_path)
        .map_err(|error| format!("reread legacy round {}: {error}", round_path.display()))?
        == legacy_round;
    let legacy_draft_bytes_preserved = fs::read(&draft_path)
        .map_err(|error| format!("reread legacy draft {}: {error}", draft_path.display()))?
        == legacy_draft;
    let active_draft_became_current = migrated.view.current.as_ref().is_some_and(|current| {
        current.state.feedback.iter().any(|feedback| {
            feedback.id == legacy_draft_feedback_id
                && feedback.text == "人物手部需要修正，整体光线保持不变。"
        })
    });
    let completed_history = composition
        .service
        .history(HistorySelector::Legacy(round_id))
        .await?;
    let completed_remained_history = matches!(
        completed_history.legacy.as_ref().map(|record| &record.contents),
        Some(viewer_application::review_workspace::LegacyReviewContents::Completed(snapshot))
            if snapshot.feedback.iter().any(|feedback| feedback.text == "人物手部需要修正，整体光线保持不变。")
    );
    if !legacy_index_bytes_preserved
        || !legacy_round_bytes_preserved
        || !legacy_draft_bytes_preserved
        || !active_draft_became_current
        || !completed_remained_history
    {
        return Err(format!(
            "migration checks failed: index={legacy_index_bytes_preserved} round={legacy_round_bytes_preserved} draft={legacy_draft_bytes_preserved} current={active_draft_became_current} completed={completed_remained_history}"
        )
        .into());
    }
    let mut result = composition.result(
        scenario,
        "tests/fixtures/review-protocol/review-index-v1.valid.json",
        migrated.receipt.snapshot.snapshot_id,
        migrated.receipt.snapshot.snapshot_id,
        HarnessChecks {
            legacy_index_bytes_preserved,
            legacy_round_bytes_preserved,
            legacy_draft_bytes_preserved,
            active_draft_became_current,
            completed_remained_history,
            ..HarnessChecks::empty()
        },
    );
    result.legacy_round_id = Some(round_id.to_string());
    result.legacy_draft_feedback_id = Some(legacy_draft_feedback_id.to_string());
    Ok(result)
}

async fn save_one(
    composition: &Composition,
    text: &str,
) -> Result<viewer_application::review_workspace::ReviewApplyResult, Box<dyn Error>> {
    let asset = composition
        .prepare_named_assets(&["source-1.jpg"])
        .await?
        .remove(0);
    apply(
        &composition.service,
        None,
        ReviewWorkspaceCommand::SaveFeedback {
            feedback_id: None,
            text: text.into(),
            targets: vec![TargetEdit::Add {
                asset_version_id: asset.id,
                anchor: FeedbackAnchor::Asset,
            }],
        },
    )
    .await
}

async fn save_new(
    composition: &Composition,
    text: &str,
) -> Result<viewer_application::review_workspace::ReviewApplyResult, Box<dyn Error>> {
    let expected_snapshot_id = composition
        .service
        .view(composition.stream_id)
        .await?
        .current
        .map(|current| current.reference.snapshot_id);
    let asset = composition
        .prepare_named_assets(&["source-1.jpg"])
        .await?
        .remove(0);
    apply(
        &composition.service,
        expected_snapshot_id,
        ReviewWorkspaceCommand::SaveFeedback {
            feedback_id: None,
            text: text.into(),
            targets: vec![TargetEdit::Add {
                asset_version_id: asset.id,
                anchor: FeedbackAnchor::Asset,
            }],
        },
    )
    .await
}

fn first_target_key(
    result: &viewer_application::review_workspace::ReviewApplyResult,
) -> Result<TargetVersionKey, Box<dyn Error>> {
    let state = &result
        .view
        .current
        .as_ref()
        .ok_or("missing current state")?
        .state;
    state
        .feedback
        .first()
        .and_then(|feedback| feedback.targets.first())
        .and_then(|target| state.target_key(target.id))
        .ok_or_else(|| "missing first target key".into())
}

async fn apply(
    service: &ContinuousReviewService,
    expected_snapshot_id: Option<viewer_domain::ReviewSnapshotId>,
    command: ReviewWorkspaceCommand,
) -> Result<viewer_application::review_workspace::ReviewApplyResult, Box<dyn Error>> {
    let envelope = service
        .prepare(ReviewCommandId::new(), expected_snapshot_id, command)
        .await?;
    Ok(service.apply(envelope).await?)
}
