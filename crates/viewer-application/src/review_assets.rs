use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use viewer_domain::review::{AssetVersion, ReviewabilityFailure};
use viewer_domain::{AssetVersionId, EntityId, RelativePath};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReviewScope {
    Selection {
        entity_ids: Vec<EntityId>,
    },
    Folder {
        folder_id: Option<EntityId>,
        include_descendants: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewScopeResolution {
    pub candidate_entity_ids: Vec<EntityId>,
    pub image_count: u32,
    pub video_count: u32,
    pub excluded_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedReviewAsset {
    pub entity_id: EntityId,
    pub asset: AssetVersion,
    pub failure: Option<ReviewabilityFailure>,
    pub change_revision: u64,
    pub source_path: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewAssetConflictKind {
    Missing,
    Moved,
    Replaced,
    SizeChanged,
    ContentChanged,
    MediaChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum ReviewAssetValidation {
    Current(PreparedReviewAsset),
    Conflict {
        asset_version_id: AssetVersionId,
        relative_path: RelativePath,
        kind: ReviewAssetConflictKind,
    },
    Pending {
        asset_version_id: AssetVersionId,
        relative_path: RelativePath,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewTaskProgress {
    pub completed: u32,
    pub total: u32,
}

#[derive(Clone, Default)]
pub struct ReviewTaskCancellation(Arc<AtomicBool>);

impl std::fmt::Debug for ReviewTaskCancellation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReviewTaskCancellation")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

impl PartialEq for ReviewTaskCancellation {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ReviewTaskCancellation {}

impl ReviewTaskCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

pub trait ReviewProgressPort: Send + Sync {
    fn report(&self, progress: ReviewTaskProgress);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ReviewAssetError {
    #[error("review asset index is unavailable")]
    IndexUnavailable,
    #[error("review scope is invalid")]
    InvalidScope,
    #[error("review asset was not found")]
    NotFound,
    #[error("review asset limit was exceeded")]
    LimitExceeded,
    #[error("review source path is unsafe")]
    UnsafeSource,
    #[error("review source changed during evidence capture")]
    SourceChanged,
    #[error("review media evidence is still pending")]
    Pending,
    #[error("review asset task was cancelled")]
    Cancelled,
    #[error("review asset evidence is unavailable")]
    Unavailable,
    #[error("source relocation requires an exact explicit confirmation")]
    UnconfirmedLocation,
    #[error("the confirmed source locator changed")]
    StaleLocator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewSourceLocator {
    pub entity_id: EntityId,
    pub relative_path: RelativePath,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceLocatorConfirmation {
    UserConfirmed,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRelocationDecision {
    pub previous: Option<ReviewSourceLocator>,
    pub candidate: ReviewSourceLocator,
    pub confirmation: SourceLocatorConfirmation,
}
#[async_trait]
pub trait ContinuousReviewAssetPort: Send + Sync {
    /// Captures a fresh digest and media metadata within one verified source-identity
    /// interval, without trusting Ready index caches. Image dimensions are EXIF-upright
    /// (orientations 5..=8 swap the raw probe axes), matching the captured evidence.
    async fn prepare_additions(
        &self,
        entity_ids: &[EntityId],
        cancellation: ReviewTaskCancellation,
    ) -> Result<Vec<PreparedReviewAsset>, ReviewAssetError>;
    async fn check_sources(
        &self,
        assets: &[AssetVersion],
        cancellation: ReviewTaskCancellation,
    ) -> Result<Vec<viewer_domain::review::continuous::SourceCheck>, ReviewAssetError>;
    /// Only call after an explicit user selection. Digest equality alone grants no relocation.
    async fn confirm_relocation(
        &self,
        asset: &AssetVersion,
        decision: SourceRelocationDecision,
        cancellation: ReviewTaskCancellation,
    ) -> Result<(), ReviewAssetError>;
}

#[async_trait]
pub trait ReviewAssetCatalogPort: Send + Sync {
    async fn resolve_scope(
        &self,
        scope: &ReviewScope,
    ) -> Result<ReviewScopeResolution, ReviewAssetError>;

    async fn prepare_assets(
        &self,
        entity_ids: &[EntityId],
        cancellation: ReviewTaskCancellation,
        progress: Arc<dyn ReviewProgressPort>,
    ) -> Result<Vec<PreparedReviewAsset>, ReviewAssetError>;

    async fn revalidate_assets(
        &self,
        assets: &[PreparedReviewAsset],
        cancellation: ReviewTaskCancellation,
        progress: Arc<dyn ReviewProgressPort>,
    ) -> Result<Vec<ReviewAssetValidation>, ReviewAssetError>;

    fn release_tracking(&self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_task_cancellation_is_shared_and_one_way() {
        let cancellation = ReviewTaskCancellation::default();
        let observer = cancellation.clone();
        assert!(!observer.is_cancelled());

        cancellation.cancel();

        assert!(observer.is_cancelled());
    }
}
