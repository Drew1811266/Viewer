use std::sync::Arc;

use tauri::State;
use viewer_domain::plan::PlanMarks;

use crate::dto::plan::{PlanMarksDto, PlanSnapshotDto, PlanSummaryDto};
use crate::error::CommandError;
use crate::state::DesktopRuntime;
use crate::state::plan::{self as plan_view, PlanViewError};

fn map_error(error: PlanViewError) -> CommandError {
    let (code, category, retryable) = match &error {
        PlanViewError::Invalid(_) => (
            "plan_invalid",
            crate::error::ErrorCategory::Validation,
            false,
        ),
        PlanViewError::NotFound(_) => (
            "plan_not_found",
            crate::error::ErrorCategory::Content,
            false,
        ),
        PlanViewError::Io(_) => ("plan_io", crate::error::ErrorCategory::Internal, true),
    };
    CommandError::new(code, category, &error.to_string(), retryable)
}

async fn blocking<T, F>(work: F) -> Result<T, CommandError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, PlanViewError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| {
            CommandError::new(
                "plan_io",
                crate::error::ErrorCategory::Internal,
                &error.to_string(),
                true,
            )
        })?
        .map_err(map_error)
}

#[tauri::command]
pub async fn plan_list(
    runtime: State<'_, Arc<DesktopRuntime>>,
) -> Result<Vec<PlanSummaryDto>, CommandError> {
    let root = runtime.plan_project_root().await.map_err(stale_session)?;
    blocking(move || plan_view::list_plans(&root)).await
}

#[tauri::command]
pub async fn plan_snapshot(
    runtime: State<'_, Arc<DesktopRuntime>>,
    plan_id: String,
) -> Result<PlanSnapshotDto, CommandError> {
    let root = runtime.plan_project_root().await.map_err(stale_session)?;
    blocking(move || plan_view::plan_snapshot(&root, &plan_id)).await
}

#[tauri::command]
pub async fn plan_marks_read(
    runtime: State<'_, Arc<DesktopRuntime>>,
    plan_id: String,
) -> Result<PlanMarksDto, CommandError> {
    let root = runtime.plan_project_root().await.map_err(stale_session)?;
    blocking(move || plan_view::read_marks(&root, &plan_id)).await
}

#[tauri::command]
pub async fn plan_marks_write(
    runtime: State<'_, Arc<DesktopRuntime>>,
    marks: PlanMarks,
) -> Result<(), CommandError> {
    let root = runtime.plan_project_root().await.map_err(stale_session)?;
    blocking(move || plan_view::write_marks(&root, marks)).await
}

fn stale_session(error: crate::state::RuntimeError) -> CommandError {
    CommandError::new(
        "plan_no_project",
        crate::error::ErrorCategory::Conflict,
        &error.to_string(),
        true,
    )
}
