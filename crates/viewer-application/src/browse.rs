use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use viewer_domain::{
    EntityId, RelativePath,
    file::{FileKind, FileNode, ImageMetadata, Marker, ReviewState},
    search::{SearchSort, SearchSortKey, SortDirection},
};

use crate::{metadata::IndexedNode, search::natural_cmp};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FolderTreeItem {
    pub entity_id: EntityId,
    pub parent_entity_id: Option<EntityId>,
    pub relative_path: RelativePath,
    pub name: String,
    pub marker: Marker,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserFile {
    pub entity_id: EntityId,
    pub relative_path: RelativePath,
    pub name: String,
    pub kind: FileKind,
    pub size: u64,
    pub modified_ns: i128,
    pub marker: Marker,
    pub image_metadata: Option<ImageMetadata>,
}

impl From<IndexedNode> for BrowserFile {
    fn from(indexed: IndexedNode) -> Self {
        let node = indexed.node;
        let name = file_name(&node.relative_path).to_owned();
        Self {
            entity_id: node.entity_id,
            relative_path: node.relative_path,
            name,
            kind: node.kind,
            size: node.size,
            modified_ns: node.modified_ns,
            marker: indexed.marker,
            image_metadata: indexed.image_metadata,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FolderReviewProgress {
    pub total: u64,
    pub keep: u64,
    pub pending: u64,
    pub reject: u64,
    pub unmarked: u64,
    pub favorite: u64,
}

impl FolderReviewProgress {
    fn record(&mut self, marker: Marker) {
        self.total = self.total.saturating_add(1);
        match marker.review_state {
            Some(ReviewState::Keep) => self.keep = self.keep.saturating_add(1),
            Some(ReviewState::Pending) => self.pending = self.pending.saturating_add(1),
            Some(ReviewState::Reject) => self.reject = self.reject.saturating_add(1),
            None => self.unmarked = self.unmarked.saturating_add(1),
        }
        if marker.favorite {
            self.favorite = self.favorite.saturating_add(1);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentFolderCard {
    pub entity_id: EntityId,
    pub relative_path: RelativePath,
    pub name: String,
    pub marker: Marker,
    pub image_count: u64,
    pub text_count: u64,
    pub review_progress: FolderReviewProgress,
    pub representative_images: Vec<BrowserFile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FolderWorkspace {
    Category {
        folders: Vec<ContentFolderCard>,
    },
    Content {
        images: Vec<BrowserFile>,
        text_files: Vec<BrowserFile>,
    },
    Empty,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionAgreement<T> {
    NoneSelected,
    Common(T),
    Mixed,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SelectionTypeCounts {
    pub folders: u64,
    pub images: u64,
    pub text_files: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionInfo {
    pub relative_paths: Vec<RelativePath>,
    pub total_size: u64,
    pub types: SelectionTypeCounts,
    pub common_review: SelectionAgreement<Option<ReviewState>>,
    pub common_favorite: SelectionAgreement<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum BrowseIndexError {
    #[error("browse index is unavailable: {0}")]
    Unavailable(String),
}

pub trait BrowseIndexPort: Send + Sync {
    fn all_folders(&self) -> Result<Vec<FileNode>, BrowseIndexError>;
    fn all_indexed_nodes(&self) -> Result<Vec<IndexedNode>, BrowseIndexError>;
    fn node(&self, entity_id: EntityId) -> Result<Option<FileNode>, BrowseIndexError>;
    fn node_by_relative_path(
        &self,
        path: &RelativePath,
    ) -> Result<Option<FileNode>, BrowseIndexError>;
    fn direct_children(&self, folder: Option<EntityId>) -> Result<Vec<FileNode>, BrowseIndexError>;
    fn descendants(&self, folder: Option<EntityId>) -> Result<Vec<FileNode>, BrowseIndexError>;
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum BrowseError {
    #[error(transparent)]
    Index(#[from] BrowseIndexError),
    #[error("folder does not exist")]
    FolderNotFound,
    #[error("selected entity is not a folder")]
    NotAFolder,
    #[error("selected entity does not exist: {0}")]
    SelectionNotFound(EntityId),
    #[error("selected entities must be unique")]
    DuplicateSelection,
    #[error("selected file sizes exceed the supported range")]
    SelectionSizeOverflow,
}

pub struct BrowseService<'a> {
    index: &'a dyn BrowseIndexPort,
}

impl<'a> BrowseService<'a> {
    pub fn new(index: &'a dyn BrowseIndexPort) -> Self {
        Self { index }
    }

    pub fn folder_tree(&self) -> Result<Vec<FolderTreeItem>, BrowseError> {
        let indexed = self.index.all_indexed_nodes()?;
        let ids_by_path = indexed
            .iter()
            .filter(|indexed| indexed.node.kind == FileKind::Directory)
            .map(|indexed| {
                (
                    indexed.node.relative_path.as_str().to_owned(),
                    indexed.node.entity_id,
                )
            })
            .collect::<HashMap<_, _>>();
        let mut folders = indexed
            .into_iter()
            .filter(|indexed| indexed.node.kind == FileKind::Directory)
            .map(|indexed| {
                let node = indexed.node;
                let parent_entity_id = parent_path(&node.relative_path)
                    .and_then(|parent| ids_by_path.get(parent).copied());
                FolderTreeItem {
                    entity_id: node.entity_id,
                    parent_entity_id,
                    name: file_name(&node.relative_path).to_owned(),
                    relative_path: node.relative_path,
                    marker: indexed.marker,
                }
            })
            .collect::<Vec<_>>();
        folders.sort_by(|left, right| {
            natural_cmp(left.relative_path.as_str(), right.relative_path.as_str())
                .then_with(|| left.entity_id.to_string().cmp(&right.entity_id.to_string()))
        });
        Ok(folders)
    }

    pub fn folder_workspace(
        &self,
        folder: Option<EntityId>,
    ) -> Result<FolderWorkspace, BrowseError> {
        self.folder_workspace_sorted(folder, SearchSort::default())
    }

    pub fn folder_workspace_sorted(
        &self,
        folder: Option<EntityId>,
        sort: SearchSort,
    ) -> Result<FolderWorkspace, BrowseError> {
        let all = self.index.all_indexed_nodes()?;
        let folder_path = validate_folder(&all, folder)?;

        let direct = all
            .iter()
            .filter(|indexed| is_direct_child(&indexed.node.relative_path, folder_path.as_ref()))
            .cloned()
            .collect::<Vec<_>>();
        let (images, text_files) = split_files(direct, sort);
        if !images.is_empty() || !text_files.is_empty() {
            return Ok(FolderWorkspace::Content { images, text_files });
        }

        let descendants = all
            .iter()
            .filter(|indexed| is_in_scope(&indexed.node.relative_path, folder_path.as_ref()))
            .cloned()
            .collect::<Vec<_>>();
        let folders_by_path = descendants
            .iter()
            .filter(|indexed| indexed.node.kind == FileKind::Directory)
            .map(|indexed| {
                (
                    indexed.node.relative_path.as_str().to_owned(),
                    indexed.clone(),
                )
            })
            .collect::<HashMap<_, _>>();
        let mut direct_files_by_parent = HashMap::<String, Vec<IndexedNode>>::new();
        for indexed in descendants
            .iter()
            .filter(|indexed| is_supported_file(indexed.node.kind))
        {
            if let Some(parent) = parent_path(&indexed.node.relative_path)
                && folders_by_path.contains_key(parent)
            {
                direct_files_by_parent
                    .entry(parent.to_owned())
                    .or_default()
                    .push(indexed.clone());
            }
        }

        let mut content_folders = folders_by_path
            .into_iter()
            .filter_map(|(path, folder)| {
                let files = direct_files_by_parent.remove(&path)?;
                let (images, text_files) = split_files(files, SearchSort::default());
                if images.is_empty() && text_files.is_empty() {
                    return None;
                }
                let mut review_progress = FolderReviewProgress::default();
                for descendant in descendants.iter().filter(|indexed| {
                    is_supported_file(indexed.node.kind)
                        && is_descendant_of(&indexed.node.relative_path, &folder.node.relative_path)
                }) {
                    review_progress.record(descendant.marker);
                }
                Some(ContentFolderCard {
                    entity_id: folder.node.entity_id,
                    name: file_name(&folder.node.relative_path).to_owned(),
                    relative_path: folder.node.relative_path,
                    marker: folder.marker,
                    image_count: images.len() as u64,
                    text_count: text_files.len() as u64,
                    review_progress,
                    representative_images: images.into_iter().take(4).collect(),
                })
            })
            .collect::<Vec<_>>();
        content_folders.sort_by(|left, right| {
            natural_cmp(left.relative_path.as_str(), right.relative_path.as_str())
                .then_with(|| left.entity_id.to_string().cmp(&right.entity_id.to_string()))
        });
        if content_folders.is_empty() {
            Ok(FolderWorkspace::Empty)
        } else {
            Ok(FolderWorkspace::Category {
                folders: content_folders,
            })
        }
    }

    pub fn node_by_relative_path(
        &self,
        path: &RelativePath,
    ) -> Result<Option<FileNode>, BrowseError> {
        self.index.node_by_relative_path(path).map_err(Into::into)
    }

    pub fn aggregate_workspace(
        &self,
        folder: Option<EntityId>,
    ) -> Result<FolderWorkspace, BrowseError> {
        let all = self.index.all_indexed_nodes()?;
        let folder_path = validate_folder(&all, folder)?;
        let descendants = all
            .into_iter()
            .filter(|indexed| is_in_scope(&indexed.node.relative_path, folder_path.as_ref()))
            .collect::<Vec<_>>();
        let (images, text_files) = split_files(descendants, SearchSort::default());
        if images.is_empty() && text_files.is_empty() {
            Ok(FolderWorkspace::Empty)
        } else {
            Ok(FolderWorkspace::Content { images, text_files })
        }
    }

    pub fn selection_info(&self, entity_ids: &[EntityId]) -> Result<SelectionInfo, BrowseError> {
        let mut unique = HashSet::with_capacity(entity_ids.len());
        if entity_ids
            .iter()
            .any(|entity_id| !unique.insert(*entity_id))
        {
            return Err(BrowseError::DuplicateSelection);
        }
        if entity_ids.is_empty() {
            return Ok(SelectionInfo {
                relative_paths: Vec::new(),
                total_size: 0,
                types: SelectionTypeCounts::default(),
                common_review: SelectionAgreement::NoneSelected,
                common_favorite: SelectionAgreement::NoneSelected,
            });
        }

        let all = self.index.all_indexed_nodes()?;
        let by_id = all
            .into_iter()
            .map(|indexed| (indexed.node.entity_id, indexed))
            .collect::<HashMap<_, _>>();
        let mut selected = entity_ids
            .iter()
            .map(|entity_id| {
                by_id
                    .get(entity_id)
                    .cloned()
                    .ok_or(BrowseError::SelectionNotFound(*entity_id))
            })
            .collect::<Result<Vec<_>, _>>()?;
        selected.sort_by(|left, right| {
            natural_cmp(
                left.node.relative_path.as_str(),
                right.node.relative_path.as_str(),
            )
            .then_with(|| {
                left.node
                    .entity_id
                    .to_string()
                    .cmp(&right.node.entity_id.to_string())
            })
        });

        let mut total_size = 0_u64;
        let mut types = SelectionTypeCounts::default();
        for indexed in &selected {
            total_size = total_size
                .checked_add(indexed.node.size)
                .ok_or(BrowseError::SelectionSizeOverflow)?;
            match indexed.node.kind {
                FileKind::Directory => types.folders = types.folders.saturating_add(1),
                FileKind::Jpeg | FileKind::Png => {
                    types.images = types.images.saturating_add(1);
                }
                FileKind::Markdown | FileKind::Text => {
                    types.text_files = types.text_files.saturating_add(1);
                }
            }
        }
        Ok(SelectionInfo {
            relative_paths: selected
                .iter()
                .map(|indexed| indexed.node.relative_path.clone())
                .collect(),
            total_size,
            types,
            common_review: agreement(selected.iter().map(|indexed| indexed.marker.review_state)),
            common_favorite: agreement(selected.iter().map(|indexed| indexed.marker.favorite)),
        })
    }
}

fn validate_folder(
    nodes: &[IndexedNode],
    folder: Option<EntityId>,
) -> Result<Option<RelativePath>, BrowseError> {
    let Some(folder) = folder else {
        return Ok(None);
    };
    let Some(indexed) = nodes
        .iter()
        .find(|indexed| indexed.node.entity_id == folder)
    else {
        return Err(BrowseError::FolderNotFound);
    };
    if indexed.node.kind != FileKind::Directory {
        return Err(BrowseError::NotAFolder);
    }
    Ok(Some(indexed.node.relative_path.clone()))
}

fn agreement<T: Copy + Eq>(mut values: impl Iterator<Item = T>) -> SelectionAgreement<T> {
    let Some(first) = values.next() else {
        return SelectionAgreement::NoneSelected;
    };
    if values.all(|value| value == first) {
        SelectionAgreement::Common(first)
    } else {
        SelectionAgreement::Mixed
    }
}

fn split_files(nodes: Vec<IndexedNode>, sort: SearchSort) -> (Vec<BrowserFile>, Vec<BrowserFile>) {
    let mut images = Vec::new();
    let mut text_files = Vec::new();
    for indexed in nodes {
        match indexed.node.kind {
            FileKind::Jpeg | FileKind::Png => images.push(indexed.into()),
            FileKind::Markdown | FileKind::Text => text_files.push(indexed.into()),
            FileKind::Directory => {}
        }
    }
    images.sort_by(|left, right| compare_files(left, right, sort));
    text_files.sort_by(|left, right| compare_files(left, right, sort));
    (images, text_files)
}

fn compare_files(left: &BrowserFile, right: &BrowserFile, sort: SearchSort) -> Ordering {
    let ascending = match sort.key {
        SearchSortKey::Relevance | SearchSortKey::NaturalName => {
            natural_cmp(&left.name, &right.name)
        }
        SearchSortKey::ModifiedTime => left.modified_ns.cmp(&right.modified_ns),
        SearchSortKey::Size => left.size.cmp(&right.size),
        SearchSortKey::PixelDimensions => pixel_count(left).cmp(&pixel_count(right)),
        SearchSortKey::ReviewState => review_rank(left).cmp(&review_rank(right)),
    }
    .then_with(|| natural_cmp(left.relative_path.as_str(), right.relative_path.as_str()))
    .then_with(|| left.entity_id.to_string().cmp(&right.entity_id.to_string()));
    match sort.direction {
        SortDirection::Ascending => ascending,
        SortDirection::Descending => ascending.reverse(),
    }
}

fn pixel_count(file: &BrowserFile) -> Option<u64> {
    file.image_metadata
        .map(|metadata| u64::from(metadata.width) * u64::from(metadata.height))
}

fn review_rank(file: &BrowserFile) -> u8 {
    match file.marker.review_state {
        None => 0,
        Some(ReviewState::Keep) => 1,
        Some(ReviewState::Pending) => 2,
        Some(ReviewState::Reject) => 3,
    }
}

fn is_supported_file(kind: FileKind) -> bool {
    kind != FileKind::Directory
}

fn is_direct_child(path: &RelativePath, folder: Option<&RelativePath>) -> bool {
    match folder {
        Some(folder) => parent_path(path) == Some(folder.as_str()),
        None => parent_path(path).is_none(),
    }
}

fn is_in_scope(path: &RelativePath, folder: Option<&RelativePath>) -> bool {
    folder.is_none_or(|folder| is_descendant_of(path, folder))
}

fn is_descendant_of(path: &RelativePath, folder: &RelativePath) -> bool {
    let folder = folder.as_str();
    let path = path.as_str();
    path.len() > folder.len()
        && path.starts_with(folder)
        && path.as_bytes().get(folder.len()) == Some(&b'/')
}

fn parent_path(path: &RelativePath) -> Option<&str> {
    path.as_str().rsplit_once('/').map(|(parent, _)| parent)
}

fn file_name(path: &RelativePath) -> &str {
    path.as_str()
        .rsplit_once('/')
        .map_or(path.as_str(), |(_, name)| name)
}
