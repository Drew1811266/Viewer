use std::collections::HashSet;
use viewer_domain::{
    EntityId, RelativePath,
    file::{FileKind, ReviewState},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Marker {
    pub review_state: Option<ReviewState>,
    pub favorite: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkerTarget {
    pub entity_id: EntityId,
    pub relative_path: RelativePath,
    pub kind: FileKind,
    pub size: u64,
    pub modified_ns: i128,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReviewPatch {
    #[default]
    Unchanged,
    Set(ReviewState),
    Clear,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FavoritePatch {
    #[default]
    Unchanged,
    Set(bool),
    Toggle,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MarkerPatch {
    pub review: ReviewPatch,
    pub favorite: FavoritePatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkerChange {
    pub target: MarkerTarget,
    pub marker: Marker,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortableMarker {
    pub relative_path: RelativePath,
    pub kind: FileKind,
    pub marker: Marker,
    pub evidence_size: Option<u64>,
    pub evidence_modified_ns: Option<i128>,
    pub content_hash: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum MarkerStoreError {
    #[error("portable marker store is read-only")]
    ReadOnly,
    #[error("portable marker target is invalid")]
    InvalidTarget,
    #[error("portable marker store is unavailable")]
    Unavailable,
}

pub trait PortableMetadataPort: Send + Sync {
    fn markers_for_paths(
        &self,
        paths: &[RelativePath],
    ) -> Result<Vec<PortableMarker>, MarkerStoreError>;

    fn apply_batch(
        &self,
        targets: &[MarkerTarget],
        patch: MarkerPatch,
        updated_at_ms: i64,
    ) -> Result<Vec<MarkerChange>, MarkerStoreError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum MarkerProjectionError {
    #[error("review projection is unavailable")]
    Unavailable,
}

pub trait MarkerProjectionPort: Send + Sync {
    fn sync_markers(&self, changes: &[MarkerChange]) -> Result<(), MarkerProjectionError>;
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum MarkerServiceError {
    #[error("the current project is read-only")]
    ReadOnly,
    #[error("at least one marker target is required")]
    EmptyTargets,
    #[error("marker targets must be unique")]
    DuplicateTarget,
    #[error(transparent)]
    Store(#[from] MarkerStoreError),
    #[error("review metadata was saved but the session projection needs rebuilding")]
    CommittedButProjectionStale,
}

pub struct MarkerService<'a> {
    store: &'a dyn PortableMetadataPort,
    projection: &'a dyn MarkerProjectionPort,
    writable: bool,
}

impl<'a> MarkerService<'a> {
    pub fn new(
        store: &'a dyn PortableMetadataPort,
        projection: &'a dyn MarkerProjectionPort,
        writable: bool,
    ) -> Self {
        Self {
            store,
            projection,
            writable,
        }
    }

    pub fn apply(
        &self,
        targets: &[MarkerTarget],
        patch: MarkerPatch,
        updated_at_ms: i64,
    ) -> Result<Vec<MarkerChange>, MarkerServiceError> {
        if !self.writable {
            return Err(MarkerServiceError::ReadOnly);
        }
        if targets.is_empty() {
            return Err(MarkerServiceError::EmptyTargets);
        }
        let mut entities = HashSet::with_capacity(targets.len());
        let mut paths = HashSet::with_capacity(targets.len());
        if targets.iter().any(|target| {
            !entities.insert(target.entity_id) || !paths.insert(target.relative_path.clone())
        }) {
            return Err(MarkerServiceError::DuplicateTarget);
        }

        let changes = self.store.apply_batch(targets, patch, updated_at_ms)?;
        self.projection
            .sync_markers(&changes)
            .map_err(|_| MarkerServiceError::CommittedButProjectionStale)?;
        Ok(changes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_default_is_unreviewed_and_not_favorite() {
        assert_eq!(
            Marker::default(),
            Marker {
                review_state: None,
                favorite: false,
            }
        );
    }
}
