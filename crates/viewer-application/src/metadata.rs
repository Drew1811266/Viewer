use std::collections::HashSet;
use viewer_domain::{
    EntityId, OperationId, RelativePath,
    file::{FileKind, FileNode, ImageIndexStatus, ImageMetadata, ReviewState, TextIndexStatus},
};

pub use viewer_domain::{file::Marker, search::IndexProgress};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkerTarget {
    pub entity_id: EntityId,
    pub relative_path: RelativePath,
    pub kind: FileKind,
    pub size: u64,
    pub modified_ns: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilePathMove {
    pub source: RelativePath,
    pub destination: RelativePath,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileCopyProjection {
    pub source: FileNode,
    pub destination: FileNode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileMoveProjection {
    pub source: FileNode,
    pub destination: RelativePath,
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
pub struct MarkerRestore {
    pub target: MarkerTarget,
    pub expected: Marker,
    pub previous: Marker,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedNode {
    pub node: FileNode,
    pub marker: Marker,
    pub image_metadata: Option<ImageMetadata>,
    pub image_status: ImageIndexStatus,
    pub text_status: TextIndexStatus,
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

    fn restore_batch(
        &self,
        restores: &[MarkerRestore],
        updated_at_ms: i64,
    ) -> Result<Vec<MarkerChange>, MarkerStoreError>;

    fn clear_paths(
        &self,
        _paths: &[RelativePath],
        _updated_at_ms: i64,
    ) -> Result<usize, MarkerStoreError> {
        Err(MarkerStoreError::Unavailable)
    }

    fn commit_replace(
        &self,
        _operation_id: OperationId,
        _replaced_destination: Option<&RelativePath>,
        _moves: &[FilePathMove],
        _case_sensitive: bool,
        _updated_at_ms: i64,
    ) -> Result<usize, MarkerStoreError> {
        Err(MarkerStoreError::Unavailable)
    }

    fn move_paths(
        &self,
        moves: &[FilePathMove],
        case_sensitive: bool,
        updated_at_ms: i64,
    ) -> Result<usize, MarkerStoreError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum OperationProjectionError {
    #[error("operation projection input is invalid")]
    InvalidInput,
    #[error("operation projection conflicts with the current index")]
    Conflict,
    #[error("operation projection is stale and must be rebuilt from disk")]
    Stale,
    #[error("operation projection is unavailable")]
    Unavailable,
}

pub trait OperationProjectionPort: Send + Sync {
    fn apply_copy(
        &self,
        copies: &[FileCopyProjection],
        case_sensitive: bool,
    ) -> Result<(), OperationProjectionError>;

    fn apply_move(
        &self,
        moves: &[FileMoveProjection],
        case_sensitive: bool,
    ) -> Result<(), OperationProjectionError>;

    fn apply_trash(&self, sources: &[FileNode]) -> Result<(), OperationProjectionError>;
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
        self.validate_targets(targets)?;

        let changes = self.store.apply_batch(targets, patch, updated_at_ms)?;
        self.projection
            .sync_markers(&changes)
            .map_err(|_| MarkerServiceError::CommittedButProjectionStale)?;
        Ok(changes)
    }

    pub fn apply_with_undo(
        &self,
        targets: &[MarkerTarget],
        patch: MarkerPatch,
        updated_at_ms: i64,
        undo: &mut crate::undo::UndoStack,
    ) -> Result<Vec<MarkerChange>, MarkerServiceError> {
        self.validate_targets(targets)?;
        let paths = targets
            .iter()
            .map(|target| target.relative_path.clone())
            .collect::<Vec<_>>();
        let previous = self
            .store
            .markers_for_paths(&paths)?
            .into_iter()
            .map(|stored| (stored.relative_path, stored.marker))
            .collect::<std::collections::HashMap<_, _>>();
        let changes = self.store.apply_batch(targets, patch, updated_at_ms)?;
        let kind = match (patch.review, patch.favorite) {
            (ReviewPatch::Set(_) | ReviewPatch::Clear, FavoritePatch::Unchanged) => {
                Some(viewer_domain::operation::OperationKind::SetReviewState)
            }
            (ReviewPatch::Unchanged, FavoritePatch::Set(_) | FavoritePatch::Toggle) => {
                Some(viewer_domain::operation::OperationKind::SetFavorite)
            }
            _ => None,
        };
        if let Some(kind) = kind {
            let actions = changes
                .iter()
                .filter_map(|change| {
                    let previous = previous
                        .get(&change.target.relative_path)
                        .copied()
                        .unwrap_or_default();
                    (previous != change.marker).then(|| match kind {
                        viewer_domain::operation::OperationKind::SetReviewState => {
                            crate::undo::UndoAction::ReviewState {
                                target: change.target.clone(),
                                previous,
                                expected: change.marker,
                            }
                        }
                        viewer_domain::operation::OperationKind::SetFavorite => {
                            crate::undo::UndoAction::Favorite {
                                target: change.target.clone(),
                                previous,
                                expected: change.marker,
                            }
                        }
                        _ => unreachable!("marker undo kind is fixed above"),
                    })
                })
                .collect();
            undo.record_batch(viewer_domain::OperationId::new(), kind, actions);
        }
        self.projection
            .sync_markers(&changes)
            .map_err(|_| MarkerServiceError::CommittedButProjectionStale)?;
        Ok(changes)
    }

    fn validate_targets(&self, targets: &[MarkerTarget]) -> Result<(), MarkerServiceError> {
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
        Ok(())
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
