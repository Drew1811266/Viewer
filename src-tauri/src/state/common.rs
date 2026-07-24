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

pub(super) fn validated_marker_target(
    active: &ActiveProject,
    node: &viewer_domain::file::FileNode,
) -> Option<MarkerTarget> {
    let candidate = active.root.join(node.relative_path.as_str());
    let metadata = std::fs::symlink_metadata(&candidate).ok()?;
    if metadata.file_type().is_symlink() || is_macos_alias(&candidate) {
        return None;
    }
    let kind_matches = match node.kind {
        FileKind::Directory => metadata.is_dir(),
        FileKind::Jpeg | FileKind::Png | FileKind::Markdown | FileKind::Text => metadata.is_file(),
    };
    if !kind_matches || entity_id_for_metadata(&metadata, &node.relative_path) != node.entity_id {
        return None;
    }
    let canonical = std::fs::canonicalize(&candidate).ok()?;
    if !canonical.starts_with(&active.root) {
        return None;
    }
    Some(MarkerTarget {
        entity_id: node.entity_id,
        relative_path: node.relative_path.clone(),
        kind: node.kind,
        size: if node.kind == FileKind::Directory {
            0
        } else {
            metadata.len()
        },
        modified_ns: modified_ns(&metadata),
    })
}

pub(super) fn image_not_found() -> CommandError {
    CommandError::new(
        "image_not_found",
        crate::error::ErrorCategory::Content,
        "该图片已不可用，请刷新项目后重试。",
        true,
    )
}

pub(super) fn text_not_found() -> CommandError {
    CommandError::new(
        "text_not_found",
        ErrorCategory::Content,
        "该文本文件已不可用，请刷新项目后重试。",
        true,
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

pub(super) fn validated_indexed_source(
    active: &ActiveProject,
    node: &viewer_domain::file::FileNode,
) -> Result<(PathBuf, u64, i128), ()> {
    let candidate = active.root.join(node.relative_path.as_str());
    let metadata = std::fs::symlink_metadata(&candidate).map_err(|_| ())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || is_macos_alias(&candidate) {
        return Err(());
    }
    if entity_id_for_metadata(&metadata, &node.relative_path) != node.entity_id {
        return Err(());
    }
    let parent = candidate.parent().ok_or(())?;
    let canonical_parent = std::fs::canonicalize(parent).map_err(|_| ())?;
    let canonical_source = std::fs::canonicalize(&candidate).map_err(|_| ())?;
    if !canonical_parent.starts_with(&active.root) || !canonical_source.starts_with(&active.root) {
        return Err(());
    }
    Ok((canonical_source, metadata.len(), modified_ns(&metadata)))
}

pub(super) fn resolve_markdown_image_path(
    markdown_path: &RelativePath,
    destination: &str,
) -> Option<RelativePath> {
    if destination.is_empty()
        || destination.starts_with('/')
        || destination.contains("//")
        || destination
            .chars()
            .any(|character| matches!(character, '\0' | '\\' | '%' | '?' | '#' | ':'))
    {
        return None;
    }
    let extension = destination.rsplit_once('.')?.1;
    if !matches!(
        extension.to_ascii_lowercase().as_str(),
        "jpg" | "jpeg" | "png"
    ) {
        return None;
    }

    let mut segments = markdown_path.as_str().split('/').collect::<Vec<_>>();
    segments.pop()?;
    for segment in destination.split('/') {
        match segment {
            "" => return None,
            "." => {}
            ".." => {
                segments.pop()?;
            }
            _ if segment.eq_ignore_ascii_case(".viewer") => return None,
            _ => segments.push(segment),
        }
    }
    RelativePath::parse(&segments.join("/")).ok()
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
        SessionIndexError::MissingTextNode { .. } | SessionIndexError::MissingNode(_)
    )
}
