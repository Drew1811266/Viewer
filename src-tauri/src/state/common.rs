use super::*;

pub(super) fn validate_project_request(
    active: &ActiveProject,
    expected_session: SessionId,
    expected_generation: Generation,
) -> Result<(), CommandError> {
    if active.session_id == expected_session && active.generation == expected_generation {
        Ok(())
    } else {
        Err(stale_project_session())
    }
}

pub(super) fn claim_search_revision(latest: &AtomicU64, revision: u64) -> bool {
    revision != 0
        && latest
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (revision > current).then_some(revision)
            })
            .is_ok()
}

pub(crate) fn stale_project_session() -> CommandError {
    CommandError::new(
        "stale_project_session",
        ErrorCategory::Conflict,
        "该请求不属于当前项目会话。",
        false,
    )
}

pub(super) fn selection_not_found() -> CommandError {
    CommandError::new(
        "selection_not_found",
        ErrorCategory::Content,
        "部分所选文件已不可用，请刷新项目后重试。",
        true,
    )
}

pub(crate) fn project_not_open() -> CommandError {
    CommandError::new(
        "project_not_open",
        crate::error::ErrorCategory::Conflict,
        "请先打开一个项目。",
        false,
    )
}

pub(crate) fn internal_command_error() -> CommandError {
    CommandError::new(
        "internal_error",
        ErrorCategory::Internal,
        "Viewer 遇到内部错误，请重试。",
        true,
    )
}

pub(super) fn operation_backend_unavailable() -> CommandError {
    CommandError::new(
        "operation_backend_unavailable",
        ErrorCategory::Environment,
        "文件操作服务不可用，请重新打开项目。",
        true,
    )
}

pub(super) fn project_read_only_operation() -> CommandError {
    CommandError::new(
        "project_read_only",
        ErrorCategory::Conflict,
        "当前项目为只读，无法执行文件操作。",
        false,
    )
}

pub(super) fn invalid_rename_preview() -> CommandError {
    CommandError::new(
        "invalid_rename_preview",
        ErrorCategory::Validation,
        "无法生成安全的重命名预览，请刷新项目后重试。",
        false,
    )
}

pub(super) fn operation_runtime_error(error: OperationRuntimeError) -> CommandError {
    use viewer_application::file_commands::FileCommandServiceError as ServiceError;
    match error {
        OperationRuntimeError::Service(ServiceError::ReadOnly) => project_read_only_operation(),
        OperationRuntimeError::Service(ServiceError::StaleSession) => stale_project_session(),
        OperationRuntimeError::Service(ServiceError::EmptyTargets)
        | OperationRuntimeError::Service(ServiceError::TooManyTargets)
        | OperationRuntimeError::Service(ServiceError::DuplicateTarget)
        | OperationRuntimeError::Service(ServiceError::ActionKindMismatch) => CommandError::new(
            "invalid_operation_targets",
            ErrorCategory::Validation,
            "所选文件操作目标无效，请检查选择后重试。",
            false,
        ),
        OperationRuntimeError::Service(ServiceError::BatchActive) => CommandError::new(
            "operation_batch_active",
            ErrorCategory::Conflict,
            "已有文件操作正在进行，请等待或取消后重试。",
            true,
        ),
        OperationRuntimeError::Service(ServiceError::PreflightBlocked)
        | OperationRuntimeError::Service(ServiceError::MissingConflictResolution)
        | OperationRuntimeError::Service(ServiceError::InvalidConflictResolution) => {
            CommandError::new(
                "operation_conflict_unresolved",
                ErrorCategory::Conflict,
                "部分文件存在冲突，请选择处理方式后重试。",
                false,
            )
        }
        OperationRuntimeError::Service(ServiceError::AlreadyExecuted)
        | OperationRuntimeError::Service(ServiceError::PreflightContract)
        | OperationRuntimeError::Service(ServiceError::BackendUnavailable)
        | OperationRuntimeError::BatchFailed => operation_backend_unavailable(),
        OperationRuntimeError::BatchNotFound => CommandError::new(
            "operation_not_found",
            ErrorCategory::Content,
            "该文件操作记录已不可用。",
            false,
        ),
    }
}

#[cfg(unix)]
pub(super) fn entity_id_for_metadata(
    metadata: &std::fs::Metadata,
    _relative_path: &viewer_domain::RelativePath,
) -> EntityId {
    use std::os::unix::fs::MetadataExt;
    EntityId::from_u128((u128::from(metadata.dev()) << 64) | u128::from(metadata.ino()))
}

#[cfg(not(unix))]
pub(super) fn entity_id_for_metadata(
    metadata: &std::fs::Metadata,
    relative_path: &viewer_domain::RelativePath,
) -> EntityId {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    relative_path.as_str().hash(&mut hasher);
    metadata.len().hash(&mut hasher);
    EntityId::from_u128(u128::from(hasher.finish()))
}

#[cfg(unix)]
pub(super) fn modified_ns(metadata: &std::fs::Metadata) -> i128 {
    use std::os::unix::fs::MetadataExt;
    i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec())
}

#[cfg(not(unix))]
pub(super) fn modified_ns(metadata: &std::fs::Metadata) -> i128 {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos() as i128)
}

pub(crate) fn is_stale_derived_write_error(error: &SessionIndexError) -> bool {
    matches!(
        error,
        SessionIndexError::MissingTextNode { .. }
            | SessionIndexError::MissingNode(_)
            | SessionIndexError::StaleDerivedMetadata { .. }
    )
}
