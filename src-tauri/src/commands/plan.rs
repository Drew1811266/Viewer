use std::path::Path;

use tauri::command;
use viewer_domain::plan::PlanMarks;

use crate::dto::plan::{PlanMarksDto, PlanSnapshotDto, PlanSummaryDto};
use crate::error::{CommandError, ErrorCategory};
use crate::state::plan::{self as plan_view, PlanViewError};

fn map_error(error: PlanViewError) -> CommandError {
    match error {
        PlanViewError::Invalid(message) => {
            CommandError::new("plan_invalid", ErrorCategory::Validation, &message, false)
        }
        PlanViewError::NotFound(message) => {
            CommandError::new("plan_not_found", ErrorCategory::Content, &message, false)
        }
        PlanViewError::Io(message) => {
            CommandError::new("plan_io", ErrorCategory::Internal, &message, true)
        }
    }
}

#[command]
pub fn plan_list(project_root: String) -> Result<Vec<PlanSummaryDto>, CommandError> {
    plan_view::list_plans(Path::new(&project_root)).map_err(map_error)
}

#[command]
pub fn plan_snapshot(
    project_root: String,
    plan_id: String,
) -> Result<PlanSnapshotDto, CommandError> {
    plan_view::plan_snapshot(Path::new(&project_root), &plan_id).map_err(map_error)
}

#[command]
pub fn plan_marks_read(
    project_root: String,
    plan_id: String,
) -> Result<PlanMarksDto, CommandError> {
    plan_view::read_marks(Path::new(&project_root), &plan_id).map_err(map_error)
}

#[command]
pub fn plan_marks_write(project_root: String, marks: PlanMarks) -> Result<(), CommandError> {
    plan_view::write_marks(Path::new(&project_root), marks).map_err(map_error)
}
