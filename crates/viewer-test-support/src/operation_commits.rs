use async_trait::async_trait;
use std::sync::Mutex;
use viewer_application::{CommitStage, OperationCommit, OperationCommitError, OperationCommitPort};

#[derive(Default)]
pub struct InMemoryOperationCommitPort {
    events: Mutex<Vec<(CommitStage, OperationCommit)>>,
}

impl InMemoryOperationCommitPort {
    pub fn events(&self) -> Vec<(CommitStage, OperationCommit)> {
        self.events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn record(&self, stage: CommitStage, commit: &OperationCommit) {
        self.events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push((stage, commit.clone()));
    }
}

#[async_trait]
impl OperationCommitPort for InMemoryOperationCommitPort {
    async fn commit_metadata(&self, commit: &OperationCommit) -> Result<(), OperationCommitError> {
        self.record(CommitStage::Metadata, commit);
        Ok(())
    }

    async fn sync_index(&self, commit: &OperationCommit) -> Result<(), OperationCommitError> {
        self.record(CommitStage::Index, commit);
        Ok(())
    }
}
