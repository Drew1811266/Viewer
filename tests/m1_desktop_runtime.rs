use serde_json::json;
use std::{
    fs,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use viewer_application::{
    ImageArtifact, ImageBackend, ImageError, ImagePort, ImageRequest, ProjectAccess,
    ProjectOpenError, ProjectProbeError, ProjectProbeOperation, ProjectProbePort, TextEncoding,
};
use viewer_desktop::{
    dto::{FolderWorkspaceDto, ProjectAccessDto},
    error::{CommandError, ErrorCategory},
    state::{DesktopEventSink, DesktopImageFactory, DesktopRuntime, ScanEventDto},
};
use viewer_domain::{
    EntityId, SessionId,
    image::{ImageFormat, ImageProbe, ImageRepresentationKind},
};
use viewer_infrastructure::image_cache::ImageArtifactRegistry;
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

struct CountingImageFactory {
    renders: Arc<AtomicUsize>,
}

impl DesktopImageFactory for CountingImageFactory {
    fn create(&self, cache_root: &Path) -> Result<Arc<dyn ImagePort>, ImageError> {
        Ok(Arc::new(CountingImagePort {
            cache_root: cache_root.to_path_buf(),
            renders: Arc::clone(&self.renders),
        }))
    }
}

struct CountingImagePort {
    cache_root: std::path::PathBuf,
    renders: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl ImagePort for CountingImagePort {
    async fn probe(&self, _source: &Path) -> Result<ImageProbe, ImageError> {
        Ok(ImageProbe {
            format: ImageFormat::Jpeg,
            width: 640,
            height: 480,
            orientation: 1,
            has_alpha: false,
            icc_profile_name: None,
        })
    }

    async fn render(&self, _request: ImageRequest) -> Result<ImageArtifact, ImageError> {
        let render = self.renders.fetch_add(1, Ordering::SeqCst) + 1;
        let cache_path = self.cache_root.join(format!("render-{render}.png"));
        fs::write(&cache_path, b"png artifact").unwrap();
        Ok(ImageArtifact {
            cache_path,
            mime: "image/png",
            width: 320,
            height: 240,
            backend: ImageBackend::ImageIo,
        })
    }

    async fn cancel_session(&self, _session_id: SessionId) {}
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

#[tokio::test]
async fn folder_query_and_image_representation_revalidate_and_reuse_cache() {
    let cache_base = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("front.jpg"), b"jpeg source").unwrap();
    fs::write(project.path().join("notes.txt"), b"notes").unwrap();
    let renders = Arc::new(AtomicUsize::new(0));
    let runtime = DesktopRuntime::new_with_image_factory(
        cache_base.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        Arc::new(ProjectWalker),
        Arc::new(RecordingEvents::default()),
        Arc::new(CountingImageFactory {
            renders: Arc::clone(&renders),
        }),
        Arc::new(ImageArtifactRegistry::default()),
    );
    runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();

    let FolderWorkspaceDto::Content { images, text_files } =
        runtime.query_folder(None).await.unwrap()
    else {
        panic!("root should be a content folder")
    };
    assert_eq!(images.len(), 1);
    assert_eq!(text_files.len(), 1);
    let entity_id = images[0].entity_id.parse::<EntityId>().unwrap();
    let kind = ImageRepresentationKind::Thumbnail {
        max_pixels: 320,
        scale_milli: 2_000,
    };

    let first = runtime.request_image(entity_id, kind).await.unwrap();
    let second = runtime.request_image(entity_id, kind).await.unwrap();

    assert_eq!(first.cache_key, second.cache_key);
    assert_ne!(first.url, second.url);
    assert_eq!(renders.load(Ordering::SeqCst), 1);
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn markdown_preview_strips_active_remote_and_escaping_resources() {
    let cache_base = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("front.jpg"), b"jpeg source").unwrap();
    fs::write(
        project.path().join("prompt.md"),
        br#"# Product

<script>alert('unsafe')</script>
<img src="https://remote.example/raw.png" onerror="unsafe()">

![local](front.jpg)
![remote](https://remote.example/image.png)
![escape](../../outside.png)
"#,
    )
    .unwrap();
    let runtime = DesktopRuntime::new_with_image_factory(
        cache_base.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        Arc::new(ProjectWalker),
        Arc::new(RecordingEvents::default()),
        Arc::new(CountingImageFactory {
            renders: Arc::new(AtomicUsize::new(0)),
        }),
        Arc::new(ImageArtifactRegistry::default()),
    );
    runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let FolderWorkspaceDto::Content { text_files, .. } = runtime.query_folder(None).await.unwrap()
    else {
        panic!("root should contain Markdown")
    };
    let markdown_id = text_files
        .iter()
        .find(|file| file.name == "prompt.md")
        .unwrap()
        .entity_id
        .parse::<EntityId>()
        .unwrap();

    let preview = runtime.preview_text(markdown_id, None).await.unwrap();
    let html = preview.markdown_html.unwrap();

    assert_eq!(preview.encoding, TextEncoding::Utf8);
    assert!(!html.contains("<script"));
    assert!(!html.contains("onerror"));
    assert!(!html.contains("remote.example"));
    assert!(!html.contains("../../outside.png"));
    assert!(html.contains("viewer-image://localhost/"));
    runtime.close_project().await.unwrap();
}
