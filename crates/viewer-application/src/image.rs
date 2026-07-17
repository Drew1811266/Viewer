use std::path::PathBuf;
use viewer_domain::{EntityId, SessionId, image::ImageRepresentationKind};

#[derive(Clone, Debug)]
pub struct ImageRequest {
    pub session_id: SessionId,
    pub entity_id: EntityId,
    pub source: PathBuf,
    pub kind: ImageRepresentationKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageBackend {
    QuickLook,
    ImageIo,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageArtifact {
    pub cache_path: PathBuf,
    pub mime: &'static str,
    pub width: u32,
    pub height: u32,
    pub backend: ImageBackend,
}

#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("unsupported image")]
    Unsupported,
    #[error("image is corrupt")]
    Corrupt,
    #[error("decode exceeds budget")]
    BudgetExceeded,
    #[error("image request was cancelled")]
    Cancelled,
    #[error("image io failed: {0}")]
    Io(String),
}
