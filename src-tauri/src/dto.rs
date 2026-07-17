use serde::{Deserialize, Serialize};
use viewer_application::scan::{ScanEvent, ScanTotals};
use viewer_application::{
    ActiveProject, ImageBackend, ProjectAccess,
    browse::{
        BrowserFile, ContentFolderCard, FolderReviewProgress, FolderTreeItem, FolderWorkspace,
        SelectionAgreement, SelectionInfo, SelectionTypeCounts,
    },
    metadata::IndexProgress,
};
use viewer_domain::{
    RelativePath, TaskId,
    file::{FileNode, ImageMetadata, Marker, ReviewState},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectAccessDto {
    ReadWrite,
    ReadOnly,
}

impl From<ProjectAccess> for ProjectAccessDto {
    fn from(access: ProjectAccess) -> Self {
        match access {
            ProjectAccess::ReadWrite => Self::ReadWrite,
            ProjectAccess::ReadOnly => Self::ReadOnly,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshot {
    pub project_id: String,
    pub session_id: String,
    pub generation: u64,
    pub display_name: String,
    pub access: ProjectAccessDto,
}

impl From<&ActiveProject> for ProjectSnapshot {
    fn from(project: &ActiveProject) -> Self {
        Self {
            project_id: project.project_id.to_string(),
            session_id: project.session_id.to_string(),
            generation: project.generation.get(),
            display_name: project.display_name.clone(),
            access: project.access.into(),
        }
    }
}

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

fn safe_relative_display(value: &str) -> String {
    RelativePath::parse(value)
        .map(|path| path.as_str().to_owned())
        .unwrap_or_else(|_| "unavailable".to_owned())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkerDto {
    pub review_state: Option<ReviewState>,
    pub favorite: bool,
}

impl From<Marker> for MarkerDto {
    fn from(marker: Marker) -> Self {
        Self {
            review_state: marker.review_state,
            favorite: marker.favorite,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageMetadataDto {
    pub width: u32,
    pub height: u32,
}

impl From<ImageMetadata> for ImageMetadataDto {
    fn from(metadata: ImageMetadata) -> Self {
        Self {
            width: metadata.width,
            height: metadata.height,
        }
    }
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
    pub text_count: u64,
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
            text_count: folder.text_count,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionTypeCountsDto {
    pub folders: u64,
    pub images: u64,
    pub text_files: u64,
}

impl From<SelectionTypeCounts> for SelectionTypeCountsDto {
    fn from(types: SelectionTypeCounts) -> Self {
        Self {
            folders: types.folders,
            images: types.images,
            text_files: types.text_files,
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
#[serde(tag = "workspace", rename_all = "snake_case")]
pub enum FolderWorkspaceDto {
    Category {
        folders: Vec<ContentFolderCardDto>,
    },
    Content {
        images: Vec<BrowserFileDto>,
        #[serde(rename = "textFiles")]
        text_files: Vec<BrowserFileDto>,
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
            FolderWorkspace::Content { images, text_files } => Self::Content {
                images: images.into_iter().map(BrowserFileDto::from).collect(),
                text_files: text_files.into_iter().map(BrowserFileDto::from).collect(),
            },
            FolderWorkspace::Empty => Self::Empty,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageBackendDto {
    QuickLook,
    ImageIo,
}

impl From<ImageBackend> for ImageBackendDto {
    fn from(backend: ImageBackend) -> Self {
        match backend {
            ImageBackend::QuickLook => Self::QuickLook,
            ImageBackend::ImageIo => Self::ImageIo,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageRepresentationDto {
    pub cache_key: String,
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub backend: ImageBackendDto,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextPreviewFormatDto {
    PlainText,
    Markdown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPreviewDto {
    pub entity_id: String,
    pub format: TextPreviewFormatDto,
    pub plain_text: Option<String>,
    pub markdown_html: Option<String>,
    pub encoding: viewer_application::TextEncoding,
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImageRepresentationRequestDto {
    Thumbnail {
        #[serde(rename = "maxPixels")]
        max_pixels: u32,
        #[serde(rename = "scaleMilli")]
        scale_milli: u16,
    },
    FitPreview {
        #[serde(rename = "maxWidth")]
        max_width: u32,
        #[serde(rename = "maxHeight")]
        max_height: u32,
        #[serde(rename = "scaleMilli")]
        scale_milli: u16,
    },
    Original100Percent,
}

impl From<ImageRepresentationRequestDto> for viewer_domain::image::ImageRepresentationKind {
    fn from(request: ImageRepresentationRequestDto) -> Self {
        match request {
            ImageRepresentationRequestDto::Thumbnail {
                max_pixels,
                scale_milli,
            } => Self::Thumbnail {
                max_pixels,
                scale_milli,
            },
            ImageRepresentationRequestDto::FitPreview {
                max_width,
                max_height,
                scale_milli,
            } => Self::FitPreview {
                max_width,
                max_height,
                scale_milli,
            },
            ImageRepresentationRequestDto::Original100Percent => Self::Original100Percent,
        }
    }
}

#[cfg(test)]
mod tests {
    use viewer_application::browse::{SelectionAgreement, SelectionInfo, SelectionTypeCounts};
    use viewer_domain::RelativePath;

    #[test]
    fn failed_scan_item_display_never_exposes_an_absolute_or_reserved_path() {
        assert_eq!(super::safe_relative_display("catalog/id-1"), "catalog/id-1");
        for unsafe_value in ["/Users/example/secret", "../outside", ".viewer/index"] {
            assert_eq!(super::safe_relative_display(unsafe_value), "unavailable");
        }
    }

    #[test]
    fn selection_dto_keeps_unmarked_distinct_from_mixed_and_exposes_relative_paths_only() {
        let dto = super::SelectionInfoDto::from(SelectionInfo {
            relative_paths: vec![RelativePath::parse("catalog/id-2/image.jpg").unwrap()],
            total_size: 42,
            types: SelectionTypeCounts {
                folders: 0,
                images: 1,
                text_files: 0,
            },
            common_review: SelectionAgreement::Common(None),
            common_favorite: SelectionAgreement::Mixed,
        });

        let json = serde_json::to_value(dto).unwrap();
        assert_eq!(json["relativePaths"][0], "catalog/id-2/image.jpg");
        assert_eq!(json["commonReview"]["state"], "common");
        assert!(json["commonReview"]["value"].is_null());
        assert_eq!(json["commonFavorite"]["state"], "mixed");
    }
}
