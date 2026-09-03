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
                        FeedbackAnchor::ImagePoint(_)
                            | FeedbackAnchor::ImageArrow(_)
                            | FeedbackAnchor::ImageStroke(_)
                            | FeedbackAnchor::ImageRect(_)
                            | FeedbackAnchor::ImageEllipse(_)
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

#[cfg(test)]
mod tests {
    use super::*;
    use viewer_domain::RelativePath;
    use viewer_domain::review::{
        AssetEvidence, NormalizedArrow, NormalizedPoint, NormalizedRect, ReviewMedia,
    };

    fn request(anchor: FeedbackAnchor) -> ReviewArtifactRenderRequest {
        ReviewArtifactRenderRequest {
            source_path: PathBuf::from("image.png"),
            expected_asset: AssetVersion {
                id: AssetVersionId::from_u128(1),
                source_entity_id: None,
                relative_path: RelativePath::parse("image.png").unwrap(),
                evidence: AssetEvidence {
                    size_bytes: 100,
                    modified_ns: 1,
                    blake3: Some([1; 32]),
                },
                media: ReviewMedia::Image {
                    width: Some(100),
                    height: Some(100),
                },
                producer_asset_id: None,
                parent_asset_version_id: None,
            },
            cancellation: ReviewTaskCancellation::default(),
            annotations: vec![NumberedImageAnnotation {
                ordinal: 1,
                feedback_id: FeedbackId::from_u128(2),
                anchor,
            }],
        }
    }

    #[test]
    fn render_requests_accept_all_extended_image_anchor_shapes() {
        let point = NormalizedPoint::new(0.2, 0.3).unwrap();
        let arrow = NormalizedArrow::new(point, NormalizedPoint::new(0.8, 0.7).unwrap()).unwrap();
        let ellipse = NormalizedRect::new(0.1, 0.2, 0.3, 0.4).unwrap();

        for anchor in [
            FeedbackAnchor::ImagePoint(point),
            FeedbackAnchor::ImageArrow(arrow),
            FeedbackAnchor::ImageEllipse(ellipse),
        ] {
            assert_eq!(request(anchor).validate(), Ok(()));
        }
    }
}
