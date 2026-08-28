use super::*;
use crate::{
    ReviewTaskCancellation,
    review_evidence::{BoundReviewImage, EvidenceRole},
};
use viewer_domain::{
    EntityId,
    review::{AssetVersion, ReviewMedia},
};

/// A returned batch must fit the desktop's session cache without evicting itself.
pub const MAX_REVIEW_PREVIEW_BATCH: usize = 128;

#[derive(Clone, Debug)]
pub struct PreparedReviewPreview {
    pub asset: AssetVersion,
    /// Video has no fabricated image backup. Image previews always bind these exact bytes.
    pub image: Option<BoundReviewImage>,
}

impl ContinuousReviewService {
    pub async fn prepare_asset_previews(
        &self,
        entities: &[EntityId],
        cancellation: ReviewTaskCancellation,
    ) -> Result<Vec<PreparedReviewPreview>, ReviewWorkspaceError> {
        if entities.len() > MAX_REVIEW_PREVIEW_BATCH {
            return Err(crate::ReviewAssetError::LimitExceeded.into());
        }
        let assets = self.prepare_assets(entities, cancellation.clone()).await?;
        let mut previews = Vec::with_capacity(assets.len());
        let mut retained = 0_u64;
        for asset in assets {
            check_cancelled(&cancellation)?;
            let image = if matches!(asset.media, ReviewMedia::Image { .. }) {
                let prepared = self
                    .prepared
                    .lock()
                    .await
                    .get(&asset.id)
                    .cloned()
                    .ok_or(ReviewWorkspaceError::PreviewRequired)?;
                let image = self
                    .evidence
                    .capture_base(prepared, cancellation.clone())
                    .await?;
                check_cancelled(&cancellation)?;
                if image.asset() != &asset || image.role() != EvidenceRole::Base {
                    return Err(crate::ReviewArtifactError::InvalidRequest.into());
                }
                retained = retained.saturating_add(image.reference().size_bytes);
                if retained > crate::MAX_REVIEW_ARTIFACT_BYTES {
                    return Err(crate::ReviewArtifactError::LimitExceeded.into());
                }
                Some(image)
            } else {
                None
            };
            previews.push(PreparedReviewPreview { asset, image });
        }
        check_cancelled(&cancellation)?;
        Ok(previews)
    }
}

pub(super) fn check_cancelled(
    cancellation: &ReviewTaskCancellation,
) -> Result<(), ReviewWorkspaceError> {
    if cancellation.is_cancelled() {
        Err(ReviewWorkspaceError::Cancelled)
    } else {
        Ok(())
    }
}
