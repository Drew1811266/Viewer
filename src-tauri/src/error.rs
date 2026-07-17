use serde::Serialize;
use viewer_application::{ProjectOpenError, ProjectProbeError};
use viewer_infrastructure::{search::index::SessionIndexError, session_cache::SessionCacheError};

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
