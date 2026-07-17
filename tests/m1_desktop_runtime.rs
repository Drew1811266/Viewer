use serde_json::json;
use std::{
    fs,
    path::Path,
    sync::{Arc, Mutex},
};
use viewer_application::{
    ProjectAccess, ProjectOpenError, ProjectProbeError, ProjectProbeOperation, ProjectProbePort,
};
use viewer_desktop::{
    dto::ProjectAccessDto,
    error::{CommandError, ErrorCategory},
    state::{DesktopEventSink, DesktopRuntime, ScanEventDto},
};
use viewer_infrastructure::scan::walker::ProjectWalker;

struct FixedProbe(ProjectAccess);

impl ProjectProbePort for FixedProbe {
    fn probe(&self, _root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
        Ok(self.0)
    }
}

#[derive(Default)]
struct RecordingEvents(Mutex<Vec<ScanEventDto>>);

impl RecordingEvents {
    fn snapshot(&self) -> Vec<ScanEventDto> {
        self.0.lock().unwrap().clone()
    }
}

impl DesktopEventSink for RecordingEvents {
    fn emit_scan(&self, event: ScanEventDto) {
        self.0.lock().unwrap().push(event);
    }
}

#[test]
fn command_error_is_safe_and_has_the_frozen_shape() {
    let secret = std::path::PathBuf::from("/Users/example/secret/project");
    let error = CommandError::from(ProjectOpenError::Probe(ProjectProbeError::NotDirectory {
        path: secret.clone(),
    }));

    let value = serde_json::to_value(&error).unwrap();
    let serialized = value.to_string();

    assert_eq!(error.category, ErrorCategory::Validation);
    assert_eq!(
        value,
        json!({
            "code": "invalid_project_root",
            "category": "validation",
            "userMessage": "请选择一个可读取的真实文件夹。",
            "retryable": true,
            "taskId": null,
            "itemId": null
        })
    );
    assert!(!serialized.contains(secret.to_str().unwrap()));
}

#[test]
fn environment_errors_do_not_expose_operating_system_details() {
    let secret = std::path::PathBuf::from("/Volumes/private/project");
    let error = CommandError::from(ProjectProbeError::Io {
        operation: ProjectProbeOperation::ReadDirectory,
        path: secret.clone(),
        message: "permission denied for secret user".to_owned(),
    });
    let serialized = serde_json::to_string(&error).unwrap();

    assert_eq!(error.code, "project_unreadable");
    assert_eq!(error.category, ErrorCategory::Environment);
    assert!(error.retryable);
    assert!(!serialized.contains(secret.to_str().unwrap()));
    assert!(!serialized.contains("secret user"));
}

#[tokio::test]
async fn runtime_allows_exactly_one_active_desktop_session() {
    let cache_base = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let runtime = DesktopRuntime::new(
        cache_base.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
    );

    let opened = runtime.open_project(project.path()).await.unwrap();

    assert_eq!(
        opened.display_name,
        project.path().file_name().unwrap().to_string_lossy()
    );
    assert_eq!(opened.access, ProjectAccessDto::ReadWrite);
    assert_eq!(runtime.snapshot().await, Some(opened.clone()));
    assert!(runtime.resources_ready().await);
    let second = runtime.open_project(project.path()).await.unwrap_err();
    assert_eq!(second.code, "project_already_open");
}

#[tokio::test]
async fn project_snapshot_contains_no_absolute_or_cache_path() {
    let cache_base = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let runtime = DesktopRuntime::new(
        cache_base.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadOnly)),
    );

    let opened = runtime.open_project(project.path()).await.unwrap();
    let serialized = serde_json::to_string(&opened).unwrap();

    assert_eq!(opened.access, ProjectAccessDto::ReadOnly);
    assert!(!serialized.contains(project.path().to_str().unwrap()));
    assert!(!serialized.contains(cache_base.path().to_str().unwrap()));
    assert_eq!(serde_json::to_value(opened).unwrap()["access"], "read_only");
}

#[tokio::test]
async fn project_scan_commits_progressive_batches_and_close_removes_cache() {
    let cache_base = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::create_dir_all(project.path().join("catalog/id-001")).unwrap();
    fs::write(project.path().join("catalog/id-001/front.jpg"), b"jpeg").unwrap();
    fs::write(project.path().join("catalog/id-001/prompt.md"), b"prompt").unwrap();
    let events = Arc::new(RecordingEvents::default());
    let runtime = DesktopRuntime::new_with_dependencies(
        cache_base.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        Arc::new(ProjectWalker),
        events.clone(),
    );

    runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();

    let published = events.snapshot();
    let first_nodes = published
        .iter()
        .position(|event| matches!(event, ScanEventDto::Folders { .. }))
        .unwrap();
    let first_files = published
        .iter()
        .position(|event| matches!(event, ScanEventDto::Files { .. }))
        .unwrap();
    assert!(first_nodes < first_files);
    assert!(matches!(
        published.last(),
        Some(ScanEventDto::Finished { .. })
    ));
    assert_eq!(fs::read_dir(cache_base.path()).unwrap().count(), 1);

    runtime.close_project().await.unwrap();

    assert_eq!(runtime.snapshot().await, None);
    assert_eq!(fs::read_dir(cache_base.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn read_only_project_scan_never_writes_viewer_metadata_into_the_project() {
    let cache_base = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("notes.txt"), b"notes").unwrap();
    let events = Arc::new(RecordingEvents::default());
    let runtime = DesktopRuntime::new_with_dependencies(
        cache_base.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadOnly)),
        Arc::new(ProjectWalker),
        events,
    );

    let opened = runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();

    assert_eq!(opened.access, ProjectAccessDto::ReadOnly);
    assert!(!project.path().join(".viewer").exists());
    runtime.close_project().await.unwrap();
}
