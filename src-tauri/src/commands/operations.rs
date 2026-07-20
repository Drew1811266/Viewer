use crate::{
    commands::{browse::parse_entity_id, search::parse_session_id},
    dto::{
        CancelOperationRequestDto, ExecuteFileCommandRequestDto, FileCommandPreflightDto,
        OperationProgressDto, OperationResultPageDto, OperationResultsRequestDto,
        OperationStartedDto, OperationStatusRequestDto, PreflightFileCommandRequestDto,
        PreviewRenameRequestDto, RenamePreviewDto, UndoLastOperationRequestDto, UndoReceiptDto,
    },
    error::{CommandError, ErrorCategory},
    state::DesktopRuntime,
};
use std::{str::FromStr, sync::Arc};
use tauri::State;
use viewer_domain::{OperationId, search::Generation};

#[tauri::command]
pub async fn preview_rename(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: PreviewRenameRequestDto,
) -> Result<RenamePreviewDto, CommandError> {
    let entity_ids = request
        .entity_ids
        .iter()
        .map(|value| parse_entity_id(value))
        .collect::<Result<Vec<_>, _>>()?;
    runtime
        .preview_rename(
            parse_session_id(&request.session_id)?,
            Generation::new(request.generation),
            &entity_ids,
            request.rules.into(),
        )
        .await
        .map(Into::into)
}

#[tauri::command]
pub async fn execute_file_command(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ExecuteFileCommandRequestDto,
) -> Result<OperationStartedDto, CommandError> {
    let items = request
        .items
        .into_iter()
        .map(|item| item.into_domain().map_err(|_| invalid_operation_target()))
        .collect::<Result<Vec<_>, _>>()?;
    let conflicts = request.conflicts.into_iter().map(Into::into).collect();
    runtime
        .execute_file_command(
            parse_session_id(&request.session_id)?,
            Generation::new(request.generation),
            request.kind,
            items,
            conflicts,
        )
        .await
        .map(|started| OperationStartedDto {
            batch_id: started.batch_id.to_string(),
        })
}

#[tauri::command]
pub async fn preflight_file_command(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: PreflightFileCommandRequestDto,
) -> Result<FileCommandPreflightDto, CommandError> {
    let items = request
        .items
        .into_iter()
        .map(|item| item.into_domain().map_err(|_| invalid_operation_target()))
        .collect::<Result<Vec<_>, _>>()?;
    runtime
        .preflight_file_command(viewer_application::file_commands::FileCommand {
            session_id: parse_session_id(&request.session_id)?,
            generation: Generation::new(request.generation),
            kind: request.kind,
            items,
        })
        .await
        .map(Into::into)
}

#[tauri::command]
pub async fn operation_status(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: OperationStatusRequestDto,
) -> Result<OperationProgressDto, CommandError> {
    let session_id = parse_session_id(&request.session_id)?;
    let progress = runtime
        .operation_status(
            session_id,
            Generation::new(request.generation),
            parse_batch_id(&request.batch_id)?,
        )
        .await?;
    Ok(OperationProgressDto::from_progress(
        session_id,
        request.generation,
        progress,
    ))
}

#[tauri::command]
pub async fn operation_results(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: OperationResultsRequestDto,
) -> Result<OperationResultPageDto, CommandError> {
    runtime
        .operation_results(
            parse_session_id(&request.session_id)?,
            Generation::new(request.generation),
            parse_batch_id(&request.batch_id)?,
            usize::try_from(request.offset).unwrap_or(usize::MAX),
            usize::try_from(request.limit).unwrap_or(usize::MAX),
        )
        .await
        .map(Into::into)
}

#[tauri::command]
pub async fn cancel_operation(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: CancelOperationRequestDto,
) -> Result<bool, CommandError> {
    runtime
        .cancel_operation(
            parse_session_id(&request.session_id)?,
            Generation::new(request.generation),
            parse_batch_id(&request.batch_id)?,
        )
        .await
}

#[tauri::command]
pub async fn undo_last_operation(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: UndoLastOperationRequestDto,
) -> Result<Option<UndoReceiptDto>, CommandError> {
    runtime
        .undo_last_operation(
            parse_session_id(&request.session_id)?,
            Generation::new(request.generation),
        )
        .await
        .map(|receipt| receipt.map(Into::into))
}

#[tauri::command]
pub async fn open_permission_settings() -> Result<(), CommandError> {
    viewer_platform_macos::settings::open_privacy_and_security().map_err(|_| {
        CommandError::new(
            "permission_settings_unavailable",
            ErrorCategory::Environment,
            "无法打开系统权限设置，请手动打开系统设置。",
            true,
        )
    })
}

fn parse_batch_id(value: &str) -> Result<OperationId, CommandError> {
    OperationId::from_str(value).map_err(|_| invalid_operation_id())
}

fn invalid_operation_id() -> CommandError {
    CommandError::new(
        "invalid_operation_id",
        ErrorCategory::Validation,
        "文件操作标识无效。",
        false,
    )
}

fn invalid_operation_target() -> CommandError {
    CommandError::new(
        "invalid_operation_target",
        ErrorCategory::Validation,
        "文件操作目标无效，请刷新项目后重试。",
        false,
    )
}
