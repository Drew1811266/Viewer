use crate::file_commands::MAX_FILE_COMMAND_ITEMS;
use crate::{BrowseIndexPort, FinderDragPort};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};
use viewer_domain::{EntityId, file::FileKind};

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum FinderDragError {
    #[error("at least one file must be selected")]
    EmptySelection,
    #[error("selected entities must be unique")]
    DuplicateSelection,
    #[error("too many files were selected")]
    TooManySelection,
    #[error("selected entity is no longer available")]
    EntityNotFound,
    #[error("directories cannot be exported by file drag")]
    DirectoryNotAllowed,
    #[error("symbolic links cannot be exported")]
    SymlinkNotAllowed,
    #[error("macOS aliases cannot be exported")]
    AliasNotAllowed,
    #[error("selected item is not a regular file")]
    NotRegularFile,
    #[error("selected item resolved outside the project")]
    OutsideProject,
    #[error("project root is unavailable")]
    ProjectRootUnavailable,
    #[error("project index is unavailable")]
    IndexUnavailable,
    #[error("native Finder drag is unavailable")]
    NativeUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedFinderDrag {
    canonical_root: PathBuf,
    files: Vec<PathBuf>,
    entity_ids: Vec<EntityId>,
}

impl PreparedFinderDrag {
    pub fn canonical_root(&self) -> &Path {
        &self.canonical_root
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    pub fn revalidate_file(&self, index: usize) -> Result<&Path, FinderDragError> {
        let file = self
            .files
            .get(index)
            .ok_or(FinderDragError::EntityNotFound)?;
        let expected = self
            .entity_ids
            .get(index)
            .copied()
            .ok_or(FinderDragError::EntityNotFound)?;
        let canonical_root = fs::canonicalize(&self.canonical_root)
            .map_err(|_| FinderDragError::ProjectRootUnavailable)?;
        reject_absolute_symlink_components(&canonical_root, file)?;
        let metadata = fs::metadata(file).map_err(|_| FinderDragError::NotRegularFile)?;
        if !metadata.is_file() {
            return Err(FinderDragError::NotRegularFile);
        }
        if !matches_entity_identity(&metadata, expected) {
            return Err(FinderDragError::EntityNotFound);
        }
        let canonical = fs::canonicalize(file).map_err(|_| FinderDragError::NotRegularFile)?;
        if !canonical.starts_with(&canonical_root) {
            return Err(FinderDragError::OutsideProject);
        }
        Ok(file)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinderDragReceipt {
    pub file_count: usize,
}

pub fn prepare_finder_drag(
    project_root: &Path,
    index: &dyn BrowseIndexPort,
    entity_ids: &[EntityId],
) -> Result<PreparedFinderDrag, FinderDragError> {
    if entity_ids.is_empty() {
        return Err(FinderDragError::EmptySelection);
    }
    if entity_ids.len() > MAX_FILE_COMMAND_ITEMS {
        return Err(FinderDragError::TooManySelection);
    }
    let unique = entity_ids.iter().copied().collect::<HashSet<_>>();
    if unique.len() != entity_ids.len() {
        return Err(FinderDragError::DuplicateSelection);
    }
    let canonical_root =
        fs::canonicalize(project_root).map_err(|_| FinderDragError::ProjectRootUnavailable)?;
    let mut files = Vec::with_capacity(entity_ids.len());
    let mut canonical_files = HashSet::with_capacity(entity_ids.len());
    for entity_id in entity_ids {
        let node = index
            .node(*entity_id)
            .map_err(|_| FinderDragError::IndexUnavailable)?
            .ok_or(FinderDragError::EntityNotFound)?;
        if node.kind == FileKind::Directory {
            return Err(FinderDragError::DirectoryNotAllowed);
        }
        let candidate = canonical_root.join(node.relative_path.as_str());
        reject_symlink_components(&canonical_root, &node.relative_path, &candidate)?;
        let metadata = fs::metadata(&candidate).map_err(|_| FinderDragError::NotRegularFile)?;
        if !metadata.is_file() {
            return Err(FinderDragError::NotRegularFile);
        }
        if !matches_entity_identity(&metadata, node.entity_id) {
            return Err(FinderDragError::EntityNotFound);
        }
        let canonical =
            fs::canonicalize(&candidate).map_err(|_| FinderDragError::NotRegularFile)?;
        if !canonical.starts_with(&canonical_root) {
            return Err(FinderDragError::OutsideProject);
        }
        if !canonical_files.insert(canonical.clone()) {
            return Err(FinderDragError::DuplicateSelection);
        }
        files.push(canonical);
    }
    Ok(PreparedFinderDrag {
        canonical_root,
        files,
        entity_ids: entity_ids.to_vec(),
    })
}

#[cfg(unix)]
fn matches_entity_identity(metadata: &fs::Metadata, expected: EntityId) -> bool {
    use std::os::unix::fs::MetadataExt;
    EntityId::from_u128((u128::from(metadata.dev()) << 64) | u128::from(metadata.ino())) == expected
}

#[cfg(not(unix))]
fn matches_entity_identity(_metadata: &fs::Metadata, _expected: EntityId) -> bool {
    true
}

pub fn begin_finder_drag(
    port: &dyn FinderDragPort,
    selection: &PreparedFinderDrag,
) -> Result<FinderDragReceipt, FinderDragError> {
    port.begin_drag(selection)?;
    Ok(FinderDragReceipt {
        file_count: selection.files.len(),
    })
}

fn reject_symlink_components(
    canonical_root: &Path,
    relative_path: &viewer_domain::RelativePath,
    candidate: &Path,
) -> Result<(), FinderDragError> {
    let mut cursor = canonical_root.to_path_buf();
    for component in Path::new(relative_path.as_str()).components() {
        cursor.push(component);
        let metadata =
            fs::symlink_metadata(&cursor).map_err(|_| FinderDragError::NotRegularFile)?;
        if metadata.file_type().is_symlink() {
            return Err(FinderDragError::SymlinkNotAllowed);
        }
    }
    if cursor != candidate {
        return Err(FinderDragError::OutsideProject);
    }
    Ok(())
}

fn reject_absolute_symlink_components(root: &Path, file: &Path) -> Result<(), FinderDragError> {
    let relative = file
        .strip_prefix(root)
        .map_err(|_| FinderDragError::OutsideProject)?;
    let mut cursor = root.to_path_buf();
    for component in relative.components() {
        cursor.push(component);
        let metadata =
            fs::symlink_metadata(&cursor).map_err(|_| FinderDragError::NotRegularFile)?;
        if metadata.file_type().is_symlink() {
            return Err(FinderDragError::SymlinkNotAllowed);
        }
    }
    Ok(())
}
