use crate::{
    ClockPort, PreparedReviewAsset, ReviewAssetCatalogPort, ReviewAssetError,
    ReviewAssetValidation, ReviewCatalog, ReviewProgressPort, ReviewRepositoryError,
    ReviewRepositoryPort, ReviewRepositoryProviderPort, ReviewScope, ReviewScopeResolution,
    ReviewStreamHead, ReviewTaskCancellation,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};
use viewer_domain::review::{
    Feedback, ReviewAssetKind, ReviewDraft, ReviewMedia, ReviewOutcomeKind, ReviewSnapshot,
    ReviewabilityFailure,
};
use viewer_domain::{
    AssetVersionId, EntityId, FeedbackId, ProjectId, RelativePath, ReviewRoundId, ReviewStreamId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewSessionPhase {
    Idle,
    Preparing,
    Active,
    Completing,
    CompletedReadOnly,
    WriteUnavailable,
    RecoveryRequired,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ReviewProposalId(u64);

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewScopeProposal {
    pub id: ReviewProposalId,
    pub scope: ReviewScope,
    pub resolution: ReviewScopeResolution,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewSessionSnapshot {
    pub phase: ReviewSessionPhase,
    pub resume: Option<ReviewResumeSnapshot>,
    pub review_stream_id: Option<ReviewStreamId>,
    pub review_round_id: Option<ReviewRoundId>,
    pub revision: u64,
    pub members: Vec<ReviewMemberSnapshot>,
    pub feedback: Vec<ReviewFeedbackSnapshot>,
    pub unreviewable: Vec<ReviewUnreviewableSnapshot>,
    pub conflicts: Vec<ReviewConflictSnapshot>,
    pub counts: ReviewSessionCounts,
    pub error: Option<ReviewUserError>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewResumeSnapshot {
    pub review_stream_id: ReviewStreamId,
    pub review_round_id: ReviewRoundId,
    pub created_at_ms: i64,
    pub total: u32,
    pub feedback_items: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewMemberSnapshot {
    pub asset_version_id: AssetVersionId,
    pub entity_id: Option<EntityId>,
    pub relative_path: RelativePath,
    pub display_name: String,
    pub kind: ReviewAssetKind,
    pub feedback_items: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewFeedbackSnapshot {
    pub feedback_id: FeedbackId,
    pub text: String,
    pub created_at_ms: i64,
    pub target_entity_ids: Vec<EntityId>,
    pub target_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewUnreviewableSnapshot {
    pub asset_version_id: AssetVersionId,
    pub relative_path: RelativePath,
    pub failure: ReviewabilityFailure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewConflictSnapshot {
    pub asset_version_id: AssetVersionId,
    pub relative_path: RelativePath,
    pub kind: ReviewConflictKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewConflictKind {
    Missing,
    Moved,
    Replaced,
    SizeChanged,
    ContentChanged,
    MediaChanged,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReviewSessionCounts {
    pub total: u32,
    pub feedback_items: u32,
    pub revise: u32,
    pub unreviewable: u32,
    pub pass: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewUserError {
    pub code: &'static str,
    pub retryable: bool,
    pub affected_paths: Vec<RelativePath>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ReviewSessionError {
    #[error("review session state does not allow this operation")]
    InvalidState,
    #[error("review proposal is stale")]
    StaleProposal,
    #[error("review scope contains no candidates")]
    EmptyScope,
    #[error("review scope changed after confirmation")]
    ScopeChanged,
    #[error("a review Draft is already active")]
    DraftAlreadyActive,
    #[error("review repository is busy")]
    Busy,
    #[error("review repository is read-only")]
    ReadOnly,
    #[error("review data requires explicit recovery")]
    RecoveryRequired,
    #[error("review repository is unavailable")]
    RepositoryUnavailable,
    #[error("review asset evidence is unavailable")]
    AssetUnavailable,
    #[error("review task was cancelled")]
    Cancelled,
    #[error("review Draft could not be saved")]
    SaveFailed,
    #[error("review domain data is invalid")]
    InvalidData,
}

impl ReviewSessionError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidState => "review_state_invalid",
            Self::StaleProposal => "review_proposal_stale",
            Self::EmptyScope => "review_scope_empty",
            Self::ScopeChanged => "review_scope_changed",
            Self::DraftAlreadyActive => "review_draft_already_active",
            Self::Busy => "review_writer_busy",
            Self::ReadOnly => "review_project_read_only",
            Self::RecoveryRequired => "review_recovery_required",
            Self::RepositoryUnavailable => "review_repository_unavailable",
            Self::AssetUnavailable => "review_asset_unavailable",
            Self::Cancelled => "review_task_cancelled",
            Self::SaveFailed => "review_save_failed",
            Self::InvalidData => "review_invalid_data",
        }
    }

    const fn retryable(self) -> bool {
        matches!(
            self,
            Self::Busy
                | Self::RepositoryUnavailable
                | Self::AssetUnavailable
                | Self::Cancelled
                | Self::SaveFailed
        )
    }
}

pub struct ReviewSessionService {
    project_id: ProjectId,
    catalog: Arc<dyn ReviewAssetCatalogPort>,
    repositories: Arc<dyn ReviewRepositoryProviderPort>,
    clock: Arc<dyn ClockPort>,
    state: Mutex<ReviewSessionState>,
    task_changed: Notify,
}

struct ReviewSessionState {
    phase: ReviewSessionPhase,
    resume: Option<ReviewResumeSnapshot>,
    proposal: Option<ReviewScopeProposal>,
    next_proposal_id: u64,
    task: Option<ReviewTask>,
    next_task_id: u64,
    active: Option<ActiveReviewSession>,
    displayed_completed: Option<ReviewSnapshot>,
    writer: Option<Box<dyn ReviewRepositoryPort>>,
    error: Option<ReviewUserError>,
}

struct ReviewTask {
    id: u64,
    cancellation: ReviewTaskCancellation,
    return_phase: ReviewSessionPhase,
}

struct ActiveReviewSession {
    draft: ReviewDraft,
    prepared: Vec<PreparedReviewAsset>,
    conflicts: Vec<ReviewConflictSnapshot>,
    revision: u64,
}

struct StartedReview {
    active: ActiveReviewSession,
    writer: Box<dyn ReviewRepositoryPort>,
}

impl ReviewSessionService {
    pub fn new(
        project_id: ProjectId,
        catalog: Arc<dyn ReviewAssetCatalogPort>,
        repositories: Arc<dyn ReviewRepositoryProviderPort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            project_id,
            catalog,
            repositories,
            clock,
            state: Mutex::new(ReviewSessionState::new()),
            task_changed: Notify::new(),
        }
    }

    pub async fn inspect(&self) -> ReviewSessionSnapshot {
        {
            let state = self.state.lock().await;
            if matches!(
                state.phase,
                ReviewSessionPhase::Preparing
                    | ReviewSessionPhase::Active
                    | ReviewSessionPhase::Completing
            ) {
                return snapshot(&state);
            }
        }

        let inspection = self.repositories.inspect();
        let projection = match inspection {
            Ok(inspection) => self.project_inspection(inspection),
            Err(error) => Err(map_repository_error(error)),
        };
        let mut state = self.state.lock().await;
        state.proposal = None;
        state.active = None;
        state.writer = None;
        state.error = None;
        match projection {
            Ok(InspectionProjection::Idle { resume }) => {
                state.phase = ReviewSessionPhase::Idle;
                state.resume = resume;
                state.displayed_completed = None;
            }
            Ok(InspectionProjection::Completed(completed)) => {
                state.phase = ReviewSessionPhase::CompletedReadOnly;
                state.resume = None;
                state.displayed_completed = Some(completed);
            }
            Err(error) => {
                state.phase = phase_for_error(error);
                state.resume = None;
                state.displayed_completed = None;
                state.error = Some(user_error(error));
            }
        }
        snapshot(&state)
    }

    fn project_inspection(
        &self,
        inspection: crate::ReviewRepositoryInspection,
    ) -> Result<InspectionProjection, ReviewSessionError> {
        if inspection.catalog.project_id != self.project_id {
            return Err(ReviewSessionError::RecoveryRequired);
        }
        if let Some(draft) = inspection.active_draft {
            if draft.project_id != self.project_id || draft.production.is_some() {
                return Err(ReviewSessionError::RecoveryRequired);
            }
            return Ok(InspectionProjection::Idle {
                resume: Some(resume_snapshot(&draft)?),
            });
        }
        let Some(stream) = manual_stream(&inspection.catalog)? else {
            return Ok(InspectionProjection::Idle { resume: None });
        };
        let Some(round_id) = stream.latest_completed_round_id else {
            return Ok(InspectionProjection::Idle { resume: None });
        };
        let reader = self
            .repositories
            .open_reader()
            .map_err(map_repository_error)?;
        let completed = reader
            .load_completed(stream.review_stream_id, round_id)
            .map_err(map_repository_error)?
            .ok_or(ReviewSessionError::RecoveryRequired)?;
        Ok(InspectionProjection::Completed(completed))
    }

    pub async fn preview_start(
        &self,
        scope: ReviewScope,
    ) -> Result<ReviewScopeProposal, ReviewSessionError> {
        {
            let state = self.state.lock().await;
            if !matches!(
                state.phase,
                ReviewSessionPhase::Idle
                    | ReviewSessionPhase::CompletedReadOnly
                    | ReviewSessionPhase::WriteUnavailable
            ) || state.task.is_some()
            {
                return Err(ReviewSessionError::InvalidState);
            }
        }
        let resolution = self
            .catalog
            .resolve_scope(&scope)
            .await
            .map_err(map_asset_error)?;
        if resolution.candidate_entity_ids.is_empty() {
            return Err(ReviewSessionError::EmptyScope);
        }
        let mut state = self.state.lock().await;
        if !matches!(
            state.phase,
            ReviewSessionPhase::Idle
                | ReviewSessionPhase::CompletedReadOnly
                | ReviewSessionPhase::WriteUnavailable
        ) || state.task.is_some()
        {
            return Err(ReviewSessionError::InvalidState);
        }
        state.next_proposal_id = state.next_proposal_id.saturating_add(1);
        let proposal = ReviewScopeProposal {
            id: ReviewProposalId(state.next_proposal_id),
            scope,
            resolution,
        };
        state.proposal = Some(proposal.clone());
        Ok(proposal)
    }

    pub async fn start(
        &self,
        proposal_id: ReviewProposalId,
        progress: Arc<dyn ReviewProgressPort>,
    ) -> Result<ReviewSessionSnapshot, ReviewSessionError> {
        let (task_id, proposal, cancellation) = {
            let mut state = self.state.lock().await;
            let proposal = state
                .proposal
                .as_ref()
                .filter(|proposal| proposal.id == proposal_id)
                .cloned()
                .ok_or(ReviewSessionError::StaleProposal)?;
            if !matches!(
                state.phase,
                ReviewSessionPhase::Idle
                    | ReviewSessionPhase::CompletedReadOnly
                    | ReviewSessionPhase::WriteUnavailable
            ) || state.task.is_some()
            {
                return Err(ReviewSessionError::InvalidState);
            }
            let task = transition_to_preparing(&mut state)?;
            (task.id, proposal, task.cancellation)
        };

        let result = self.perform_start(proposal, cancellation, progress).await;
        self.finish_start(task_id, result).await
    }

    async fn perform_start(
        &self,
        proposal: ReviewScopeProposal,
        cancellation: ReviewTaskCancellation,
        progress: Arc<dyn ReviewProgressPort>,
    ) -> Result<StartedReview, ReviewSessionError> {
        let writer = self
            .repositories
            .open_writer()
            .map_err(map_repository_error)?;
        if writer
            .load_active_draft()
            .map_err(map_repository_error)?
            .is_some()
        {
            return Err(ReviewSessionError::DraftAlreadyActive);
        }
        let catalog = writer.load_catalog().map_err(map_repository_error)?;
        if catalog.project_id != self.project_id {
            return Err(ReviewSessionError::RecoveryRequired);
        }
        let stream = manual_stream(&catalog)?;
        let current = self
            .catalog
            .resolve_scope(&proposal.scope)
            .await
            .map_err(map_asset_error)?;
        if current != proposal.resolution {
            return Err(ReviewSessionError::ScopeChanged);
        }
        if cancellation.is_cancelled() {
            return Err(ReviewSessionError::Cancelled);
        }
        let prepared = self
            .catalog
            .prepare_assets(
                &proposal.resolution.candidate_entity_ids,
                cancellation.clone(),
                progress,
            )
            .await
            .map_err(map_asset_error)?;
        if prepared.is_empty() {
            return Err(ReviewSessionError::EmptyScope);
        }
        validate_prepared_scope(&prepared, &proposal.resolution.candidate_entity_ids)?;
        let stream_id = stream
            .map(|stream| stream.review_stream_id)
            .unwrap_or_else(ReviewStreamId::new);
        let previous_completed_round_id =
            stream.and_then(|stream| stream.latest_completed_round_id);
        let mut draft = ReviewDraft::new(
            self.project_id,
            stream_id,
            ReviewRoundId::new(),
            None,
            previous_completed_round_id,
            self.clock.unix_millis(),
            prepared
                .iter()
                .map(|prepared| prepared.asset.clone())
                .collect(),
        )
        .map_err(|_| ReviewSessionError::InvalidData)?;
        for prepared in &prepared {
            if let Some(failure) = prepared.failure {
                draft
                    .mark_unreviewable(prepared.asset.id, failure)
                    .map_err(|_| ReviewSessionError::InvalidData)?;
            }
        }
        if cancellation.is_cancelled() {
            return Err(ReviewSessionError::Cancelled);
        }
        writer
            .save_draft(&draft)
            .map_err(|_| ReviewSessionError::SaveFailed)?;
        Ok(StartedReview {
            active: ActiveReviewSession {
                draft,
                prepared,
                conflicts: vec![],
                revision: 1,
            },
            writer,
        })
    }

    async fn finish_start(
        &self,
        task_id: u64,
        result: Result<StartedReview, ReviewSessionError>,
    ) -> Result<ReviewSessionSnapshot, ReviewSessionError> {
        let mut state = self.state.lock().await;
        if state.task.as_ref().map(|task| task.id) != Some(task_id) {
            drop(state);
            self.catalog.release_tracking();
            self.task_changed.notify_waiters();
            return Err(ReviewSessionError::Cancelled);
        }
        let return_phase = state.task.as_ref().unwrap().return_phase;
        state.task = None;
        match result {
            Ok(started) => {
                install_active(&mut state, started.active, started.writer);
                let result = snapshot(&state);
                drop(state);
                self.task_changed.notify_waiters();
                Ok(result)
            }
            Err(error) => {
                state.phase = match error {
                    ReviewSessionError::Busy | ReviewSessionError::ReadOnly => {
                        ReviewSessionPhase::WriteUnavailable
                    }
                    ReviewSessionError::RecoveryRequired => ReviewSessionPhase::RecoveryRequired,
                    _ => return_phase,
                };
                state.error = Some(user_error(error));
                drop(state);
                self.catalog.release_tracking();
                self.task_changed.notify_waiters();
                Err(error)
            }
        }
    }

    pub async fn resume(
        &self,
        progress: Arc<dyn ReviewProgressPort>,
    ) -> Result<ReviewSessionSnapshot, ReviewSessionError> {
        let (task_id, resume, cancellation) = {
            let mut state = self.state.lock().await;
            if state.phase != ReviewSessionPhase::Idle || state.task.is_some() {
                return Err(ReviewSessionError::InvalidState);
            }
            let resume = state
                .resume
                .clone()
                .ok_or(ReviewSessionError::InvalidState)?;
            let task = transition_to_preparing(&mut state)?;
            (task.id, resume, task.cancellation)
        };
        let result = self.perform_resume(resume, cancellation, progress).await;
        self.finish_start(task_id, result).await
    }

    async fn perform_resume(
        &self,
        resume: ReviewResumeSnapshot,
        cancellation: ReviewTaskCancellation,
        progress: Arc<dyn ReviewProgressPort>,
    ) -> Result<StartedReview, ReviewSessionError> {
        let writer = self
            .repositories
            .open_writer()
            .map_err(map_repository_error)?;
        let active = writer
            .load_active_draft()
            .map_err(map_repository_error)?
            .ok_or(ReviewSessionError::RecoveryRequired)?;
        if active.project_id != self.project_id
            || active.production.is_some()
            || active.review_stream_id != resume.review_stream_id
            || active.review_round_id != resume.review_round_id
        {
            return Err(ReviewSessionError::RecoveryRequired);
        }
        let catalog = writer.load_catalog().map_err(map_repository_error)?;
        if catalog.project_id != self.project_id {
            return Err(ReviewSessionError::RecoveryRequired);
        }
        match manual_stream(&catalog)? {
            Some(stream)
                if stream.review_stream_id == active.review_stream_id
                    && stream.latest_completed_round_id == active.previous_completed_round_id => {}
            None if active.previous_completed_round_id.is_none() => {}
            _ => return Err(ReviewSessionError::RecoveryRequired),
        }
        let mut prepared = Vec::new();
        let mut conflicts = Vec::new();
        for asset in &active.assets {
            let Some(entity_id) = asset.source_entity_id else {
                conflicts.push(conflict_for(asset.id, asset.relative_path.clone()));
                continue;
            };
            let failure = active
                .unreviewable
                .iter()
                .find(|failure| failure.asset_version_id == asset.id)
                .map(|failure| failure.failure);
            prepared.push(PreparedReviewAsset {
                entity_id,
                asset: asset.clone(),
                failure,
                change_revision: 0,
            });
        }
        let validations = self
            .catalog
            .revalidate_assets(&prepared, cancellation.clone(), progress)
            .await
            .map_err(map_asset_error)?;
        if validations.len() != prepared.len() {
            return Err(ReviewSessionError::AssetUnavailable);
        }
        let mut validated = Vec::with_capacity(prepared.len());
        for (original, validation) in prepared.into_iter().zip(validations) {
            match validation {
                ReviewAssetValidation::Current(current)
                    if same_fixed_member(&original, &current) =>
                {
                    validated.push(current);
                }
                ReviewAssetValidation::Conflict {
                    asset_version_id,
                    relative_path,
                } if asset_version_id == original.asset.id
                    && relative_path == original.asset.relative_path =>
                {
                    conflicts.push(conflict_for(asset_version_id, relative_path));
                    validated.push(original);
                }
                ReviewAssetValidation::Pending {
                    asset_version_id,
                    relative_path,
                } if asset_version_id == original.asset.id
                    && relative_path == original.asset.relative_path =>
                {
                    conflicts.push(ReviewConflictSnapshot {
                        asset_version_id,
                        relative_path,
                        kind: ReviewConflictKind::MediaChanged,
                    });
                    validated.push(original);
                }
                _ => return Err(ReviewSessionError::AssetUnavailable),
            }
        }
        if cancellation.is_cancelled() {
            return Err(ReviewSessionError::Cancelled);
        }
        Ok(StartedReview {
            active: ActiveReviewSession {
                draft: active,
                prepared: validated,
                conflicts,
                revision: 1,
            },
            writer,
        })
    }

    pub async fn cancel_task(&self) -> bool {
        let state = self.state.lock().await;
        let Some(task) = state.task.as_ref() else {
            return false;
        };
        task.cancellation.cancel();
        true
    }

    pub async fn shutdown(&self) {
        loop {
            let notified = self.task_changed.notified();
            let writer = {
                let mut state = self.state.lock().await;
                if let Some(task) = state.task.as_ref() {
                    task.cancellation.cancel();
                    None
                } else {
                    state.active = None;
                    state.writer.take()
                }
            };
            if let Some(writer) = writer {
                drop(writer);
                break;
            }
            let done = { self.state.lock().await.task.is_none() };
            if done {
                break;
            }
            notified.await;
        }
        self.catalog.release_tracking();
        let mut state = self.state.lock().await;
        let next_proposal_id = state.next_proposal_id;
        let next_task_id = state.next_task_id;
        *state = ReviewSessionState::new();
        state.next_proposal_id = next_proposal_id;
        state.next_task_id = next_task_id;
    }
}

impl ReviewSessionState {
    fn new() -> Self {
        Self {
            phase: ReviewSessionPhase::Idle,
            resume: None,
            proposal: None,
            next_proposal_id: 0,
            task: None,
            next_task_id: 0,
            active: None,
            displayed_completed: None,
            writer: None,
            error: None,
        }
    }
}

enum InspectionProjection {
    Idle {
        resume: Option<ReviewResumeSnapshot>,
    },
    Completed(ReviewSnapshot),
}

fn transition_to_preparing(
    state: &mut ReviewSessionState,
) -> Result<ReviewTask, ReviewSessionError> {
    if state.task.is_some() {
        return Err(ReviewSessionError::InvalidState);
    }
    state.next_task_id = state.next_task_id.saturating_add(1);
    let task = ReviewTask {
        id: state.next_task_id,
        cancellation: ReviewTaskCancellation::default(),
        return_phase: state.phase,
    };
    state.phase = ReviewSessionPhase::Preparing;
    state.error = None;
    state.task = Some(ReviewTask {
        id: task.id,
        cancellation: task.cancellation.clone(),
        return_phase: task.return_phase,
    });
    Ok(task)
}

fn install_active(
    state: &mut ReviewSessionState,
    active: ActiveReviewSession,
    writer: Box<dyn ReviewRepositoryPort>,
) {
    state.phase = ReviewSessionPhase::Active;
    state.resume = None;
    state.proposal = None;
    state.displayed_completed = None;
    state.error = None;
    state.active = Some(active);
    state.writer = Some(writer);
}

fn manual_stream(catalog: &ReviewCatalog) -> Result<Option<&ReviewStreamHead>, ReviewSessionError> {
    let mut streams = catalog
        .streams
        .iter()
        .filter(|stream| stream.production.is_none());
    let first = streams.next();
    if streams.next().is_some() {
        Err(ReviewSessionError::RecoveryRequired)
    } else {
        Ok(first)
    }
}

fn snapshot(state: &ReviewSessionState) -> ReviewSessionSnapshot {
    if let Some(active) = state.active.as_ref() {
        let bindings = active
            .prepared
            .iter()
            .map(|prepared| (prepared.asset.id, prepared.entity_id))
            .collect();
        return snapshot_from_draft(
            state.phase,
            &active.draft,
            active.revision,
            &bindings,
            active.conflicts.clone(),
            state.resume.clone(),
            state.error.clone(),
        );
    }
    if let Some(completed) = state.displayed_completed.as_ref() {
        return snapshot_from_completed(state.phase, completed, state.error.clone());
    }
    ReviewSessionSnapshot {
        phase: state.phase,
        resume: state.resume.clone(),
        review_stream_id: state.resume.as_ref().map(|resume| resume.review_stream_id),
        review_round_id: state.resume.as_ref().map(|resume| resume.review_round_id),
        revision: 0,
        members: vec![],
        feedback: vec![],
        unreviewable: vec![],
        conflicts: vec![],
        counts: ReviewSessionCounts::default(),
        error: state.error.clone(),
    }
}

#[allow(clippy::too_many_arguments)]
fn snapshot_from_draft(
    phase: ReviewSessionPhase,
    draft: &ReviewDraft,
    revision: u64,
    bindings: &HashMap<AssetVersionId, EntityId>,
    conflicts: Vec<ReviewConflictSnapshot>,
    resume: Option<ReviewResumeSnapshot>,
    error: Option<ReviewUserError>,
) -> ReviewSessionSnapshot {
    ReviewSessionSnapshot {
        phase,
        resume,
        review_stream_id: Some(draft.review_stream_id),
        review_round_id: Some(draft.review_round_id),
        revision,
        members: member_snapshots(&draft.assets, &draft.feedback, bindings),
        feedback: feedback_snapshots(&draft.feedback, bindings),
        unreviewable: unreviewable_snapshots(draft),
        conflicts,
        counts: draft_counts(draft),
        error,
    }
}

fn snapshot_from_completed(
    phase: ReviewSessionPhase,
    completed: &ReviewSnapshot,
    error: Option<ReviewUserError>,
) -> ReviewSessionSnapshot {
    let bindings = completed
        .assets
        .iter()
        .filter_map(|asset| {
            asset
                .source_entity_id
                .map(|entity_id| (asset.id, entity_id))
        })
        .collect::<HashMap<_, _>>();
    ReviewSessionSnapshot {
        phase,
        resume: None,
        review_stream_id: Some(completed.review_stream_id),
        review_round_id: Some(completed.review_round_id),
        revision: 0,
        members: member_snapshots(&completed.assets, &completed.feedback, &bindings),
        feedback: feedback_snapshots(&completed.feedback, &bindings),
        unreviewable: completed
            .outcomes
            .iter()
            .filter_map(|outcome| {
                let failure = outcome.failure?;
                let asset = completed
                    .assets
                    .iter()
                    .find(|asset| asset.id == outcome.asset_version_id)?;
                Some(ReviewUnreviewableSnapshot {
                    asset_version_id: outcome.asset_version_id,
                    relative_path: asset.relative_path.clone(),
                    failure,
                })
            })
            .collect(),
        conflicts: vec![],
        counts: completed_counts(completed),
        error,
    }
}

fn member_snapshots(
    assets: &[viewer_domain::review::AssetVersion],
    feedback: &[Feedback],
    bindings: &HashMap<AssetVersionId, EntityId>,
) -> Vec<ReviewMemberSnapshot> {
    assets
        .iter()
        .map(|asset| ReviewMemberSnapshot {
            asset_version_id: asset.id,
            entity_id: bindings.get(&asset.id).copied(),
            relative_path: asset.relative_path.clone(),
            display_name: asset
                .relative_path
                .as_str()
                .rsplit('/')
                .next()
                .unwrap_or(asset.relative_path.as_str())
                .to_owned(),
            kind: match asset.media {
                ReviewMedia::Image { .. } => ReviewAssetKind::Image,
                ReviewMedia::Video { .. } => ReviewAssetKind::Video,
            },
            feedback_items: feedback
                .iter()
                .filter(|feedback| {
                    feedback
                        .targets
                        .iter()
                        .any(|target| target.asset_version_id == asset.id)
                })
                .count() as u32,
        })
        .collect()
}

fn feedback_snapshots(
    feedback: &[Feedback],
    bindings: &HashMap<AssetVersionId, EntityId>,
) -> Vec<ReviewFeedbackSnapshot> {
    feedback
        .iter()
        .map(|feedback| ReviewFeedbackSnapshot {
            feedback_id: feedback.id,
            text: feedback.text.clone(),
            created_at_ms: feedback.created_at_ms,
            target_entity_ids: feedback
                .targets
                .iter()
                .filter_map(|target| bindings.get(&target.asset_version_id).copied())
                .collect(),
            target_count: feedback.targets.len() as u32,
        })
        .collect()
}

fn unreviewable_snapshots(draft: &ReviewDraft) -> Vec<ReviewUnreviewableSnapshot> {
    draft
        .unreviewable
        .iter()
        .filter_map(|unreviewable| {
            draft
                .assets
                .iter()
                .find(|asset| asset.id == unreviewable.asset_version_id)
                .map(|asset| ReviewUnreviewableSnapshot {
                    asset_version_id: asset.id,
                    relative_path: asset.relative_path.clone(),
                    failure: unreviewable.failure,
                })
        })
        .collect()
}

fn draft_counts(draft: &ReviewDraft) -> ReviewSessionCounts {
    let revised = draft
        .feedback
        .iter()
        .flat_map(|feedback| {
            feedback
                .targets
                .iter()
                .map(|target| target.asset_version_id)
        })
        .collect::<HashSet<_>>();
    let unreviewable = draft
        .unreviewable
        .iter()
        .filter(|failure| !revised.contains(&failure.asset_version_id))
        .count();
    let total = draft.assets.len();
    ReviewSessionCounts {
        total: total as u32,
        feedback_items: draft.feedback.len() as u32,
        revise: revised.len() as u32,
        unreviewable: unreviewable as u32,
        pass: total.saturating_sub(revised.len() + unreviewable) as u32,
    }
}

fn completed_counts(completed: &ReviewSnapshot) -> ReviewSessionCounts {
    ReviewSessionCounts {
        total: completed.outcomes.len() as u32,
        feedback_items: completed.feedback.len() as u32,
        revise: completed
            .outcomes
            .iter()
            .filter(|outcome| outcome.kind == ReviewOutcomeKind::Revise)
            .count() as u32,
        unreviewable: completed
            .outcomes
            .iter()
            .filter(|outcome| outcome.kind == ReviewOutcomeKind::Unreviewable)
            .count() as u32,
        pass: completed
            .outcomes
            .iter()
            .filter(|outcome| outcome.kind == ReviewOutcomeKind::Pass)
            .count() as u32,
    }
}

fn resume_snapshot(draft: &ReviewDraft) -> Result<ReviewResumeSnapshot, ReviewSessionError> {
    Ok(ReviewResumeSnapshot {
        review_stream_id: draft.review_stream_id,
        review_round_id: draft.review_round_id,
        created_at_ms: draft.created_at_ms,
        total: u32::try_from(draft.assets.len()).map_err(|_| ReviewSessionError::InvalidData)?,
        feedback_items: u32::try_from(draft.feedback.len())
            .map_err(|_| ReviewSessionError::InvalidData)?,
    })
}

fn conflict_for(
    asset_version_id: AssetVersionId,
    relative_path: RelativePath,
) -> ReviewConflictSnapshot {
    ReviewConflictSnapshot {
        asset_version_id,
        relative_path,
        kind: ReviewConflictKind::ContentChanged,
    }
}

fn validate_prepared_scope(
    prepared: &[PreparedReviewAsset],
    expected_entity_ids: &[EntityId],
) -> Result<(), ReviewSessionError> {
    if prepared.len() != expected_entity_ids.len()
        || prepared
            .iter()
            .zip(expected_entity_ids)
            .any(|(prepared, expected)| {
                prepared.entity_id != *expected
                    || prepared.asset.source_entity_id != Some(*expected)
            })
    {
        return Err(ReviewSessionError::AssetUnavailable);
    }
    Ok(())
}

fn same_fixed_member(expected: &PreparedReviewAsset, current: &PreparedReviewAsset) -> bool {
    current.entity_id == expected.entity_id
        && current.asset.id == expected.asset.id
        && current.asset.source_entity_id == expected.asset.source_entity_id
        && current.asset.relative_path == expected.asset.relative_path
}

fn map_repository_error(error: ReviewRepositoryError) -> ReviewSessionError {
    match error {
        ReviewRepositoryError::Busy => ReviewSessionError::Busy,
        ReviewRepositoryError::ReadOnly => ReviewSessionError::ReadOnly,
        ReviewRepositoryError::RecoveryRequired
        | ReviewRepositoryError::UnsupportedVersion
        | ReviewRepositoryError::InvalidData => ReviewSessionError::RecoveryRequired,
        ReviewRepositoryError::NotFound | ReviewRepositoryError::Conflict => {
            ReviewSessionError::RecoveryRequired
        }
        ReviewRepositoryError::LimitExceeded | ReviewRepositoryError::Unavailable => {
            ReviewSessionError::RepositoryUnavailable
        }
    }
}

fn map_asset_error(error: ReviewAssetError) -> ReviewSessionError {
    match error {
        ReviewAssetError::Cancelled => ReviewSessionError::Cancelled,
        ReviewAssetError::InvalidScope | ReviewAssetError::NotFound => {
            ReviewSessionError::ScopeChanged
        }
        ReviewAssetError::LimitExceeded => ReviewSessionError::InvalidData,
        ReviewAssetError::IndexUnavailable
        | ReviewAssetError::UnsafeSource
        | ReviewAssetError::SourceChanged
        | ReviewAssetError::Pending
        | ReviewAssetError::Unavailable => ReviewSessionError::AssetUnavailable,
    }
}

fn phase_for_error(error: ReviewSessionError) -> ReviewSessionPhase {
    match error {
        ReviewSessionError::RecoveryRequired => ReviewSessionPhase::RecoveryRequired,
        ReviewSessionError::Busy | ReviewSessionError::ReadOnly => {
            ReviewSessionPhase::WriteUnavailable
        }
        _ => ReviewSessionPhase::Idle,
    }
}

fn user_error(error: ReviewSessionError) -> ReviewUserError {
    ReviewUserError {
        code: error.code(),
        retryable: error.retryable(),
        affected_paths: vec![],
    }
}
