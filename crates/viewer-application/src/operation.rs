use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileSnapshot {
    pub len: u64,
    pub volume_id: u64,
    pub file_id: Option<u128>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum FileOperationError {
    #[error("file operation path is outside the current project")]
    OutsideProject,
    #[error("Viewer metadata is not a valid file operation target")]
    ReservedPath,
    #[error("file operation requires a destination")]
    DestinationRequired,
    #[error("file operation source does not exist")]
    SourceMissing,
    #[error("file operation destination already exists")]
    DestinationExists,
    #[error("file identity changed before the operation completed")]
    IdentityChanged,
    #[error("copied file did not match its recorded size and hash")]
    VerificationFailed,
    #[error("file operation was cancelled")]
    Cancelled,
    #[error("{action} failed at {path}: {message}")]
    Io {
        action: &'static str,
        path: PathBuf,
        message: String,
    },
}

impl FileOperationError {
    pub fn io(action: &'static str, path: impl Into<PathBuf>, error: &std::io::Error) -> Self {
        Self::Io {
            action,
            path: path.into(),
            message: error.to_string(),
        }
    }
}
