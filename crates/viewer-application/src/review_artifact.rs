use crate::{ReviewProtocolVersion, ReviewTaskCancellation};
use async_trait::async_trait;
use std::collections::HashSet;
use std::path::PathBuf;
use viewer_domain::review::{
    AssetVersion, FeedbackAnchor, MAX_FEEDBACK_ITEMS_PER_ROUND, MAX_IMAGE_STROKE_POINTS_PER_ROUND,
    ReviewSnapshot,
};
use viewer_domain::{AssetVersionId, FeedbackId};

pub const MAX_REVIEW_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_REVIEW_ARTIFACT_PIXELS: u64 = 16_777_216;
pub const MAX_REVIEW_BUNDLE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
pub const REVIEW_ANNOTATION_MAX_EDGE: u32 = 4_096;

#[derive(Clone, Debug, PartialEq)]
pub struct NumberedImageAnnotation {
    pub ordinal: u32,
    pub feedback_id: FeedbackId,
    pub anchor: FeedbackAnchor,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewArtifactRenderRequest {
    pub source_path: PathBuf,
    pub expected_asset: AssetVersion,
    pub cancellation: ReviewTaskCancellation,
    pub annotations: Vec<NumberedImageAnnotation>,
}

impl ReviewArtifactRenderRequest {
    pub fn validate(&self) -> Result<(), ReviewArtifactError> {
        if self.annotations.is_empty()
            || self.annotations.len() > MAX_FEEDBACK_ITEMS_PER_ROUND
            || self.annotations.iter().enumerate().any(|(index, item)| {
                item.ordinal != u32::try_from(index + 1).unwrap_or(u32::MAX)
                    || !matches!(
                        item.anchor,
                        FeedbackAnchor::ImageRect(_) | FeedbackAnchor::ImageStroke(_)
                    )
            })
        {
            return Err(ReviewArtifactError::InvalidRequest);
        }
        if self
            .annotations
            .iter()
            .map(|annotation| annotation.feedback_id)
            .collect::<HashSet<_>>()
            .len()
            != self.annotations.len()
        {
            return Err(ReviewArtifactError::InvalidRequest);
        }
        let points = self.annotations.iter().try_fold(0_usize, |total, item| {
            let count = match &item.anchor {
                FeedbackAnchor::ImageStroke(stroke) => stroke.points().len(),
                _ => 0,
            };
            total.checked_add(count)
        });
        if points.is_none_or(|points| points > MAX_IMAGE_STROKE_POINTS_PER_ROUND) {
            return Err(ReviewArtifactError::LimitExceeded);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ReviewArtifactError {
    #[error("review annotation request is invalid")]
    InvalidRequest,
    #[error("review annotation source is unsafe")]
    UnsafeSource,
    #[error("review annotation source changed")]
    SourceChanged,
    #[error("review annotation decode failed")]
    DecodeFailed,
    #[error("review annotation limit was exceeded")]
    LimitExceeded,
    #[error("review annotation render was cancelled")]
    Cancelled,
    #[error("review annotation renderer is unavailable")]
    Unavailable,
}

#[async_trait]
pub trait ReviewArtifactPort: Send + Sync {
    async fn render(
        &self,
        request: ReviewArtifactRenderRequest,
    ) -> Result<ReviewRenderedArtifact, ReviewArtifactError>;
}

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
