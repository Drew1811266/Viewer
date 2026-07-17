use std::path::{Path, PathBuf};
use viewer_domain::{OperationId, SessionId, search::Generation};

pub const WATCHER_DEBOUNCE_MS: u64 = 250;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FileIdentity {
    pub volume: u64,
    pub file: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatcherEventKind {
    Added,
    Removed,
    Modified,
    Renamed,
    Overflow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatcherEvent {
    pub kind: WatcherEventKind,
    pub paths: Vec<PathBuf>,
    pub identity: Option<FileIdentity>,
}

impl WatcherEvent {
    pub fn new(kind: WatcherEventKind, paths: Vec<PathBuf>) -> Self {
        Self {
            kind,
            paths,
            identity: None,
        }
    }

    pub fn added(path: PathBuf) -> Self {
        Self::new(WatcherEventKind::Added, vec![path])
    }

    pub fn removed(path: PathBuf) -> Self {
        Self::new(WatcherEventKind::Removed, vec![path])
    }

    pub fn modified(path: PathBuf) -> Self {
        Self::new(WatcherEventKind::Modified, vec![path])
    }

    pub fn renamed(old_path: PathBuf, new_path: PathBuf) -> Self {
        Self::new(WatcherEventKind::Renamed, vec![old_path, new_path])
    }

    pub fn overflow(paths: Vec<PathBuf>) -> Self {
        Self::new(WatcherEventKind::Overflow, paths)
    }

    pub fn with_identity(mut self, identity: FileIdentity) -> Self {
        self.identity = Some(identity);
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReconcileReason {
    ExternalChange,
    ExpectedViewerChange(OperationId),
    Overflow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconcileRequest {
    pub session_id: SessionId,
    pub generation: Generation,
    pub roots: Vec<PathBuf>,
    pub reason: ReconcileReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReconcileSummary {
    pub reason: ReconcileReason,
    pub added: u64,
    pub removed: u64,
    pub modified: u64,
    pub moved: u64,
    pub marker_paths_moved: u64,
    pub failed: u64,
}

pub type WatcherSink = tokio::sync::mpsc::Sender<Vec<WatcherEvent>>;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum WatcherError {
    #[error("watcher failed at {path}: {message}")]
    Backend { path: PathBuf, message: String },
}

pub trait WatchSubscription: Send {}

pub trait WatcherPort: Send + Sync {
    fn watch(
        &self,
        root: &Path,
        sink: WatcherSink,
    ) -> Result<Box<dyn WatchSubscription>, WatcherError>;
}
