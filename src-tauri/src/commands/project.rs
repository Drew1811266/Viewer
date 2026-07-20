use crate::{
    ExitGate,
    dto::{CloseChoiceDto, CloseRequestOutcomeDto, CloseTargetDto, ProjectSnapshot},
    error::CommandError,
    state::{CloseChoice, CloseCompletionAction, CloseRequestOutcome, CloseTarget, DesktopRuntime},
};
use std::{path::PathBuf, sync::Arc};
use tauri::{AppHandle, State, WebviewWindow};

#[tauri::command]
pub async fn open_project(
    runtime: State<'_, Arc<DesktopRuntime>>,
    root: String,
) -> Result<ProjectSnapshot, CommandError> {
    runtime.open_project(&PathBuf::from(root)).await
}

#[tauri::command]
pub async fn close_project(
    runtime: State<'_, Arc<DesktopRuntime>>,
    choice: Option<CloseChoiceDto>,
    target: Option<CloseTargetDto>,
    app: AppHandle,
    window: WebviewWindow,
    exit_gate: State<'_, ExitGate>,
) -> Result<CloseRequestOutcomeDto, CommandError> {
    let choice = choice.map(|choice| match choice {
        CloseChoiceDto::Wait => CloseChoice::Wait,
        CloseChoiceDto::CancelPending => CloseChoice::CancelPending,
        CloseChoiceDto::Stay => CloseChoice::Stay,
    });
    let target = match target.unwrap_or(CloseTargetDto::Project) {
        CloseTargetDto::Project => CloseTarget::Project,
        CloseTargetDto::Window => CloseTarget::Window,
        CloseTargetDto::Application => CloseTarget::Application,
    };
    let outcome = runtime.request_close_for(choice, target).await?;
    match outcome.completion_action(target) {
        CloseCompletionAction::KeepOpen | CloseCompletionAction::ShowEmptyProject => {}
        CloseCompletionAction::HideWindow => {
            let _ = window.hide();
        }
        CloseCompletionAction::ExitApplication => {
            exit_gate.allow_exit();
            app.exit(0);
        }
    }
    Ok(match outcome {
        CloseRequestOutcome::Closed => CloseRequestOutcomeDto::Closed,
        CloseRequestOutcome::Stayed => CloseRequestOutcomeDto::Stayed,
    })
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
