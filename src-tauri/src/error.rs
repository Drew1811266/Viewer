use serde::Serialize;
use viewer_application::{
    BrowseError, BrowseIndexError, FinderDragError, ImageError, ProjectOpenError,
    ProjectProbeError, TextPreviewError, ViewerSettingsError,
    metadata::{MarkerServiceError, MarkerStoreError},
    search::SearchError,
    undo::{UndoError, UndoServiceError},
};

impl From<FinderDragError> for CommandError {
    fn from(error: FinderDragError) -> Self {
        match error {
            FinderDragError::EmptySelection | FinderDragError::DuplicateSelection => Self::new(
                "invalid_finder_drag_selection",
                ErrorCategory::Validation,
                "请选择至少一个且不重复的文件。",
                false,
            ),
            FinderDragError::TooManySelection => Self::new(
                "finder_drag_selection_too_large",
                ErrorCategory::Validation,
                "一次拖动的文件数量过多，请减少选择后重试。",
                false,
            ),
            FinderDragError::EntityNotFound | FinderDragError::IndexUnavailable => Self::new(
                "finder_drag_selection_stale",
                ErrorCategory::Consistency,
                "部分所选文件已不可用，请刷新项目后重试。",
                true,
            ),
            FinderDragError::DirectoryNotAllowed
            | FinderDragError::SymlinkNotAllowed
            | FinderDragError::AliasNotAllowed
            | FinderDragError::NotRegularFile
            | FinderDragError::OutsideProject
            | FinderDragError::ProjectRootUnavailable => Self::new(
                "finder_drag_target_rejected",
                ErrorCategory::Validation,
                "该选择不能拖出 Viewer。",
                false,
            ),
            FinderDragError::NativeUnavailable => Self::new(
                "finder_drag_unavailable",
                ErrorCategory::Environment,
                "当前无法启动 Finder 拖动，请重试。",
                true,
            ),
        }
    }
}
use viewer_infrastructure::{
    image_cache::ImageArtifactRegistryError,
    portable::{PortableMarkerStoreError, PortableMetadataError},
    search::index::SessionIndexError,
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

impl From<PortableMetadataError> for CommandError {
    fn from(_error: PortableMetadataError) -> Self {
        Self::new(
            "portable_metadata_unavailable",
            ErrorCategory::Consistency,
            "项目审阅数据不可用，请检查项目权限或元数据后重试。",
            true,
        )
    }
}

impl From<PortableMarkerStoreError> for CommandError {
    fn from(_error: PortableMarkerStoreError) -> Self {
        Self::new(
            "portable_metadata_unavailable",
            ErrorCategory::Consistency,
            "项目审阅数据不可用，请重新打开项目。",
            true,
        )
    }
}

impl From<MarkerServiceError> for CommandError {
    fn from(error: MarkerServiceError) -> Self {
        match error {
            MarkerServiceError::ReadOnly
            | MarkerServiceError::Store(MarkerStoreError::ReadOnly) => Self::new(
                "project_read_only",
                ErrorCategory::Conflict,
                "当前项目为只读，无法保存标记。",
                false,
            ),
            MarkerServiceError::EmptyTargets
            | MarkerServiceError::DuplicateTarget
            | MarkerServiceError::Store(MarkerStoreError::InvalidTarget) => Self::new(
                "invalid_marker_targets",
                ErrorCategory::Validation,
                "所选标记目标无效，请刷新后重试。",
                false,
            ),
            MarkerServiceError::Store(MarkerStoreError::Unavailable) => Self::new(
                "portable_metadata_unavailable",
                ErrorCategory::Consistency,
                "无法保存项目审阅数据，请重试。",
                true,
            ),
            MarkerServiceError::CommittedButProjectionStale => Self::new(
                "marker_projection_stale",
                ErrorCategory::Consistency,
                "标记已保存，界面索引需要重新加载。",
                true,
            ),
        }
    }
}

impl From<UndoServiceError> for CommandError {
    fn from(error: UndoServiceError) -> Self {
        match error {
            UndoServiceError::StaleSession => Self::new(
                "stale_project_session",
                ErrorCategory::Conflict,
                "项目会话已变化，请重试。",
                true,
            ),
            UndoServiceError::ReadOnly | UndoServiceError::Store(MarkerStoreError::ReadOnly) => {
                Self::new(
                    "project_read_only",
                    ErrorCategory::Conflict,
                    "当前项目为只读，无法撤销。",
                    false,
                )
            }
            UndoServiceError::Undo(UndoError::DestinationOccupied) => Self::new(
                "undo_destination_occupied",
                ErrorCategory::Conflict,
                "原位置已被占用，未执行撤销。",
                false,
            ),
            UndoServiceError::Undo(UndoError::IdentityChanged | UndoError::OutsideProject) => {
                Self::new(
                    "undo_target_changed",
                    ErrorCategory::Conflict,
                    "文件已变化，未执行撤销。",
                    false,
                )
            }
            UndoServiceError::Undo(UndoError::File(_))
            | UndoServiceError::Store(MarkerStoreError::Unavailable)
            | UndoServiceError::StackChanged => Self::new(
                "undo_unavailable",
                ErrorCategory::Consistency,
                "暂时无法撤销，请重试。",
                true,
            ),
            UndoServiceError::Store(MarkerStoreError::InvalidTarget) => Self::new(
                "undo_target_changed",
                ErrorCategory::Conflict,
                "标记已变化，未执行撤销。",
                false,
            ),
            UndoServiceError::CommittedButProjectionStale => Self::new(
                "undo_projection_stale",
                ErrorCategory::Consistency,
                "撤销已保存，界面索引需要重新加载。",
                true,
            ),
        }
    }
}

impl From<SearchError> for CommandError {
    fn from(error: SearchError) -> Self {
        match error {
            SearchError::InvalidSession | SearchError::Stale => stale_search_revision(),
            SearchError::InvalidQuery => Self::new(
                "invalid_search_query",
                ErrorCategory::Validation,
                "搜索条件无效，请调整后重试。",
                false,
            ),
            SearchError::Backend(_) => Self::new(
                "search_unavailable",
                ErrorCategory::Consistency,
                "搜索索引暂时不可用，请重试。",
                true,
            ),
        }
    }
}

pub fn stale_search_revision() -> CommandError {
    CommandError::new(
        "stale_search_revision",
        ErrorCategory::Conflict,
        "该搜索请求已过期。",
        false,
    )
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

impl From<ViewerSettingsError> for CommandError {
    fn from(_error: ViewerSettingsError) -> Self {
        Self::new(
            "settings_write_failed",
            ErrorCategory::Environment,
            "设置未能保存",
            true,
        )
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

#[cfg(test)]
mod tests {
    use super::{CommandError, ErrorCategory};
    use viewer_application::ViewerSettingsError;

    #[test]
    fn settings_write_failure_is_retryable_environment_error() {
        assert_eq!(
            CommandError::from(ViewerSettingsError::Unavailable),
            CommandError::new(
                "settings_write_failed",
                ErrorCategory::Environment,
                "设置未能保存",
                true,
            )
        );
    }
}
