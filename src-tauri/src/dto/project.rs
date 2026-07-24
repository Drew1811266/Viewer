use serde::{Deserialize, Serialize};
use viewer_application::{ActiveProject, ProjectAccess};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseChoiceDto {
    Wait,
    CancelPending,
    Stay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseRequestOutcomeDto {
    Closed,
    Stayed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseTargetDto {
    Project,
    Window,
    Application,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectChangeReasonDto {
    ExternalChange,
    ExpectedViewerChange,
    Overflow,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectChangedDto {
    pub session_id: String,
    pub generation: u64,
    pub reason: ProjectChangeReasonDto,
    pub added: u64,
    pub removed: u64,
    pub modified: u64,
    pub moved: u64,
    pub marker_paths_moved: u64,
    pub failed: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseBlockedDto {
    pub session_id: String,
    pub generation: u64,
    pub batch_id: String,
    pub target: CloseTargetDto,
}

impl ProjectChangedDto {
    pub fn from_summary(
        session_id: impl ToString,
        generation: u64,
        summary: viewer_application::watcher::ReconcileSummary,
    ) -> Self {
        let reason = match summary.reason {
            viewer_application::watcher::ReconcileReason::ExternalChange => {
                ProjectChangeReasonDto::ExternalChange
            }
            viewer_application::watcher::ReconcileReason::ExpectedViewerChange(_) => {
                ProjectChangeReasonDto::ExpectedViewerChange
            }
            viewer_application::watcher::ReconcileReason::Overflow => {
                ProjectChangeReasonDto::Overflow
            }
        };
        Self {
            session_id: session_id.to_string(),
            generation,
            reason,
            added: summary.added,
            removed: summary.removed,
            modified: summary.modified,
            moved: summary.moved,
            marker_paths_moved: summary.marker_paths_moved,
            failed: summary.failed,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectAccessDto {
    ReadWrite,
    ReadOnly,
}

impl From<ProjectAccess> for ProjectAccessDto {
    fn from(access: ProjectAccess) -> Self {
        match access {
            ProjectAccess::ReadWrite => Self::ReadWrite,
            ProjectAccess::ReadOnly => Self::ReadOnly,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshot {
    pub project_id: String,
    pub session_id: String,
    pub generation: u64,
    pub display_name: String,
    pub access: ProjectAccessDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_report: Option<RecoveryReportDto>,
}

impl From<&ActiveProject> for ProjectSnapshot {
    fn from(project: &ActiveProject) -> Self {
        Self {
            project_id: project.project_id.to_string(),
            session_id: project.session_id.to_string(),
            generation: project.generation.get(),
            display_name: project.display_name.clone(),
            access: project.access.into(),
            recovery_report: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryReportDto {
    pub recovered: u32,
    pub needs_user_review: u32,
}
