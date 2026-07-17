use serde::Serialize;
use viewer_application::{
    BrowseError, BrowseIndexError, ImageError, ProjectOpenError, ProjectProbeError,
    TextPreviewError,
};
use viewer_infrastructure::{
    image_cache::ImageArtifactRegistryError, search::index::SessionIndexError,
    session_cache::SessionCacheError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    Validation,
    Conflict,
    Environment,
    Content,
    Consistency,
    Internal,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub category: ErrorCategory,
    pub user_message: String,
    pub retryable: bool,
    pub task_id: Option<String>,
    pub item_id: Option<String>,
}

impl CommandError {
    pub fn new(code: &str, category: ErrorCategory, user_message: &str, retryable: bool) -> Self {
        Self {
            code: code.to_owned(),
            category,
            user_message: user_message.to_owned(),
            retryable,
            task_id: None,
            item_id: None,
        }
    }
}

impl From<ProjectOpenError> for CommandError {
    fn from(error: ProjectOpenError) -> Self {
        match error {
            ProjectOpenError::AlreadyOpen => Self::new(
                "project_already_open",
                ErrorCategory::Conflict,
                "请先关闭当前项目。",
                false,
            ),
            ProjectOpenError::UnsafeRoot => invalid_project_root(),
            ProjectOpenError::Probe(error) => error.into(),
            ProjectOpenError::Transition(_) => internal_error(),
        }
    }
}

impl From<ProjectProbeError> for CommandError {
    fn from(error: ProjectProbeError) -> Self {
        match error {
            ProjectProbeError::NotDirectory { .. } => invalid_project_root(),
            ProjectProbeError::Io { .. } => Self::new(
                "project_unreadable",
                ErrorCategory::Environment,
                "无法读取该文件夹，请检查权限后重试。",
                true,
            ),
        }
    }
}

impl From<SessionCacheError> for CommandError {
    fn from(error: SessionCacheError) -> Self {
        match error {
            SessionCacheError::ArtifactTooLarge => Self::new(
                "cache_budget_exceeded",
                ErrorCategory::Environment,
                "临时预览超出缓存预算。",
                false,
            ),
            SessionCacheError::Io(_)
            | SessionCacheError::UnsafeRoot
            | SessionCacheError::ArtifactOutsideCache
            | SessionCacheError::ArtifactNotFile => internal_error(),
        }
    }
}

impl From<SessionIndexError> for CommandError {
    fn from(_error: SessionIndexError) -> Self {
        Self::new(
            "session_index_unavailable",
            ErrorCategory::Consistency,
            "项目临时索引不可用，请重新打开项目。",
            true,
        )
    }
}

impl From<BrowseIndexError> for CommandError {
    fn from(_error: BrowseIndexError) -> Self {
        Self::new(
            "session_index_unavailable",
            ErrorCategory::Consistency,
            "项目临时索引不可用，请重新打开项目。",
            true,
        )
    }
}

impl From<BrowseError> for CommandError {
    fn from(error: BrowseError) -> Self {
        match error {
            BrowseError::Index(error) => error.into(),
            BrowseError::FolderNotFound | BrowseError::NotAFolder => Self::new(
                "folder_not_found",
                ErrorCategory::Content,
                "该文件夹已不可用，请刷新项目后重试。",
                true,
            ),
            BrowseError::SelectionNotFound(_) => Self::new(
                "selection_not_found",
                ErrorCategory::Content,
                "部分所选文件已不可用，请刷新项目后重试。",
                true,
            ),
            BrowseError::DuplicateSelection => Self::new(
                "duplicate_selection",
                ErrorCategory::Validation,
                "所选文件不能重复。",
                false,
            ),
            BrowseError::SelectionSizeOverflow => internal_error(),
        }
    }
}

impl From<ImageError> for CommandError {
    fn from(error: ImageError) -> Self {
        match error {
            ImageError::Unsupported | ImageError::Corrupt => Self::new(
                "image_unavailable",
                ErrorCategory::Content,
                "无法预览该图片。",
                false,
            ),
            ImageError::BudgetExceeded => Self::new(
                "image_budget_exceeded",
                ErrorCategory::Content,
                "图片尺寸超出安全预览限制。",
                false,
            ),
            ImageError::Cancelled => Self::new(
                "image_cancelled",
                ErrorCategory::Conflict,
                "图片预览已取消。",
                true,
            ),
            ImageError::Io(_) => internal_error(),
        }
    }
}

impl From<ImageArtifactRegistryError> for CommandError {
    fn from(_error: ImageArtifactRegistryError) -> Self {
        internal_error()
    }
}

impl From<TextPreviewError> for CommandError {
    fn from(error: TextPreviewError) -> Self {
        match error {
            TextPreviewError::EncodingRequired => Self::new(
                "text_encoding_required",
                ErrorCategory::Content,
                "无法自动识别文本编码，请选择编码后重试。",
                true,
            ),
            TextPreviewError::Io(_) => Self::new(
                "text_unavailable",
                ErrorCategory::Content,
                "无法读取该文本文件。",
                true,
            ),
        }
    }
}

fn invalid_project_root() -> CommandError {
    CommandError::new(
        "invalid_project_root",
        ErrorCategory::Validation,
        "请选择一个可读取的真实文件夹。",
        true,
    )
}

fn internal_error() -> CommandError {
    CommandError::new(
        "internal_error",
        ErrorCategory::Internal,
        "Viewer 遇到内部错误，请重试。",
        true,
    )
}
