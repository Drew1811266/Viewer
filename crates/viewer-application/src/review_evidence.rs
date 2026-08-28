//! Trusted in-process image capabilities; deliberately no IPC/serde construction.
use crate::{
    MAX_REVIEW_ARTIFACT_BYTES, MAX_REVIEW_ARTIFACT_PIXELS, PreparedReviewAsset,
    ReviewArtifactError, ReviewTaskCancellation,
    review_workspace::{EvidenceAnnotation, EvidenceRef, PreparedEvidenceFile},
};
use async_trait::async_trait;
use std::{collections::HashSet, sync::Arc};
use viewer_domain::{
    ReviewArchiveId, ReviewRoundId,
    review::{
        AssetVersion, FeedbackAnchor, MAX_FEEDBACK_ITEMS_PER_ROUND,
        MAX_IMAGE_STROKE_POINTS_PER_ROUND, ReviewMedia,
        continuous::{SnapshotRef, TargetVersionKey},
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistorySelector {
    Snapshot(SnapshotRef),
    Archive(ReviewArchiveId),
    Legacy(ReviewRoundId),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceRole {
    Base,
    Annotated,
}

/// Both coordinate spaces are EXIF-upright, normalized from the top-left.
/// Original EXIF transforms are baked into the PNG, never applied a second time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewPixelMapping {
    pub oriented_source_width: u32,
    pub oriented_source_height: u32,
    pub png_width: u32,
    pub png_height: u32,
}

#[derive(Clone)]
pub struct BoundReviewImage {
    asset: AssetVersion,
    reference: EvidenceRef,
    role: EvidenceRole,
    mapping: ReviewPixelMapping,
    png: Arc<[u8]>,
}
impl std::fmt::Debug for BoundReviewImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoundReviewImage")
            .field("asset", &self.asset.id)
            .field("reference", &self.reference)
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}
impl BoundReviewImage {
    /// Adapter-only construction contract: verify the complete PNG digest, file identity,
    /// and its binding to the captured/committed AssetVersion before calling.
    /// This accepts owned bytes, never a path that can later point at different content.
    pub fn from_verified_png(
        asset: AssetVersion,
        reference: EvidenceRef,
        role: EvidenceRole,
        png: Vec<u8>,
    ) -> Result<Self, ReviewArtifactError> {
        let (width, height) = match asset.media {
            ReviewMedia::Image {
                width: Some(w),
                height: Some(h),
            } if w > 0 && h > 0 && asset.evidence.blake3.is_some() => (w, h),
            _ => return Err(ReviewArtifactError::InvalidRequest),
        };
        if reference.size_bytes > MAX_REVIEW_ARTIFACT_BYTES
            || png.len() as u64 > MAX_REVIEW_ARTIFACT_BYTES
            || u64::from(reference.width) * u64::from(reference.height) > MAX_REVIEW_ARTIFACT_PIXELS
        {
            return Err(ReviewArtifactError::LimitExceeded);
        }
        if png.len() < 33
            || png.len() as u64 != reference.size_bytes
            || &png[..8] != b"\x89PNG\r\n\x1a\n"
            || png[8..12] != 13_u32.to_be_bytes()
            || &png[12..16] != b"IHDR"
            || png[16..20] != reference.width.to_be_bytes()
            || png[20..24] != reference.height.to_be_bytes()
            || reference.width == 0
            || reference.height == 0
        {
            return Err(ReviewArtifactError::InvalidRequest);
        }
        let mapping = ReviewPixelMapping {
            oriented_source_width: width,
            oriented_source_height: height,
            png_width: reference.width,
            png_height: reference.height,
        };
        Ok(Self {
            asset,
            reference,
            role,
            mapping,
            png: png.into(),
        })
    }
    pub fn asset(&self) -> &AssetVersion {
        &self.asset
    }
    pub fn reference(&self) -> &EvidenceRef {
        &self.reference
    }
    pub fn role(&self) -> EvidenceRole {
        self.role
    }
    pub fn mapping(&self) -> ReviewPixelMapping {
        self.mapping
    }
    pub fn blake3(&self) -> [u8; 32] {
        self.reference.blake3
    }
    pub fn png(&self) -> &[u8] {
        &self.png
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NumberedTargetAnnotation {
    pub ordinal: u32,
    pub key: TargetVersionKey,
    pub anchor: FeedbackAnchor,
}
#[derive(Clone, Debug)]
pub struct ReviewEvidenceRequest {
    pub base: BoundReviewImage,
    pub annotations: Vec<NumberedTargetAnnotation>,
    pub cancellation: ReviewTaskCancellation,
}
impl ReviewEvidenceRequest {
    pub fn validate(&self) -> Result<(), ReviewArtifactError> {
        if self.base.role() != EvidenceRole::Base {
            return Err(ReviewArtifactError::InvalidRequest);
        }
        if self.annotations.len() > MAX_FEEDBACK_ITEMS_PER_ROUND {
            return Err(ReviewArtifactError::LimitExceeded);
        }
        let mut targets = HashSet::new();
        let mut revisions = HashSet::new();
        let mut points = 0;
        for (i, annotation) in self.annotations.iter().enumerate() {
            if annotation.ordinal as usize != i + 1
                || !targets.insert(annotation.key.target_id)
                || !revisions.insert(annotation.key.target_revision_id)
            {
                return Err(ReviewArtifactError::InvalidRequest);
            }
            match &annotation.anchor {
                FeedbackAnchor::ImageRect(_) => {}
                FeedbackAnchor::ImageStroke(stroke) => points += stroke.points().len(),
                _ => return Err(ReviewArtifactError::InvalidRequest),
            }
        }
        if points > MAX_IMAGE_STROKE_POINTS_PER_ROUND {
            return Err(ReviewArtifactError::LimitExceeded);
        }
        Ok(())
    }
}

/// The adapter owns temporary files until the last lease is dropped. Keep the
/// result alive until repository.commit finishes; repository copies, never moves them.
pub trait ReviewEvidenceStaging: Send + Sync {
    fn files(&self) -> &[PreparedEvidenceFile];
}
pub struct ReviewEvidenceResult {
    pub base_ref: EvidenceRef,
    pub annotated_ref: Option<EvidenceRef>,
    pub annotations: Vec<EvidenceAnnotation>,
    pub staging: Arc<dyn ReviewEvidenceStaging>,
}
impl ReviewEvidenceResult {
    pub fn files(&self) -> &[PreparedEvidenceFile] {
        self.staging.files()
    }
}
#[async_trait]
pub trait ReviewEvidencePort: Send + Sync {
    async fn capture_base(
        &self,
        asset: PreparedReviewAsset,
        cancellation: ReviewTaskCancellation,
    ) -> Result<BoundReviewImage, ReviewArtifactError>;
    async fn render(
        &self,
        request: ReviewEvidenceRequest,
    ) -> Result<ReviewEvidenceResult, ReviewArtifactError>;
}
