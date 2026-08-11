use super::{ImageMetadataDto, MarkerDto};
use serde::Serialize;
use viewer_application::{
    ActiveProject,
    browse::{
        BrowserFile, ContentFolderCard, FolderReviewProgress, FolderTreeItem, FolderWorkspace,
        SelectionAgreement, SelectionInfo, SelectionTypeCounts,
    },
    metadata::IndexProgress,
    scan::{ScanEvent, ScanTotals},
};
use viewer_domain::{
    RelativePath, TaskId,
    file::{FileNode, ReviewState},
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannedNodeDto {
    pub entity_id: String,
    pub relative_path: String,
    pub kind: viewer_domain::file::FileKind,
    pub size: u64,
    pub modified_ns: String,
}

impl From<&FileNode> for ScannedNodeDto {
    fn from(node: &FileNode) -> Self {
        Self {
            entity_id: node.entity_id.to_string(),
            relative_path: node.relative_path.as_str().to_owned(),
            kind: node.kind,
            size: node.size,
            modified_ns: node.modified_ns.to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanTotalsDto {
    pub folders: u64,
    pub files: u64,
    pub failed: u64,
}

impl From<ScanTotals> for ScanTotalsDto {
    fn from(totals: ScanTotals) -> Self {
        Self {
            folders: totals.folders,
            files: totals.files,
            failed: totals.failed,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScanEventDto {
    Folders {
        #[serde(rename = "sessionId")]
        session_id: String,
        generation: u64,
        #[serde(rename = "taskId")]
        task_id: String,
        nodes: Vec<ScannedNodeDto>,
    },
    Files {
        #[serde(rename = "sessionId")]
        session_id: String,
        generation: u64,
        #[serde(rename = "taskId")]
        task_id: String,
        nodes: Vec<ScannedNodeDto>,
    },
    FailedItem {
        #[serde(rename = "sessionId")]
        session_id: String,
        generation: u64,
        #[serde(rename = "taskId")]
        task_id: String,
        #[serde(rename = "relativePath")]
        relative_path: String,
        code: String,
    },
    Finished {
        #[serde(rename = "sessionId")]
        session_id: String,
        generation: u64,
        #[serde(rename = "taskId")]
        task_id: String,
        totals: ScanTotalsDto,
    },
}

impl ScanEventDto {
    pub fn from_event(active: &ActiveProject, task_id: TaskId, event: &ScanEvent) -> Self {
        let session_id = active.session_id.to_string();
        let generation = event.generation().get();
        let task_id = task_id.to_string();
        match event {
            ScanEvent::Folders { nodes, .. } => Self::Folders {
                session_id,
                generation,
                task_id,
                nodes: nodes.iter().map(ScannedNodeDto::from).collect(),
            },
            ScanEvent::Files { nodes, .. } => Self::Files {
                session_id,
                generation,
                task_id,
                nodes: nodes.iter().map(ScannedNodeDto::from).collect(),
            },
            ScanEvent::FailedItem {
                relative_display,
                code,
                ..
            } => Self::FailedItem {
                session_id,
                generation,
                task_id,
                relative_path: safe_relative_display(relative_display),
                code: code.clone(),
            },
            ScanEvent::Finished { totals, .. } => Self::Finished {
                session_id,
                generation,
                task_id,
                totals: (*totals).into(),
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexProgressDto {
    pub session_id: String,
    pub generation: u64,
    pub images_total: u64,
    pub images_ready: u64,
    pub images_failed: u64,
    pub text_total: u64,
    pub text_ready: u64,
    pub text_skipped: u64,
    pub text_failed: u64,
    pub complete: bool,
}

impl IndexProgressDto {
    pub fn from_progress(active: &ActiveProject, progress: IndexProgress) -> Self {
        Self {
            session_id: active.session_id.to_string(),
            generation: active.generation.get(),
            images_total: progress.images_total,
            images_ready: progress.images_ready,
            images_failed: progress.images_failed,
            text_total: progress.text_total,
            text_ready: progress.text_ready,
            text_skipped: progress.text_skipped,
            text_failed: progress.text_failed,
            complete: progress.is_complete(),
        }
    }
}

pub(super) fn safe_relative_display(value: &str) -> String {
    RelativePath::parse(value)
        .map(|path| path.as_str().to_owned())
        .unwrap_or_else(|_| "unavailable".to_owned())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderReviewProgressDto {
    pub total: u64,
    pub keep: u64,
    pub pending: u64,
    pub reject: u64,
    pub unmarked: u64,
    pub favorite: u64,
}

impl From<FolderReviewProgress> for FolderReviewProgressDto {
    fn from(progress: FolderReviewProgress) -> Self {
        Self {
            total: progress.total,
            keep: progress.keep,
            pending: progress.pending,
            reject: progress.reject,
            unmarked: progress.unmarked,
            favorite: progress.favorite,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderTreeItemDto {
    pub entity_id: String,
    pub parent_entity_id: Option<String>,
    pub relative_path: String,
    pub name: String,
    pub marker: MarkerDto,
}

impl From<FolderTreeItem> for FolderTreeItemDto {
    fn from(folder: FolderTreeItem) -> Self {
        Self {
            entity_id: folder.entity_id.to_string(),
            parent_entity_id: folder.parent_entity_id.map(|id| id.to_string()),
            relative_path: folder.relative_path.as_str().to_owned(),
            name: folder.name,
            marker: folder.marker.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserFileDto {
    pub entity_id: String,
    pub relative_path: String,
    pub name: String,
    pub kind: viewer_domain::file::FileKind,
    pub size: u64,
    pub modified_ns: String,
    pub marker: MarkerDto,
    pub image_metadata: Option<ImageMetadataDto>,
    pub image_url: Option<String>,
}

impl From<BrowserFile> for BrowserFileDto {
    fn from(file: BrowserFile) -> Self {
        Self {
            entity_id: file.entity_id.to_string(),
            relative_path: file.relative_path.as_str().to_owned(),
            name: file.name,
            kind: file.kind,
            size: file.size,
            modified_ns: file.modified_ns.to_string(),
            marker: file.marker.into(),
            image_metadata: file.image_metadata.map(Into::into),
            image_url: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentFolderCardDto {
    pub entity_id: String,
    pub relative_path: String,
    pub name: String,
    pub marker: MarkerDto,
    pub image_count: u64,
    pub other_file_count: u64,
    pub review_progress: FolderReviewProgressDto,
    pub representative_images: Vec<BrowserFileDto>,
}

impl From<ContentFolderCard> for ContentFolderCardDto {
    fn from(folder: ContentFolderCard) -> Self {
        Self {
            entity_id: folder.entity_id.to_string(),
            relative_path: folder.relative_path.as_str().to_owned(),
            name: folder.name,
            marker: folder.marker.into(),
            image_count: folder.image_count,
            other_file_count: folder.other_file_count,
            review_progress: folder.review_progress.into(),
            representative_images: folder
                .representative_images
                .into_iter()
                .map(BrowserFileDto::from)
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "state", content = "value", rename_all = "snake_case")]
pub enum SelectionAgreementDto<T> {
    NoneSelected,
    Common(T),
    Mixed,
}

impl<T, U> From<SelectionAgreement<T>> for SelectionAgreementDto<U>
where
    U: From<T>,
{
    fn from(agreement: SelectionAgreement<T>) -> Self {
        match agreement {
            SelectionAgreement::NoneSelected => Self::NoneSelected,
            SelectionAgreement::Common(value) => Self::Common(value.into()),
            SelectionAgreement::Mixed => Self::Mixed,
        }
    }
}

impl<T> SelectionAgreementDto<T> {
    pub const fn state_name(&self) -> &'static str {
        match self {
            Self::NoneSelected => "none_selected",
            Self::Common(_) => "common",
            Self::Mixed => "mixed",
        }
    }
}

impl SelectionAgreementDto<Option<ReviewState>> {
    pub const fn review_value(&self) -> Option<ReviewState> {
        match self {
            Self::Common(value) => *value,
            Self::NoneSelected | Self::Mixed => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionTypeCountsDto {
    pub folders: u64,
    pub images: u64,
    pub other_files: u64,
}

impl From<SelectionTypeCounts> for SelectionTypeCountsDto {
    fn from(types: SelectionTypeCounts) -> Self {
        Self {
            folders: types.folders,
            images: types.images,
            other_files: types.other_files,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionInfoDto {
    pub relative_paths: Vec<String>,
    pub total_size: u64,
    pub types: SelectionTypeCountsDto,
    pub common_review: SelectionAgreementDto<Option<ReviewState>>,
    pub common_favorite: SelectionAgreementDto<bool>,
}

impl From<SelectionInfo> for SelectionInfoDto {
    fn from(info: SelectionInfo) -> Self {
        Self {
            relative_paths: info
                .relative_paths
                .into_iter()
                .map(|path| path.as_str().to_owned())
                .collect(),
            total_size: info.total_size,
            types: info.types.into(),
            common_review: match info.common_review {
                SelectionAgreement::NoneSelected => SelectionAgreementDto::NoneSelected,
                SelectionAgreement::Common(value) => SelectionAgreementDto::Common(value),
                SelectionAgreement::Mixed => SelectionAgreementDto::Mixed,
            },
            common_favorite: info.common_favorite.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(
    tag = "workspace",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum FolderWorkspaceDto {
    Category {
        folders: Vec<ContentFolderCardDto>,
    },
    Content {
        images: Vec<BrowserFileDto>,
        other_files: Vec<BrowserFileDto>,
    },
    Empty,
}

impl From<FolderWorkspace> for FolderWorkspaceDto {
    fn from(workspace: FolderWorkspace) -> Self {
        match workspace {
            FolderWorkspace::Category { folders } => Self::Category {
                folders: folders
                    .into_iter()
                    .map(ContentFolderCardDto::from)
                    .collect(),
            },
            FolderWorkspace::Content {
                images,
                videos: _,
                other_files,
            } => Self::Content {
                images: images.into_iter().map(BrowserFileDto::from).collect(),
                other_files: other_files.into_iter().map(BrowserFileDto::from).collect(),
            },
            FolderWorkspace::Empty => Self::Empty,
        }
    }
}
