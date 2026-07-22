use crate::{
    ExitGate, close_completion_after_attempt,
    dto::{CloseChoiceDto, CloseRequestOutcomeDto, CloseTargetDto, ProjectSnapshot},
    error::CommandError,
    is_terminal_close_cleanup_failure,
    state::{CloseChoice, CloseCompletionAction, CloseRequestOutcome, CloseTarget, DesktopRuntime},
};
use std::{path::PathBuf, sync::Arc};
use tauri::{AppHandle, State, WebviewWindow};

struct CloseCommandResolution {
    completion_action: Option<CloseCompletionAction>,
    response: Result<CloseRequestOutcomeDto, CommandError>,
}

fn resolve_close_command_attempt(
    attempt: Result<CloseRequestOutcome, CommandError>,
    target: CloseTarget,
) -> CloseCommandResolution {
    let completion_action = close_completion_after_attempt(&attempt, target);
    let response = attempt.map(|outcome| match outcome {
        CloseRequestOutcome::Closed => CloseRequestOutcomeDto::Closed,
        CloseRequestOutcome::Stayed => CloseRequestOutcomeDto::Stayed,
    });
    CloseCommandResolution {
        completion_action,
        response,
    }
}

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
    let attempt = runtime.request_close_for(choice, target).await;
    if attempt
        .as_ref()
        .is_err_and(is_terminal_close_cleanup_failure)
        && let Err(error) = runtime.cleanup_session_caches_for_process_exit()
    {
        eprintln!(
            "Viewer command-close cache cleanup retry failed: {}",
            error.code
        );
    }
    let resolution = resolve_close_command_attempt(attempt, target);
    match resolution.completion_action {
        None | Some(CloseCompletionAction::KeepOpen | CloseCompletionAction::ShowEmptyProject) => {}
        Some(CloseCompletionAction::HideWindow) => {
            let _ = window.hide();
        }
        Some(CloseCompletionAction::ExitApplication) => {
            exit_gate.allow_exit();
            app.exit(0);
        }
    }
    resolution.response
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCategory;

    #[test]
    fn terminal_cleanup_error_preserves_the_error_and_commits_native_actions() {
        for (target, expected_action) in [
            (CloseTarget::Window, CloseCompletionAction::HideWindow),
            (
                CloseTarget::Application,
                CloseCompletionAction::ExitApplication,
            ),
        ] {
            let resolution = resolve_close_command_attempt(
                Err(CommandError::new(
                    "project_closed_cache_cleanup_failed",
                    ErrorCategory::Environment,
                    "cache cleanup failed",
                    true,
                )),
                target,
            );

            assert_eq!(resolution.completion_action, Some(expected_action));
            assert_eq!(
                resolution.response.unwrap_err().code,
                "project_closed_cache_cleanup_failed"
            );
        }
    }

    #[test]
    fn stayed_close_keeps_the_native_target_open_and_returns_stayed() {
        let resolution =
            resolve_close_command_attempt(Ok(CloseRequestOutcome::Stayed), CloseTarget::Window);

        assert_eq!(resolution.completion_action, None);
        assert!(matches!(
            resolution.response,
            Ok(CloseRequestOutcomeDto::Stayed)
        ));
    }
}
