use std::path::PathBuf;
use viewer_domain::{SessionId, file::FileNode, search::Generation};

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
}
