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
