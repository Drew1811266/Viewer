use std::collections::HashMap;
use viewer_domain::{
    EntityId, RelativePath,
    file::{FileKind, FileNode},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FolderTreeItem {
    pub entity_id: EntityId,
    pub parent_entity_id: Option<EntityId>,
    pub relative_path: RelativePath,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserFile {
    pub entity_id: EntityId,
    pub relative_path: RelativePath,
    pub name: String,
    pub kind: FileKind,
    pub size: u64,
    pub modified_ns: i128,
}

impl From<FileNode> for BrowserFile {
    fn from(node: FileNode) -> Self {
        let name = file_name(&node.relative_path).to_owned();
        Self {
            entity_id: node.entity_id,
            relative_path: node.relative_path,
            name,
            kind: node.kind,
            size: node.size,
            modified_ns: node.modified_ns,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentFolderCard {
    pub entity_id: EntityId,
    pub relative_path: RelativePath,
    pub name: String,
    pub image_count: u64,
    pub text_count: u64,
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

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum BrowseIndexError {
    #[error("browse index is unavailable: {0}")]
    Unavailable(String),
}

pub trait BrowseIndexPort: Send + Sync {
    fn all_folders(&self) -> Result<Vec<FileNode>, BrowseIndexError>;
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
}

pub struct BrowseService<'a> {
    index: &'a dyn BrowseIndexPort,
}

impl<'a> BrowseService<'a> {
    pub fn new(index: &'a dyn BrowseIndexPort) -> Self {
        Self { index }
    }

    pub fn folder_tree(&self) -> Result<Vec<FolderTreeItem>, BrowseError> {
        let folders = self.index.all_folders()?;
        let ids_by_path = folders
            .iter()
            .map(|node| (node.relative_path.as_str().to_owned(), node.entity_id))
            .collect::<HashMap<_, _>>();
        Ok(folders
            .into_iter()
            .map(|node| {
                let parent_entity_id = parent_path(&node.relative_path)
                    .and_then(|parent| ids_by_path.get(parent).copied());
                FolderTreeItem {
                    entity_id: node.entity_id,
                    parent_entity_id,
                    name: file_name(&node.relative_path).to_owned(),
                    relative_path: node.relative_path,
                }
            })
            .collect())
    }

    pub fn folder_workspace(
        &self,
        folder: Option<EntityId>,
    ) -> Result<FolderWorkspace, BrowseError> {
        if let Some(folder_id) = folder {
            let Some(node) = self.index.node(folder_id)? else {
                return Err(BrowseError::FolderNotFound);
            };
            if node.kind != FileKind::Directory {
                return Err(BrowseError::NotAFolder);
            }
        }

        let direct = self.index.direct_children(folder)?;
        let (images, text_files) = split_files(direct);
        if !images.is_empty() || !text_files.is_empty() {
            return Ok(FolderWorkspace::Content { images, text_files });
        }

        let descendants = self.index.descendants(folder)?;
        let folders_by_path = descendants
            .iter()
            .filter(|node| node.kind == FileKind::Directory)
            .map(|node| (node.relative_path.as_str().to_owned(), node.clone()))
            .collect::<HashMap<_, _>>();
        let mut files_by_parent = HashMap::<String, Vec<FileNode>>::new();
        for node in descendants
            .into_iter()
            .filter(|node| node.kind != FileKind::Directory)
        {
            if let Some(parent) = parent_path(&node.relative_path)
                && folders_by_path.contains_key(parent)
            {
                files_by_parent
                    .entry(parent.to_owned())
                    .or_default()
                    .push(node);
            }
        }
        let mut content_folders = folders_by_path
            .into_iter()
            .filter_map(|(path, folder)| {
                let files = files_by_parent.remove(&path)?;
                let (images, text_files) = split_files(files);
                (!images.is_empty() || !text_files.is_empty()).then(|| ContentFolderCard {
                    entity_id: folder.entity_id,
                    name: file_name(&folder.relative_path).to_owned(),
                    relative_path: folder.relative_path,
                    image_count: images.len() as u64,
                    text_count: text_files.len() as u64,
                    representative_images: images.into_iter().take(4).collect(),
                })
            })
            .collect::<Vec<_>>();
        content_folders.sort_by(|left, right| {
            left.relative_path
                .as_str()
                .to_lowercase()
                .cmp(&right.relative_path.as_str().to_lowercase())
                .then_with(|| {
                    left.relative_path
                        .as_str()
                        .cmp(right.relative_path.as_str())
                })
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
        if let Some(folder_id) = folder {
            let Some(node) = self.index.node(folder_id)? else {
                return Err(BrowseError::FolderNotFound);
            };
            if node.kind != FileKind::Directory {
                return Err(BrowseError::NotAFolder);
            }
        }
        let descendants = self.index.descendants(folder)?;
        let (images, text_files) = split_files(descendants);
        if images.is_empty() && text_files.is_empty() {
            Ok(FolderWorkspace::Empty)
        } else {
            Ok(FolderWorkspace::Content { images, text_files })
        }
    }
}

fn split_files(nodes: Vec<FileNode>) -> (Vec<BrowserFile>, Vec<BrowserFile>) {
    let mut images = Vec::new();
    let mut text_files = Vec::new();
    for node in nodes {
        match node.kind {
            FileKind::Jpeg | FileKind::Png => images.push(node.into()),
            FileKind::Markdown | FileKind::Text => text_files.push(node.into()),
            FileKind::Directory => {}
        }
    }
    (images, text_files)
}

fn parent_path(path: &RelativePath) -> Option<&str> {
    path.as_str().rsplit_once('/').map(|(parent, _)| parent)
}

fn file_name(path: &RelativePath) -> &str {
    path.as_str()
        .rsplit_once('/')
        .map_or(path.as_str(), |(_, name)| name)
}
