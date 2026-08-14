use std::{
    fs,
    path::Path,
    str::FromStr,
    sync::{Arc, Mutex},
};

use viewer_application::{
    EngineOpenRequest, FrameDirection, PlaybackRate, ProjectAccess, ProjectProbeError,
    ProjectProbePort, SeekIntent, SeekRequest, SurfaceRect, VideoEngine, VideoEngineError,
};
use viewer_desktop::state::{DesktopRuntime, VideoClosePort};
use viewer_desktop::{
    dto::{VideoEventDto, VideoMediaDto},
    video_events::ProgressCoalescer,
};
use viewer_domain::{EntityId, SessionId};
use viewer_infrastructure::image_cache::{ImageArtifactLookup, ImageArtifactRegistry};
use viewer_test_support::video_engine::{FakeVideoEngine, FakeVideoEngineCall};

#[derive(Default)]
struct RecordingVideoEvents(Mutex<Vec<VideoEventDto>>);

impl viewer_desktop::video_runtime::VideoEventPort for RecordingVideoEvents {
    fn publish(&self, event: VideoEventDto) {
        self.0.lock().unwrap().push(event);
    }
}

struct BlockingThumbnailPort {
    first_started: tokio::sync::Notify,
    first_cancelled: std::sync::atomic::AtomicBool,
    revoked: Mutex<Vec<SessionId>>,
}

#[derive(Default)]
struct RecordingPlaybackActivity(Mutex<Vec<(u64, bool)>>);

struct BlockingTransportEngine {
    inner: FakeVideoEngine,
    pause_started: tokio::sync::Notify,
    pause_release: tokio::sync::Notify,
}

impl Default for BlockingTransportEngine {
    fn default() -> Self {
        Self {
            inner: FakeVideoEngine::default(),
            pause_started: tokio::sync::Notify::new(),
            pause_release: tokio::sync::Notify::new(),
        }
    }
}

#[async_trait::async_trait]
impl VideoEngine for BlockingTransportEngine {
    async fn open_paused(&self, request: EngineOpenRequest) -> Result<(), VideoEngineError> {
        self.inner.open_paused(request).await
    }

    async fn reveal_surface(&self, generation: u64) -> Result<(), VideoEngineError> {
        self.inner.reveal_surface(generation).await
    }

    async fn close(&self, generation: u64) -> Result<(), VideoEngineError> {
        self.inner.close(generation).await
    }

    async fn play(&self, generation: u64) -> Result<(), VideoEngineError> {
        self.inner.play(generation).await
    }

    async fn pause(&self, generation: u64) -> Result<(), VideoEngineError> {
        self.pause_started.notify_one();
        self.pause_release.notified().await;
        self.inner.pause(generation).await
    }

    fn publish_seek(&self, generation: u64, request: SeekRequest) -> Result<(), VideoEngineError> {
        self.inner.publish_seek(generation, request)
    }

    async fn step(
        &self,
        generation: u64,
        direction: FrameDirection,
    ) -> Result<(), VideoEngineError> {
        self.inner.step(generation, direction).await
    }

    async fn set_volume(&self, generation: u64, percent: u8) -> Result<(), VideoEngineError> {
        self.inner.set_volume(generation, percent).await
    }

    async fn set_muted(&self, generation: u64, muted: bool) -> Result<(), VideoEngineError> {
        self.inner.set_muted(generation, muted).await
    }

    async fn set_rate(&self, generation: u64, rate: PlaybackRate) -> Result<(), VideoEngineError> {
        self.inner.set_rate(generation, rate).await
    }

    fn publish_surface_rect(
        &self,
        generation: u64,
        sequence: u64,
        rect: SurfaceRect,
    ) -> Result<(), VideoEngineError> {
        self.inner.publish_surface_rect(generation, sequence, rect)
    }
}

impl viewer_desktop::video_runtime::PlaybackActivityPort for RecordingPlaybackActivity {
    fn set_active(&self, generation: u64, active: bool) {
        self.0.lock().unwrap().push((generation, active));
    }
}

#[tokio::test]
async fn high_frequency_publication_bypasses_the_transition_lane() {
    let engine = Arc::new(BlockingTransportEngine::default());
    let runtime = Arc::new(viewer_desktop::video_runtime::VideoRuntime::new(
        Arc::clone(&engine),
    ));
    let generation = runtime
        .open_authorized(video_source(7))
        .await
        .unwrap()
        .generation;
    runtime
        .handle_engine_event(generation, viewer_application::EngineEvent::FirstFrameReady)
        .await;

    let pausing_runtime = Arc::clone(&runtime);
    let pause = tokio::spawn(async move { pausing_runtime.pause(generation).await });
    engine.pause_started.notified().await;

    tokio::time::timeout(
        std::time::Duration::from_millis(50),
        runtime.seek(
            generation,
            SeekRequest {
                request_id: 40,
                time_us: 500_000,
                intent: SeekIntent::Preview,
            },
        ),
    )
    .await
    .expect("preview publication must not wait for the transition lane")
    .unwrap();
    runtime
        .set_surface_rect(
            generation,
            41,
            SurfaceRect {
                x: 5,
                y: 6,
                width: 640,
                height: 360,
            },
        )
        .expect("geometry publication must not wait for the transition lane");

    assert!(
        engine
            .inner
            .calls()
            .contains(&FakeVideoEngineCall::PublishSeek(
                generation,
                SeekRequest {
                    request_id: 40,
                    time_us: 500_000,
                    intent: SeekIntent::Preview,
                },
            ))
    );
    assert!(
        engine
            .inner
            .calls()
            .contains(&FakeVideoEngineCall::PublishSurfaceRect(
                generation,
                41,
                SurfaceRect {
                    x: 5,
                    y: 6,
                    width: 640,
                    height: 360,
                },
            ))
    );

    engine.pause_release.notify_one();
    pause.await.unwrap().unwrap();
}

#[async_trait::async_trait]
impl viewer_desktop::video_runtime::TimelineThumbnailPort for BlockingThumbnailPort {
    async fn request_cover(
        &self,
        _request: viewer_desktop::video_runtime::CoverThumbnailBridgeRequest,
    ) -> Result<String, viewer_desktop::video_runtime::VideoCommandError> {
        Ok("viewer-image://localhost/session/cover".into())
    }

    async fn request(
        &self,
        request: viewer_desktop::video_runtime::TimelineThumbnailBridgeRequest,
    ) -> Result<
        viewer_desktop::video_runtime::TimelineThumbnailBridgeResult,
        viewer_desktop::video_runtime::VideoCommandError,
    > {
        if request.request_id == "first" {
            self.first_started.notify_one();
            request.cancellation.cancelled().await;
            self.first_cancelled
                .store(true, std::sync::atomic::Ordering::SeqCst);
            return Err(viewer_desktop::video_runtime::VideoCommandError::ThumbnailCancelled);
        }
        Ok(
            viewer_desktop::video_runtime::TimelineThumbnailBridgeResult {
                request_id: request.request_id,
                bucket_us: 500_000,
                artifact_url: "viewer-image://localhost/session/token".into(),
            },
        )
    }

    fn revoke_session(&self, session_id: SessionId) {
        self.revoked.lock().unwrap().push(session_id);
    }
}

#[tokio::test]
async fn browse_cover_request_uses_the_authorized_video_source_without_opening_playback() {
    let engine = Arc::new(FakeVideoEngine::default());
    let events = Arc::new(RecordingVideoEvents::default());
    let thumbnails = Arc::new(BlockingThumbnailPort {
        first_started: tokio::sync::Notify::new(),
        first_cancelled: std::sync::atomic::AtomicBool::new(false),
        revoked: Mutex::new(Vec::new()),
    });
    let runtime =
        viewer_desktop::video_runtime::VideoRuntime::with_bridges(engine, events, thumbnails);

    assert_eq!(
        runtime.request_cover(video_source(7)).await.unwrap(),
        "viewer-image://localhost/session/cover"
    );
    assert_eq!(runtime.active_generation(), None);
}

struct FixedProbe;

impl ProjectProbePort for FixedProbe {
    fn probe(&self, _root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
        Ok(ProjectAccess::ReadWrite)
    }
}

struct BlockingVideoClose {
    order: Arc<Mutex<Vec<&'static str>>>,
    started: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
}

struct NotifyingVideoClose {
    started: Arc<tokio::sync::Notify>,
}

#[async_trait::async_trait]
impl VideoClosePort for NotifyingVideoClose {
    async fn close_video(&self) -> Result<(), viewer_desktop::state::RuntimeError> {
        self.started.notify_one();
        Ok(())
    }
}

#[async_trait::async_trait]
impl VideoClosePort for BlockingVideoClose {
    async fn close_video(&self) -> Result<(), viewer_desktop::state::RuntimeError> {
        self.order.lock().unwrap().push("video-close");
        self.started.notify_one();
        self.release.notified().await;
        Ok(())
    }
}

#[tokio::test]
async fn project_close_closes_video_before_artifact_revoke_and_session_close() {
    let cache = tempfile::tempdir().expect("create cache root");
    let project = tempfile::tempdir().expect("create project");
    let next_project = tempfile::tempdir().expect("create next project");
    let artifact_path = project.path().join("preview.png");
    fs::write(&artifact_path, b"png artifact").expect("write artifact");
    let registry = Arc::new(ImageArtifactRegistry::default());
    let runtime = Arc::new(DesktopRuntime::new_with_image_factory(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe),
        Arc::new(viewer_infrastructure::scan::walker::ProjectWalker),
        Arc::new(NoopEvents),
        Arc::new(viewer_desktop::state::MacDesktopImageFactory),
        Arc::clone(&registry),
    ));
    let opened = runtime
        .open_project(project.path())
        .await
        .expect("open project");
    let session_id = SessionId::from_str(&opened.session_id).expect("parse session ID");
    let token = registry
        .insert(session_id, EntityId::new(), artifact_path, "image/png")
        .expect("register artifact");
    let order = Arc::new(Mutex::new(Vec::new()));
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    runtime.register_video_lifecycle(Arc::new(BlockingVideoClose {
        order: Arc::clone(&order),
        started: Arc::clone(&started),
        release: Arc::clone(&release),
    }));

    let closing_runtime = Arc::clone(&runtime);
    let close = tokio::spawn(async move { closing_runtime.close_project().await });
    started.notified().await;

    assert_eq!(order.lock().unwrap().as_slice(), ["video-close"]);
    assert!(matches!(
        registry.lookup(session_id, token.as_str()),
        ImageArtifactLookup::Found(_)
    ));
    assert_eq!(
        runtime
            .open_project(next_project.path())
            .await
            .unwrap_err()
            .code,
        "project_already_open"
    );

    release.notify_one();
    close.await.expect("join close").expect("close project");

    assert_eq!(
        registry.lookup(session_id, token.as_str()),
        ImageArtifactLookup::NotFound
    );
    runtime
        .open_project(next_project.path())
        .await
        .expect("session closes after video");
    release.notify_one();
    runtime.close_project().await.expect("close next project");
}

#[tokio::test]
async fn video_open_project_lease_blocks_project_close_before_native_teardown() {
    let cache = tempfile::tempdir().expect("create cache root");
    let project = tempfile::tempdir().expect("create project");
    let runtime = Arc::new(DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe),
    ));
    runtime
        .open_project(project.path())
        .await
        .expect("open project");
    let native_close_started = Arc::new(tokio::sync::Notify::new());
    runtime.register_video_lifecycle(Arc::new(NotifyingVideoClose {
        started: Arc::clone(&native_close_started),
    }));
    let open_lease = runtime.video_open_project_lease().await;

    let closing_runtime = Arc::clone(&runtime);
    let close = tokio::spawn(async move { closing_runtime.close_project().await });
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(50),
            native_close_started.notified(),
        )
        .await
        .is_err(),
        "project close must not begin native teardown while open owns the lease"
    );

    drop(open_lease);
    native_close_started.notified().await;
    close.await.unwrap().unwrap();
}

#[tokio::test]
async fn video_open_serializes_concurrent_replacements_through_publication() {
    let engine = Arc::new(FakeVideoEngine::default());
    let runtime = Arc::new(viewer_desktop::video_runtime::VideoRuntime::new(
        Arc::clone(&engine),
    ));
    let first_started = Arc::new(tokio::sync::Notify::new());
    let first_release = Arc::new(tokio::sync::Notify::new());
    let second_started = Arc::new(tokio::sync::Notify::new());

    let first_runtime = Arc::clone(&runtime);
    let first_started_task = Arc::clone(&first_started);
    let first_release_task = Arc::clone(&first_release);
    let first = tokio::spawn(async move {
        first_runtime
            .replace_authorized(video_source(7), || async move {
                first_started_task.notify_one();
                first_release_task.notified().await;
                Ok(())
            })
            .await
    });
    first_started.notified().await;

    let second_runtime = Arc::clone(&runtime);
    let second_started_task = Arc::clone(&second_started);
    let second = tokio::spawn(async move {
        second_runtime
            .replace_authorized(video_source(9), || async move {
                second_started_task.notify_one();
                Ok(())
            })
            .await
    });
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(50),
            second_started.notified(),
        )
        .await
        .is_err(),
        "second open must not prepare while the first open is unpublished"
    );

    first_release.notify_one();
    first.await.unwrap().unwrap();
    second_started.notified().await;
    second.await.unwrap().unwrap();

    let calls = engine.calls();
    assert_eq!(calls.len(), 3);
    assert!(matches!(
        &calls[0],
        FakeVideoEngineCall::OpenPaused(request)
            if request.generation == 1 && request.source.entity_id == EntityId::from_u128(7)
    ));
    assert_eq!(calls[1], FakeVideoEngineCall::Close(1));
    assert!(matches!(
        &calls[2],
        FakeVideoEngineCall::OpenPaused(request)
            if request.generation == 2 && request.source.entity_id == EntityId::from_u128(9)
    ));
}

fn video_source(entity_id: u128) -> viewer_desktop::state::AuthorizedVideoSource {
    viewer_desktop::state::AuthorizedVideoSource {
        entity_id: EntityId::from_u128(entity_id),
        session_id: SessionId::from_u128(8),
        canonical_path: Path::new("/project/clip.mp4").to_path_buf(),
        metadata: viewer_domain::video::VideoMetadata {
            duration_us: Some(2_000_000),
            display_width: Some(640),
            display_height: Some(360),
            rotation_degrees: 0,
            frame_rate_millihertz: Some(24_000),
            video_codec: Some("h264".into()),
            audio_codec: None,
            probe_status: viewer_domain::video::VideoProbeStatus::Ready,
        },
        metadata_refreshed_for_retry: false,
    }
}

#[derive(Default)]
struct NoopEvents;

impl viewer_desktop::state::DesktopEventSink for NoopEvents {
    fn emit_scan(&self, _event: viewer_desktop::dto::ScanEventDto) {}
}

#[test]
fn video_events_use_the_tagged_camel_case_contract() {
    let value = serde_json::to_value(VideoEventDto::Prepared {
        generation: 7,
        media: VideoMediaDto {
            duration_us: Some(2_000_000),
            display_width: Some(1_920),
            display_height: Some(1_080),
            rotation_degrees: 0,
        },
    })
    .expect("serialize prepared event");

    assert_eq!(
        value,
        serde_json::json!({
            "type": "prepared",
            "generation": 7,
            "media": {
                "durationUs": 2_000_000,
                "displayWidth": 1_920,
                "displayHeight": 1_080,
                "rotationDegrees": 0
            }
        })
    );
    assert_eq!(
        serde_json::to_value(VideoEventDto::FirstFrameReady { generation: 7 }).unwrap()["type"],
        "firstFrameReady"
    );
    assert_eq!(
        serde_json::to_value(VideoEventDto::Progress {
            generation: 7,
            time_us: 1_250_000,
            duration_us: Some(2_000_000),
        })
        .unwrap(),
        serde_json::json!({
            "type": "progress",
            "generation": 7,
            "timeUs": 1_250_000,
            "durationUs": 2_000_000,
        })
    );
    assert_eq!(
        serde_json::to_value(VideoEventDto::TimelineThumbnailReady {
            generation: 7,
            request_id: "request-1".into(),
            bucket_us: 1_000_000,
            artifact_url: "viewer-image://localhost/session/token".into(),
        })
        .unwrap(),
        serde_json::json!({
            "type": "timelineThumbnailReady",
            "generation": 7,
            "requestId": "request-1",
            "bucketUs": 1_000_000,
            "artifactUrl": "viewer-image://localhost/session/token",
        })
    );
}

#[test]
fn progress_is_coalesced_to_ten_hertz_and_flushed_before_terminal_events() {
    use std::time::Duration;

    let mut coalescer = ProgressCoalescer::default();
    let first = coalescer.push_at(
        Duration::ZERO,
        VideoEventDto::Progress {
            generation: 3,
            time_us: 10,
            duration_us: Some(100),
        },
    );
    assert_eq!(first.len(), 1);
    assert!(
        coalescer
            .push_at(
                Duration::from_millis(30),
                VideoEventDto::Progress {
                    generation: 3,
                    time_us: 20,
                    duration_us: Some(100),
                },
            )
            .is_empty()
    );
    assert!(
        coalescer
            .push_at(
                Duration::from_millis(80),
                VideoEventDto::Progress {
                    generation: 3,
                    time_us: 30,
                    duration_us: Some(100),
                },
            )
            .is_empty()
    );

    let terminal = coalescer.push_at(
        Duration::from_millis(90),
        VideoEventDto::Ended { generation: 3 },
    );
    assert_eq!(terminal.len(), 2);
    assert!(matches!(
        terminal[0],
        VideoEventDto::Progress { time_us: 30, .. }
    ));
    assert!(matches!(
        terminal[1],
        VideoEventDto::Ended { generation: 3 }
    ));
}

#[tokio::test]
async fn typed_runtime_rejects_stale_generation_before_engine_mutation() {
    let engine = Arc::new(FakeVideoEngine::default());
    let runtime = viewer_desktop::video_runtime::VideoRuntime::new(Arc::clone(&engine));

    let generation = runtime
        .open_authorized(viewer_desktop::state::AuthorizedVideoSource {
            entity_id: EntityId::from_u128(7),
            session_id: SessionId::from_u128(8),
            canonical_path: Path::new("/project/clip.mp4").to_path_buf(),
            metadata: viewer_domain::video::VideoMetadata {
                duration_us: Some(1_000_000),
                display_width: Some(640),
                display_height: Some(360),
                rotation_degrees: 0,
                frame_rate_millihertz: Some(24_000),
                video_codec: Some("h264".into()),
                audio_codec: None,
                probe_status: viewer_domain::video::VideoProbeStatus::Ready,
            },
            metadata_refreshed_for_retry: false,
        })
        .await
        .expect("open authorized source")
        .generation;
    let calls_after_open = engine.calls();

    let error = runtime.play(generation + 1).await.unwrap_err();

    assert_eq!(error.code(), "stale_video_generation");
    assert_eq!(engine.calls(), calls_after_open);
    assert!(matches!(
        calls_after_open.as_slice(),
        [FakeVideoEngineCall::OpenPaused(_)]
    ));
}

#[tokio::test]
async fn stale_engine_events_are_not_published_by_the_bridge() {
    let engine = Arc::new(FakeVideoEngine::default());
    let events = Arc::new(RecordingVideoEvents::default());
    let runtime = viewer_desktop::video_runtime::VideoRuntime::with_cache_and_events(
        engine,
        tempfile::tempdir().unwrap().path(),
        events.clone(),
    )
    .unwrap();
    let generation = runtime
        .open_authorized(viewer_desktop::state::AuthorizedVideoSource {
            entity_id: EntityId::from_u128(7),
            session_id: SessionId::from_u128(8),
            canonical_path: Path::new("/project/clip.mp4").to_path_buf(),
            metadata: viewer_domain::video::VideoMetadata {
                duration_us: Some(1_000_000),
                display_width: Some(640),
                display_height: Some(360),
                rotation_degrees: 0,
                frame_rate_millihertz: Some(24_000),
                video_codec: Some("h264".into()),
                audio_codec: None,
                probe_status: viewer_domain::video::VideoProbeStatus::Ready,
            },
            metadata_refreshed_for_retry: false,
        })
        .await
        .unwrap()
        .generation;
    events.0.lock().unwrap().clear();

    runtime
        .handle_engine_event(
            generation + 1,
            viewer_application::EngineEvent::TimeChanged {
                time_us: 750_000,
                duration_us: Some(1_000_000),
            },
        )
        .await;

    assert!(events.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn cache_commands_use_the_verified_video_cache_and_clear_it_safely() {
    let cache = tempfile::tempdir().expect("create cache root");
    let engine = Arc::new(FakeVideoEngine::default());
    let runtime = viewer_desktop::video_runtime::VideoRuntime::with_cache(engine, cache.path())
        .expect("initialize verified cache");

    let before = runtime.cache_stats().expect("query cache stats");
    let cleared = runtime.cache_clear().await.expect("clear video cache");

    assert_eq!(before.entry_count, 0);
    assert_eq!(cleared.entry_count, 0);
    assert_eq!(before.budget_bytes, 1_073_741_824);
    assert_eq!(cleared.budget_bytes, before.budget_bytes);
    assert!(cache.path().join("video/.viewer-video-cache-v1").is_file());
}

#[tokio::test]
async fn fullscreen_gate_rejects_stale_generation_before_window_action() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let engine = Arc::new(FakeVideoEngine::default());
    let runtime = viewer_desktop::video_runtime::VideoRuntime::new(engine);
    let window_actions = AtomicUsize::new(0);

    let error = viewer_desktop::commands::video::apply_fullscreen_after_generation_gate(
        &runtime,
        99,
        true,
        |_| {
            window_actions.fetch_add(1, Ordering::SeqCst);
            Ok(())
        },
    )
    .unwrap_err();

    assert_eq!(error.code, "stale_video_generation");
    assert_eq!(window_actions.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fullscreen_transition_serializes_confirmation_and_event_before_close() {
    let engine = Arc::new(FakeVideoEngine::default());
    let events = Arc::new(RecordingVideoEvents::default());
    let runtime = Arc::new(
        viewer_desktop::video_runtime::VideoRuntime::with_cache_and_events(
            engine,
            tempfile::tempdir().unwrap().path(),
            events.clone(),
        )
        .unwrap(),
    );
    let generation = runtime
        .open_authorized(video_source(7))
        .await
        .unwrap()
        .generation;
    events.0.lock().unwrap().clear();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();

    let fullscreen_runtime = Arc::clone(&runtime);
    let fullscreen = tokio::spawn(async move {
        fullscreen_runtime
            .set_fullscreen_guarded(generation, true, move |requested| {
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok(requested)
            })
            .await
    });
    started_rx.recv().unwrap();
    let close_runtime = Arc::clone(&runtime);
    let mut close = tokio::spawn(async move { close_runtime.close(generation).await });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut close)
            .await
            .is_err(),
        "close must not change generation while fullscreen confirmation is pending"
    );

    release_tx.send(()).unwrap();
    fullscreen.await.unwrap().unwrap();
    close.await.unwrap().unwrap();
    let published = events.0.lock().unwrap();
    let fullscreen_index = published
        .iter()
        .position(|event| matches!(event, VideoEventDto::FullscreenChanged { .. }))
        .expect("publish confirmed fullscreen");
    let closed_index = published
        .iter()
        .position(|event| matches!(event, VideoEventDto::Closed { .. }))
        .expect("publish close");
    assert!(fullscreen_index < closed_index);
}

#[tokio::test]
async fn newer_thumbnail_request_cancels_old_work_and_only_latest_result_is_published() {
    let engine = Arc::new(FakeVideoEngine::default());
    let events = Arc::new(RecordingVideoEvents::default());
    let thumbnails = Arc::new(BlockingThumbnailPort {
        first_started: tokio::sync::Notify::new(),
        first_cancelled: std::sync::atomic::AtomicBool::new(false),
        revoked: Mutex::new(Vec::new()),
    });
    let runtime = Arc::new(viewer_desktop::video_runtime::VideoRuntime::with_bridges(
        engine,
        events.clone(),
        thumbnails.clone(),
    ));
    let generation = runtime
        .open_authorized(viewer_desktop::state::AuthorizedVideoSource {
            entity_id: EntityId::from_u128(7),
            session_id: SessionId::from_u128(8),
            canonical_path: Path::new("/project/clip.mp4").to_path_buf(),
            metadata: viewer_domain::video::VideoMetadata {
                duration_us: Some(2_000_000),
                display_width: Some(640),
                display_height: Some(360),
                rotation_degrees: 0,
                frame_rate_millihertz: Some(24_000),
                video_codec: Some("h264".into()),
                audio_codec: None,
                probe_status: viewer_domain::video::VideoProbeStatus::Ready,
            },
            metadata_refreshed_for_retry: false,
        })
        .await
        .unwrap()
        .generation;
    let first_runtime = Arc::clone(&runtime);
    let first = tokio::spawn(async move {
        first_runtime
            .request_thumbnail(generation, "first".into(), 250_000)
            .await
    });
    thumbnails.first_started.notified().await;

    runtime
        .request_thumbnail(generation, "second".into(), 700_000)
        .await
        .expect("publish newest thumbnail");
    assert_eq!(
        first.await.unwrap(),
        Err(viewer_desktop::video_runtime::VideoCommandError::ThumbnailCancelled)
    );
    assert!(
        thumbnails
            .first_cancelled
            .load(std::sync::atomic::Ordering::SeqCst)
    );
    let published = events.0.lock().unwrap();
    let ready = published
        .iter()
        .filter_map(|event| match event {
            VideoEventDto::TimelineThumbnailReady { request_id, .. } => Some(request_id.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(ready, ["second"]);
}

#[tokio::test]
async fn close_failure_still_resets_session_and_revokes_thumbnail_artifacts() {
    let engine = Arc::new(FakeVideoEngine::default());
    let events = Arc::new(RecordingVideoEvents::default());
    let thumbnails = Arc::new(BlockingThumbnailPort {
        first_started: tokio::sync::Notify::new(),
        first_cancelled: std::sync::atomic::AtomicBool::new(false),
        revoked: Mutex::new(Vec::new()),
    });
    let runtime = viewer_desktop::video_runtime::VideoRuntime::with_bridges(
        Arc::clone(&engine),
        events,
        thumbnails.clone(),
    );
    let project_session = SessionId::from_u128(8);
    runtime
        .open_authorized(viewer_desktop::state::AuthorizedVideoSource {
            entity_id: EntityId::from_u128(7),
            session_id: project_session,
            canonical_path: Path::new("/project/clip.mp4").to_path_buf(),
            metadata: viewer_domain::video::VideoMetadata {
                duration_us: Some(2_000_000),
                display_width: Some(640),
                display_height: Some(360),
                rotation_degrees: 0,
                frame_rate_millihertz: Some(24_000),
                video_codec: Some("h264".into()),
                audio_codec: None,
                probe_status: viewer_domain::video::VideoProbeStatus::Ready,
            },
            metadata_refreshed_for_retry: false,
        })
        .await
        .unwrap();
    engine.fail_next(viewer_application::VideoEngineError::Unavailable);

    assert_eq!(
        runtime.close_active().await,
        Err(viewer_desktop::video_runtime::VideoCommandError::EngineUnavailable)
    );
    assert_eq!(runtime.active_generation(), None);
    assert_eq!(
        thumbnails.revoked.lock().unwrap().as_slice(),
        [project_session]
    );
}

#[tokio::test]
async fn playback_activity_is_generation_scoped_and_tracks_play_pause_fail_and_close() {
    let engine = Arc::new(FakeVideoEngine::default());
    let activity = Arc::new(RecordingPlaybackActivity::default());
    let runtime = viewer_desktop::video_runtime::VideoRuntime::with_playback_activity(
        engine,
        activity.clone(),
    );
    let generation = runtime
        .open_authorized(video_source(7))
        .await
        .unwrap()
        .generation;
    runtime
        .handle_engine_event(generation, viewer_application::EngineEvent::FirstFrameReady)
        .await;
    runtime.pause(generation).await.unwrap();
    runtime.play(generation).await.unwrap();
    runtime
        .handle_engine_event(
            generation + 1,
            viewer_application::EngineEvent::Failed(
                viewer_domain::video::VideoFailureKind::DecodeFallbackFailed,
            ),
        )
        .await;
    runtime
        .handle_engine_event(
            generation,
            viewer_application::EngineEvent::Failed(
                viewer_domain::video::VideoFailureKind::DecodeFallbackFailed,
            ),
        )
        .await;
    runtime.close_active().await.unwrap();

    assert_eq!(
        activity.0.lock().unwrap().as_slice(),
        [
            (generation, false),
            (generation, true),
            (generation, false),
            (generation, true),
            (generation, false),
            (generation, false),
        ]
    );
}

#[tokio::test]
async fn command_failure_releases_playback_activity_without_allowing_stale_watch_mutation() {
    let engine = Arc::new(FakeVideoEngine::default());
    let activity = Arc::new(RecordingPlaybackActivity::default());
    let runtime = viewer_desktop::video_runtime::VideoRuntime::with_playback_activity(
        Arc::clone(&engine),
        activity.clone(),
    );
    let generation = runtime
        .open_authorized(video_source(7))
        .await
        .unwrap()
        .generation;
    runtime
        .handle_engine_event(generation, viewer_application::EngineEvent::FirstFrameReady)
        .await;
    activity.0.lock().unwrap().clear();
    engine.fail_next(viewer_application::VideoEngineError::Unavailable);

    assert_eq!(
        runtime.pause(generation).await,
        Err(viewer_desktop::video_runtime::VideoCommandError::EngineUnavailable)
    );
    assert!(matches!(
        runtime.playback_state(),
        viewer_application::VideoPlaybackState::Failed(_)
    ));
    assert_eq!(activity.0.lock().unwrap().as_slice(), [(generation, false)]);

    assert_eq!(
        runtime.pause(generation + 1).await,
        Err(viewer_desktop::video_runtime::VideoCommandError::StaleGeneration)
    );
    assert_eq!(activity.0.lock().unwrap().as_slice(), [(generation, false)]);
}

#[test]
fn typed_video_requests_reject_unknown_fields_and_untyped_rates() {
    assert!(
        serde_json::from_value::<viewer_desktop::dto::GenerationDto>(
            serde_json::json!({ "generation": 3, "path": "/secret.mp4" })
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<viewer_desktop::dto::VideoRateDto>(
            serde_json::json!({ "generation": 3, "rate": 1.1 })
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<viewer_desktop::dto::VideoOpenRequestDto>(serde_json::json!({
            "entityId": EntityId::from_u128(1).to_string(),
            "path": "/secret.mp4",
            "surfaceRect": { "x": 0, "y": 0, "width": 640, "height": 360 }
        }))
        .is_err()
    );
    let preview = serde_json::from_value::<viewer_desktop::dto::VideoSeekDto>(serde_json::json!({
        "generation": 3,
        "requestId": 9,
        "timeUs": 750_000,
        "intent": "preview"
    }))
    .expect("typed preview request");
    assert_eq!(
        preview.request(),
        SeekRequest {
            request_id: 9,
            time_us: 750_000,
            intent: SeekIntent::Preview,
        }
    );
    assert!(
        serde_json::from_value::<viewer_desktop::dto::VideoSeekDto>(serde_json::json!({
            "generation": 3,
            "timeUs": 750_000
        }))
        .is_err(),
        "legacy seeks without request ownership or intent must be rejected"
    );
    assert!(
        serde_json::from_value::<viewer_desktop::dto::VideoSurfaceRectDto>(serde_json::json!({
            "generation": 3,
            "x": 0,
            "y": 0,
            "width": 640,
            "height": 360
        }))
        .is_err(),
        "geometry publication must carry a monotonic sequence"
    );
}
