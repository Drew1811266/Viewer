use std::{path::PathBuf, sync::Arc};
use viewer_domain::{SessionId, file::FileNode, search::Generation};

use crate::{
    ScanPort,
    scheduler::{TaskClass, TaskCoordinator},
};

pub const SCAN_BATCH_SIZE: usize = 128;
pub const SCAN_BATCH_MAX_LATENCY_MS: u64 = 20;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScanBatch {
    pub generation: Generation,
    pub nodes: Vec<FileNode>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScanTotals {
    pub folders: u64,
    pub files: u64,
    pub failed: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScanEvent {
    Folders {
        generation: Generation,
        nodes: Vec<FileNode>,
    },
    Files {
        generation: Generation,
        nodes: Vec<FileNode>,
    },
    FailedItem {
        generation: Generation,
        relative_display: String,
        code: String,
    },
    Finished {
        generation: Generation,
        totals: ScanTotals,
    },
}

impl ScanEvent {
    pub const fn generation(&self) -> Generation {
        match self {
            Self::Folders { generation, .. }
            | Self::Files { generation, .. }
            | Self::FailedItem { generation, .. }
            | Self::Finished { generation, .. } => *generation,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScanRequest {
    pub session_id: SessionId,
    pub generation: Generation,
    pub root: PathBuf,
}

pub type ScanSink = tokio::sync::mpsc::Sender<ScanEvent>;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ScanError {
    #[error("project root cannot be read: {0}")]
    RootUnreadable(String),
    #[error("scan request was cancelled")]
    Cancelled,
    #[error("scan worker failed: {0}")]
    WorkerFailed(String),
}

pub struct CoordinatedScan {
    inner: Arc<dyn ScanPort>,
    coordinator: Arc<TaskCoordinator>,
}

impl CoordinatedScan {
    pub fn new(inner: Arc<dyn ScanPort>, coordinator: Arc<TaskCoordinator>) -> Self {
        Self { inner, coordinator }
    }
}

#[async_trait::async_trait]
impl ScanPort for CoordinatedScan {
    async fn scan(&self, request: ScanRequest, sink: ScanSink) -> Result<(), ScanError> {
        let session_id = request.session_id;
        let generation = request.generation;
        if !self.coordinator.is_publishable(session_id, generation) {
            return Err(ScanError::Cancelled);
        }
        let (worker_sink, mut worker_events) =
            tokio::sync::mpsc::channel(TaskClass::FolderPublication.queue_capacity());
        let inner = Arc::clone(&self.inner);
        let worker = tokio::spawn(async move { inner.scan(request, worker_sink).await });
        let mut output_closed = false;
        while let Some(event) = worker_events.recv().await {
            let event_is_current = event.generation() == generation
                && self.coordinator.is_publishable(session_id, generation);
            if event_is_current && !output_closed {
                let permit = match sink.reserve().await {
                    Ok(permit) => permit,
                    Err(_) => {
                        output_closed = true;
                        continue;
                    }
                };
                self.coordinator
                    .publish_if_current(session_id, generation, || permit.send(event));
            }
        }
        let worker_result = worker
            .await
            .map_err(|error| ScanError::WorkerFailed(error.to_string()))?;
        if output_closed || !self.coordinator.is_publishable(session_id, generation) {
            return Err(ScanError::Cancelled);
        }
        worker_result
    }
}
