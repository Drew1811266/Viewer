use serde::Serialize;
use viewer_application::{
    BrowseError, BrowseIndexError, FinderDragError, ImageError, ProjectOpenError,
    ProjectProbeError, ReviewSessionError, TextPreviewError, ViewerSettingsError,
    metadata::{MarkerServiceError, MarkerStoreError},
    search::SearchError,
    undo::{UndoError, UndoServiceError},
};

impl From<ReviewSessionError> for CommandError {
    fn from(error: ReviewSessionError) -> Self {
        let (code, category, message, retryable) = match error {
            ReviewSessionError::ReadOnly => (
                "review_read_only",
                ErrorCategory::Conflict,
                "当前项目为只读，不能修改评审。",
                false,
            ),
            ReviewSessionError::Busy => (
                "review_busy",
                ErrorCategory::Conflict,
                "另一个 Viewer 正在修改该项目的评审，请稍后重试。",
                true,
            ),
            ReviewSessionError::StaleRound => (
                "review_stale_round",
                ErrorCategory::Conflict,
                "评审轮次已变化，请刷新后重试。",
                true,
            ),
            ReviewSessionError::StaleRevision => (
                "review_stale_revision",
                ErrorCategory::Conflict,
                "评审内容已变化，请刷新后重试。",
                true,
            ),
            ReviewSessionError::StaleProposal | ReviewSessionError::StaleCompletionProposal => (
                "review_proposal_stale",
                ErrorCategory::Conflict,
                "评审确认信息已过期，请重新确认。",
                true,
            ),
            ReviewSessionError::CompletionChanged => (
                "review_completion_changed",
                ErrorCategory::Conflict,
                "素材状态在确认后发生变化，请重新检查完成摘要。",
                true,
            ),
            ReviewSessionError::CompletionBlocked => (
                "review_pending_validation",
                ErrorCategory::Conflict,
                "仍有素材冲突或等待验证，暂时不能完成评审。",
                true,
            ),
            ReviewSessionError::RecoveryRequired => (
                "review_recovery_required",
                ErrorCategory::Consistency,
                "评审记录需要恢复处理；普通素材浏览仍可继续。",
                false,
            ),
            ReviewSessionError::UnsupportedVersion => (
                "review_unsupported_version",
                ErrorCategory::Consistency,
                "该项目的评审协议版本暂不受支持；普通素材浏览仍可继续。",
                false,
            ),
            ReviewSessionError::ScopeChanged | ReviewSessionError::DraftAlreadyActive => (
                "review_version_conflict",
                ErrorCategory::Conflict,
                "评审范围或活动轮次已变化，请刷新后重试。",
                true,
            ),
            ReviewSessionError::EmptyScope
            | ReviewSessionError::InvalidFeedback
            | ReviewSessionError::FeedbackNotFound
            | ReviewSessionError::InvalidData => (
                "review_invalid_data",
                ErrorCategory::Validation,
                "评审请求无效，请刷新后重试。",
                false,
            ),
            ReviewSessionError::Cancelled => (
                "review_cancelled",
                ErrorCategory::Conflict,
                "评审任务已取消。",
                true,
            ),
            ReviewSessionError::InvalidState => (
                "review_invalid_state",
                ErrorCategory::Conflict,
                "当前评审状态不支持该操作。",
                false,
            ),
            ReviewSessionError::AbandonFailed
            | ReviewSessionError::SaveFailed
            | ReviewSessionError::RepositoryUnavailable
            | ReviewSessionError::AssetUnavailable => (
                "review_unavailable",
                ErrorCategory::Environment,
                "评审数据当前不可用，请稍后重试。",
                true,
            ),
        };
        Self::new(code, category, message, retryable)
    }
}

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

impl From<crate::state::RuntimeError> for CommandError {
    fn from(error: crate::state::RuntimeError) -> Self {
        use crate::state::RuntimeError;

        match error {
            RuntimeError::NotImage => Self::new(
                error.code(),
                ErrorCategory::Validation,
                "所选文件不是可预览图片。",
                false,
            ),
            RuntimeError::NotVideo => Self::new(
                error.code(),
                ErrorCategory::Validation,
                "所选文件不是视频。",
                false,
            ),
            RuntimeError::StaleSession => Self::new(
                error.code(),
                ErrorCategory::Consistency,
                "视频已不属于当前项目会话，请刷新后重试。",
                true,
            ),
            RuntimeError::PathNotAuthorized => Self::new(
                error.code(),
                ErrorCategory::Validation,
                "该视频路径不再受当前项目授权。",
                false,
            ),
            RuntimeError::MetadataUnavailable => Self::new(
                error.code(),
                ErrorCategory::Content,
                "视频信息尚不可用，请稍后重试。",
                true,
            ),
            RuntimeError::VideoRetryFailed(_) => Self::new(
                error.code(),
                ErrorCategory::Content,
                "视频仍无法打开，请稍后重试。",
                true,
            ),
            RuntimeError::CloseFailed => Self::new(
                error.code(),
                ErrorCategory::Environment,
                "视频已关闭，但原生播放资源未能完整清理。",
                true,
            ),
            RuntimeError::ImageRenderCloseFailed => Self::new(
                error.code(),
                ErrorCategory::Environment,
                "图片预览已关闭，但原生渲染资源未能完整清理。",
                true,
            ),
        }
    }
}

impl From<crate::video_runtime::VideoCommandError> for CommandError {
    fn from(error: crate::video_runtime::VideoCommandError) -> Self {
        use crate::video_runtime::VideoCommandError;

        let (category, message, retryable) = match error {
            VideoCommandError::StaleOpenAttempt => (
                ErrorCategory::Conflict,
                "视频打开请求已被更新的请求替换。",
                false,
            ),
            VideoCommandError::StaleGeneration => {
                (ErrorCategory::Conflict, "视频会话已变化，请重试。", true)
            }
            VideoCommandError::NoActiveSession => {
                (ErrorCategory::Conflict, "当前没有打开的视频。", false)
            }
            VideoCommandError::InvalidState => {
                (ErrorCategory::Conflict, "当前播放状态不支持此操作。", false)
            }
            VideoCommandError::InvalidVolume => (
                ErrorCategory::Validation,
                "音量必须在 0 到 100 之间。",
                false,
            ),
            VideoCommandError::EngineUnavailable => {
                (ErrorCategory::Environment, "视频播放引擎当前不可用。", true)
            }
            VideoCommandError::CacheUnavailable => {
                (ErrorCategory::Environment, "视频缓存当前不可用。", true)
            }
            VideoCommandError::ThumbnailCancelled => (
                ErrorCategory::Conflict,
                "视频缩略图请求已被更新的请求替换。",
                false,
            ),
            VideoCommandError::ThumbnailUnavailable => (
                ErrorCategory::Environment,
                "视频时间轴缩略图当前不可用。",
                true,
            ),
            VideoCommandError::InvalidThumbnailRequest => {
                (ErrorCategory::Validation, "视频缩略图请求标识无效。", false)
            }
        };
        Self::new(error.code(), category, message, retryable)
    }
}

impl From<crate::image_render_runtime::ImageRenderRuntimeError> for CommandError {
    fn from(error: crate::image_render_runtime::ImageRenderRuntimeError) -> Self {
        use crate::image_render_runtime::ImageRenderRuntimeError;

        let (code, category, message, retryable) = match error {
            ImageRenderRuntimeError::InvalidEnvelope => (
                "image_render_invalid_envelope",
                ErrorCategory::Validation,
                "图片渲染命令标识无效。",
                false,
            ),
            ImageRenderRuntimeError::InvalidEntity => (
                "image_render_invalid_entity",
                ErrorCategory::Validation,
                "图片标识无效。",
                false,
            ),
            ImageRenderRuntimeError::UnauthorizedEntity => (
                "image_render_unauthorized_entity",
                ErrorCategory::Consistency,
                "图片已不属于当前项目会话，请刷新后重试。",
                true,
            ),
            ImageRenderRuntimeError::InvalidCommand => (
                "image_render_invalid_command",
                ErrorCategory::Validation,
                "图片渲染命令内容无效。",
                false,
            ),
            ImageRenderRuntimeError::DriverUnavailable => (
                "image_render_driver_unavailable",
                ErrorCategory::Environment,
                "原生图片渲染器暂时不可用。",
                true,
            ),
            ImageRenderRuntimeError::DriverFailed => (
                "image_render_driver_failed",
                ErrorCategory::Environment,
                "原生图片渲染操作失败，请重试。",
                true,
            ),
        };
        Self::new(code, category, message, retryable)
    }
}

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
                "image_request_cancelled",
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
    use super::{
        BrowseError, BrowseIndexError, CommandError, ErrorCategory, FinderDragError,
        ImageArtifactRegistryError, ImageError, MarkerServiceError, ProjectOpenError,
        ProjectProbeError, SearchError, SessionCacheError, TextPreviewError, UndoServiceError,
        ViewerSettingsError,
    };
    use crate::{state::RuntimeError, video_runtime::VideoCommandError};
    use std::path::PathBuf;

    fn assert_command_contract(
        source: impl Into<CommandError>,
        code: &str,
        category: ErrorCategory,
        user_message: &str,
        retryable: bool,
    ) {
        assert_eq!(
            source.into(),
            CommandError::new(code, category, user_message, retryable)
        );
    }

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

    #[test]
    fn finder_drag_failures_preserve_the_public_command_contract() {
        for (source, code, category, message, retryable) in [
            (
                FinderDragError::EmptySelection,
                "invalid_finder_drag_selection",
                ErrorCategory::Validation,
                "请选择至少一个且不重复的文件。",
                false,
            ),
            (
                FinderDragError::TooManySelection,
                "finder_drag_selection_too_large",
                ErrorCategory::Validation,
                "一次拖动的文件数量过多，请减少选择后重试。",
                false,
            ),
            (
                FinderDragError::EntityNotFound,
                "finder_drag_selection_stale",
                ErrorCategory::Consistency,
                "部分所选文件已不可用，请刷新项目后重试。",
                true,
            ),
            (
                FinderDragError::DirectoryNotAllowed,
                "finder_drag_target_rejected",
                ErrorCategory::Validation,
                "该选择不能拖出 Viewer。",
                false,
            ),
            (
                FinderDragError::NativeUnavailable,
                "finder_drag_unavailable",
                ErrorCategory::Environment,
                "当前无法启动 Finder 拖动，请重试。",
                true,
            ),
        ] {
            assert_command_contract(source, code, category, message, retryable);
        }
    }

    #[test]
    fn video_failures_preserve_the_public_command_contract() {
        for (source, code, category, message, retryable) in [
            (
                VideoCommandError::StaleOpenAttempt,
                "stale_video_open_attempt",
                ErrorCategory::Conflict,
                "视频打开请求已被更新的请求替换。",
                false,
            ),
            (
                VideoCommandError::StaleGeneration,
                "stale_video_generation",
                ErrorCategory::Conflict,
                "视频会话已变化，请重试。",
                true,
            ),
            (
                VideoCommandError::NoActiveSession,
                "video_not_open",
                ErrorCategory::Conflict,
                "当前没有打开的视频。",
                false,
            ),
            (
                VideoCommandError::InvalidState,
                "invalid_video_state",
                ErrorCategory::Conflict,
                "当前播放状态不支持此操作。",
                false,
            ),
            (
                VideoCommandError::InvalidVolume,
                "invalid_video_volume",
                ErrorCategory::Validation,
                "音量必须在 0 到 100 之间。",
                false,
            ),
            (
                VideoCommandError::EngineUnavailable,
                "video_engine_unavailable",
                ErrorCategory::Environment,
                "视频播放引擎当前不可用。",
                true,
            ),
            (
                VideoCommandError::CacheUnavailable,
                "video_cache_unavailable",
                ErrorCategory::Environment,
                "视频缓存当前不可用。",
                true,
            ),
            (
                VideoCommandError::ThumbnailCancelled,
                "video_thumbnail_cancelled",
                ErrorCategory::Conflict,
                "视频缩略图请求已被更新的请求替换。",
                false,
            ),
            (
                VideoCommandError::ThumbnailUnavailable,
                "video_thumbnail_unavailable",
                ErrorCategory::Environment,
                "视频时间轴缩略图当前不可用。",
                true,
            ),
            (
                VideoCommandError::InvalidThumbnailRequest,
                "invalid_video_thumbnail_request",
                ErrorCategory::Validation,
                "视频缩略图请求标识无效。",
                false,
            ),
        ] {
            assert_command_contract(source, code, category, message, retryable);
        }
    }

    #[test]
    fn application_failures_preserve_the_public_command_contract() {
        assert_command_contract(
            RuntimeError::NotVideo,
            "not_video",
            ErrorCategory::Validation,
            "所选文件不是视频。",
            false,
        );
        assert_command_contract(
            ProjectOpenError::AlreadyOpen,
            "project_already_open",
            ErrorCategory::Conflict,
            "请先关闭当前项目。",
            false,
        );
        assert_command_contract(
            ProjectProbeError::NotDirectory {
                path: PathBuf::from("/fixture/file"),
            },
            "invalid_project_root",
            ErrorCategory::Validation,
            "请选择一个可读取的真实文件夹。",
            true,
        );
        assert_command_contract(
            SessionCacheError::ArtifactTooLarge,
            "cache_budget_exceeded",
            ErrorCategory::Environment,
            "临时预览超出缓存预算。",
            false,
        );
        assert_command_contract(
            BrowseError::DuplicateSelection,
            "duplicate_selection",
            ErrorCategory::Validation,
            "所选文件不能重复。",
            false,
        );
        assert_command_contract(
            BrowseIndexError::Unavailable("fixture index failure".into()),
            "session_index_unavailable",
            ErrorCategory::Consistency,
            "项目临时索引不可用，请重新打开项目。",
            true,
        );
        assert_command_contract(
            MarkerServiceError::EmptyTargets,
            "invalid_marker_targets",
            ErrorCategory::Validation,
            "所选标记目标无效，请刷新后重试。",
            false,
        );
        assert_command_contract(
            UndoServiceError::StaleSession,
            "stale_project_session",
            ErrorCategory::Conflict,
            "项目会话已变化，请重试。",
            true,
        );
        assert_command_contract(
            SearchError::InvalidQuery,
            "invalid_search_query",
            ErrorCategory::Validation,
            "搜索条件无效，请调整后重试。",
            false,
        );
        assert_command_contract(
            ImageError::Unsupported,
            "image_unavailable",
            ErrorCategory::Content,
            "无法预览该图片。",
            false,
        );
        assert_command_contract(
            ImageArtifactRegistryError::NotAFile,
            "internal_error",
            ErrorCategory::Internal,
            "Viewer 遇到内部错误，请重试。",
            true,
        );
        assert_command_contract(
            TextPreviewError::EncodingRequired,
            "text_encoding_required",
            ErrorCategory::Content,
            "无法自动识别文本编码，请选择编码后重试。",
            true,
        );
    }
}
