use crate::{dto::ProjectSnapshot, error::CommandError, state::DesktopRuntime};
use std::{path::PathBuf, sync::Arc};
use tauri::State;

#[tauri::command]
pub async fn open_project(
    runtime: State<'_, Arc<DesktopRuntime>>,
    root: String,
) -> Result<ProjectSnapshot, CommandError> {
    runtime.open_project(&PathBuf::from(root)).await
}

#[tauri::command]
pub async fn close_project(runtime: State<'_, Arc<DesktopRuntime>>) -> Result<(), CommandError> {
    runtime.close_project().await
}

#[tauri::command]
pub async fn project_snapshot(
    runtime: State<'_, Arc<DesktopRuntime>>,
) -> Result<Option<ProjectSnapshot>, CommandError> {
    Ok(runtime.snapshot().await)
}

#[tauri::command]
pub async fn cancel_task(
    runtime: State<'_, Arc<DesktopRuntime>>,
    task_id: String,
) -> Result<bool, CommandError> {
    Ok(runtime.cancel_task(&task_id).await)
}
