use std::{
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio_util::sync::CancellationToken;

use viewer_application::{ProjectAccess, ProjectProbeError, ProjectProbePort};
use viewer_desktop::{
    dto::FolderWorkspaceDto,
    state::DesktopRuntime,
    video_runtime::{VideoCommandError, VideoRuntime},
};
use viewer_domain::{
    EntityId,
    video::{VideoFailureKind, VideoMetadata, VideoProbeStatus},
};
use viewer_infrastructure::{
    scan::walker::ProjectWalker,
    video_probe::{MediaFileIdentity, VideoMetadataProbe, VideoProbeError},
};
use viewer_test_support::video_engine::{FakeVideoEngine, FakeVideoEngineCall};

struct FixedProbe;

impl ProjectProbePort for FixedProbe {
    fn probe(&self, _root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
        Ok(ProjectAccess::ReadWrite)
    }
}

#[derive(Default)]
struct NoopEvents;

impl viewer_desktop::state::DesktopEventSink for NoopEvents {
    fn emit_scan(&self, _event: viewer_desktop::dto::ScanEventDto) {}
}

struct ReadyVideoProbe;

#[async_trait::async_trait]
impl VideoMetadataProbe for ReadyVideoProbe {
    async fn probe(
        &self,
        _canonical_path: &Path,
        cancellation: CancellationToken,
    ) -> Result<VideoMetadata, VideoProbeError> {
        if cancellation.is_cancelled() {
            return Err(VideoProbeError::Cancelled);
        }
        Ok(ready_video_metadata())
    }

    async fn probe_identity_bound(
        &self,
        canonical_path: &Path,
        _expected_identity: &MediaFileIdentity,
        cancellation: CancellationToken,
    ) -> Result<VideoMetadata, VideoProbeError> {
        self.probe(canonical_path, cancellation).await
    }
}

fn ready_video_metadata() -> VideoMetadata {
    VideoMetadata {
        duration_us: Some(2_000_000),
        display_width: Some(1_920),
        display_height: Some(1_080),
        rotation_degrees: 0,
        frame_rate_millihertz: Some(30_000),
        video_codec: Some("h264".into()),
        audio_codec: Some("aac".into()),
        probe_status: VideoProbeStatus::Ready,
    }
}

#[derive(Default)]
struct FailThenReadyVideoProbe(AtomicUsize);

#[async_trait::async_trait]
impl VideoMetadataProbe for FailThenReadyVideoProbe {
    async fn probe(
        &self,
        canonical_path: &Path,
        cancellation: CancellationToken,
    ) -> Result<VideoMetadata, VideoProbeError> {
        let metadata = fs::metadata(canonical_path).unwrap();
        self.probe_identity_bound(
            canonical_path,
            &MediaFileIdentity::from_metadata(&metadata),
            cancellation,
        )
        .await
    }

    async fn probe_identity_bound(
        &self,
        _canonical_path: &Path,
        _expected_identity: &MediaFileIdentity,
        cancellation: CancellationToken,
    ) -> Result<VideoMetadata, VideoProbeError> {
        if cancellation.is_cancelled() {
            return Err(VideoProbeError::Cancelled);
        }
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(VideoProbeError::Failed(VideoFailureKind::Damaged));
        }
        Ok(ready_video_metadata())
    }
}

#[derive(Default)]
struct DeferredTerminalRetryProbe {
    failed_video_calls: AtomicUsize,
    retry_started: tokio::sync::Notify,
    retry_release: tokio::sync::Notify,
}

#[async_trait::async_trait]
impl VideoMetadataProbe for DeferredTerminalRetryProbe {
    async fn probe(
        &self,
        canonical_path: &Path,
        cancellation: CancellationToken,
    ) -> Result<VideoMetadata, VideoProbeError> {
        let metadata = fs::metadata(canonical_path).unwrap();
        self.probe_identity_bound(
            canonical_path,
            &MediaFileIdentity::from_metadata(&metadata),
            cancellation,
        )
        .await
    }

    async fn probe_identity_bound(
        &self,
        canonical_path: &Path,
        _expected_identity: &MediaFileIdentity,
        _cancellation: CancellationToken,
    ) -> Result<VideoMetadata, VideoProbeError> {
        if canonical_path.file_name().and_then(|name| name.to_str()) != Some("failed.mp4") {
            return Ok(ready_video_metadata());
        }
        if self.failed_video_calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(VideoProbeError::Failed(VideoFailureKind::Damaged));
        }
        self.retry_started.notify_one();
        self.retry_release.notified().await;
        Ok(ready_video_metadata())
    }
}

async fn scanned_runtime(project: &Path) -> (DesktopRuntime, Option<EntityId>, Option<EntityId>) {
    let cache = tempfile::tempdir().expect("create cache root");
    let cache = cache.keep();
    let runtime = DesktopRuntime::new_with_video_probe(
        cache,
        Arc::new(FixedProbe),
        Arc::new(ProjectWalker),
        Arc::new(NoopEvents),
        Arc::new(ReadyVideoProbe),
    );
    runtime.open_project(project).await.expect("open project");
    runtime.wait_for_scan().await.expect("finish project scan");
    let FolderWorkspaceDto::Content { images, videos, .. } =
        runtime.query_folder(None).await.expect("query root")
    else {
        panic!("root should contain previewable files")
    };
    let image = images
        .iter()
        .find(|file| file.name == "still.png")
        .map(|file| file.entity_id.parse().expect("parse image entity"));
    let video = videos
        .iter()
        .find(|file| file.name == "clip.mp4")
        .map(|file| file.entity_id.parse().expect("parse video entity"));
    (runtime, image, video)
}

#[tokio::test]
async fn open_rejects_non_video_stale_and_outside_project_entities() {
    let project = tempfile::tempdir().expect("create project");
    fs::write(
        project.path().join("still.png"),
        b"not an image decoder fixture",
    )
    .expect("write image candidate");
    fs::write(project.path().join("clip.mp4"), b"video candidate").expect("write video candidate");
    let outside = tempfile::NamedTempFile::new().expect("create outside file");

    let (runtime, image, video) = scanned_runtime(project.path()).await;
    let image = image.expect("image should be indexed");
    let video = video.expect("video should be indexed");

    assert_eq!(
        runtime
            .resolve_video_entity(image)
            .await
            .unwrap_err()
            .code(),
        "not_video"
    );
    assert_eq!(
        runtime
            .resolve_video_entity(EntityId::from_u128(u128::MAX))
            .await
            .unwrap_err()
            .code(),
        "stale_session"
    );

    fs::remove_file(project.path().join("clip.mp4")).expect("remove indexed video");
    std::os::unix::fs::symlink(outside.path(), project.path().join("clip.mp4"))
        .expect("replace video with escaping symlink");
    assert_eq!(
        runtime
            .resolve_video_entity(video)
            .await
            .unwrap_err()
            .code(),
        "path_not_authorized"
    );
}

#[tokio::test]
async fn resolved_video_source_is_canonical_and_carries_indexed_metadata() {
    let project = tempfile::tempdir().expect("create project");
    fs::write(project.path().join("clip.mp4"), b"video candidate").expect("write video candidate");
    let (runtime, _, video) = scanned_runtime(project.path()).await;
    let video = video.expect("video should be indexed");

    let source = runtime
        .resolve_video_entity(video)
        .await
        .expect("resolve indexed video");

    assert_eq!(source.entity_id, video);
    assert_eq!(
        source.canonical_path,
        project.path().join("clip.mp4").canonicalize().unwrap()
    );
    assert!(source.canonical_path.is_absolute());
    assert_eq!(
        source.session_id.to_string(),
        runtime.snapshot().await.unwrap().session_id
    );
}

#[tokio::test]
async fn resolving_a_terminal_video_reprobes_the_same_authorized_identity_for_retry() {
    let cache = tempfile::tempdir().expect("create cache root");
    let project = tempfile::tempdir().expect("create project");
    fs::write(project.path().join("clip.mp4"), b"video candidate").unwrap();
    let probe = Arc::new(FailThenReadyVideoProbe::default());
    let runtime = DesktopRuntime::new_with_video_probe(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe),
        Arc::new(ProjectWalker),
        Arc::new(NoopEvents),
        probe.clone(),
    );
    runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let video = loop {
        let FolderWorkspaceDto::Content { videos, .. } = runtime.query_folder(None).await.unwrap()
        else {
            panic!("root should contain the video candidate")
        };
        let video = videos
            .into_iter()
            .find(|video| video.name == "clip.mp4")
            .unwrap();
        if video.video_metadata.as_ref().is_some_and(|metadata| {
            metadata.probe_status == viewer_desktop::dto::VideoProbeStatusDto::Failed
        }) {
            break video.entity_id.parse().unwrap();
        }
        tokio::task::yield_now().await;
    };

    let source = runtime.resolve_video_entity(video).await.unwrap();

    assert_eq!(source.metadata.probe_status, VideoProbeStatus::Ready);
    assert_eq!(source.metadata.display_width, Some(1_920));
    assert_eq!(source.metadata.display_height, Some(1_080));
    assert!(source.metadata_refreshed_for_retry);
    assert_eq!(probe.0.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn pending_terminal_retry_cannot_replace_navigation_or_survive_done() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("failed.mp4"), b"failed candidate").unwrap();
    fs::write(project.path().join("ready.mp4"), b"ready candidate").unwrap();
    let probe = Arc::new(DeferredTerminalRetryProbe::default());
    let desktop = Arc::new(DesktopRuntime::new_with_video_probe(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe),
        Arc::new(ProjectWalker),
        Arc::new(NoopEvents),
        probe.clone(),
    ));
    desktop.open_project(project.path()).await.unwrap();
    desktop.wait_for_scan().await.unwrap();
    let (failed, ready) = loop {
        let FolderWorkspaceDto::Content { videos, .. } = desktop.query_folder(None).await.unwrap()
        else {
            panic!("root should contain both videos")
        };
        let failed = videos.iter().find(|video| video.name == "failed.mp4");
        let ready = videos.iter().find(|video| video.name == "ready.mp4");
        if let (Some(failed), Some(ready)) = (failed, ready)
            && failed.video_metadata.as_ref().is_some_and(|metadata| {
                metadata.probe_status == viewer_desktop::dto::VideoProbeStatusDto::Failed
            })
            && ready.video_metadata.as_ref().is_some_and(|metadata| {
                metadata.probe_status == viewer_desktop::dto::VideoProbeStatusDto::Ready
            })
        {
            break (
                failed.entity_id.parse::<EntityId>().unwrap(),
                ready.entity_id.parse::<EntityId>().unwrap(),
            );
        }
        tokio::task::yield_now().await;
    };
    let engine = Arc::new(FakeVideoEngine::default());
    let video = Arc::new(VideoRuntime::new(engine.clone()));

    let stale_attempt = video.begin_open_attempt("stale-attempt").unwrap();
    let stale_desktop = desktop.clone();
    let stale_token = stale_attempt.cancellation().clone();
    let stale_resolve = tokio::spawn(async move {
        stale_desktop
            .resolve_video_entity_for_open(failed, stale_token)
            .await
    });
    probe.retry_started.notified().await;

    let navigation_attempt = video.begin_open_attempt("navigation-attempt").unwrap();
    let ready_source = desktop
        .resolve_video_entity_for_open(ready, navigation_attempt.cancellation().clone())
        .await
        .unwrap();
    let session = video
        .replace_authorized_for_attempt(&navigation_attempt, ready_source, || async { Ok(()) })
        .await
        .unwrap();
    video.finish_open_attempt(navigation_attempt.id());
    probe.retry_release.notify_one();

    assert_eq!(
        stale_resolve.await.unwrap().unwrap_err().code(),
        "stale_session"
    );
    assert_eq!(video.active_generation(), Some(session.generation));
    assert!(matches!(
        engine.calls().as_slice(),
        [FakeVideoEngineCall::OpenPaused(request)] if request.source.entity_id == ready
    ));

    let done_attempt = video.begin_open_attempt("done-attempt").unwrap();
    let done_desktop = desktop.clone();
    let done_token = done_attempt.cancellation().clone();
    let done_resolve = tokio::spawn(async move {
        done_desktop
            .resolve_video_entity_for_open(failed, done_token)
            .await
    });
    probe.retry_started.notified().await;
    assert!(video.cancel_open_attempt(done_attempt.id()));
    probe.retry_release.notify_one();

    assert_eq!(
        done_resolve.await.unwrap().unwrap_err().code(),
        "stale_session"
    );
    assert_eq!(video.active_generation(), Some(session.generation));
    assert_eq!(
        video.ensure_open_attempt(&done_attempt),
        Err(VideoCommandError::StaleOpenAttempt)
    );
    assert!(matches!(
        engine.calls().as_slice(),
        [FakeVideoEngineCall::OpenPaused(request)] if request.source.entity_id == ready
    ));
}
