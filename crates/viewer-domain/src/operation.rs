use crate::{EntityId, OperationId, RelativePath};
use serde::{Deserialize, Serialize};

pub type BatchId = OperationId;

pub const MAX_BATCH_RESULT_PAGE_SIZE: usize = 200;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchLifecycle {
    Queued,
    Running,
    Cancelling,
    Completed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchItemStatus {
    Completed,
    Failed,
    Skipped,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchResultCode {
    Renamed,
    Copied,
    Moved,
    MovedToTrash,
    ConflictSkipped,
    Cancelled,
    SessionStale,
    SourceMissing,
    DestinationOccupied,
    PermissionDenied,
    VerificationFailed,
    ProjectionStale,
    BackendUnavailable,
    InvalidTarget,
}

impl BatchResultCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Renamed => "renamed",
            Self::Copied => "copied",
            Self::Moved => "moved",
            Self::MovedToTrash => "moved_to_trash",
            Self::ConflictSkipped => "conflict_skipped",
            Self::Cancelled => "cancelled",
            Self::SessionStale => "session_stale",
            Self::SourceMissing => "source_missing",
            Self::DestinationOccupied => "destination_occupied",
            Self::PermissionDenied => "permission_denied",
            Self::VerificationFailed => "verification_failed",
            Self::ProjectionStale => "projection_stale",
            Self::BackendUnavailable => "backend_unavailable",
            Self::InvalidTarget => "invalid_target",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BatchProgress {
    pub batch_id: BatchId,
    pub lifecycle: BatchLifecycle,
    pub requested: u32,
    pub completed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub cancelled: u32,
    pub active_entity_id: Option<EntityId>,
}

impl BatchProgress {
    pub const fn processed(&self) -> u32 {
        self.completed + self.failed + self.skipped + self.cancelled
    }

    pub fn counts_are_consistent(&self) -> bool {
        self.processed() <= self.requested
            && (self.lifecycle != BatchLifecycle::Completed || self.processed() == self.requested)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BatchItemResult {
    pub entity_id: EntityId,
    pub relative_path: RelativePath,
    pub status: BatchItemStatus,
    pub code: BatchResultCode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchResultPage {
    pub total: u32,
    pub offset: u32,
    pub items: Vec<BatchItemResult>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchSummary {
    batch_id: BatchId,
    lifecycle: BatchLifecycle,
    requested: u32,
    completed: u32,
    failed: u32,
    skipped: u32,
    cancelled: u32,
    results: Vec<BatchItemResult>,
}

impl BatchSummary {
    pub fn try_from_results(
        batch_id: BatchId,
        requested: u32,
        results: Vec<BatchItemResult>,
    ) -> Option<Self> {
        if usize::try_from(requested).ok() != Some(results.len()) {
            return None;
        }
        let mut summary = Self {
            batch_id,
            lifecycle: BatchLifecycle::Completed,
            requested,
            completed: 0,
            failed: 0,
            skipped: 0,
            cancelled: 0,
            results,
        };
        for item in &summary.results {
            match item.status {
                BatchItemStatus::Completed => summary.completed += 1,
                BatchItemStatus::Failed => summary.failed += 1,
                BatchItemStatus::Skipped => summary.skipped += 1,
                BatchItemStatus::Cancelled => summary.cancelled += 1,
            }
        }
        debug_assert!(summary.counts_are_consistent());
        Some(summary)
    }

    pub const fn batch_id(&self) -> BatchId {
        self.batch_id
    }

    pub const fn lifecycle(&self) -> BatchLifecycle {
        self.lifecycle
    }

    pub const fn requested(&self) -> u32 {
        self.requested
    }

    pub const fn completed(&self) -> u32 {
        self.completed
    }

    pub const fn failed(&self) -> u32 {
        self.failed
    }

    pub const fn skipped(&self) -> u32 {
        self.skipped
    }

    pub const fn cancelled(&self) -> u32 {
        self.cancelled
    }

    pub const fn processed(&self) -> u32 {
        self.completed + self.failed + self.skipped + self.cancelled
    }

    pub const fn counts_are_consistent(&self) -> bool {
        self.processed() == self.requested
    }

    pub fn result_page(&self, offset: usize, limit: usize) -> BatchResultPage {
        let offset = offset.min(self.results.len());
        let limit = limit.min(MAX_BATCH_RESULT_PAGE_SIZE);
        let end = offset.saturating_add(limit).min(self.results.len());
        BatchResultPage {
            total: self.requested,
            offset: u32::try_from(offset).unwrap_or(u32::MAX),
            items: self.results[offset..end].to_vec(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    Rename,
    Copy,
    Move,
    Trash,
    SetReviewState,
    SetFavorite,
}

impl OperationKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rename => "rename",
            Self::Copy => "copy",
            Self::Move => "move",
            Self::Trash => "trash",
            Self::SetReviewState => "set_review_state",
            Self::SetFavorite => "set_favorite",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    Skip,
    KeepBoth,
    Replace,
}

impl ConflictPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Skip => "skip",
            Self::KeepBoth => "keep_both",
            Self::Replace => "replace",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    Prepared,
    Staged,
    FsApplied,
    Verified,
    MetaCommitted,
    IndexSynced,
    Completed,
    Failed,
}

impl OperationState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Staged => "staged",
            Self::FsApplied => "fs_applied",
            Self::Verified => "verified",
            Self::MetaCommitted => "meta_committed",
            Self::IndexSynced => "index_synced",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn transition_to(&mut self, next: Self) -> Result<(), OperationTransitionError> {
        let allowed = matches!(
            (*self, next),
            (Self::Prepared, Self::Staged)
                | (Self::Staged, Self::FsApplied)
                | (Self::FsApplied, Self::Verified)
                | (Self::Verified, Self::MetaCommitted)
                | (Self::MetaCommitted, Self::IndexSynced)
                | (Self::IndexSynced, Self::Completed)
        ) || (next == Self::Failed
            && !matches!(*self, Self::Completed | Self::Failed));

        if allowed {
            *self = next;
            Ok(())
        } else {
            Err(OperationTransitionError::Invalid {
                from: *self,
                to: next,
            })
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum OperationTransitionError {
    #[error("invalid operation state transition from {from:?} to {to:?}")]
    Invalid {
        from: OperationState,
        to: OperationState,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OperationItemPlan {
    pub batch_id: OperationId,
    pub operation_id: OperationId,
    pub entity_id: EntityId,
    pub kind: OperationKind,
    pub source: RelativePath,
    pub destination: Option<RelativePath>,
    pub conflict_policy: ConflictPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OperationPlan {
    pub batch_id: OperationId,
    pub kind: OperationKind,
    pub items: Vec<OperationItemPlan>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SequenceRule {
    pub start: u32,
    pub digits: u8,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RenameRuleSet {
    pub find: String,
    pub replacement: String,
    pub prefix: String,
    pub suffix: String,
    pub sequence: Option<SequenceRule>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RenameTarget {
    pub entity_id: EntityId,
    pub relative_path: RelativePath,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenameErrorCode {
    InvalidSequence,
    SequenceOutOfRange,
    EmptyName,
    DotName,
    ContainsSeparator,
    ContainsNul,
    ReservedName,
    TemporaryName,
    NameTooLong,
    NoOp,
    DuplicateSource,
    DuplicateDestination,
    CaseCollision,
    DestinationOccupied,
    SourceMissing,
    UnsafeParent,
    DestinationReadOnly,
}

impl RenameErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidSequence => "invalid_sequence",
            Self::SequenceOutOfRange => "sequence_out_of_range",
            Self::EmptyName => "empty_name",
            Self::DotName => "dot_name",
            Self::ContainsSeparator => "contains_separator",
            Self::ContainsNul => "contains_nul",
            Self::ReservedName => "reserved_name",
            Self::TemporaryName => "temporary_name",
            Self::NameTooLong => "name_too_long",
            Self::NoOp => "no_op",
            Self::DuplicateSource => "duplicate_source",
            Self::DuplicateDestination => "duplicate_destination",
            Self::CaseCollision => "case_collision",
            Self::DestinationOccupied => "destination_occupied",
            Self::SourceMissing => "source_missing",
            Self::UnsafeParent => "unsafe_parent",
            Self::DestinationReadOnly => "destination_read_only",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RenamePreviewRow {
    pub entity_id: EntityId,
    pub source: RelativePath,
    pub destination: Option<RelativePath>,
    pub proposed_name: String,
    pub errors: Vec<RenameErrorCode>,
}

impl RenamePreviewRow {
    pub fn push_error(&mut self, error: RenameErrorCode) {
        if !self.errors.contains(&error) {
            self.errors.push(error);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RenamePreflight {
    pub rows: Vec<RenamePreviewRow>,
    pub executable: bool,
}

impl RenamePreflight {
    pub fn refresh_executable(&mut self) {
        self.executable =
            !self.rows.is_empty() && self.rows.iter().all(|row| row.errors.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BatchItemResult, BatchItemStatus, BatchResultCode, BatchSummary, OperationState,
        OperationTransitionError,
    };
    use crate::{EntityId, OperationId, RelativePath};

    #[test]
    fn operation_accepts_only_forward_protocol_transitions() {
        let mut state = OperationState::Prepared;
        state.transition_to(OperationState::Staged).unwrap();
        state.transition_to(OperationState::FsApplied).unwrap();
        state.transition_to(OperationState::Verified).unwrap();
        state.transition_to(OperationState::MetaCommitted).unwrap();
        state.transition_to(OperationState::IndexSynced).unwrap();
        state.transition_to(OperationState::Completed).unwrap();
        assert_eq!(state, OperationState::Completed);
    }

    #[test]
    fn operation_rejects_skipping_verification() {
        let mut state = OperationState::FsApplied;
        assert_eq!(
            state.transition_to(OperationState::MetaCommitted),
            Err(OperationTransitionError::Invalid {
                from: OperationState::FsApplied,
                to: OperationState::MetaCommitted,
            })
        );
        assert_eq!(state, OperationState::FsApplied);
    }

    #[test]
    fn failure_is_terminal_and_completed_cannot_fail() {
        let mut failed = OperationState::Verified;
        failed.transition_to(OperationState::Failed).unwrap();
        assert!(failed.transition_to(OperationState::Prepared).is_err());

        let mut completed = OperationState::Completed;
        assert!(completed.transition_to(OperationState::Failed).is_err());
    }

    #[test]
    fn batch_summary_requires_exactly_one_terminal_result_per_request() {
        let result = BatchItemResult {
            entity_id: EntityId::new(),
            relative_path: RelativePath::parse("front.png").unwrap(),
            status: BatchItemStatus::Completed,
            code: BatchResultCode::Copied,
        };

        assert!(
            BatchSummary::try_from_results(OperationId::new(), 2, vec![result.clone()]).is_none()
        );
        let summary = BatchSummary::try_from_results(OperationId::new(), 1, vec![result]).unwrap();
        assert!(summary.counts_are_consistent());
        assert_eq!(summary.requested(), 1);
        assert_eq!(summary.completed(), 1);
    }
}
