use std::collections::{HashMap, HashSet};

use super::mutation::prepare_next;
use super::validation::ValidatedStates;
use super::*;
use crate::review::{AssetVersion, FeedbackAnchor, round::validate_anchor};
use crate::{
    AssetVersionId, FeedbackId, ReviewSnapshotId, ReviewTargetId, ReviewTargetRevisionId,
    ReviewTextRevisionId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestoreDisposition {
    Restore,
    ConfirmConflict,
    AlreadyCurrent,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RestoreDecision {
    pub historical_key: TargetVersionKey,
    pub choice: RestoreChoice,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RestoreChoice {
    PreserveCurrent,
    UseHistorical,
    ContinueAsNew {
        feedback_id: FeedbackId,
        text_revision_id: ReviewTextRevisionId,
        target_id: ReviewTargetId,
        target_revision_id: ReviewTargetRevisionId,
        target_asset_version_id: AssetVersionId,
        /// None retains a provisional historical anchor, never executable without confirmation.
        confirmed_anchor: Option<FeedbackAnchor>,
        created_at_ms: i64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct RestoredFeedback {
    pub historical_key: TargetVersionKey,
    /// Exactly one selected target; merging must not replace an entire shared feedback item.
    pub feedback: VersionedFeedback,
    pub asset: AssetVersion,
    pub continued_as_new: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RestorePlan {
    pub expected_snapshot_id: ReviewSnapshotId,
    pub restored: Vec<RestoredFeedback>,
    pub conflicts: Vec<TargetVersionKey>,
    pub coverage_reversals: Vec<ArchiveCoverage>,
    /// Restoring saved availability does not authorize execution. Readers must check the source.
    pub requires_source_check: Vec<ReviewTargetId>,
}

pub fn classify_restore(
    current: Option<TargetVersionKey>,
    historical: TargetVersionKey,
) -> RestoreDisposition {
    match current {
        None => RestoreDisposition::Restore,
        Some(key) if key == historical => RestoreDisposition::AlreadyCurrent,
        Some(_) => RestoreDisposition::ConfirmConflict,
    }
}

/// Decisions are the explicit selection. An empty selection never resurrects deleted work.
/// The application implements immediate undo by selecting precisely the checkpoint's removed keys.
pub fn plan_restore(
    current: &ContinuousReviewState,
    archive: &ArchiveCheckpoint,
    bases: &[ContinuousReviewState],
    decisions: &[RestoreDecision],
) -> Result<RestorePlan, ContinuousReviewError> {
    use ContinuousReviewError::*;
    let states = ValidatedStates::new(current, bases)?;
    validate_checkpoint(current, archive, bases, &states)?;
    let historical: HashMap<_, _> = archive
        .groups
        .iter()
        .flat_map(|group| group.targets.iter().map(move |key| (*key, group)))
        .collect();
    if decisions.len() > historical.len() {
        return Err(
            if decisions.len() > crate::review::MAX_TARGETS_PER_FEEDBACK {
                LimitExceeded
            } else {
                DuplicateIdentity
            },
        );
    }
    let mut selected = HashSet::new();
    let mut selected_targets = HashSet::new();
    let mut selected_texts = HashMap::new();
    for decision in decisions {
        if !historical.contains_key(&decision.historical_key) {
            return Err(MissingReference);
        }
        if !selected.insert(decision.historical_key) {
            return Err(DuplicateIdentity);
        }
        if matches!(decision.choice, RestoreChoice::UseHistorical) {
            selected_targets.insert(decision.historical_key.target_id);
            if selected_texts
                .insert(
                    decision.historical_key.feedback_id,
                    decision.historical_key.text_revision_id,
                )
                .is_some_and(|previous| previous != decision.historical_key.text_revision_id)
            {
                return Err(SelectionConflict);
            }
        }
    }
    let current_feedback: HashMap<_, _> = current
        .feedback
        .iter()
        .map(|feedback| (feedback.id, feedback))
        .collect();
    let current_targets = &states.targets[&current.snapshot_id];
    let current_assets: HashMap<_, _> = current
        .assets
        .iter()
        .map(|asset| (asset.id, asset))
        .collect();
    let assets: HashMap<_, _> = bases
        .iter()
        .flat_map(|state| &state.assets)
        .map(|asset| (asset.id, asset))
        .collect();
    let mut identities = RestoreIdentities::new(&states);
    let mut plan = RestorePlan {
        expected_snapshot_id: current.snapshot_id,
        restored: vec![],
        conflicts: vec![],
        coverage_reversals: vec![],
        requires_source_check: vec![],
    };
    for decision in decisions {
        let key = decision.historical_key;
        let group = historical[&key];
        let reference = match group.basis {
            ArchiveBasis::Known { snapshot, .. } => snapshot,
            ArchiveBasis::Unknown => archive.before,
        };
        let (old_feedback, old_target) = states.targets[&reference.snapshot_id][&key.target_id];
        if matches!(decision.choice, RestoreChoice::PreserveCurrent) {
            continue;
        }
        let mut feedback = old_feedback.clone();
        feedback.targets = vec![old_target.clone()];
        let continued_as_new = matches!(decision.choice, RestoreChoice::ContinueAsNew { .. });
        let asset = match &decision.choice {
            RestoreChoice::UseHistorical => {
                if current_targets
                    .get(&key.target_id)
                    .is_some_and(|(feedback, target)| feedback.key(target) == key)
                {
                    continue;
                }
                if current_feedback
                    .get(&key.feedback_id)
                    .is_some_and(|feedback| {
                        feedback.text_revision_id != key.text_revision_id
                            && feedback
                                .targets
                                .iter()
                                .any(|target| !selected_targets.contains(&target.id))
                    })
                {
                    plan.conflicts.push(key);
                    continue;
                }
                (*assets
                    .get(&old_target.asset_version_id)
                    .ok_or(MissingReference)?)
                .clone()
            }
            RestoreChoice::ContinueAsNew {
                feedback_id,
                text_revision_id,
                target_id,
                target_revision_id,
                target_asset_version_id,
                confirmed_anchor,
                created_at_ms,
            } => {
                if *created_at_ms < archive.created_at_ms {
                    return Err(InvalidData);
                }
                feedback.id = *feedback_id;
                feedback.text_revision_id = *text_revision_id;
                feedback.created_at_ms = *created_at_ms;
                feedback.history_ref = Some(HistoryRef {
                    project_id: current.project_id,
                    stream_id: current.stream_id,
                    source: HistorySource::Snapshot {
                        snapshot: reference,
                        keys: vec![key],
                    },
                });
                feedback.targets = vec![VersionedTarget {
                    id: *target_id,
                    revision_id: *target_revision_id,
                    asset_version_id: *target_asset_version_id,
                    anchor: confirmed_anchor
                        .clone()
                        .unwrap_or_else(|| old_target.anchor.clone()),
                    availability: if confirmed_anchor.is_some() {
                        ReviewAvailability::Ready
                    } else {
                        ReviewAvailability::NeedsConfirmation(vec![
                            ReviewPendingReason::ApplicabilityUnconfirmed,
                        ])
                    },
                }];
                if current_feedback
                    .get(feedback_id)
                    .is_some_and(|existing| **existing == feedback)
                {
                    continue;
                }
                identities.claim(&feedback)?;
                (*current_assets
                    .get(target_asset_version_id)
                    .ok_or(MissingReference)?)
                .clone()
            }
            RestoreChoice::PreserveCurrent => {
                unreachable!("handled before preparing a restoration")
            }
        };
        feedback.validate()?;
        validate_anchor(&asset, &feedback.targets[0].anchor).map_err(|_| InvalidData)?;
        if !continued_as_new {
            plan.coverage_reversals.push(ArchiveCoverage {
                archive_id: archive.archive_id,
                key,
                active: false,
            });
        }
        plan.requires_source_check.push(feedback.targets[0].id);
        plan.restored.push(RestoredFeedback {
            historical_key: key,
            feedback,
            asset,
            continued_as_new,
        });
    }
    Ok(plan)
}

pub fn apply_restore(
    current: &ContinuousReviewState,
    archive: &ArchiveCheckpoint,
    bases: &[ContinuousReviewState],
    decisions: &[RestoreDecision],
    snapshot_id: ReviewSnapshotId,
) -> Result<(ContinuousReviewState, RestorePlan), ContinuousReviewError> {
    let plan = plan_restore(current, archive, bases, decisions)?;
    if !plan.conflicts.is_empty() {
        return Err(ContinuousReviewError::NeedsConfirmation);
    }
    if plan.restored.is_empty() {
        return Ok((current.clone(), plan));
    }
    if bases.iter().any(|basis| basis.snapshot_id == snapshot_id) {
        return Err(ContinuousReviewError::DuplicateIdentity);
    }
    let mut next = prepare_next(current, snapshot_id)?;
    for item in &plan.restored {
        if !next.assets.iter().any(|asset| asset.id == item.asset.id) {
            next.assets.push(item.asset.clone());
        }
        let incoming = &item.feedback;
        if let Some(feedback) = next
            .feedback
            .iter_mut()
            .find(|feedback| feedback.id == incoming.id)
        {
            feedback.text.clone_from(&incoming.text);
            feedback.text_revision_id = incoming.text_revision_id;
            if let Some(target) = feedback
                .targets
                .iter_mut()
                .find(|target| target.id == incoming.targets[0].id)
            {
                *target = incoming.targets[0].clone();
            } else {
                feedback.targets.push(incoming.targets[0].clone());
            }
        } else {
            next.feedback.push(incoming.clone());
        }
    }
    next.validate()?;
    Ok((next, plan))
}

fn validate_checkpoint(
    current: &ContinuousReviewState,
    archive: &ArchiveCheckpoint,
    bases: &[ContinuousReviewState],
    states: &ValidatedStates<'_>,
) -> Result<(), ContinuousReviewError> {
    use ContinuousReviewError::*;
    if archive.project_id != current.project_id
        || archive.stream_id != current.stream_id
        || archive.created_at_ms < 0
        || archive.groups.is_empty()
    {
        return Err(InvalidData);
    }
    let before = states
        .states
        .get(&archive.before.snapshot_id)
        .filter(|_| states.bases.contains(&archive.before.snapshot_id))
        .ok_or(MissingReference)?;
    let expected = plan_archive(
        before,
        bases,
        &ArchiveSelection {
            expected_snapshot_id: archive.before.snapshot_id,
            groups: archive.groups.clone(),
        },
        &[],
    )?;
    if expected.removed != archive.removed || expected.retained != archive.retained {
        return Err(InvalidData);
    }
    ArchiveCheckpoint::from_plan(
        before,
        archive.before,
        &expected,
        archive.archive_id,
        archive.created_at_ms,
    )?;
    Ok(())
}

struct RestoreIdentities {
    feedback: HashSet<FeedbackId>,
    text: HashSet<ReviewTextRevisionId>,
    targets: HashSet<ReviewTargetId>,
    revisions: HashSet<ReviewTargetRevisionId>,
}

impl RestoreIdentities {
    fn new(states: &ValidatedStates<'_>) -> Self {
        let mut ids = Self {
            feedback: HashSet::new(),
            text: HashSet::new(),
            targets: HashSet::new(),
            revisions: HashSet::new(),
        };
        for state in states.states.values() {
            for feedback in &state.feedback {
                ids.feedback.insert(feedback.id);
                ids.text.insert(feedback.text_revision_id);
                for target in &feedback.targets {
                    ids.targets.insert(target.id);
                    ids.revisions.insert(target.revision_id);
                }
            }
        }
        ids
    }

    fn claim(&mut self, feedback: &VersionedFeedback) -> Result<(), ContinuousReviewError> {
        if !self.feedback.insert(feedback.id)
            || !self.text.insert(feedback.text_revision_id)
            || !self.targets.insert(feedback.targets[0].id)
            || !self.revisions.insert(feedback.targets[0].revision_id)
        {
            return Err(ContinuousReviewError::DuplicateIdentity);
        }
        Ok(())
    }
}
