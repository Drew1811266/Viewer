use crate::{
    ClockPort, PersistedReviewDraft, PreparedReviewAsset, ReviewAssetCatalogPort,
    ReviewAssetConflictKind, ReviewAssetError, ReviewAssetValidation, ReviewCatalog,
    ReviewProgressPort, ReviewProtocolVersion, ReviewPublication, ReviewRepositoryError,
    ReviewRepositoryPort, ReviewRepositoryProviderPort, ReviewScope, ReviewScopeResolution,
    ReviewStreamHead, ReviewTaskCancellation,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};
use viewer_domain::review::{
    Feedback, FeedbackAnchor, FeedbackTarget, ReviewAssetKind, ReviewDraft, ReviewMedia,
    ReviewOutcomeKind, ReviewSnapshot, ReviewabilityFailure,
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

impl ReviewProposalId {
    pub const fn from_raw(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewScopeProposal {
    pub id: ReviewProposalId,
    pub scope: ReviewScope,
    pub resolution: ReviewScopeResolution,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewMutationGuard {
    pub review_round_id: ReviewRoundId,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddReviewFeedback {
    pub guard: ReviewMutationGuard,
    pub text: String,
    pub target_entity_ids: Vec<EntityId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateReviewFeedback {
    pub guard: ReviewMutationGuard,
    pub feedback_id: FeedbackId,
    pub text: String,
    pub target_entity_ids: Vec<EntityId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteReviewFeedback {
    pub guard: ReviewMutationGuard,
    pub feedback_id: FeedbackId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewFeedbackSummary {
    pub feedback_id: FeedbackId,
    pub text: String,
    pub target_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewCompletionSummary {
    pub review_round_id: ReviewRoundId,
    pub revision: u64,
    pub total: u32,
    pub revise: u32,
    pub unreviewable: u32,
    pub default_pass: u32,
    pub feedback: Vec<ReviewFeedbackSummary>,
    pub conflicts: Vec<ReviewConflictSnapshot>,
    pub pending: Vec<RelativePath>,
    pub can_complete: bool,
}

impl ReviewCompletionSummary {
    pub const fn guard(&self) -> ReviewMutationGuard {
        ReviewMutationGuard {
            review_round_id: self.review_round_id,
            expected_revision: self.revision,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ReviewCompletionProposalId(u64);

impl ReviewCompletionProposalId {
    pub const fn from_raw(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewCompletionProposal {
    pub id: ReviewCompletionProposalId,
    pub summary: ReviewCompletionSummary,
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

pub type ReviewConflictKind = ReviewAssetConflictKind;

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
    #[error("review protocol version is unsupported")]
    UnsupportedVersion,
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
    #[error("review command targets a stale Round")]
    StaleRound,
    #[error("review command targets a stale revision")]
    StaleRevision,
    #[error("review Feedback is invalid")]
    InvalidFeedback,
    #[error("review Feedback does not exist")]
    FeedbackNotFound,
    #[error("review completion proposal is stale")]
    StaleCompletionProposal,
    #[error("review completion is blocked by unresolved assets")]
    CompletionBlocked,
    #[error("review completion facts changed after confirmation")]
    CompletionChanged,
    #[error("review Draft could not be abandoned")]
    AbandonFailed,
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
            Self::UnsupportedVersion => "review_unsupported_version",
            Self::RepositoryUnavailable => "review_repository_unavailable",
            Self::AssetUnavailable => "review_asset_unavailable",
            Self::Cancelled => "review_task_cancelled",
            Self::SaveFailed => "review_save_failed",
            Self::InvalidData => "review_invalid_data",
            Self::StaleRound => "review_round_stale",
            Self::StaleRevision => "review_revision_stale",
            Self::InvalidFeedback => "review_feedback_invalid",
            Self::FeedbackNotFound => "review_feedback_not_found",
            Self::StaleCompletionProposal => "review_completion_proposal_stale",
            Self::CompletionBlocked => "review_completion_blocked",
            Self::CompletionChanged => "review_completion_changed",
            Self::AbandonFailed => "review_abandon_failed",
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
    completion_proposal: Option<ReviewCompletionProposal>,
    next_completion_proposal_id: u64,
    displayed_completed: Option<ReviewSnapshot>,
    writer: Option<Box<dyn ReviewRepositoryPort>>,
    error: Option<ReviewUserError>,
}

struct ReviewTask {
    id: u64,
    cancellation: ReviewTaskCancellation,
    return_phase: ReviewSessionPhase,
    cancellable: bool,
}

struct ActiveReviewSession {
    protocol_version: ReviewProtocolVersion,
    draft: ReviewDraft,
    prepared: Vec<PreparedReviewAsset>,
    conflicts: Vec<ReviewConflictSnapshot>,
    revision: u64,
}

struct StartedReview {
    active: ActiveReviewSession,
    writer: Box<dyn ReviewRepositoryPort>,
}

struct ReviewValidationWork {
    protocol_version: ReviewProtocolVersion,
    review_round_id: ReviewRoundId,
    revision: u64,
    draft: ReviewDraft,
    prepared: Vec<PreparedReviewAsset>,
    fixed_conflicts: Vec<ReviewConflictSnapshot>,
}

struct ValidatedReview {
    protocol_version: ReviewProtocolVersion,
    work_revision: u64,
    draft: ReviewDraft,
    prepared: Vec<PreparedReviewAsset>,
    conflicts: Vec<ReviewConflictSnapshot>,
    pending: Vec<RelativePath>,
    stable_facts_changed: bool,
}

#[derive(Default)]
struct SilentReviewProgress;

impl ReviewProgressPort for SilentReviewProgress {
    fn report(&self, _progress: crate::ReviewTaskProgress) {}
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
        state.completion_proposal = None;
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
            if draft.draft.project_id != self.project_id || draft.draft.production.is_some() {
                return Err(ReviewSessionError::RecoveryRequired);
            }
            return Ok(InspectionProjection::Idle {
                resume: Some(resume_snapshot(&draft.draft)?),
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
            .save_draft(&PersistedReviewDraft {
                protocol_version: ReviewProtocolVersion::V2,
                draft: draft.clone(),
            })
            .map_err(|_| ReviewSessionError::SaveFailed)?;
        Ok(StartedReview {
            active: ActiveReviewSession {
                protocol_version: ReviewProtocolVersion::V2,
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
                    ReviewSessionError::RecoveryRequired
                    | ReviewSessionError::UnsupportedVersion => {
                        ReviewSessionPhase::RecoveryRequired
                    }
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
        let persisted = writer
            .load_active_draft()
            .map_err(map_repository_error)?
            .ok_or(ReviewSessionError::RecoveryRequired)?;
        let protocol_version = persisted.protocol_version;
        let active = persisted.draft;
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
        let mut fixed_conflicts = Vec::new();
        for asset in &active.assets {
            let Some(entity_id) = asset.source_entity_id else {
                fixed_conflicts.push(conflict_for(asset.id, asset.relative_path.clone()));
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
        if cancellation.is_cancelled() {
            return Err(ReviewSessionError::Cancelled);
        }
        let mut validated = apply_validations(
            ReviewValidationWork {
                protocol_version,
                review_round_id: active.review_round_id,
                revision: 0,
                draft: active,
                prepared,
                fixed_conflicts,
            },
            validations,
        )?;
        for relative_path in validated.pending.drain(..) {
            let asset_version_id = validated
                .draft
                .assets
                .iter()
                .find(|asset| asset.relative_path == relative_path)
                .map(|asset| asset.id)
                .ok_or(ReviewSessionError::AssetUnavailable)?;
            validated.conflicts.push(ReviewConflictSnapshot {
                asset_version_id,
                relative_path,
                kind: ReviewConflictKind::MediaChanged,
            });
        }
        if validated.stable_facts_changed {
            writer
                .save_draft(&PersistedReviewDraft {
                    protocol_version: validated.protocol_version,
                    draft: validated.draft.clone(),
                })
                .map_err(|_| ReviewSessionError::SaveFailed)?;
        }
        Ok(StartedReview {
            active: ActiveReviewSession {
                protocol_version: validated.protocol_version,
                draft: validated.draft,
                prepared: validated.prepared,
                conflicts: validated.conflicts,
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
        if !task.cancellable {
            return false;
        }
        task.cancellation.cancel();
        true
    }

    pub async fn add_feedback(
        &self,
        command: AddReviewFeedback,
    ) -> Result<ReviewSessionSnapshot, ReviewSessionError> {
        let feedback_id = FeedbackId::new();
        let created_at_ms = self.clock.unix_millis();
        let mut state = self.state.lock().await;
        mutate_active(&mut state, command.guard, |draft, bindings| {
            let targets = asset_targets(&command.target_entity_ids, bindings)?;
            let feedback = Feedback::new(feedback_id, command.text, created_at_ms, targets)
                .map_err(|_| ReviewSessionError::InvalidFeedback)?;
            draft
                .upsert_feedback(feedback)
                .map_err(|_| ReviewSessionError::InvalidFeedback)
        })
    }

    pub async fn update_feedback(
        &self,
        command: UpdateReviewFeedback,
    ) -> Result<ReviewSessionSnapshot, ReviewSessionError> {
        let mut state = self.state.lock().await;
        mutate_active(&mut state, command.guard, |draft, bindings| {
            let created_at_ms = draft
                .feedback
                .iter()
                .find(|feedback| feedback.id == command.feedback_id)
                .map(|feedback| feedback.created_at_ms)
                .ok_or(ReviewSessionError::FeedbackNotFound)?;
            let targets = asset_targets(&command.target_entity_ids, bindings)?;
            let feedback = Feedback::new(command.feedback_id, command.text, created_at_ms, targets)
                .map_err(|_| ReviewSessionError::InvalidFeedback)?;
            draft
                .upsert_feedback(feedback)
                .map_err(|_| ReviewSessionError::InvalidFeedback)
        })
    }

    pub async fn delete_feedback(
        &self,
        command: DeleteReviewFeedback,
    ) -> Result<ReviewSessionSnapshot, ReviewSessionError> {
        let mut state = self.state.lock().await;
        mutate_active(&mut state, command.guard, |draft, _bindings| {
            if draft.remove_feedback(command.feedback_id) {
                Ok(())
            } else {
                Err(ReviewSessionError::FeedbackNotFound)
            }
        })
    }

    pub async fn snapshot(&self) -> ReviewSessionSnapshot {
        let state = self.state.lock().await;
        snapshot(&state)
    }

    pub async fn completion_summary(
        &self,
        guard: ReviewMutationGuard,
    ) -> Result<ReviewCompletionProposal, ReviewSessionError> {
        let (task_id, work, cancellation) = {
            let mut state = self.state.lock().await;
            validate_active_guard(&state, guard)?;
            state.completion_proposal = None;
            let work = validation_work(
                state
                    .active
                    .as_ref()
                    .ok_or(ReviewSessionError::InvalidState)?,
            );
            let task = transition_to_preparing(&mut state)?;
            state.phase = ReviewSessionPhase::Completing;
            (task.id, work, task.cancellation)
        };
        let result = self
            .perform_validation(work, cancellation, Arc::new(SilentReviewProgress))
            .await;
        self.finish_completion_summary(task_id, result).await
    }

    async fn perform_validation(
        &self,
        work: ReviewValidationWork,
        cancellation: ReviewTaskCancellation,
        progress: Arc<dyn ReviewProgressPort>,
    ) -> Result<ValidatedReview, ReviewSessionError> {
        let validations = self
            .catalog
            .revalidate_assets(&work.prepared, cancellation.clone(), progress)
            .await
            .map_err(map_asset_error)?;
        if cancellation.is_cancelled() {
            return Err(ReviewSessionError::Cancelled);
        }
        apply_validations(work, validations)
    }

    async fn finish_completion_summary(
        &self,
        task_id: u64,
        result: Result<ValidatedReview, ReviewSessionError>,
    ) -> Result<ReviewCompletionProposal, ReviewSessionError> {
        let mut state = self.state.lock().await;
        if state.task.as_ref().map(|task| task.id) != Some(task_id) {
            drop(state);
            self.task_changed.notify_waiters();
            return Err(ReviewSessionError::Cancelled);
        }
        state.task = None;
        let validated = match result {
            Ok(validated) => validated,
            Err(error) => {
                state.phase = ReviewSessionPhase::Active;
                drop(state);
                self.task_changed.notify_waiters();
                return Err(error);
            }
        };
        let active = state
            .active
            .as_ref()
            .ok_or(ReviewSessionError::InvalidState)?;
        if active.draft.review_round_id != validated.draft.review_round_id
            || active.revision != validated.work_revision
        {
            state.phase = ReviewSessionPhase::Active;
            drop(state);
            self.task_changed.notify_waiters();
            return Err(ReviewSessionError::StaleRevision);
        }
        let next_revision = if validated.stable_facts_changed {
            Some(
                active
                    .revision
                    .checked_add(1)
                    .ok_or(ReviewSessionError::InvalidData)?,
            )
        } else {
            None
        };
        let next_proposal_id = state
            .next_completion_proposal_id
            .checked_add(1)
            .ok_or(ReviewSessionError::InvalidData)?;
        if validated.stable_facts_changed
            && state
                .writer
                .as_deref()
                .ok_or(ReviewSessionError::InvalidState)?
                .save_draft(&PersistedReviewDraft {
                    protocol_version: validated.protocol_version,
                    draft: validated.draft.clone(),
                })
                .is_err()
        {
            state.phase = ReviewSessionPhase::Active;
            drop(state);
            self.task_changed.notify_waiters();
            return Err(ReviewSessionError::SaveFailed);
        }
        let active = state
            .active
            .as_mut()
            .ok_or(ReviewSessionError::InvalidState)?;
        active.draft = validated.draft;
        active.prepared = validated.prepared;
        active.conflicts = validated.conflicts.clone();
        if let Some(next_revision) = next_revision {
            active.revision = next_revision;
        }
        let summary = completion_summary_projection(
            &active.draft,
            active.revision,
            validated.conflicts,
            validated.pending,
        );
        let proposal = ReviewCompletionProposal {
            id: ReviewCompletionProposalId(next_proposal_id),
            summary,
        };
        state.next_completion_proposal_id = next_proposal_id;
        state.completion_proposal = Some(proposal.clone());
        state.phase = ReviewSessionPhase::Active;
        state.error = None;
        drop(state);
        self.task_changed.notify_waiters();
        Ok(proposal)
    }

    pub async fn complete(
        &self,
        proposal_id: ReviewCompletionProposalId,
        guard: ReviewMutationGuard,
        progress: Arc<dyn ReviewProgressPort>,
    ) -> Result<ReviewSessionSnapshot, ReviewSessionError> {
        let (task_id, confirmed, work, cancellation) = {
            let mut state = self.state.lock().await;
            if state.phase != ReviewSessionPhase::Active || state.task.is_some() {
                return Err(ReviewSessionError::InvalidState);
            }
            let proposal = state
                .completion_proposal
                .as_ref()
                .filter(|proposal| proposal.id == proposal_id)
                .cloned()
                .ok_or(ReviewSessionError::StaleCompletionProposal)?;
            if !proposal.summary.can_complete {
                return Err(ReviewSessionError::CompletionBlocked);
            }
            if proposal.summary.guard() != guard {
                return Err(ReviewSessionError::StaleCompletionProposal);
            }
            validate_active_guard(&state, guard)?;
            let work = validation_work(
                state
                    .active
                    .as_ref()
                    .ok_or(ReviewSessionError::InvalidState)?,
            );
            state.completion_proposal = None;
            let task = transition_to_preparing(&mut state)?;
            state.phase = ReviewSessionPhase::Completing;
            (task.id, proposal.summary, work, task.cancellation)
        };
        let result = self
            .perform_validation(work, cancellation.clone(), progress)
            .await;
        self.finish_complete(task_id, confirmed, cancellation, result)
            .await
    }

    async fn finish_complete(
        &self,
        task_id: u64,
        confirmed: ReviewCompletionSummary,
        cancellation: ReviewTaskCancellation,
        result: Result<ValidatedReview, ReviewSessionError>,
    ) -> Result<ReviewSessionSnapshot, ReviewSessionError> {
        let (protocol_version, draft, writer) = {
            let mut state = self.state.lock().await;
            if state.task.as_ref().map(|task| task.id) != Some(task_id) {
                drop(state);
                self.task_changed.notify_waiters();
                return Err(ReviewSessionError::Cancelled);
            }
            let validated = match result {
                Ok(validated) if !cancellation.is_cancelled() => validated,
                Ok(_) => {
                    state.task = None;
                    state.phase = ReviewSessionPhase::Active;
                    drop(state);
                    self.task_changed.notify_waiters();
                    return Err(ReviewSessionError::Cancelled);
                }
                Err(error) => {
                    state.task = None;
                    state.phase = ReviewSessionPhase::Active;
                    drop(state);
                    self.task_changed.notify_waiters();
                    return Err(error);
                }
            };
            let active = state
                .active
                .as_ref()
                .ok_or(ReviewSessionError::InvalidState)?;
            if active.draft.review_round_id != validated.draft.review_round_id
                || active.revision != validated.work_revision
            {
                state.task = None;
                state.phase = ReviewSessionPhase::Active;
                drop(state);
                self.task_changed.notify_waiters();
                return Err(ReviewSessionError::StaleRevision);
            }
            let next_revision = if validated.stable_facts_changed {
                Some(
                    active
                        .revision
                        .checked_add(1)
                        .ok_or(ReviewSessionError::InvalidData)?,
                )
            } else {
                None
            };
            if validated.stable_facts_changed
                && state
                    .writer
                    .as_deref()
                    .ok_or(ReviewSessionError::InvalidState)?
                    .save_draft(&PersistedReviewDraft {
                        protocol_version: validated.protocol_version,
                        draft: validated.draft.clone(),
                    })
                    .is_err()
            {
                state.task = None;
                state.phase = ReviewSessionPhase::Active;
                drop(state);
                self.task_changed.notify_waiters();
                return Err(ReviewSessionError::SaveFailed);
            }
            let active = state
                .active
                .as_mut()
                .ok_or(ReviewSessionError::InvalidState)?;
            active.draft = validated.draft;
            active.prepared = validated.prepared;
            active.conflicts = validated.conflicts.clone();
            if let Some(next_revision) = next_revision {
                active.revision = next_revision;
            }
            let current = completion_summary_projection(
                &active.draft,
                active.revision,
                validated.conflicts,
                validated.pending,
            );
            if current != confirmed {
                state.task = None;
                state.phase = ReviewSessionPhase::Active;
                drop(state);
                self.task_changed.notify_waiters();
                return Err(ReviewSessionError::CompletionChanged);
            }
            let protocol_version = active.protocol_version;
            let draft = active.draft.clone();
            if cancellation.is_cancelled() {
                state.task = None;
                state.phase = ReviewSessionPhase::Active;
                drop(state);
                self.task_changed.notify_waiters();
                return Err(ReviewSessionError::Cancelled);
            }
            state
                .task
                .as_mut()
                .ok_or(ReviewSessionError::InvalidState)?
                .cancellable = false;
            let writer = state
                .writer
                .take()
                .ok_or(ReviewSessionError::InvalidState)?;
            (protocol_version, draft, writer)
        };

        let completed = match draft.complete(self.clock.unix_millis()) {
            Ok(completed) => completed,
            Err(_) => {
                return self
                    .restore_after_prepublication_error(
                        task_id,
                        writer,
                        ReviewSessionError::InvalidData,
                    )
                    .await;
            }
        };
        let publish_result = writer.publish(&ReviewPublication {
            protocol_version,
            snapshot: completed.clone(),
            artifacts: vec![],
        });
        let directly_verified = publish_result.is_ok()
            && repository_has_exact_completed(writer.as_ref(), self.project_id, &completed);
        drop(writer);
        let verified = if directly_verified {
            true
        } else {
            match self.repositories.open_writer() {
                Ok(recovered) => {
                    let verified = repository_has_exact_completed(
                        recovered.as_ref(),
                        self.project_id,
                        &completed,
                    );
                    drop(recovered);
                    verified
                }
                Err(_) => false,
            }
        };

        let mut state = self.state.lock().await;
        if state.task.as_ref().map(|task| task.id) != Some(task_id) {
            state.writer = None;
            state.active = None;
            state.phase = ReviewSessionPhase::RecoveryRequired;
            state.error = Some(user_error(ReviewSessionError::RecoveryRequired));
            drop(state);
            self.catalog.release_tracking();
            self.task_changed.notify_waiters();
            return Err(ReviewSessionError::RecoveryRequired);
        }
        state.task = None;
        state.writer = None;
        state.completion_proposal = None;
        state.active = None;
        if verified {
            state.phase = ReviewSessionPhase::CompletedReadOnly;
            state.displayed_completed = Some(completed);
            state.error = None;
            let result = snapshot(&state);
            drop(state);
            self.catalog.release_tracking();
            self.task_changed.notify_waiters();
            Ok(result)
        } else {
            state.phase = ReviewSessionPhase::RecoveryRequired;
            state.displayed_completed = None;
            state.error = Some(user_error(ReviewSessionError::RecoveryRequired));
            drop(state);
            self.catalog.release_tracking();
            self.task_changed.notify_waiters();
            Err(ReviewSessionError::RecoveryRequired)
        }
    }

    async fn restore_after_prepublication_error(
        &self,
        task_id: u64,
        writer: Box<dyn ReviewRepositoryPort>,
        error: ReviewSessionError,
    ) -> Result<ReviewSessionSnapshot, ReviewSessionError> {
        let mut state = self.state.lock().await;
        if state.task.as_ref().map(|task| task.id) == Some(task_id) {
            state.task = None;
            state.writer = Some(writer);
            state.phase = ReviewSessionPhase::Active;
        } else {
            drop(writer);
            state.active = None;
            state.phase = ReviewSessionPhase::RecoveryRequired;
        }
        drop(state);
        self.task_changed.notify_waiters();
        Err(error)
    }

    pub async fn abandon(
        &self,
        guard: ReviewMutationGuard,
    ) -> Result<ReviewSessionSnapshot, ReviewSessionError> {
        let writer = {
            let mut state = self.state.lock().await;
            validate_active_guard(&state, guard)?;
            let active = state
                .active
                .as_ref()
                .ok_or(ReviewSessionError::InvalidState)?;
            let stream_id = active.draft.review_stream_id;
            let round_id = active.draft.review_round_id;
            if state
                .writer
                .as_deref()
                .ok_or(ReviewSessionError::InvalidState)?
                .delete_draft(stream_id, round_id)
                .is_err()
            {
                return Err(ReviewSessionError::AbandonFailed);
            }
            state.active = None;
            state.completion_proposal = None;
            state.displayed_completed = None;
            state.resume = None;
            state.proposal = None;
            state.phase = ReviewSessionPhase::Idle;
            state.error = None;
            state.writer.take()
        };
        drop(writer);
        self.catalog.release_tracking();
        let state = self.state.lock().await;
        Ok(snapshot(&state))
    }

    pub async fn shutdown(&self) {
        loop {
            let notified = self.task_changed.notified();
            let writer = {
                let mut state = self.state.lock().await;
                if let Some(task) = state.task.as_ref() {
                    if task.cancellable {
                        task.cancellation.cancel();
                    }
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
        let next_completion_proposal_id = state.next_completion_proposal_id;
        *state = ReviewSessionState::new();
        state.next_proposal_id = next_proposal_id;
        state.next_task_id = next_task_id;
        state.next_completion_proposal_id = next_completion_proposal_id;
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
            completion_proposal: None,
            next_completion_proposal_id: 0,
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
        cancellable: true,
    };
    state.phase = ReviewSessionPhase::Preparing;
    state.error = None;
    state.task = Some(ReviewTask {
        id: task.id,
        cancellation: task.cancellation.clone(),
        return_phase: task.return_phase,
        cancellable: task.cancellable,
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
    state.completion_proposal = None;
    state.displayed_completed = None;
    state.error = None;
    state.active = Some(active);
    state.writer = Some(writer);
}

fn mutate_active<F>(
    state: &mut ReviewSessionState,
    guard: ReviewMutationGuard,
    change: F,
) -> Result<ReviewSessionSnapshot, ReviewSessionError>
where
    F: FnOnce(
        &mut ReviewDraft,
        &HashMap<EntityId, AssetVersionId>,
    ) -> Result<(), ReviewSessionError>,
{
    if state.phase != ReviewSessionPhase::Active || state.task.is_some() {
        return Err(ReviewSessionError::InvalidState);
    }
    let active = state
        .active
        .as_ref()
        .ok_or(ReviewSessionError::InvalidState)?;
    if guard.review_round_id != active.draft.review_round_id {
        return Err(ReviewSessionError::StaleRound);
    }
    if guard.expected_revision != active.revision {
        return Err(ReviewSessionError::StaleRevision);
    }
    let next_revision = active
        .revision
        .checked_add(1)
        .ok_or(ReviewSessionError::InvalidData)?;
    let bindings = active
        .prepared
        .iter()
        .map(|prepared| (prepared.entity_id, prepared.asset.id))
        .collect();
    let protocol_version = active.protocol_version;
    let mut draft = active.draft.clone();
    change(&mut draft, &bindings)?;
    state
        .writer
        .as_deref()
        .ok_or(ReviewSessionError::InvalidState)?
        .save_draft(&PersistedReviewDraft {
            protocol_version,
            draft: draft.clone(),
        })
        .map_err(|_| ReviewSessionError::SaveFailed)?;
    let active = state
        .active
        .as_mut()
        .ok_or(ReviewSessionError::InvalidState)?;
    active.draft = draft;
    active.revision = next_revision;
    state.completion_proposal = None;
    state.error = None;
    Ok(snapshot(state))
}

fn validate_active_guard(
    state: &ReviewSessionState,
    guard: ReviewMutationGuard,
) -> Result<(), ReviewSessionError> {
    if state.phase != ReviewSessionPhase::Active || state.task.is_some() {
        return Err(ReviewSessionError::InvalidState);
    }
    let active = state
        .active
        .as_ref()
        .ok_or(ReviewSessionError::InvalidState)?;
    if active.draft.review_round_id != guard.review_round_id {
        return Err(ReviewSessionError::StaleRound);
    }
    if active.revision != guard.expected_revision {
        return Err(ReviewSessionError::StaleRevision);
    }
    Ok(())
}

fn validation_work(active: &ActiveReviewSession) -> ReviewValidationWork {
    let prepared_ids = active
        .prepared
        .iter()
        .map(|prepared| prepared.asset.id)
        .collect::<HashSet<_>>();
    ReviewValidationWork {
        protocol_version: active.protocol_version,
        review_round_id: active.draft.review_round_id,
        revision: active.revision,
        draft: active.draft.clone(),
        prepared: active.prepared.clone(),
        fixed_conflicts: active
            .conflicts
            .iter()
            .filter(|conflict| !prepared_ids.contains(&conflict.asset_version_id))
            .cloned()
            .collect(),
    }
}

fn apply_validations(
    work: ReviewValidationWork,
    validations: Vec<ReviewAssetValidation>,
) -> Result<ValidatedReview, ReviewSessionError> {
    if work.review_round_id != work.draft.review_round_id
        || validations.len() != work.prepared.len()
    {
        return Err(ReviewSessionError::AssetUnavailable);
    }
    let mut draft = work.draft;
    let previous_unreviewable = draft.unreviewable.clone();
    let existing_failures = draft
        .unreviewable
        .iter()
        .map(|item| (item.asset_version_id, item.failure))
        .collect::<HashMap<_, _>>();
    let mut resolved_failures = HashMap::new();
    let mut prepared = Vec::with_capacity(work.prepared.len());
    let mut conflicts = work.fixed_conflicts;
    let mut pending = Vec::new();
    for (original, validation) in work.prepared.into_iter().zip(validations) {
        match validation {
            ReviewAssetValidation::Current(current) if same_fixed_member(&original, &current) => {
                resolved_failures.insert(current.asset.id, current.failure);
                prepared.push(current);
            }
            ReviewAssetValidation::Conflict {
                asset_version_id,
                relative_path,
                kind,
            } if asset_version_id == original.asset.id
                && relative_path == original.asset.relative_path =>
            {
                conflicts.push(ReviewConflictSnapshot {
                    asset_version_id,
                    relative_path,
                    kind,
                });
                prepared.push(original);
            }
            ReviewAssetValidation::Pending {
                asset_version_id,
                relative_path,
            } if asset_version_id == original.asset.id
                && relative_path == original.asset.relative_path =>
            {
                pending.push(relative_path);
                prepared.push(original);
            }
            _ => return Err(ReviewSessionError::AssetUnavailable),
        }
    }
    let stable_failures = draft
        .assets
        .iter()
        .filter_map(|asset| {
            resolved_failures
                .get(&asset.id)
                .copied()
                .unwrap_or_else(|| existing_failures.get(&asset.id).copied())
                .map(|failure| (asset.id, failure))
        })
        .collect::<Vec<_>>();
    draft.unreviewable.clear();
    for (asset_version_id, failure) in stable_failures {
        draft
            .mark_unreviewable(asset_version_id, failure)
            .map_err(|_| ReviewSessionError::InvalidData)?;
    }
    let stable_facts_changed = draft.unreviewable != previous_unreviewable;
    Ok(ValidatedReview {
        protocol_version: work.protocol_version,
        work_revision: work.revision,
        draft,
        prepared,
        conflicts,
        pending,
        stable_facts_changed,
    })
}

fn completion_summary_projection(
    draft: &ReviewDraft,
    revision: u64,
    conflicts: Vec<ReviewConflictSnapshot>,
    pending: Vec<RelativePath>,
) -> ReviewCompletionSummary {
    let counts = draft_counts(draft);
    ReviewCompletionSummary {
        review_round_id: draft.review_round_id,
        revision,
        total: counts.total,
        revise: counts.revise,
        unreviewable: counts.unreviewable,
        default_pass: counts
            .total
            .saturating_sub(counts.revise + counts.unreviewable),
        feedback: draft
            .feedback
            .iter()
            .map(|feedback| ReviewFeedbackSummary {
                feedback_id: feedback.id,
                text: feedback.text.clone(),
                target_count: feedback.targets.len() as u32,
            })
            .collect(),
        can_complete: conflicts.is_empty() && pending.is_empty(),
        conflicts,
        pending,
    }
}

fn asset_targets(
    entity_ids: &[EntityId],
    bindings: &HashMap<EntityId, AssetVersionId>,
) -> Result<Vec<FeedbackTarget>, ReviewSessionError> {
    if entity_ids.is_empty() {
        return Err(ReviewSessionError::InvalidFeedback);
    }
    let mut seen = HashSet::with_capacity(entity_ids.len());
    entity_ids
        .iter()
        .map(|entity_id| {
            if !seen.insert(*entity_id) {
                return Err(ReviewSessionError::InvalidFeedback);
            }
            let asset_version_id = bindings
                .get(entity_id)
                .copied()
                .ok_or(ReviewSessionError::InvalidFeedback)?;
            Ok(FeedbackTarget {
                asset_version_id,
                anchor: FeedbackAnchor::Asset,
            })
        })
        .collect()
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

fn exact_head_is_published(
    catalog: &ReviewCatalog,
    stream_id: ReviewStreamId,
    round_id: ReviewRoundId,
) -> bool {
    let mut matches = catalog
        .streams
        .iter()
        .filter(|stream| stream.review_stream_id == stream_id);
    let Some(stream) = matches.next() else {
        return false;
    };
    matches.next().is_none()
        && stream.latest_completed_round_id == Some(round_id)
        && stream.completed_round_ids().last() == Some(round_id)
        && stream
            .completed_round_ids()
            .any(|candidate| candidate == round_id)
}

fn repository_has_exact_completed(
    repository: &dyn ReviewRepositoryPort,
    project_id: ProjectId,
    completed: &ReviewSnapshot,
) -> bool {
    let Ok(catalog) = repository.load_catalog() else {
        return false;
    };
    catalog.project_id == project_id
        && exact_head_is_published(
            &catalog,
            completed.review_stream_id,
            completed.review_round_id,
        )
        && repository.load_completed(completed.review_stream_id, completed.review_round_id)
            == Ok(Some(completed.clone()))
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
        pass: 0,
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
        kind: ReviewAssetConflictKind::ContentChanged,
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
        ReviewRepositoryError::UnsupportedVersion => ReviewSessionError::UnsupportedVersion,
        ReviewRepositoryError::RecoveryRequired | ReviewRepositoryError::InvalidData => {
            ReviewSessionError::RecoveryRequired
        }
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
        ReviewSessionError::RecoveryRequired | ReviewSessionError::UnsupportedVersion => {
            ReviewSessionPhase::RecoveryRequired
        }
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
