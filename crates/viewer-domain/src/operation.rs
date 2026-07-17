use crate::{EntityId, OperationId, RelativePath};
use serde::{Deserialize, Serialize};

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
    use super::{OperationState, OperationTransitionError};

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
}
