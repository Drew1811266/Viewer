use async_trait::async_trait;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tempfile::tempdir;
use tokio::sync::{Notify, Semaphore, watch};
use tokio_util::sync::CancellationToken;
use viewer_application::scheduler::TaskCoordinator;
use viewer_domain::{SessionId, VideoThumbnailRequestId};
use viewer_infrastructure::{
    video_cache::{VideoCache, VideoSourceIdentity},
    video_probe::MediaFileIdentity,
    video_thumbnail::{
        CoverThumbnailRequest, FrameExtractionError, FrameSample, TimelineThumbnailRequest,
        VideoFrameExtractor, VideoFrameOutput, VideoThumbnailContext, VideoThumbnailError,
        VideoThumbnailService, choose_cover_sample, quantize_timeline_time,
    },
};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nthumbnail";

fn sample(luminance: f64, variance: f64) -> FrameSample {
    FrameSample::decoded(luminance, variance)
}

fn failed_sample() -> FrameSample {
    FrameSample::decode_failed()
}

#[test]
fn first_useful_frame_skips_black_intro() {
    let frames = [sample(0.01, 0.00), sample(0.08, 0.02), sample(0.42, 0.16)];
    assert_eq!(choose_cover_sample(&frames), Some(2));
}

#[test]
fn fallback_uses_first_decodable_frame() {
    let frames = [failed_sample(), sample(0.03, 0.01), sample(0.02, 0.01)];
    assert_eq!(choose_cover_sample(&frames), Some(1));
}

#[test]
fn no_decodable_sample_has_no_cover() {
    assert_eq!(
        choose_cover_sample(&[failed_sample(), failed_sample()]),
        None
    );
}

#[test]
fn timeline_time_is_clamped_then_quantized_to_half_second_buckets() {
    assert_eq!(quantize_timeline_time(1_249_999, 5_000_000), 1_000_000);
    assert_eq!(quantize_timeline_time(5_900_000, 5_200_000), 5_000_000);
    assert_eq!(quantize_timeline_time(100_000, 0), 0);
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExtractionRequest {
    path: PathBuf,
    time_us: u64,
    output: VideoFrameOutput,
}

struct FakeExtractor {
    requests: Mutex<Vec<ExtractionRequest>>,
    gray_calls: AtomicUsize,
    active: AtomicUsize,
    max_active: AtomicUsize,
    delay: Duration,
    block: bool,
    started: Notify,
    cancelled: Notify,
    release: Semaphore,
}

impl FakeExtractor {
    fn immediate() -> Arc<Self> {
        Arc::new(Self::new(Duration::ZERO, false))
    }

    fn delayed(delay: Duration) -> Arc<Self> {
        Arc::new(Self::new(delay, false))
    }

    fn blocked() -> Arc<Self> {
        Arc::new(Self::new(Duration::ZERO, true))
    }

    fn new(delay: Duration, block: bool) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            gray_calls: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
            delay,
            block,
            started: Notify::new(),
            cancelled: Notify::new(),
            release: Semaphore::new(0),
        }
    }

    fn requests(&self) -> Vec<ExtractionRequest> {
        self.requests.lock().unwrap().clone()
    }

    async fn wait_started(&self) {
        self.started.notified().await;
    }

    async fn wait_cancelled(&self) {
        self.cancelled.notified().await;
    }

    fn release_one(&self) {
        self.release.add_permits(1);
    }
}

#[async_trait]
impl VideoFrameExtractor for FakeExtractor {
    async fn extract_frame(
        &self,
        canonical_path: &Path,
        _expected_identity: &MediaFileIdentity,
        time_us: u64,
        output: VideoFrameOutput,
        cancellation: CancellationToken,
    ) -> Result<Vec<u8>, FrameExtractionError> {
        self.requests.lock().unwrap().push(ExtractionRequest {
            path: canonical_path.to_path_buf(),
            time_us,
            output,
        });
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_active.fetch_max(active, Ordering::SeqCst);
        self.started.notify_waiters();
        if self.block {
            tokio::select! {
                permit = self.release.acquire() => permit.unwrap().forget(),
                _ = cancellation.cancelled() => {
                    self.active.fetch_sub(1, Ordering::SeqCst);
                    self.cancelled.notify_waiters();
                    return Err(FrameExtractionError::Cancelled);
                }
            }
        }
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        self.active.fetch_sub(1, Ordering::SeqCst);
        match output {
            VideoFrameOutput::Gray160 => {
                let call = self.gray_calls.fetch_add(1, Ordering::SeqCst);
                if call == 0 {
                    Ok(vec![0; 160])
                } else {
                    Ok((0..160)
                        .map(|index| if index % 2 == 0 { 32 } else { 224 })
                        .collect())
                }
            }
            VideoFrameOutput::Png320 | VideoFrameOutput::Png640 => Ok(PNG.to_vec()),
        }
    }
}

struct SourceFixture {
    _directory: tempfile::TempDir,
    path: PathBuf,
    source: VideoSourceIdentity,
    identity: MediaFileIdentity,
}

impl SourceFixture {
    fn new() -> Self {
        let directory = tempdir().unwrap();
        let path = directory.path().join("clip.mp4");
        fs::write(&path, b"video fixture").unwrap();
        let path = path.canonicalize().unwrap();
        let metadata = fs::metadata(&path).unwrap();
        let source = VideoSourceIdentity::new(&path, metadata.len(), modified_ns(&metadata));
        let identity = MediaFileIdentity::from_metadata(&metadata);
        Self {
            _directory: directory,
            path,
            source,
            identity,
        }
    }
}

fn modified_ns(metadata: &fs::Metadata) -> i128 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec())
    }
    #[cfg(not(unix))]
    {
        metadata
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i128
    }
}

fn service(
    extractor: Arc<FakeExtractor>,
    playback: watch::Receiver<bool>,
) -> (
    tempfile::TempDir,
    Arc<VideoCache>,
    Arc<TaskCoordinator>,
    VideoThumbnailService,
) {
    let app_cache = tempdir().unwrap();
    let cache = Arc::new(VideoCache::initialize(app_cache.path()).unwrap());
    let coordinator = Arc::new(TaskCoordinator::default());
    let service = VideoThumbnailService::new(
        extractor,
        Arc::clone(&cache),
        Arc::clone(&coordinator),
        playback,
    );
    (app_cache, cache, coordinator, service)
}

#[tokio::test]
async fn cover_scores_early_gray_samples_then_renders_the_first_useful_time_as_png640() {
    let source = SourceFixture::new();
    let extractor = FakeExtractor::immediate();
    let (_playback_tx, playback_rx) = watch::channel(false);
    let (_app_cache, cache, coordinator, service) = service(extractor.clone(), playback_rx);
    let session = SessionId::new();
    let generation = coordinator.begin_session(session);

    let artifact = service
        .cover(CoverThumbnailRequest::new(
            VideoThumbnailContext::new(
                session,
                generation,
                source.source.clone(),
                source.identity.clone(),
                CancellationToken::new(),
            ),
            10_000_000,
        ))
        .await
        .unwrap();

    assert!(artifact.path().starts_with(cache.root()));
    assert_eq!(artifact.session_id(), session);
    assert_eq!(artifact.generation(), generation);
    assert_eq!(artifact.request_id(), None);
    assert_eq!(
        extractor.requests(),
        [
            ExtractionRequest {
                path: source.path.clone(),
                time_us: 300_000,
                output: VideoFrameOutput::Gray160,
            },
            ExtractionRequest {
                path: source.path.clone(),
                time_us: 800_000,
                output: VideoFrameOutput::Gray160,
            },
            ExtractionRequest {
                path: source.path,
                time_us: 800_000,
                output: VideoFrameOutput::Png640,
            },
        ]
    );
}

#[tokio::test]
async fn timeline_quantizes_reuses_cache_and_carries_the_request_generation() {
    let source = SourceFixture::new();
    let extractor = FakeExtractor::immediate();
    let (_playback_tx, playback_rx) = watch::channel(false);
    let (_app_cache, _cache, coordinator, service) = service(extractor.clone(), playback_rx);
    let session = SessionId::new();
    let generation = coordinator.begin_session(session);
    let first_request = VideoThumbnailRequestId::new();
    let second_request = VideoThumbnailRequestId::new();

    let first = service
        .timeline(TimelineThumbnailRequest::new(
            VideoThumbnailContext::new(
                session,
                generation,
                source.source.clone(),
                source.identity.clone(),
                CancellationToken::new(),
            ),
            first_request,
            1_249_999,
            5_000_000,
        ))
        .await
        .unwrap();
    let second = service
        .timeline(TimelineThumbnailRequest::new(
            VideoThumbnailContext::new(
                session,
                generation,
                source.source,
                source.identity,
                CancellationToken::new(),
            ),
            second_request,
            1_000_001,
            5_000_000,
        ))
        .await
        .unwrap();

    assert_eq!(first.path(), second.path());
    assert_eq!(first.request_id(), Some(first_request));
    assert_eq!(second.request_id(), Some(second_request));
    assert_eq!(extractor.requests().len(), 1);
    assert_eq!(extractor.requests()[0].time_us, 1_000_000);
    assert_eq!(extractor.requests()[0].output, VideoFrameOutput::Png320);
}

#[tokio::test]
async fn active_playback_yields_before_launching_thumbnail_work() {
    let source = SourceFixture::new();
    let extractor = FakeExtractor::immediate();
    let (playback_tx, playback_rx) = watch::channel(true);
    let (_app_cache, _cache, coordinator, service) = service(extractor.clone(), playback_rx);
    let session = SessionId::new();
    let generation = coordinator.begin_session(session);
    let task = tokio::spawn(async move {
        service
            .timeline(TimelineThumbnailRequest::new(
                VideoThumbnailContext::new(
                    session,
                    generation,
                    source.source,
                    source.identity,
                    CancellationToken::new(),
                ),
                VideoThumbnailRequestId::new(),
                1_000_000,
                5_000_000,
            ))
            .await
    });
    tokio::time::sleep(Duration::from_millis(25)).await;
    assert!(extractor.requests().is_empty());

    playback_tx.send(false).unwrap();
    task.await.unwrap().unwrap();
    assert_eq!(extractor.requests().len(), 1);
}

#[tokio::test]
async fn playback_activation_after_launch_cancels_and_reaps_before_retrying() {
    let source = SourceFixture::new();
    let extractor = FakeExtractor::blocked();
    let (playback_tx, playback_rx) = watch::channel(false);
    let (_app_cache, _cache, coordinator, service) = service(extractor.clone(), playback_rx);
    let session = SessionId::new();
    let generation = coordinator.begin_session(session);
    let task = tokio::spawn(async move {
        service
            .timeline(TimelineThumbnailRequest::new(
                VideoThumbnailContext::new(
                    session,
                    generation,
                    source.source,
                    source.identity,
                    CancellationToken::new(),
                ),
                VideoThumbnailRequestId::new(),
                1_000_000,
                5_000_000,
            ))
            .await
    });
    extractor.wait_started().await;

    playback_tx.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(1), extractor.wait_cancelled())
        .await
        .expect("playback must cancel and await the launched extractor");
    assert_eq!(extractor.active.load(Ordering::SeqCst), 0);
    assert!(
        !task.is_finished(),
        "request must yield while playback is active"
    );

    playback_tx.send(false).unwrap();
    extractor.release_one();
    task.await.unwrap().unwrap();
    assert_eq!(extractor.requests().len(), 2);
}

#[tokio::test]
async fn cancellation_after_decode_prevents_cache_publication() {
    let source = SourceFixture::new();
    let extractor = FakeExtractor::blocked();
    let (_playback_tx, playback_rx) = watch::channel(false);
    let (_app_cache, cache, coordinator, service) = service(extractor.clone(), playback_rx);
    let session = SessionId::new();
    let generation = coordinator.begin_session(session);
    let cancellation = CancellationToken::new();
    let task_cancellation = cancellation.clone();
    let task = tokio::spawn(async move {
        service
            .timeline(TimelineThumbnailRequest::new(
                VideoThumbnailContext::new(
                    session,
                    generation,
                    source.source,
                    source.identity,
                    task_cancellation,
                ),
                VideoThumbnailRequestId::new(),
                1_000_000,
                5_000_000,
            ))
            .await
    });
    extractor.wait_started().await;

    cancellation.cancel();
    extractor.release_one();

    assert_eq!(task.await.unwrap(), Err(VideoThumbnailError::Cancelled));
    assert_eq!(cache.stats().unwrap().entry_count, 0);
}

#[tokio::test]
async fn stale_session_generation_after_decode_prevents_cache_publication() {
    let source = SourceFixture::new();
    let extractor = FakeExtractor::blocked();
    let (_playback_tx, playback_rx) = watch::channel(false);
    let (_app_cache, cache, coordinator, service) = service(extractor.clone(), playback_rx);
    let session = SessionId::new();
    let generation = coordinator.begin_session(session);
    let task = tokio::spawn(async move {
        service
            .timeline(TimelineThumbnailRequest::new(
                VideoThumbnailContext::new(
                    session,
                    generation,
                    source.source,
                    source.identity,
                    CancellationToken::new(),
                ),
                VideoThumbnailRequestId::new(),
                1_000_000,
                5_000_000,
            ))
            .await
    });
    extractor.wait_started().await;

    coordinator.bump_generation(session).unwrap();
    extractor.release_one();

    assert_eq!(task.await.unwrap(), Err(VideoThumbnailError::Stale));
    assert_eq!(cache.stats().unwrap().entry_count, 0);
}

#[tokio::test]
async fn thumbnail_service_runs_at_most_one_extractor_job_concurrently() {
    let first_source = SourceFixture::new();
    let second_source = SourceFixture::new();
    let extractor = FakeExtractor::delayed(Duration::from_millis(30));
    let (_playback_tx, playback_rx) = watch::channel(false);
    let (_app_cache, _cache, coordinator, service) = service(extractor.clone(), playback_rx);
    let session = SessionId::new();
    let generation = coordinator.begin_session(session);
    let first_service = service.clone();
    let first = tokio::spawn(async move {
        first_service
            .timeline(TimelineThumbnailRequest::new(
                VideoThumbnailContext::new(
                    session,
                    generation,
                    first_source.source,
                    first_source.identity,
                    CancellationToken::new(),
                ),
                VideoThumbnailRequestId::new(),
                500_000,
                5_000_000,
            ))
            .await
    });
    let second = tokio::spawn(async move {
        service
            .timeline(TimelineThumbnailRequest::new(
                VideoThumbnailContext::new(
                    session,
                    generation,
                    second_source.source,
                    second_source.identity,
                    CancellationToken::new(),
                ),
                VideoThumbnailRequestId::new(),
                1_500_000,
                5_000_000,
            ))
            .await
    });

    first.await.unwrap().unwrap();
    second.await.unwrap().unwrap();
    assert_eq!(extractor.max_active.load(Ordering::SeqCst), 1);
}
