use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use tokio::sync::Notify;
use viewer_application::{
    ImageArtifact, ImageBackend, ImageError, ImagePort, ImageRequest, ProjectAccess,
    ProjectProbeError, ProjectProbePort,
};
use viewer_desktop::{
    dto::IndexProgressDto,
    state::{DesktopEventSink, DesktopImageFactory, DesktopRuntime, ScanEventDto},
};
use viewer_domain::{
    SessionId,
    image::{ImageFormat, ImageProbe},
};
use viewer_infrastructure::{image_cache::ImageArtifactRegistry, scan::walker::ProjectWalker};

struct FixedProbe(ProjectAccess);

impl ProjectProbePort for FixedProbe {
    fn probe(&self, _root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
        Ok(self.0)
    }
}

#[derive(Default)]
struct RecordingEvents {
    scan: Mutex<Vec<ScanEventDto>>,
    index: Mutex<Vec<IndexProgressDto>>,
}

impl RecordingEvents {
    fn index_events(&self) -> Vec<IndexProgressDto> {
        self.index.lock().unwrap().clone()
    }
}

impl DesktopEventSink for RecordingEvents {
    fn emit_scan(&self, event: ScanEventDto) {
        self.scan.lock().unwrap().push(event);
    }

    fn emit_index(&self, event: IndexProgressDto) {
        self.index.lock().unwrap().push(event);
    }
}

struct ProbeFactory {
    probes: Arc<AtomicUsize>,
    renders: Arc<AtomicUsize>,
}

impl DesktopImageFactory for ProbeFactory {
    fn create(&self, cache_root: &Path) -> Result<Arc<dyn ImagePort>, ImageError> {
        Ok(Arc::new(ProbeImagePort {
            cache_root: cache_root.to_owned(),
            probes: Arc::clone(&self.probes),
            renders: Arc::clone(&self.renders),
        }))
    }
}

struct ProbeImagePort {
    cache_root: PathBuf,
    probes: Arc<AtomicUsize>,
    renders: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl ImagePort for ProbeImagePort {
    async fn probe(&self, source: &Path) -> Result<ImageProbe, ImageError> {
        self.probes.fetch_add(1, Ordering::SeqCst);
        if source.file_name().is_some_and(|name| name == "bad.jpg") {
            return Err(ImageError::Corrupt);
        }
        Ok(ImageProbe {
            format: ImageFormat::Jpeg,
            width: 1_200,
            height: 800,
            orientation: 1,
            has_alpha: false,
            icc_profile_name: None,
        })
    }

    async fn render(&self, _request: ImageRequest) -> Result<ImageArtifact, ImageError> {
        self.renders.fetch_add(1, Ordering::SeqCst);
        let path = self.cache_root.join("unexpected-render.png");
        fs::write(&path, b"unexpected").unwrap();
        Ok(ImageArtifact {
            cache_path: path,
            mime: "image/png",
            width: 1,
            height: 1,
            backend: ImageBackend::ImageIo,
        })
    }

    async fn cancel_session(&self, _session_id: SessionId) {}
}

#[tokio::test]
async fn derived_indexing_probes_headers_without_render_and_publishes_safe_progress() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("good.jpg"), b"fixture").unwrap();
    fs::write(project.path().join("bad.jpg"), b"fixture").unwrap();
    fs::write(project.path().join("notes.txt"), "产品说明").unwrap();
    let probes = Arc::new(AtomicUsize::new(0));
    let renders = Arc::new(AtomicUsize::new(0));
    let events = Arc::new(RecordingEvents::default());
    let runtime = DesktopRuntime::new_with_image_factory(
        cache.path().to_owned(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        Arc::new(ProjectWalker),
        events.clone(),
        Arc::new(ProbeFactory {
            probes: Arc::clone(&probes),
            renders: Arc::clone(&renders),
        }),
        Arc::new(ImageArtifactRegistry::default()),
    );

    let project_snapshot = runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let progress = runtime.index_progress().await.unwrap();

    assert_eq!(probes.load(Ordering::SeqCst), 2);
    assert_eq!(renders.load(Ordering::SeqCst), 0);
    assert_eq!(
        (
            progress.images_total,
            progress.images_ready,
            progress.images_failed
        ),
        (2, 1, 1)
    );
    assert_eq!((progress.text_total, progress.text_ready), (1, 1));
    assert!(progress.complete);
    let published = events.index_events();
    assert!(published.len() >= 2);
    assert_eq!(published.last(), Some(&progress));
    let serialized = serde_json::to_string(&published).unwrap();
    assert!(serialized.contains(&project_snapshot.session_id));
    assert!(!serialized.contains(project.path().to_str().unwrap()));
    assert!(!serialized.contains("产品说明"));
    runtime.close_project().await.unwrap();
}

struct BlockingFactory {
    started: Arc<Notify>,
    cancelled: Arc<AtomicBool>,
}

impl DesktopImageFactory for BlockingFactory {
    fn create(&self, _cache_root: &Path) -> Result<Arc<dyn ImagePort>, ImageError> {
        Ok(Arc::new(BlockingImagePort {
            started: Arc::clone(&self.started),
            cancelled: Arc::clone(&self.cancelled),
        }))
    }
}

struct BlockingImagePort {
    started: Arc<Notify>,
    cancelled: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl ImagePort for BlockingImagePort {
    async fn probe(&self, _source: &Path) -> Result<ImageProbe, ImageError> {
        self.started.notify_one();
        std::future::pending().await
    }

    async fn render(&self, _request: ImageRequest) -> Result<ImageArtifact, ImageError> {
        unreachable!("derived indexing must not render")
    }

    async fn cancel_session(&self, _session_id: SessionId) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn closing_project_cancels_inflight_derived_probe_and_removes_session_cache() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("front.jpg"), b"fixture").unwrap();
    let started = Arc::new(Notify::new());
    let cancelled = Arc::new(AtomicBool::new(false));
    let events = Arc::new(RecordingEvents::default());
    let runtime = DesktopRuntime::new_with_image_factory(
        cache.path().to_owned(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        Arc::new(ProjectWalker),
        events.clone(),
        Arc::new(BlockingFactory {
            started: Arc::clone(&started),
            cancelled: Arc::clone(&cancelled),
        }),
        Arc::new(ImageArtifactRegistry::default()),
    );

    runtime.open_project(project.path()).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), started.notified())
        .await
        .expect("derived probe started");
    runtime.close_project().await.unwrap();
    let event_count = events.index_events().len();
    tokio::time::sleep(std::time::Duration::from_millis(25)).await;

    assert!(cancelled.load(Ordering::SeqCst));
    assert_eq!(fs::read_dir(cache.path()).unwrap().count(), 0);
    assert_eq!(events.index_events().len(), event_count);
}
