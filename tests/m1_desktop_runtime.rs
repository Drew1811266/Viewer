use serde_json::json;
use std::{
    fs,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio_util::sync::CancellationToken;
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
    EntityId, ImageRequestId, SessionId,
    image::{ImageFormat, ImageProbe, ImageRepresentationKind},
    video::{VideoMetadata, VideoProbeStatus},
};
use viewer_infrastructure::image_cache::ImageArtifactRegistry;
use viewer_infrastructure::scan::walker::ProjectWalker;
use viewer_infrastructure::video_probe::{VideoMetadataProbe, VideoProbeError};

struct FixedProbe(ProjectAccess);

impl ProjectProbePort for FixedProbe {
    fn probe(&self, _root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
        Ok(self.0)
    }
}

struct BlockedVideoProbe {
    started: tokio::sync::Notify,
    release: tokio::sync::Notify,
    cancellation_observed: std::sync::atomic::AtomicBool,
}

impl BlockedVideoProbe {
    fn new() -> Self {
        Self {
            started: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
            cancellation_observed: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

#[async_trait::async_trait]
impl VideoMetadataProbe for BlockedVideoProbe {
    async fn probe(
        &self,
        _canonical_path: &Path,
        cancellation: CancellationToken,
    ) -> Result<VideoMetadata, VideoProbeError> {
        self.started.notify_one();
        tokio::select! {
            _ = cancellation.cancelled() => {
                self.cancellation_observed.store(true, Ordering::SeqCst);
                Err(VideoProbeError::Cancelled)
            },
            _ = self.release.notified() => Ok(VideoMetadata {
                duration_us: Some(2_000_000),
                display_width: Some(1920),
                display_height: Some(1080),
                rotation_degrees: 0,
                frame_rate_millihertz: Some(30_000),
                video_codec: Some("h264".into()),
                audio_codec: None,
                probe_status: VideoProbeStatus::Ready,
            }),
        }
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

struct CancellableImageFactory {
    started: Arc<tokio::sync::Notify>,
    cancellation_observed: Arc<std::sync::atomic::AtomicBool>,
    renders: Arc<AtomicUsize>,
}

impl DesktopImageFactory for CancellableImageFactory {
    fn create(&self, cache_root: &Path) -> Result<Arc<dyn ImagePort>, ImageError> {
        Ok(Arc::new(CancellableImagePort {
            cache_root: cache_root.to_path_buf(),
            started: Arc::clone(&self.started),
            cancellation_observed: Arc::clone(&self.cancellation_observed),
            renders: Arc::clone(&self.renders),
        }))
    }
}

struct CancellableImagePort {
    cache_root: std::path::PathBuf,
    started: Arc<tokio::sync::Notify>,
    cancellation_observed: Arc<std::sync::atomic::AtomicBool>,
    renders: Arc<AtomicUsize>,
}

struct ExternalArtifactFactory {
    started: Arc<tokio::sync::Notify>,
    artifact: std::path::PathBuf,
}

impl DesktopImageFactory for ExternalArtifactFactory {
    fn create(&self, _cache_root: &Path) -> Result<Arc<dyn ImagePort>, ImageError> {
        Ok(Arc::new(ExternalArtifactPort {
            started: Arc::clone(&self.started),
            artifact: self.artifact.clone(),
        }))
    }
}

struct ExternalArtifactPort {
    started: Arc<tokio::sync::Notify>,
    artifact: std::path::PathBuf,
}

#[async_trait::async_trait]
impl ImagePort for ExternalArtifactPort {
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

    async fn render(&self, request: ImageRequest) -> Result<ImageArtifact, ImageError> {
        self.started.notify_one();
        request.cancellation.cancelled().await;
        Ok(ImageArtifact {
            cache_path: self.artifact.clone(),
            mime: "image/png",
            width: 320,
            height: 240,
            backend: ImageBackend::ImageIo,
        })
    }

    async fn cancel_session(&self, _session_id: SessionId) {}
}

#[async_trait::async_trait]
impl ImagePort for CancellableImagePort {
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

    async fn render(&self, request: ImageRequest) -> Result<ImageArtifact, ImageError> {
        let render = self.renders.fetch_add(1, Ordering::SeqCst) + 1;
        if render == 1 {
            self.started.notify_one();
            request.cancellation.cancelled().await;
            self.cancellation_observed.store(true, Ordering::SeqCst);
        }
        let cache_path = self.cache_root.join(format!("cancel-render-{render}.png"));
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
async fn video_candidate_is_published_pending_before_probe_finishes() {
    let cache_base = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("clip.mp4"), b"candidate").unwrap();
    let probe = Arc::new(BlockedVideoProbe::new());
    let runtime = DesktopRuntime::new_with_video_probe(
        cache_base.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        Arc::new(ProjectWalker),
        Arc::new(RecordingEvents::default()),
        probe.clone(),
    );

    runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let FolderWorkspaceDto::Content {
        videos_deferred_until_task_9: videos,
        ..
    } = runtime.query_folder(None).await.unwrap()
    else {
        panic!("root should contain the video candidate")
    };
    assert_eq!(videos.len(), 1);
    assert_eq!(
        videos[0].video_metadata.as_ref().unwrap().probe_status,
        VideoProbeStatus::Pending
    );
    probe.started.notified().await;

    probe.release.notify_one();
    let ready = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let FolderWorkspaceDto::Content {
                videos_deferred_until_task_9: videos,
                ..
            } = runtime.query_folder(None).await.unwrap()
            else {
                panic!("root should remain a content workspace")
            };
            if videos[0]
                .video_metadata
                .as_ref()
                .is_some_and(|metadata| metadata.duration_us == Some(2_000_000))
            {
                break videos;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("released video metadata should be published");
    assert_eq!(
        ready[0].video_metadata.as_ref().unwrap().probe_status,
        VideoProbeStatus::Ready
    );
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn closing_project_cancels_and_awaits_a_blocked_video_probe() {
    let cache_base = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("clip.mp4"), b"candidate").unwrap();
    let probe = Arc::new(BlockedVideoProbe::new());
    let runtime = DesktopRuntime::new_with_video_probe(
        cache_base.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        Arc::new(ProjectWalker),
        Arc::new(RecordingEvents::default()),
        probe.clone(),
    );
    runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    probe.started.notified().await;

    tokio::time::timeout(std::time::Duration::from_secs(5), runtime.close_project())
        .await
        .expect("close must cancel and await the video probe")
        .unwrap();

    assert!(probe.cancellation_observed.load(Ordering::SeqCst));
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

    let workspace = runtime.query_folder(None).await.unwrap();
    let serialized = serde_json::to_value(&workspace).unwrap();
    assert_eq!(serialized["workspace"], json!("content"));
    assert_eq!(serialized["otherFiles"][0]["kind"], json!("text"));
    assert!(serialized.get("textFiles").is_none());
    let FolderWorkspaceDto::Content {
        images,
        other_files,
        ..
    } = workspace
    else {
        panic!("root should be a content folder")
    };
    assert_eq!(images.len(), 1);
    assert_eq!(other_files.len(), 1);
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
async fn cancelling_an_inflight_image_request_prevents_cache_and_token_publication() {
    let cache_base = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("front.jpg"), b"jpeg source").unwrap();
    let started = Arc::new(tokio::sync::Notify::new());
    let cancellation_observed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let renders = Arc::new(AtomicUsize::new(0));
    let runtime = Arc::new(DesktopRuntime::new_with_image_factory(
        cache_base.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        Arc::new(ProjectWalker),
        Arc::new(RecordingEvents::default()),
        Arc::new(CancellableImageFactory {
            started: Arc::clone(&started),
            cancellation_observed: Arc::clone(&cancellation_observed),
            renders: Arc::clone(&renders),
        }),
        Arc::new(ImageArtifactRegistry::default()),
    ));
    runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let FolderWorkspaceDto::Content { images, .. } = runtime.query_folder(None).await.unwrap()
    else {
        panic!("root should contain the image")
    };
    let entity_id = images[0].entity_id.parse::<EntityId>().unwrap();
    let request_id = ImageRequestId::new();
    let kind = ImageRepresentationKind::Thumbnail {
        max_pixels: 320,
        scale_milli: 2_000,
    };
    let request = {
        let runtime = Arc::clone(&runtime);
        tokio::spawn(async move {
            runtime
                .request_image_with_id(request_id, entity_id, kind)
                .await
        })
    };
    started.notified().await;

    assert!(runtime.cancel_image_request(request_id).await);
    let error = request.await.unwrap().unwrap_err();

    assert_eq!(error.code, "image_request_cancelled");
    assert!(cancellation_observed.load(Ordering::SeqCst));
    assert!(!runtime.cancel_image_request(request_id).await);
    runtime
        .request_image_with_id(ImageRequestId::new(), entity_id, kind)
        .await
        .unwrap();
    assert_eq!(
        renders.load(Ordering::SeqCst),
        2,
        "cancelled render must not be reused from cache"
    );
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn cancelling_an_external_image_artifact_never_deletes_the_external_file() {
    let cache_base = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let artifact = outside.path().join("must-survive.png");
    fs::write(&artifact, b"external bytes").unwrap();
    fs::write(project.path().join("front.jpg"), b"jpeg source").unwrap();
    let started = Arc::new(tokio::sync::Notify::new());
    let runtime = Arc::new(DesktopRuntime::new_with_image_factory(
        cache_base.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        Arc::new(ProjectWalker),
        Arc::new(RecordingEvents::default()),
        Arc::new(ExternalArtifactFactory {
            started: Arc::clone(&started),
            artifact: artifact.clone(),
        }),
        Arc::new(ImageArtifactRegistry::default()),
    ));
    runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let FolderWorkspaceDto::Content { images, .. } = runtime.query_folder(None).await.unwrap()
    else {
        panic!("root should contain the image")
    };
    let entity_id = images[0].entity_id.parse::<EntityId>().unwrap();
    let request_id = ImageRequestId::new();
    let request = {
        let runtime = Arc::clone(&runtime);
        tokio::spawn(async move {
            runtime
                .request_image_with_id(
                    request_id,
                    entity_id,
                    ImageRepresentationKind::Original100Percent,
                )
                .await
        })
    };
    started.notified().await;

    assert!(runtime.cancel_image_request(request_id).await);
    let error = request.await.unwrap().unwrap_err();

    assert_eq!(error.code, "image_request_cancelled");
    assert_eq!(fs::read(&artifact).unwrap(), b"external bytes");
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn duplicate_image_request_id_is_rejected_without_replacing_the_live_cancellation() {
    let cache_base = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("front.jpg"), b"jpeg source").unwrap();
    let started = Arc::new(tokio::sync::Notify::new());
    let runtime = Arc::new(DesktopRuntime::new_with_image_factory(
        cache_base.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        Arc::new(ProjectWalker),
        Arc::new(RecordingEvents::default()),
        Arc::new(CancellableImageFactory {
            started: Arc::clone(&started),
            cancellation_observed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            renders: Arc::new(AtomicUsize::new(0)),
        }),
        Arc::new(ImageArtifactRegistry::default()),
    ));
    runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let FolderWorkspaceDto::Content { images, .. } = runtime.query_folder(None).await.unwrap()
    else {
        panic!("root should contain the image")
    };
    let entity_id = images[0].entity_id.parse::<EntityId>().unwrap();
    let request_id = ImageRequestId::new();
    let kind = ImageRepresentationKind::Original100Percent;
    let live = {
        let runtime = Arc::clone(&runtime);
        tokio::spawn(async move {
            runtime
                .request_image_with_id(request_id, entity_id, kind)
                .await
        })
    };
    started.notified().await;

    let duplicate = runtime
        .request_image_with_id(request_id, entity_id, kind)
        .await
        .unwrap_err();
    assert_eq!(duplicate.code, "duplicate_image_request");
    assert!(runtime.cancel_image_request(request_id).await);
    let live_result = tokio::time::timeout(std::time::Duration::from_millis(100), live)
        .await
        .expect("original request should retain the registered cancellation")
        .unwrap()
        .unwrap_err();
    assert_eq!(live_result.code, "image_request_cancelled");
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn cancellation_before_request_registration_prevents_rendering() {
    let cache_base = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("front.jpg"), b"jpeg source").unwrap();
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
    let FolderWorkspaceDto::Content { images, .. } = runtime.query_folder(None).await.unwrap()
    else {
        panic!("root should contain the image")
    };
    let entity_id = images[0].entity_id.parse::<EntityId>().unwrap();
    let request_id = ImageRequestId::new();

    assert!(runtime.cancel_image_request(request_id).await);
    let error = runtime
        .request_image_with_id(
            request_id,
            entity_id,
            ImageRepresentationKind::Original100Percent,
        )
        .await
        .unwrap_err();

    assert_eq!(error.code, "image_request_cancelled");
    assert_eq!(renders.load(Ordering::SeqCst), 0);
    assert!(!runtime.cancel_image_request(request_id).await);
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn aborting_the_request_future_cleans_its_active_lifecycle_entry() {
    let cache_base = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("front.jpg"), b"jpeg source").unwrap();
    let started = Arc::new(tokio::sync::Notify::new());
    let runtime = Arc::new(DesktopRuntime::new_with_image_factory(
        cache_base.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        Arc::new(ProjectWalker),
        Arc::new(RecordingEvents::default()),
        Arc::new(CancellableImageFactory {
            started: Arc::clone(&started),
            cancellation_observed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            renders: Arc::new(AtomicUsize::new(0)),
        }),
        Arc::new(ImageArtifactRegistry::default()),
    ));
    runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let FolderWorkspaceDto::Content { images, .. } = runtime.query_folder(None).await.unwrap()
    else {
        panic!("root should contain the image")
    };
    let entity_id = images[0].entity_id.parse::<EntityId>().unwrap();
    let request_id = ImageRequestId::new();
    let request = {
        let runtime = Arc::clone(&runtime);
        tokio::spawn(async move {
            runtime
                .request_image_with_id(
                    request_id,
                    entity_id,
                    ImageRepresentationKind::Original100Percent,
                )
                .await
        })
    };
    started.notified().await;

    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());

    assert!(
        !runtime.cancel_image_request(request_id).await,
        "dropped future must not leave an active request"
    );
    let retry = runtime
        .request_image_with_id(
            request_id,
            entity_id,
            ImageRepresentationKind::Original100Percent,
        )
        .await
        .unwrap_err();
    assert_eq!(retry.code, "duplicate_image_request");
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
    let FolderWorkspaceDto::Content { other_files, .. } = runtime.query_folder(None).await.unwrap()
    else {
        panic!("root should contain Markdown")
    };
    let markdown_id = other_files
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

#[test]
fn desktop_facades_remain_available_at_the_frozen_paths() {
    fn assert_runtime(_: Option<&viewer_desktop::state::DesktopRuntime>) {}
    fn assert_operation(_: Option<&viewer_desktop::operation_runtime::OperationRuntime>) {}
    fn assert_snapshot(_: Option<viewer_desktop::dto::ProjectSnapshot>) {}

    assert_runtime(None);
    assert_operation(None);
    assert_snapshot(None);
}
