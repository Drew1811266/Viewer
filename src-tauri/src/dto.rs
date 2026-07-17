use serde::Serialize;
use viewer_application::scan::{ScanEvent, ScanTotals};
use viewer_application::{ActiveProject, ProjectAccess};
use viewer_domain::{RelativePath, TaskId, file::FileNode};

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
}

impl From<&ActiveProject> for ProjectSnapshot {
    fn from(project: &ActiveProject) -> Self {
        Self {
            project_id: project.project_id.to_string(),
            session_id: project.session_id.to_string(),
            generation: project.generation.get(),
            display_name: project.display_name.clone(),
            access: project.access.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannedNodeDto {
    pub entity_id: String,
    pub relative_path: String,
    pub kind: viewer_domain::file::FileKind,
    pub size: u64,
    pub modified_ns: String,
}

impl From<&FileNode> for ScannedNodeDto {
    fn from(node: &FileNode) -> Self {
        Self {
            entity_id: node.entity_id.to_string(),
            relative_path: node.relative_path.as_str().to_owned(),
            kind: node.kind,
            size: node.size,
            modified_ns: node.modified_ns.to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanTotalsDto {
    pub folders: u64,
    pub files: u64,
    pub failed: u64,
}

impl From<ScanTotals> for ScanTotalsDto {
    fn from(totals: ScanTotals) -> Self {
        Self {
            folders: totals.folders,
            files: totals.files,
            failed: totals.failed,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScanEventDto {
    Folders {
        #[serde(rename = "sessionId")]
        session_id: String,
        generation: u64,
        #[serde(rename = "taskId")]
        task_id: String,
        nodes: Vec<ScannedNodeDto>,
    },
    Files {
        #[serde(rename = "sessionId")]
        session_id: String,
        generation: u64,
        #[serde(rename = "taskId")]
        task_id: String,
        nodes: Vec<ScannedNodeDto>,
    },
    FailedItem {
        #[serde(rename = "sessionId")]
        session_id: String,
        generation: u64,
        #[serde(rename = "taskId")]
        task_id: String,
        #[serde(rename = "relativePath")]
        relative_path: String,
        code: String,
    },
    Finished {
        #[serde(rename = "sessionId")]
        session_id: String,
        generation: u64,
        #[serde(rename = "taskId")]
        task_id: String,
        totals: ScanTotalsDto,
    },
}

impl ScanEventDto {
    pub fn from_event(active: &ActiveProject, task_id: TaskId, event: &ScanEvent) -> Self {
        let session_id = active.session_id.to_string();
        let generation = event.generation().get();
        let task_id = task_id.to_string();
        match event {
            ScanEvent::Folders { nodes, .. } => Self::Folders {
                session_id,
                generation,
                task_id,
                nodes: nodes.iter().map(ScannedNodeDto::from).collect(),
            },
            ScanEvent::Files { nodes, .. } => Self::Files {
                session_id,
                generation,
                task_id,
                nodes: nodes.iter().map(ScannedNodeDto::from).collect(),
            },
            ScanEvent::FailedItem {
                relative_display,
                code,
                ..
            } => Self::FailedItem {
                session_id,
                generation,
                task_id,
                relative_path: safe_relative_display(relative_display),
                code: code.clone(),
            },
            ScanEvent::Finished { totals, .. } => Self::Finished {
                session_id,
                generation,
                task_id,
                totals: (*totals).into(),
            },
        }
    }
}

fn safe_relative_display(value: &str) -> String {
    RelativePath::parse(value)
        .map(|path| path.as_str().to_owned())
        .unwrap_or_else(|_| "unavailable".to_owned())
}

#[cfg(test)]
mod tests {
    #[test]
    fn failed_scan_item_display_never_exposes_an_absolute_or_reserved_path() {
        assert_eq!(super::safe_relative_display("catalog/id-1"), "catalog/id-1");
        for unsafe_value in ["/Users/example/secret", "../outside", ".viewer/index"] {
            assert_eq!(super::safe_relative_display(unsafe_value), "unavailable");
        }
    }
}
