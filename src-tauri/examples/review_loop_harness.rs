use async_trait::async_trait;
use serde::Serialize;
use std::error::Error;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;
use viewer_application::{
    AddReviewFeedback, BrowseIndexPort, ClockPort, ImagePort, ProjectAccess,
    ReviewAssetConflictKind, ReviewCompletionProposal, ReviewMutationGuard, ReviewProgressPort,
    ReviewPublication, ReviewRepositoryError, ReviewRepositoryProviderPort, ReviewScope,
    ReviewSessionPhase, ReviewSessionService, ReviewSessionSnapshot, ReviewTaskProgress,
};
use viewer_desktop::error::CommandError;
use viewer_domain::file::{FileKind, FileNode, ReviewState};
use viewer_domain::review::{ReviewOutcomeKind, ReviewSnapshot, ReviewabilityFailure};
use viewer_domain::search::Generation;
use viewer_domain::{EntityId, ProjectId, RelativePath, ReviewRoundId, ReviewStreamId};
use viewer_infrastructure::SystemClock;
use viewer_infrastructure::review::{
    IndexedReviewAssetCatalog, ProjectReviewRepository, ProjectReviewRepositoryProvider,
    ReviewChangeLedger, ReviewRepositoryAccess, ReviewRepositoryFaultInjector,
    ReviewRepositoryFaultPoint,
};
use viewer_infrastructure::search::index::SessionIndex;
use viewer_infrastructure::video_probe::{
    MediaFileIdentity, UnavailableVideoProbe, VideoMetadataProbe, VideoProbeError,
};
use viewer_platform_macos::image::MacImagePort;

const PROJECT_ID: ProjectId = ProjectId::from_u128(0xfeed_f00d);
const SINGLE_FEEDBACK: &str = "降低高光强度，保留布料纹理。";
const MULTI_FEEDBACK: &str = "统一背景色温，并修正边缘伪影。";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Scenario {
    Standard,
    ReadOnly,
    WriterBusy,
    CorruptImage,
    StableVideoFailure,
    Replaced,
    Moved,
    Deleted,
    PublishRecovery,
    MultipleDrafts,
}

impl Scenario {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "standard" => Some(Self::Standard),
            "read_only" => Some(Self::ReadOnly),
            "writer_busy" => Some(Self::WriterBusy),
            "corrupt_image" => Some(Self::CorruptImage),
            "stable_video_failure" => Some(Self::StableVideoFailure),
            "replaced" => Some(Self::Replaced),
            "moved" => Some(Self::Moved),
            "deleted" => Some(Self::Deleted),
            "publish_recovery" => Some(Self::PublishRecovery),
            "multiple_drafts" => Some(Self::MultipleDrafts),
            _ => None,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::ReadOnly => "read_only",
            Self::WriterBusy => "writer_busy",
            Self::CorruptImage => "corrupt_image",
            Self::StableVideoFailure => "stable_video_failure",
            Self::Replaced => "replaced",
            Self::Moved => "moved",
            Self::Deleted => "deleted",
            Self::PublishRecovery => "publish_recovery",
            Self::MultipleDrafts => "multiple_drafts",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Counts {
    total: u32,
    revise: u32,
    unreviewable: u32,
    pass: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HarnessResult {
    scenario: &'static str,
    status: &'static str,
    stream_id: Option<String>,
    round_id: Option<String>,
    error_code: Option<String>,
    draft_ignored_before_completion: bool,
    completed_round_published: bool,
    recovered_publish: bool,
    marker_state_ignored: bool,
    favorite_state_ignored: bool,
    counts: Counts,
    relative_paths: Vec<String>,
    feedback_texts: Vec<String>,
    feedback_target_counts: Vec<u32>,
    unreviewable_failures: Vec<&'static str>,
    conflict_kinds: Vec<&'static str>,
}

impl HarnessResult {
    fn empty(scenario: Scenario, status: &'static str) -> Self {
        Self {
            scenario: scenario.name(),
            status,
            stream_id: None,
            round_id: None,
            error_code: None,
            draft_ignored_before_completion: false,
            completed_round_published: false,
            recovered_publish: false,
            marker_state_ignored: false,
            favorite_state_ignored: false,
            counts: Counts::default(),
            relative_paths: Vec::new(),
            feedback_texts: Vec::new(),
            feedback_target_counts: Vec::new(),
            unreviewable_failures: Vec::new(),
            conflict_kinds: Vec::new(),
        }
    }
}

#[derive(Default)]
struct SilentProgress;

impl ReviewProgressPort for SilentProgress {
    fn report(&self, _progress: ReviewTaskProgress) {}
}

#[derive(Clone, Copy)]
struct StableUnreadableVideoProbe;

#[async_trait]
impl VideoMetadataProbe for StableUnreadableVideoProbe {
    async fn probe(
        &self,
        _canonical_path: &Path,
        cancellation: CancellationToken,
    ) -> Result<viewer_domain::video::VideoMetadata, VideoProbeError> {
        if cancellation.is_cancelled() {
            Err(VideoProbeError::Cancelled)
        } else {
            Err(VideoProbeError::Failed(
                viewer_domain::video::VideoFailureKind::Unreadable,
            ))
        }
    }

    async fn probe_identity_bound(
        &self,
        canonical_path: &Path,
        _expected_identity: &MediaFileIdentity,
        cancellation: CancellationToken,
    ) -> Result<viewer_domain::video::VideoMetadata, VideoProbeError> {
        self.probe(canonical_path, cancellation).await
    }
}

struct CrashAfterRoundDurable;

impl ReviewRepositoryFaultInjector for CrashAfterRoundDurable {
    fn check(&self, point: ReviewRepositoryFaultPoint) -> Result<(), ReviewRepositoryError> {
        if matches!(
            point,
            ReviewRepositoryFaultPoint::AfterRoundDurableBeforeIndex
                | ReviewRepositoryFaultPoint::AfterBundleDurableBeforeIndex
        ) {
            Err(ReviewRepositoryError::Unavailable)
        } else {
            Ok(())
        }
    }
}

struct Composition {
    _cache: TempDir,
    index: Arc<SessionIndex>,
    provider: Arc<ProjectReviewRepositoryProvider>,
    service: ReviewSessionService,
    nodes: Vec<FileNode>,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("review loop harness failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let (project, scenario) = parse_arguments(std::env::args().skip(1))?;
    let project = fs::canonicalize(project)?;
    if !project.is_dir() {
        return Err("project must be a directory".into());
    }
    let result = run_scenario(&project, scenario).await?;
    let encoded = serde_json::to_vec(&result)?;
    if encoded.len() > 64 * 1024 {
        return Err("harness result exceeds its bound".into());
    }
    println!("{}", String::from_utf8(encoded)?);
    Ok(())
}

fn parse_arguments(
    mut arguments: impl Iterator<Item = String>,
) -> Result<(PathBuf, Scenario), Box<dyn Error>> {
    let mut project = None;
    let mut scenario = None;
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
            _ => return Err("invalid harness arguments".into()),
        }
    }
    Ok((
        project.ok_or("--project is required")?,
        scenario.ok_or("--scenario is required")?,
    ))
}

async fn run_scenario(project: &Path, scenario: Scenario) -> Result<HarnessResult, Box<dyn Error>> {
    match scenario {
        Scenario::Standard => run_standard(project, scenario).await,
        Scenario::ReadOnly => run_unwritable(project, scenario, ProjectAccess::ReadOnly).await,
        Scenario::WriterBusy => run_writer_busy(project, scenario).await,
        Scenario::CorruptImage | Scenario::StableVideoFailure => {
            run_stable_failure(project, scenario).await
        }
        Scenario::Replaced | Scenario::Moved | Scenario::Deleted => {
            run_conflict(project, scenario).await
        }
        Scenario::PublishRecovery => run_publish_recovery(project, scenario).await,
        Scenario::MultipleDrafts => run_multiple_drafts(project, scenario).await,
    }
}

fn build_composition(
    project: &Path,
    scenario: Scenario,
    access: ProjectAccess,
) -> Result<Composition, Box<dyn Error>> {
    let cache = tempfile::tempdir()?;
    let index = Arc::new(SessionIndex::open(
        cache.path().join("review-index.sqlite"),
    )?);
    let nodes = indexed_media_nodes(project)?;
    index.upsert_batch(&nodes, Generation::new(1))?;
    let browse: Arc<dyn BrowseIndexPort> = index.clone();
    let image: Arc<dyn ImagePort> = Arc::new(MacImagePort::new(cache.path().join("images"))?);
    let video: Arc<dyn VideoMetadataProbe> = if scenario == Scenario::StableVideoFailure {
        Arc::new(StableUnreadableVideoProbe)
    } else {
        Arc::new(UnavailableVideoProbe)
    };
    let catalog = Arc::new(IndexedReviewAssetCatalog::new(
        project,
        browse,
        image,
        video,
        ReviewChangeLedger::default(),
    )?);
    let provider = Arc::new(ProjectReviewRepositoryProvider::new_with_access(
        project, PROJECT_ID, access,
    ));
    let repositories: Arc<dyn ReviewRepositoryProviderPort> = provider.clone();
    let service =
        ReviewSessionService::new(PROJECT_ID, catalog, repositories, Arc::new(SystemClock));
    Ok(Composition {
        _cache: cache,
        index,
        provider,
        service,
        nodes,
    })
}

fn indexed_media_nodes(project: &Path) -> Result<Vec<FileNode>, Box<dyn Error>> {
    let mut nodes = Vec::new();
    for entry in fs::read_dir(project)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            continue;
        }
        let filename = entry.file_name().to_string_lossy().into_owned();
        let extension = Path::new(&filename)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let kind = match extension.as_str() {
            "jpg" | "jpeg" => FileKind::Jpeg,
            "png" => FileKind::Png,
            "mp4" | "mov" | "mkv" | "webm" => FileKind::Video,
            _ => continue,
        };
        nodes.push(FileNode {
            entity_id: entity_id(&metadata),
            relative_path: RelativePath::parse(&filename)?,
            kind,
            size: metadata.len(),
            modified_ns: modified_ns(&metadata),
        });
    }
    nodes.sort_by(|left, right| {
        left.relative_path
            .as_str()
            .cmp(right.relative_path.as_str())
    });
    if nodes.is_empty() {
        return Err("scenario project contains no indexed media".into());
    }
    Ok(nodes)
}

fn entity_id(metadata: &fs::Metadata) -> EntityId {
    EntityId::from_u128((u128::from(metadata.dev()) << 64) | u128::from(metadata.ino()))
}

fn modified_ns(metadata: &fs::Metadata) -> i128 {
    i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec())
}

fn media_entity_ids(nodes: &[FileNode]) -> Vec<EntityId> {
    nodes.iter().map(|node| node.entity_id).collect()
}

fn entity_for_path(nodes: &[FileNode], path: &str) -> Result<EntityId, Box<dyn Error>> {
    nodes
        .iter()
        .find(|node| node.relative_path.as_str() == path)
        .map(|node| node.entity_id)
        .ok_or_else(|| format!("missing scenario asset: {path}").into())
}

async fn start_round(composition: &Composition) -> Result<ReviewSessionSnapshot, Box<dyn Error>> {
    let proposal = composition
        .service
        .preview_start(ReviewScope::Selection {
            entity_ids: media_entity_ids(&composition.nodes),
        })
        .await?;
    Ok(composition
        .service
        .start(proposal.id, Arc::new(SilentProgress))
        .await?)
}

async fn add_standard_feedback(
    composition: &Composition,
    mut active: ReviewSessionSnapshot,
) -> Result<ReviewSessionSnapshot, Box<dyn Error>> {
    let round_id = active
        .review_round_id
        .ok_or("active Round is missing its id")?;
    active = composition
        .service
        .add_feedback(AddReviewFeedback {
            guard: ReviewMutationGuard {
                review_round_id: round_id,
                expected_revision: active.revision,
            },
            text: SINGLE_FEEDBACK.to_owned(),
            target_entity_ids: vec![entity_for_path(&composition.nodes, "hero.png")?],
        })
        .await?;
    composition
        .service
        .add_feedback(AddReviewFeedback {
            guard: ReviewMutationGuard {
                review_round_id: round_id,
                expected_revision: active.revision,
            },
            text: MULTI_FEEDBACK.to_owned(),
            target_entity_ids: vec![
                entity_for_path(&composition.nodes, "hero.png")?,
                entity_for_path(&composition.nodes, "variant.jpg")?,
            ],
        })
        .await
        .map_err(Into::into)
}

async fn complete_round(
    composition: &Composition,
    active: &ReviewSessionSnapshot,
) -> Result<(ReviewSessionSnapshot, ReviewCompletionProposal), Box<dyn Error>> {
    let guard = ReviewMutationGuard {
        review_round_id: active
            .review_round_id
            .ok_or("active Round is missing its id")?,
        expected_revision: active.revision,
    };
    let proposal = composition.service.completion_summary(guard).await?;
    let completed = composition
        .service
        .complete(
            proposal.id,
            proposal.summary.guard(),
            Arc::new(SilentProgress),
        )
        .await?;
    Ok((completed, proposal))
}

async fn run_standard(project: &Path, scenario: Scenario) -> Result<HarnessResult, Box<dyn Error>> {
    let composition = build_composition(project, scenario, ProjectAccess::ReadWrite)?;
    let inspected_repository = composition.provider.inspect()?;
    let state = composition.service.inspect().await;
    if state.resume.is_none() {
        let hero = entity_for_path(&composition.nodes, "hero.png")?;
        let pass = entity_for_path(&composition.nodes, "pass.jpg")?;
        composition
            .index
            .set_review_metadata(hero, Some(ReviewState::Keep), false)?;
        composition
            .index
            .set_review_metadata(pass, Some(ReviewState::Reject), true)?;
        let active = add_standard_feedback(&composition, start_round(&composition).await?).await?;
        let mut result = HarnessResult::empty(scenario, "draft");
        result.stream_id = active.review_stream_id.map(|id| id.to_string());
        result.round_id = active.review_round_id.map(|id| id.to_string());
        result.relative_paths = active
            .members
            .iter()
            .map(|member| member.relative_path.as_str().to_owned())
            .collect();
        result.draft_ignored_before_completion =
            composition.provider.inspect()?.catalog.streams.is_empty();
        composition.service.shutdown().await;
        return Ok(result);
    }

    let draft_hidden = inspected_repository.catalog.streams.is_empty()
        && inspected_repository.active_draft.is_some();
    let active = composition.service.resume(Arc::new(SilentProgress)).await?;
    let (completed, _) = complete_round(&composition, &active).await?;
    if completed.phase != ReviewSessionPhase::CompletedReadOnly {
        return Err("standard Round did not enter completed read-only state".into());
    }
    let result = load_completed_result(
        project,
        scenario,
        composition.provider.as_ref(),
        draft_hidden,
        false,
    )?;
    composition.service.shutdown().await;
    Ok(result)
}

async fn run_unwritable(
    project: &Path,
    scenario: Scenario,
    access: ProjectAccess,
) -> Result<HarnessResult, Box<dyn Error>> {
    let composition = build_composition(project, scenario, access)?;
    let _ = composition.service.inspect().await;
    let proposal = composition
        .service
        .preview_start(ReviewScope::Selection {
            entity_ids: media_entity_ids(&composition.nodes),
        })
        .await?;
    let error = composition
        .service
        .start(proposal.id, Arc::new(SilentProgress))
        .await
        .expect_err("read-only scenario must reject the writer");
    let mut result = HarnessResult::empty(scenario, "blocked");
    result.error_code = Some(CommandError::from(error).code);
    composition.service.shutdown().await;
    Ok(result)
}

async fn run_writer_busy(
    project: &Path,
    scenario: Scenario,
) -> Result<HarnessResult, Box<dyn Error>> {
    let composition = build_composition(project, scenario, ProjectAccess::ReadWrite)?;
    let _ = composition.service.inspect().await;
    let external_writer = composition.provider.open_writer()?;
    let proposal = composition
        .service
        .preview_start(ReviewScope::Selection {
            entity_ids: media_entity_ids(&composition.nodes),
        })
        .await?;
    let error = composition
        .service
        .start(proposal.id, Arc::new(SilentProgress))
        .await
        .expect_err("contended scenario must reject the second writer");
    let mut result = HarnessResult::empty(scenario, "blocked");
    result.error_code = Some(CommandError::from(error).code);
    drop(external_writer);
    composition.service.shutdown().await;
    Ok(result)
}

async fn run_stable_failure(
    project: &Path,
    scenario: Scenario,
) -> Result<HarnessResult, Box<dyn Error>> {
    let composition = build_composition(project, scenario, ProjectAccess::ReadWrite)?;
    let _ = composition.service.inspect().await;
    let active = start_round(&composition).await?;
    let _ = complete_round(&composition, &active).await?;
    let result = load_completed_result(
        project,
        scenario,
        composition.provider.as_ref(),
        true,
        false,
    )?;
    composition.service.shutdown().await;
    Ok(result)
}

async fn run_conflict(project: &Path, scenario: Scenario) -> Result<HarnessResult, Box<dyn Error>> {
    let composition = build_composition(project, scenario, ProjectAccess::ReadWrite)?;
    let _ = composition.service.inspect().await;
    let active = start_round(&composition).await?;
    let source = composition
        .nodes
        .first()
        .cloned()
        .ok_or("conflict scenario has no source")?;
    let source_path = project.join(source.relative_path.as_str());
    match scenario {
        Scenario::Replaced => {
            let replacement = project.join("replacement.tmp");
            fs::copy(&source_path, &replacement)?;
            fs::remove_file(&source_path)?;
            fs::rename(replacement, &source_path)?;
        }
        Scenario::Moved => {
            let destination = project.join("moved.png");
            fs::rename(&source_path, &destination)?;
            let metadata = fs::symlink_metadata(&destination)?;
            composition.index.upsert_batch(
                &[FileNode {
                    entity_id: source.entity_id,
                    relative_path: RelativePath::parse("moved.png")?,
                    kind: source.kind,
                    size: metadata.len(),
                    modified_ns: modified_ns(&metadata),
                }],
                Generation::new(1),
            )?;
        }
        Scenario::Deleted => {
            fs::remove_file(&source_path)?;
            composition.index.remove_subtree(source.entity_id)?;
        }
        _ => return Err("invalid conflict scenario".into()),
    }

    let guard = ReviewMutationGuard {
        review_round_id: active
            .review_round_id
            .ok_or("active Round is missing its id")?,
        expected_revision: active.revision,
    };
    let proposal = composition.service.completion_summary(guard).await?;
    if proposal.summary.can_complete || proposal.summary.conflicts.len() != 1 {
        return Err("conflict did not block completion".into());
    }
    let mut result = HarnessResult::empty(scenario, "conflict");
    result.stream_id = active.review_stream_id.map(|id| id.to_string());
    result.round_id = active.review_round_id.map(|id| id.to_string());
    result.relative_paths = active
        .members
        .iter()
        .map(|member| member.relative_path.as_str().to_owned())
        .collect();
    result.conflict_kinds = proposal
        .summary
        .conflicts
        .iter()
        .map(|conflict| conflict_name(conflict.kind))
        .collect();
    result.completed_round_published = !composition.provider.inspect()?.catalog.streams.is_empty();
    composition.service.shutdown().await;
    Ok(result)
}

async fn run_publish_recovery(
    project: &Path,
    scenario: Scenario,
) -> Result<HarnessResult, Box<dyn Error>> {
    let rounds_directory = project.join(".viewer/reviews/rounds");
    let has_durable_round = regular_json_files(&rounds_directory)?.next().is_some()
        || (rounds_directory.is_dir()
            && fs::read_dir(&rounds_directory)?.any(|entry| {
                entry.is_ok_and(|entry| {
                    let path = entry.path();
                    !entry.file_name().to_string_lossy().starts_with('.')
                        && path.is_dir()
                        && path.join("round.json").is_file()
                })
            }));

    if !has_durable_round {
        let repository_state =
            ProjectReviewRepositoryProvider::new(project, PROJECT_ID).inspect()?;
        let Some(draft) = repository_state.active_draft else {
            return run_standard(project, scenario).await;
        };
        let protocol_version = draft.protocol_version;
        let stream_id = draft.draft.review_stream_id;
        let round_id = draft.draft.review_round_id;
        let snapshot = draft.draft.complete(SystemClock.unix_millis())?;
        let repository = ProjectReviewRepository::open_with_faults(
            project,
            PROJECT_ID,
            ReviewRepositoryAccess::ReadWrite,
            Arc::new(CrashAfterRoundDurable),
        )?;
        let error = repository
            .publish(&ReviewPublication {
                protocol_version,
                snapshot: snapshot.clone(),
                artifacts: vec![],
            })
            .expect_err("fault point must interrupt publication");
        if error != ReviewRepositoryError::Unavailable {
            return Err("publication failed at an unexpected boundary".into());
        }
        drop(repository);
        let mut result = HarnessResult::empty(scenario, "publishInterrupted");
        result.stream_id = Some(stream_id.to_string());
        result.round_id = Some(round_id.to_string());
        result.relative_paths = snapshot
            .assets
            .iter()
            .map(|asset| asset.relative_path.as_str().to_owned())
            .collect();
        return Ok(result);
    }

    let recovering =
        ProjectReviewRepository::open(project, PROJECT_ID, ReviewRepositoryAccess::ReadWrite)?;
    drop(recovering);
    let composition = build_composition(project, scenario, ProjectAccess::ReadWrite)?;
    let state = composition.service.inspect().await;
    if state.phase != ReviewSessionPhase::CompletedReadOnly {
        return Err("recovered publication is not the exact completed head".into());
    }
    let result =
        load_completed_result(project, scenario, composition.provider.as_ref(), true, true)?;
    composition.service.shutdown().await;
    Ok(result)
}

async fn run_multiple_drafts(
    project: &Path,
    scenario: Scenario,
) -> Result<HarnessResult, Box<dyn Error>> {
    let composition = build_composition(project, scenario, ProjectAccess::ReadWrite)?;
    let _ = composition.service.inspect().await;
    let _ = start_round(&composition).await?;
    composition.service.shutdown().await;

    let writer = composition.provider.open_writer()?;
    let mut duplicate = writer
        .load_active_draft()?
        .ok_or("first Draft was not persisted")?;
    duplicate.draft.review_stream_id = ReviewStreamId::new();
    duplicate.draft.review_round_id = ReviewRoundId::new();
    duplicate.draft.previous_completed_round_id = None;
    writer.save_draft(&duplicate)?;
    drop(writer);

    let second = build_composition(project, scenario, ProjectAccess::ReadWrite)?;
    let inspected = second.service.inspect().await;
    if inspected.phase != ReviewSessionPhase::RecoveryRequired {
        return Err("multiple Drafts did not fail closed".into());
    }
    let mut result = HarnessResult::empty(scenario, "recoveryRequired");
    result.error_code = inspected.error.map(|error| error.code.to_owned());
    second.service.shutdown().await;
    Ok(result)
}

fn load_completed_result(
    project: &Path,
    scenario: Scenario,
    provider: &ProjectReviewRepositoryProvider,
    draft_hidden: bool,
    recovered_publish: bool,
) -> Result<HarnessResult, Box<dyn Error>> {
    let reader = provider.open_reader()?;
    let catalog = reader.load_catalog()?;
    let stream = catalog
        .streams
        .iter()
        .find(|stream| stream.production.is_none())
        .ok_or("manual Stream is missing")?;
    let round_id = stream
        .latest_completed_round_id
        .ok_or("manual Stream has no completed head")?;
    let snapshot = reader
        .load_completed(stream.review_stream_id, round_id)?
        .ok_or("completed head is unavailable")?;
    completed_result(project, scenario, snapshot, draft_hidden, recovered_publish)
}

fn completed_result(
    project: &Path,
    scenario: Scenario,
    snapshot: ReviewSnapshot,
    draft_hidden: bool,
    recovered_publish: bool,
) -> Result<HarnessResult, Box<dyn Error>> {
    let mut counts = Counts {
        total: u32::try_from(snapshot.outcomes.len())?,
        ..Counts::default()
    };
    let mut failures = Vec::new();
    for outcome in &snapshot.outcomes {
        match outcome.kind {
            ReviewOutcomeKind::Pass => counts.pass += 1,
            ReviewOutcomeKind::Revise => counts.revise += 1,
            ReviewOutcomeKind::Unreviewable => {
                counts.unreviewable += 1;
                failures.push(failure_name(
                    outcome
                        .failure
                        .ok_or("unreviewable outcome has no failure")?,
                ));
            }
        }
    }
    let legacy_round = project.join(format!(
        ".viewer/reviews/rounds/{}.json",
        snapshot.review_round_id
    ));
    let bundled_round = project.join(format!(
        ".viewer/reviews/rounds/{}/round.json",
        snapshot.review_round_id
    ));
    let round_document = if legacy_round.is_file() {
        fs::read_to_string(legacy_round)?
    } else {
        fs::read_to_string(bundled_round)?
    };
    let protocol_omits_marker_state = !round_document.contains("\"marker\"");
    let protocol_omits_favorite_state = !round_document.contains("\"favorite\"");
    Ok(HarnessResult {
        scenario: scenario.name(),
        status: "completed",
        stream_id: Some(snapshot.review_stream_id.to_string()),
        round_id: Some(snapshot.review_round_id.to_string()),
        error_code: None,
        draft_ignored_before_completion: draft_hidden,
        completed_round_published: true,
        recovered_publish,
        marker_state_ignored: protocol_omits_marker_state,
        favorite_state_ignored: protocol_omits_favorite_state,
        counts,
        relative_paths: snapshot
            .assets
            .iter()
            .map(|asset| asset.relative_path.as_str().to_owned())
            .collect(),
        feedback_texts: snapshot
            .feedback
            .iter()
            .map(|feedback| feedback.text.clone())
            .collect(),
        feedback_target_counts: snapshot
            .feedback
            .iter()
            .map(|feedback| u32::try_from(feedback.targets.len()))
            .collect::<Result<Vec<_>, _>>()?,
        unreviewable_failures: failures,
        conflict_kinds: Vec::new(),
    })
}

fn failure_name(failure: ReviewabilityFailure) -> &'static str {
    match failure {
        ReviewabilityFailure::Unsupported => "unsupported",
        ReviewabilityFailure::Damaged => "damaged",
        ReviewabilityFailure::Unreadable => "unreadable",
        ReviewabilityFailure::PermissionDenied => "permissionDenied",
        ReviewabilityFailure::Missing => "missing",
        ReviewabilityFailure::DecodeFailed => "decodeFailed",
    }
}

fn conflict_name(conflict: ReviewAssetConflictKind) -> &'static str {
    match conflict {
        ReviewAssetConflictKind::Missing => "missing",
        ReviewAssetConflictKind::Moved => "moved",
        ReviewAssetConflictKind::Replaced => "replaced",
        ReviewAssetConflictKind::SizeChanged => "sizeChanged",
        ReviewAssetConflictKind::ContentChanged => "contentChanged",
        ReviewAssetConflictKind::MediaChanged => "mediaChanged",
    }
}

fn regular_json_files(directory: &Path) -> Result<impl Iterator<Item = PathBuf>, Box<dyn Error>> {
    let mut paths = Vec::new();
    if !directory.exists() {
        return Ok(paths.into_iter());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.path().extension().and_then(|value| value.to_str()) == Some("json")
            && entry.file_type()?.is_file()
        {
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths.into_iter())
}
