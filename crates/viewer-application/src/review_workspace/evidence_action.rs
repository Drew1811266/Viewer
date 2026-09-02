use super::{EvidenceRef, ReviewCommitError, ReviewWorkspaceError, StoredAuthoringState};
use crate::ReviewArtifactError;
use std::collections::HashMap;
use viewer_domain::{
    AssetVersionId,
    review::{
        AssetVersion, FeedbackAnchor, ReviewMedia,
        continuous::{ContinuousReviewState, VersionedTarget},
    },
};

const ACTION_KEY_DOMAIN: &[u8] = b"viewer.review.evidence-action/1";
const ANNOTATION_STYLE_VERSION: u32 = 1;
const NORMALIZED_COORDINATE_SCALE: f64 = 1_000_000_000.0;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ReviewEvidenceActionKey(pub [u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewEvidenceActionPolicy {
    pub renderer_version: u32,
    pub output_policy_version: u32,
}

pub const CURRENT_REVIEW_EVIDENCE_ACTION_POLICY: ReviewEvidenceActionPolicy =
    ReviewEvidenceActionPolicy {
        renderer_version: 1,
        output_policy_version: 1,
    };

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CachedReviewEvidenceAction {
    pub action_key: ReviewEvidenceActionKey,
    pub base: EvidenceRef,
    pub annotated: Option<EvidenceRef>,
    pub renderer_version: u32,
    pub output_policy_version: u32,
}

pub trait ReviewEvidenceActionCachePort: Send + Sync {
    fn load_verified(
        &self,
        key: ReviewEvidenceActionKey,
    ) -> Result<Option<CachedReviewEvidenceAction>, ReviewCommitError>;
    fn store_verified(&self, value: &CachedReviewEvidenceAction) -> Result<(), ReviewCommitError>;
    fn remove(&self, key: ReviewEvidenceActionKey) -> Result<(), ReviewCommitError>;
}

#[derive(Default)]
pub struct NoReviewEvidenceActionCache;

impl ReviewEvidenceActionCachePort for NoReviewEvidenceActionCache {
    fn load_verified(
        &self,
        _key: ReviewEvidenceActionKey,
    ) -> Result<Option<CachedReviewEvidenceAction>, ReviewCommitError> {
        Ok(None)
    }

    fn store_verified(&self, _value: &CachedReviewEvidenceAction) -> Result<(), ReviewCommitError> {
        Ok(())
    }

    fn remove(&self, _key: ReviewEvidenceActionKey) -> Result<(), ReviewCommitError> {
        Ok(())
    }
}

/// Hashes only inputs that can alter the evidence pixels. Feedback prose and text-revision
/// identities are deliberately absent, so a text-only edit reuses the same image evidence.
pub fn evidence_action_key(
    state: &ContinuousReviewState,
    asset_id: AssetVersionId,
    policy: ReviewEvidenceActionPolicy,
) -> Result<ReviewEvidenceActionKey, ReviewWorkspaceError> {
    state.validate()?;
    let scenes = image_targets(state);
    let asset = state
        .assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .ok_or(ReviewArtifactError::InvalidRequest)?;
    key_for_asset(asset, scenes.get(&asset_id).map(Vec::as_slice), policy)
}

pub fn dirty_evidence_assets(
    previous: Option<&StoredAuthoringState>,
    target: &StoredAuthoringState,
    policy: ReviewEvidenceActionPolicy,
) -> Result<Vec<AssetVersionId>, ReviewWorkspaceError> {
    target.state.validate()?;
    if let Some(previous) = previous {
        previous.state.validate()?;
    }
    let target_scenes = image_targets(&target.state);
    let previous_scenes = previous.map(|value| image_targets(&value.state));
    let previous_assets: HashMap<_, _> = previous
        .into_iter()
        .flat_map(|value| &value.state.assets)
        .map(|asset| (asset.id, asset))
        .collect();
    let mut dirty = Vec::new();
    for asset in &target.state.assets {
        if !matches!(asset.media, ReviewMedia::Image { .. }) {
            continue;
        }
        let target_key = key_for_asset(
            asset,
            target_scenes.get(&asset.id).map(Vec::as_slice),
            policy,
        )?;
        let unchanged = previous_assets
            .get(&asset.id)
            .is_some_and(|previous_asset| {
                key_for_asset(
                    previous_asset,
                    previous_scenes
                        .as_ref()
                        .and_then(|scenes| scenes.get(&asset.id))
                        .map(Vec::as_slice),
                    policy,
                )
                .is_ok_and(|previous_key| previous_key == target_key)
            });
        if !unchanged {
            dirty.push(asset.id);
        }
    }
    Ok(dirty)
}

fn image_targets(state: &ContinuousReviewState) -> HashMap<AssetVersionId, Vec<&VersionedTarget>> {
    let mut targets = HashMap::<_, Vec<_>>::new();
    for feedback in &state.feedback {
        for target in &feedback.targets {
            if matches!(
                target.anchor,
                FeedbackAnchor::ImageRect(_) | FeedbackAnchor::ImageStroke(_)
            ) {
                targets
                    .entry(target.asset_version_id)
                    .or_default()
                    .push(target);
            }
        }
    }
    targets
}

fn key_for_asset(
    asset: &AssetVersion,
    targets: Option<&[&VersionedTarget]>,
    policy: ReviewEvidenceActionPolicy,
) -> Result<ReviewEvidenceActionKey, ReviewWorkspaceError> {
    if policy.renderer_version == 0 || policy.output_policy_version == 0 {
        return Err(ReviewArtifactError::InvalidRequest.into());
    }
    let source_digest = asset
        .evidence
        .blake3
        .ok_or(ReviewArtifactError::InvalidRequest)?;
    let (width, height) = match asset.media {
        ReviewMedia::Image {
            width: Some(width),
            height: Some(height),
        } if width > 0 && height > 0 => (width, height),
        _ => return Err(ReviewArtifactError::InvalidRequest.into()),
    };
    let targets = targets.unwrap_or_default();
    let target_count =
        u32::try_from(targets.len()).map_err(|_| ReviewArtifactError::LimitExceeded)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(ACTION_KEY_DOMAIN);
    hasher.update(&source_digest);
    hasher.update(&ANNOTATION_STYLE_VERSION.to_be_bytes());
    hasher.update(&policy.renderer_version.to_be_bytes());
    hasher.update(&policy.output_policy_version.to_be_bytes());
    hasher.update(&width.to_be_bytes());
    hasher.update(&height.to_be_bytes());
    hasher.update(&target_count.to_be_bytes());
    for (index, target) in targets.iter().enumerate() {
        let ordinal = u32::try_from(index + 1).map_err(|_| ReviewArtifactError::LimitExceeded)?;
        hasher.update(&ordinal.to_be_bytes());
        hasher.update(target.id.to_string().as_bytes());
        hasher.update(target.revision_id.to_string().as_bytes());
        match &target.anchor {
            FeedbackAnchor::ImageRect(rect) => {
                hasher.update(&[1]);
                for value in [rect.x(), rect.y(), rect.width(), rect.height()] {
                    hasher.update(&canonical_coordinate(value)?.to_be_bytes());
                }
            }
            FeedbackAnchor::ImageStroke(stroke) => {
                hasher.update(&[2]);
                let count = u32::try_from(stroke.points().len())
                    .map_err(|_| ReviewArtifactError::LimitExceeded)?;
                hasher.update(&count.to_be_bytes());
                for point in stroke.points() {
                    hasher.update(&canonical_coordinate(point.x())?.to_be_bytes());
                    hasher.update(&canonical_coordinate(point.y())?.to_be_bytes());
                }
            }
            _ => return Err(ReviewArtifactError::InvalidRequest.into()),
        }
    }
    Ok(ReviewEvidenceActionKey(*hasher.finalize().as_bytes()))
}

fn canonical_coordinate(value: f64) -> Result<u64, ReviewWorkspaceError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(ReviewArtifactError::InvalidRequest.into());
    }
    Ok((value * NORMALIZED_COORDINATE_SCALE).round() as u64)
}
