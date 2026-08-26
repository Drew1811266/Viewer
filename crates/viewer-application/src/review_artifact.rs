use crate::ReviewProtocolVersion;
use std::path::PathBuf;
use viewer_domain::review::ReviewSnapshot;
use viewer_domain::{AssetVersionId, FeedbackId};

pub const MAX_REVIEW_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_REVIEW_ARTIFACT_PIXELS: u64 = 16_777_216;
pub const MAX_REVIEW_BUNDLE_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewArtifactAnnotation {
    pub ordinal: u32,
    pub feedback_id: FeedbackId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewRenderedArtifact {
    pub asset_version_id: AssetVersionId,
    pub temporary_path: PathBuf,
    pub media_type: String,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
    pub blake3: [u8; 32],
    pub annotations: Vec<ReviewArtifactAnnotation>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewPublication {
    pub protocol_version: ReviewProtocolVersion,
    pub snapshot: ReviewSnapshot,
    pub artifacts: Vec<ReviewRenderedArtifact>,
}
