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
    metadata::{MarkerProjectionError, MarkerProjectionPort},
};
use viewer_desktop::{
    dto::{FolderWorkspaceDto, IndexProgressDto, SelectionAgreementDto},
    state::{
        DesktopEventSink, DesktopImageFactory, DesktopMarkerProjectionFactory, DesktopRuntime,
        ScanEventDto,
    },
};
use viewer_domain::{
    EntityId, SessionId,
    file::ReviewState,
    image::{ImageFormat, ImageProbe},
    search::{
        Generation, SearchFilters, SearchLayout, SearchQuery, SearchScope, SearchSort,
        SearchSortKey, SortDirection,
    },
};
use viewer_infrastructure::{
    image_cache::ImageArtifactRegistry, scan::walker::ProjectWalker, search::index::SessionIndex,
};

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

fn review_runtime(cache: &Path, access: ProjectAccess) -> DesktopRuntime {
    DesktopRuntime::new_with_image_factory(
        cache.to_owned(),
        Arc::new(FixedProbe(access)),
        Arc::new(ProjectWalker),
        Arc::new(RecordingEvents::default()),
        Arc::new(ProbeFactory {
            probes: Arc::new(AtomicUsize::new(0)),
            renders: Arc::new(AtomicUsize::new(0)),
        }),
        Arc::new(ImageArtifactRegistry::default()),
    )
}

fn default_query(text: &str, limit: u32) -> SearchQuery {
    SearchQuery {
        text: text.to_owned(),
        scope: SearchScope::Project,
        filters: SearchFilters::default(),
        sort: SearchSort {
            key: SearchSortKey::NaturalName,
            direction: SortDirection::Ascending,
        },
        layout: SearchLayout::Flat,
        offset: 0,
        limit,
    }
}

fn parse_id(value: &str) -> EntityId {
    value.parse().unwrap()
}

async fn indexed_fixture(
    runtime: &DesktopRuntime,
    project: &Path,
) -> (
    viewer_desktop::dto::ProjectSnapshot,
    EntityId,
    EntityId,
    EntityId,
) {
    let snapshot = runtime.open_project(project).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let folder = runtime
        .folder_tree()
        .await
        .unwrap()
        .into_iter()
        .find(|folder| folder.relative_path == "id2")
        .unwrap();
    let FolderWorkspaceDto::Content { images, text_files } = runtime
        .query_folder(Some(parse_id(&folder.entity_id)))
        .await
        .unwrap()
    else {
        panic!("fixture folder should contain supported files")
    };
    (
        snapshot,
        parse_id(&folder.entity_id),
        parse_id(&images[0].entity_id),
        parse_id(&text_files[0].entity_id),
    )
}

fn create_review_project(root: &Path) {
    fs::create_dir(root.join("id2")).unwrap();
    fs::write(root.join("id2/front.jpg"), b"fixture").unwrap();
    fs::write(
        root.join("id2/prompt.txt"),
        "产品说明：蓝色运动鞋。".repeat(20),
    )
    .unwrap();
}

#[tokio::test]
async fn marker_commands_support_mixed_targets_and_survive_project_copy_with_stable_identity() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    create_review_project(project.path());
    let runtime = review_runtime(cache.path(), ProjectAccess::ReadWrite);
    let (snapshot, folder, image, text) = indexed_fixture(&runtime, project.path()).await;
    let session: SessionId = snapshot.session_id.parse().unwrap();
    let generation = Generation::new(snapshot.generation);

    let review = runtime
        .set_review_state(
            session,
            generation,
            &[folder, image, text],
            Some(ReviewState::Keep),
        )
        .await
        .unwrap();
    assert_eq!(review.changes.len(), 3);
    let favorite = runtime
        .toggle_favorite(session, generation, &[folder, image])
        .await
        .unwrap();
    assert_eq!(favorite.changes.len(), 2);
    let selection = runtime
        .selection_info(session, generation, &[folder, image, text])
        .await
        .unwrap();
    assert_eq!(selection.common_review.state_name(), "common");
    assert_eq!(selection.common_favorite.state_name(), "mixed");
    let serialized = serde_json::to_string(&selection).unwrap();
    assert!(!serialized.contains(project.path().to_str().unwrap()));
    assert!(!serialized.contains("metadata.sqlite"));

    runtime.close_project().await.unwrap();
    let copied_parent = tempfile::tempdir().unwrap();
    let copied = copied_parent.path().join("copied-project");
    copy_directory(project.path(), &copied);
    let copied_snapshot = runtime.open_project(&copied).await.unwrap();
    assert_eq!(copied_snapshot.project_id, snapshot.project_id);
    runtime.wait_for_scan().await.unwrap();
    let copied_folder = runtime
        .folder_tree()
        .await
        .unwrap()
        .into_iter()
        .find(|folder| folder.relative_path == "id2")
        .unwrap();
    assert_eq!(copied_folder.marker.review_state, Some(ReviewState::Keep));
    assert!(copied_folder.marker.favorite);
    let FolderWorkspaceDto::Content { images, text_files } = runtime
        .query_folder(Some(parse_id(&copied_folder.entity_id)))
        .await
        .unwrap()
    else {
        panic!("copied fixture should remain content")
    };
    assert_eq!(images[0].marker.review_state, Some(ReviewState::Keep));
    assert!(images[0].marker.favorite);
    assert_eq!(text_files[0].marker.review_state, Some(ReviewState::Keep));
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn marker_undo_is_lifo_and_close_reopen_starts_with_an_empty_session_stack() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    create_review_project(project.path());
    let runtime = review_runtime(cache.path(), ProjectAccess::ReadWrite);
    let (snapshot, _, image, _) = indexed_fixture(&runtime, project.path()).await;
    let session: SessionId = snapshot.session_id.parse().unwrap();
    let generation = Generation::new(snapshot.generation);

    runtime
        .set_review_state(session, generation, &[image], Some(ReviewState::Keep))
        .await
        .unwrap();
    runtime
        .toggle_favorite(session, generation, &[image])
        .await
        .unwrap();
    let favorite_undo = runtime
        .undo_last(session, generation)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        favorite_undo.kind,
        viewer_domain::operation::OperationKind::SetFavorite
    );
    let after_favorite = runtime
        .selection_info(session, generation, &[image])
        .await
        .unwrap();
    assert_eq!(after_favorite.common_review.state_name(), "common");
    assert_eq!(after_favorite.common_favorite.state_name(), "common");
    assert_eq!(
        after_favorite.common_favorite,
        SelectionAgreementDto::Common(false)
    );

    let review_undo = runtime
        .undo_last(session, generation)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        review_undo.kind,
        viewer_domain::operation::OperationKind::SetReviewState
    );
    let after_review = runtime
        .selection_info(session, generation, &[image])
        .await
        .unwrap();
    assert_eq!(
        after_review.common_review,
        SelectionAgreementDto::Common(None)
    );

    runtime.close_project().await.unwrap();
    let reopened = runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let reopened_session: SessionId = reopened.session_id.parse().unwrap();
    let reopened_generation = Generation::new(reopened.generation);
    assert_eq!(
        runtime
            .undo_last(reopened_session, reopened_generation)
            .await
            .unwrap(),
        None
    );
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn search_commands_enforce_session_generation_revision_page_and_snippet_bounds() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    create_review_project(project.path());
    let runtime = review_runtime(cache.path(), ProjectAccess::ReadWrite);
    let (snapshot, _, _, text) = indexed_fixture(&runtime, project.path()).await;
    let session: SessionId = snapshot.session_id.parse().unwrap();
    let generation = Generation::new(snapshot.generation);

    let page = runtime
        .search_project(session, generation, 1, default_query("产品说明", 200))
        .await
        .unwrap();
    assert_eq!(page.revision, 1);
    assert_eq!(page.hits.len(), 1);
    let page_json = serde_json::to_value(&page).unwrap();
    assert_eq!(page_json["revision"], 1);
    assert!(page_json["hits"][0].get("body").is_none());
    assert!(page_json["hits"][0].get("relativePath").is_some());
    assert!(
        !page_json
            .to_string()
            .contains(project.path().to_str().unwrap())
    );

    let snippet = runtime
        .search_text_snippet(session, generation, 1, text, "产品说明".to_owned())
        .await
        .unwrap();
    assert_eq!(snippet.revision, 1);
    assert!(snippet.snippet.as_ref().unwrap().chars().count() <= 160);

    runtime
        .search_project(session, generation, 2, default_query("", 200))
        .await
        .unwrap();
    let stale = runtime
        .search_project(session, generation, 1, default_query("", 200))
        .await
        .unwrap_err();
    assert_eq!(stale.code, "stale_search_revision");
    let stale_snippet = runtime
        .search_text_snippet(session, generation, 1, text, "产品说明".to_owned())
        .await
        .unwrap_err();
    assert_eq!(stale_snippet.code, "stale_search_revision");
    let wrong_session = runtime
        .search_project(SessionId::new(), generation, 3, default_query("", 200))
        .await
        .unwrap_err();
    assert_eq!(wrong_session.code, "stale_project_session");
    let wrong_generation = runtime
        .search_project(
            session,
            Generation::new(snapshot.generation + 1),
            3,
            default_query("", 200),
        )
        .await
        .unwrap_err();
    assert_eq!(wrong_generation.code, "stale_project_session");
    let oversized = runtime
        .search_project(session, generation, 3, default_query("", 201))
        .await
        .unwrap_err();
    assert_eq!(oversized.code, "invalid_search_query");
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn read_only_and_unknown_marker_targets_are_rejected_without_project_writes_or_details() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    create_review_project(project.path());
    let runtime = review_runtime(cache.path(), ProjectAccess::ReadOnly);
    let (snapshot, _, image, _) = indexed_fixture(&runtime, project.path()).await;
    let session: SessionId = snapshot.session_id.parse().unwrap();
    let generation = Generation::new(snapshot.generation);

    let read_only = runtime
        .set_review_state(session, generation, &[image], Some(ReviewState::Reject))
        .await
        .unwrap_err();
    assert_eq!(read_only.code, "project_read_only");
    assert!(!project.path().join(".viewer").exists());
    runtime.close_project().await.unwrap();

    let writable = review_runtime(cache.path(), ProjectAccess::ReadWrite);
    let (snapshot, _, _, _) = indexed_fixture(&writable, project.path()).await;
    let unknown = writable
        .toggle_favorite(
            snapshot.session_id.parse().unwrap(),
            Generation::new(snapshot.generation),
            &[EntityId::new()],
        )
        .await
        .unwrap_err();
    assert_eq!(unknown.code, "selection_not_found");
    let error_json = serde_json::to_string(&unknown).unwrap();
    assert!(!error_json.contains(project.path().to_str().unwrap()));
    assert!(!error_json.to_lowercase().contains("sqlite"));
    writable.close_project().await.unwrap();
}

struct FailingProjectionFactory;

impl DesktopMarkerProjectionFactory for FailingProjectionFactory {
    fn create(&self, _index: Arc<SessionIndex>) -> Arc<dyn MarkerProjectionPort> {
        Arc::new(FailingProjection)
    }
}

struct FailingProjection;

impl MarkerProjectionPort for FailingProjection {
    fn sync_markers(
        &self,
        _changes: &[viewer_application::metadata::MarkerChange],
    ) -> Result<(), MarkerProjectionError> {
        Err(MarkerProjectionError::Unavailable)
    }
}

#[tokio::test]
async fn committed_but_stale_marker_projection_recovers_from_portable_truth_on_reopen() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    create_review_project(project.path());
    let runtime = DesktopRuntime::new_with_marker_projection_factory(
        cache.path().to_owned(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        Arc::new(ProjectWalker),
        Arc::new(RecordingEvents::default()),
        Arc::new(ProbeFactory {
            probes: Arc::new(AtomicUsize::new(0)),
            renders: Arc::new(AtomicUsize::new(0)),
        }),
        Arc::new(ImageArtifactRegistry::default()),
        Arc::new(FailingProjectionFactory),
    );
    let (snapshot, _, image, _) = indexed_fixture(&runtime, project.path()).await;
    let error = runtime
        .set_review_state(
            snapshot.session_id.parse().unwrap(),
            Generation::new(snapshot.generation),
            &[image],
            Some(ReviewState::Reject),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "marker_projection_stale");
    assert!(error.retryable);
    runtime.close_project().await.unwrap();

    let reopened = review_runtime(cache.path(), ProjectAccess::ReadWrite);
    let (_, _, copied_image, _) = indexed_fixture(&reopened, project.path()).await;
    let selected = reopened
        .selection_info(
            reopened
                .snapshot()
                .await
                .unwrap()
                .session_id
                .parse()
                .unwrap(),
            Generation::new(reopened.snapshot().await.unwrap().generation),
            &[copied_image],
        )
        .await
        .unwrap();
    assert_eq!(selected.common_review.state_name(), "common");
    assert_eq!(
        selected.common_review.review_value(),
        Some(ReviewState::Reject)
    );
    reopened.close_project().await.unwrap();
}

fn copy_directory(source: &Path, destination: &Path) {
    fs::create_dir(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_directory(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}
