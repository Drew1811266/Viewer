use async_trait::async_trait;
use viewer_domain::{EntityId, OperationId, RelativePath, operation::OperationKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitStage {
    Metadata,
    Index,
}

impl CommitStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Metadata => "metadata",
            Self::Index => "index",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationCommit {
    pub operation_id: OperationId,
    pub entity_id: EntityId,
    pub kind: OperationKind,
    pub source: RelativePath,
    pub destination: Option<RelativePath>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataCommitOutcome {
    CallerAdvancesJournal,
    JournalAdvanced,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{stage:?} operation commit failed with code {code}")]
pub struct OperationCommitError {
    pub stage: CommitStage,
    pub code: String,
}

impl OperationCommitError {
    pub fn new(stage: CommitStage, code: impl Into<String>) -> Self {
        Self {
            stage,
            code: code.into(),
        }
    }
}

#[async_trait]
pub trait OperationCommitPort: Send + Sync {
    async fn commit_metadata(&self, commit: &OperationCommit) -> Result<(), OperationCommitError>;

    async fn commit_metadata_barrier(
        &self,
        commit: &OperationCommit,
    ) -> Result<MetadataCommitOutcome, OperationCommitError> {
        self.commit_metadata(commit).await?;
        Ok(MetadataCommitOutcome::CallerAdvancesJournal)
    }

    async fn sync_index(&self, commit: &OperationCommit) -> Result<(), OperationCommitError>;
}
