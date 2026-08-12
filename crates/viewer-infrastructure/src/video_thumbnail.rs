use crate::{
    video_cache::{
        CacheError, VideoCache, VideoCacheCommitError, VideoCacheKey, VideoSourceIdentity,
    },
    video_probe::MediaFileIdentity,
};
use async_trait::async_trait;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::{Semaphore, watch};
use tokio_util::sync::CancellationToken;
use viewer_application::scheduler::TaskCoordinator;
use viewer_domain::{SessionId, VideoThumbnailRequestId, search::Generation};
use viewer_video_mpv::{BundledMediaTools, MediaFrameOutput, MediaToolError};

const COVER_SAMPLE_FRACTIONS: [f64; 6] = [0.03, 0.08, 0.15, 0.25, 0.40, 0.55];
const MIN_COVER_LUMINANCE: f64 = 0.08;
const MIN_COVER_VARIANCE: f64 = 0.03;
const TIMELINE_BUCKET_US: u64 = 500_000;
const VIDEO_THUMBNAIL_ALGORITHM_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameSample {
    scores: Option<FrameScores>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FrameScores {
    luminance: f64,
    variance: f64,
}

impl FrameSample {
    pub const fn decoded(luminance: f64, variance: f64) -> Self {
        Self {
            scores: Some(FrameScores {
                luminance,
                variance,
            }),
        }
    }

    pub const fn decode_failed() -> Self {
        Self { scores: None }
    }

    fn is_useful(self) -> bool {
        self.scores.is_some_and(|scores| {
            scores.luminance >= MIN_COVER_LUMINANCE && scores.variance >= MIN_COVER_VARIANCE
        })
    }
}

pub fn choose_cover_sample(samples: &[FrameSample]) -> Option<usize> {
    samples
        .iter()
        .position(|sample| {
            sample.scores.is_some_and(|scores| {
                scores.luminance >= MIN_COVER_LUMINANCE && scores.variance >= MIN_COVER_VARIANCE
            })
        })
        .or_else(|| samples.iter().position(|sample| sample.scores.is_some()))
}

pub const fn quantize_timeline_time(time_us: u64, duration_us: u64) -> u64 {
    let clamped = if time_us < duration_us {
        time_us
    } else {
        duration_us
    };
    clamped / TIMELINE_BUCKET_US * TIMELINE_BUCKET_US
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VideoFrameOutput {
    Gray160,
    Png320,
    Png640,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum FrameExtractionError {
    #[error("video frame could not be decoded")]
    DecodeFailed,
    #[error("video frame extraction was cancelled")]
    Cancelled,
    #[error("video source changed")]
    SourceChanged,
    #[error("video frame extractor is unavailable")]
    Unavailable,
}

#[async_trait]
pub trait VideoFrameExtractor: Send + Sync {
    async fn extract_frame(
        &self,
        canonical_path: &Path,
        expected_identity: &MediaFileIdentity,
        time_us: u64,
        output: VideoFrameOutput,
        cancellation: CancellationToken,
    ) -> Result<Vec<u8>, FrameExtractionError>;
}

#[async_trait]
impl VideoFrameExtractor for BundledMediaTools {
    async fn extract_frame(
        &self,
        canonical_path: &Path,
        expected_identity: &MediaFileIdentity,
        time_us: u64,
        output: VideoFrameOutput,
        cancellation: CancellationToken,
    ) -> Result<Vec<u8>, FrameExtractionError> {
        let output = self
            .video_frame_identity_bound(
                canonical_path,
                expected_identity,
                time_us,
                match output {
                    VideoFrameOutput::Gray160 => MediaFrameOutput::Gray160,
                    VideoFrameOutput::Png320 => MediaFrameOutput::Png320,
                    VideoFrameOutput::Png640 => MediaFrameOutput::Png640,
                },
                cancellation,
            )
            .await
            .map_err(map_media_tool_error)?;
        if !output.status.success() || output.stdout.is_empty() {
            return Err(FrameExtractionError::DecodeFailed);
        }
        Ok(output.stdout)
    }
}

fn map_media_tool_error(error: MediaToolError) -> FrameExtractionError {
    match error {
        MediaToolError::Cancelled => FrameExtractionError::Cancelled,
        MediaToolError::InputMissing
        | MediaToolError::InputUnreadable
        | MediaToolError::InputPathNotCanonical
        | MediaToolError::InputChanged => FrameExtractionError::SourceChanged,
        MediaToolError::TimedOut
        | MediaToolError::OutputTooLarge
        | MediaToolError::StdoutTooLarge
        | MediaToolError::StderrTooLarge => FrameExtractionError::DecodeFailed,
        MediaToolError::ExecutablePathMustBeAbsolute
        | MediaToolError::UnexpectedExecutablePath
        | MediaToolError::UnsafeExecutable
        | MediaToolError::ExecutableChanged
        | MediaToolError::RuntimeIntegrity
        | MediaToolError::GateClosed
        | MediaToolError::Io(_) => FrameExtractionError::Unavailable,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VideoThumbnailArtifact {
    path: PathBuf,
    session_id: SessionId,
    generation: Generation,
    request_id: Option<VideoThumbnailRequestId>,
}

impl VideoThumbnailArtifact {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub const fn generation(&self) -> Generation {
        self.generation
    }

    pub const fn request_id(&self) -> Option<VideoThumbnailRequestId> {
        self.request_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum VideoThumbnailError {
    #[error("video thumbnail request was cancelled")]
    Cancelled,
    #[error("video thumbnail request is stale")]
    Stale,
    #[error("video source changed")]
    SourceChanged,
    #[error("video thumbnail is unavailable")]
    Unavailable,
    #[error("video thumbnail cache is unavailable")]
    Cache,
}

#[derive(Clone)]
pub struct VideoThumbnailContext {
    session_id: SessionId,
    generation: Generation,
    source: VideoSourceIdentity,
    expected_identity: MediaFileIdentity,
    cancellation: CancellationToken,
}

impl VideoThumbnailContext {
    pub fn new(
        session_id: SessionId,
        generation: Generation,
        source: VideoSourceIdentity,
        expected_identity: MediaFileIdentity,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            session_id,
            generation,
            source,
            expected_identity,
            cancellation,
        }
    }
}

pub struct CoverThumbnailRequest {
    context: VideoThumbnailContext,
    duration_us: u64,
}

impl CoverThumbnailRequest {
    pub const fn new(context: VideoThumbnailContext, duration_us: u64) -> Self {
        Self {
            context,
            duration_us,
        }
    }
}

pub struct TimelineThumbnailRequest {
    context: VideoThumbnailContext,
    request_id: VideoThumbnailRequestId,
    time_us: u64,
    duration_us: u64,
}

impl TimelineThumbnailRequest {
    pub const fn new(
        context: VideoThumbnailContext,
        request_id: VideoThumbnailRequestId,
        time_us: u64,
        duration_us: u64,
    ) -> Self {
        Self {
            context,
            request_id,
            time_us,
            duration_us,
        }
    }
}

#[derive(Clone)]
pub struct VideoThumbnailService {
    extractor: Arc<dyn VideoFrameExtractor>,
    cache: Arc<VideoCache>,
    coordinator: Arc<TaskCoordinator>,
    playback_active: watch::Receiver<bool>,
    thumbnail_gate: Arc<Semaphore>,
    #[cfg(test)]
    after_cache_hit: Option<Arc<dyn Fn() + Send + Sync>>,
    #[cfg(test)]
    after_cache_stage: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl VideoThumbnailService {
    pub fn new(
        extractor: Arc<dyn VideoFrameExtractor>,
        cache: Arc<VideoCache>,
        coordinator: Arc<TaskCoordinator>,
        playback_active: watch::Receiver<bool>,
    ) -> Self {
        Self {
            extractor,
            cache,
            coordinator,
            playback_active,
            thumbnail_gate: Arc::new(Semaphore::new(1)),
            #[cfg(test)]
            after_cache_hit: None,
            #[cfg(test)]
            after_cache_stage: None,
        }
    }

    pub async fn cover(
        &self,
        request: CoverThumbnailRequest,
    ) -> Result<VideoThumbnailArtifact, VideoThumbnailError> {
        let context = &request.context;
        self.validate_request(context)?;
        let key = VideoCacheKey::cover(context.source.clone(), VIDEO_THUMBNAIL_ALGORITHM_VERSION);
        if let Some(path) = self.cache.get_async(key).await.map_err(map_cache_error)? {
            self.after_cache_hit_for_test();
            self.validate_request(context)?;
            return Ok(artifact(path, context.session_id, context.generation, None));
        }

        let mut samples = Vec::with_capacity(COVER_SAMPLE_FRACTIONS.len());
        let mut sample_times = Vec::with_capacity(COVER_SAMPLE_FRACTIONS.len());
        for fraction in COVER_SAMPLE_FRACTIONS {
            let time_us = (request.duration_us as f64 * fraction) as u64;
            let sample = match self
                .extract_one(
                    &context.source,
                    &context.expected_identity,
                    time_us,
                    VideoFrameOutput::Gray160,
                    context.cancellation.clone(),
                )
                .await
            {
                Ok(bytes) => score_gray_frame(&bytes),
                Err(FrameExtractionError::DecodeFailed) => FrameSample::decode_failed(),
                Err(error) => return Err(map_extraction_error(error)),
            };
            samples.push(sample);
            sample_times.push(time_us);
            if sample.is_useful() {
                break;
            }
        }
        let selected = choose_cover_sample(&samples).ok_or(VideoThumbnailError::Unavailable)?;
        let png = self
            .extract_one(
                &context.source,
                &context.expected_identity,
                sample_times[selected],
                VideoFrameOutput::Png640,
                context.cancellation.clone(),
            )
            .await
            .map_err(map_extraction_error)?;
        let path = self.publish_if_current(context, &key, png).await?;
        Ok(artifact(path, context.session_id, context.generation, None))
    }

    pub async fn timeline(
        &self,
        request: TimelineThumbnailRequest,
    ) -> Result<VideoThumbnailArtifact, VideoThumbnailError> {
        let context = &request.context;
        self.validate_request(context)?;
        let bucket_us = quantize_timeline_time(request.time_us, request.duration_us);
        let key = VideoCacheKey::timeline(
            context.source.clone(),
            bucket_us,
            VIDEO_THUMBNAIL_ALGORITHM_VERSION,
        );
        if let Some(path) = self.cache.get_async(key).await.map_err(map_cache_error)? {
            self.after_cache_hit_for_test();
            self.validate_request(context)?;
            return Ok(artifact(
                path,
                context.session_id,
                context.generation,
                Some(request.request_id),
            ));
        }
        let png = self
            .extract_one(
                &context.source,
                &context.expected_identity,
                bucket_us,
                VideoFrameOutput::Png320,
                context.cancellation.clone(),
            )
            .await
            .map_err(map_extraction_error)?;
        let path = self.publish_if_current(context, &key, png).await?;
        Ok(artifact(
            path,
            context.session_id,
            context.generation,
            Some(request.request_id),
        ))
    }

    async fn extract_one(
        &self,
        source: &VideoSourceIdentity,
        expected_identity: &MediaFileIdentity,
        time_us: u64,
        output: VideoFrameOutput,
        cancellation: CancellationToken,
    ) -> Result<Vec<u8>, FrameExtractionError> {
        loop {
            self.wait_for_idle(&cancellation)
                .await
                .map_err(map_idle_error)?;
            let permit = tokio::select! {
                _ = cancellation.cancelled() => return Err(FrameExtractionError::Cancelled),
                permit = self.thumbnail_gate.acquire() => {
                    permit.map_err(|_| FrameExtractionError::Unavailable)?
                }
            };
            self.wait_for_idle(&cancellation)
                .await
                .map_err(map_idle_error)?;
            let child_cancellation = cancellation.child_token();
            let extraction = self.extractor.extract_frame(
                source.canonical_path(),
                expected_identity,
                time_us,
                output,
                child_cancellation.clone(),
            );
            tokio::pin!(extraction);
            let playback_active = wait_for_playback_active(self.playback_active.clone());
            tokio::pin!(playback_active);
            let result = tokio::select! {
                _ = cancellation.cancelled() => {
                    child_cancellation.cancel();
                    let _ = extraction.await;
                    Err(FrameExtractionError::Cancelled)
                }
                () = &mut playback_active => {
                    child_cancellation.cancel();
                    let _ = extraction.await;
                    drop(permit);
                    self.wait_for_idle(&cancellation).await.map_err(map_idle_error)?;
                    continue;
                }
                result = &mut extraction => result,
            };
            drop(permit);
            if *self.playback_active.borrow() {
                self.wait_for_idle(&cancellation)
                    .await
                    .map_err(map_idle_error)?;
                continue;
            }
            return result;
        }
    }

    async fn wait_for_idle(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), VideoThumbnailError> {
        let mut playback = self.playback_active.clone();
        while *playback.borrow_and_update() {
            tokio::select! {
                _ = cancellation.cancelled() => return Err(VideoThumbnailError::Cancelled),
                changed = playback.changed() => {
                    changed.map_err(|_| VideoThumbnailError::Unavailable)?;
                }
            }
        }
        Ok(())
    }

    fn validate_request(&self, context: &VideoThumbnailContext) -> Result<(), VideoThumbnailError> {
        if context.cancellation.is_cancelled() {
            return Err(VideoThumbnailError::Cancelled);
        }
        if !self
            .coordinator
            .is_publishable(context.session_id, context.generation)
        {
            return Err(VideoThumbnailError::Stale);
        }
        validate_source(&context.source, &context.expected_identity)
    }

    async fn publish_if_current(
        &self,
        context: &VideoThumbnailContext,
        key: &VideoCacheKey,
        png: Vec<u8>,
    ) -> Result<PathBuf, VideoThumbnailError> {
        let staged = self
            .cache
            .stage_atomic_async(*key, png)
            .await
            .map_err(map_cache_error)?;
        self.after_cache_stage_for_test();
        let path = self
            .coordinator
            .run_if_current(context.session_id, context.generation, || {
                self.cache
                    .commit_staged_if(staged, || {
                        if context.cancellation.is_cancelled() {
                            return Err(VideoThumbnailError::Cancelled);
                        }
                        validate_source(&context.source, &context.expected_identity)
                    })
                    .map_err(|error| match error {
                        VideoCacheCommitError::Cache(error) => map_cache_error(error),
                        VideoCacheCommitError::Rejected(error) => error,
                    })
            })
            .ok_or(VideoThumbnailError::Stale)??;
        self.cache
            .evict_to_budget_async()
            .await
            .map_err(map_cache_error)?;
        Ok(path)
    }

    #[cfg(test)]
    fn set_after_cache_hit_for_test(&mut self, hook: impl Fn() + Send + Sync + 'static) {
        self.after_cache_hit = Some(Arc::new(hook));
    }

    #[cfg(test)]
    fn set_after_cache_stage_for_test(&mut self, hook: impl Fn() + Send + Sync + 'static) {
        self.after_cache_stage = Some(Arc::new(hook));
    }

    fn after_cache_hit_for_test(&self) {
        #[cfg(test)]
        if let Some(hook) = &self.after_cache_hit {
            hook();
        }
    }

    fn after_cache_stage_for_test(&self) {
        #[cfg(test)]
        if let Some(hook) = &self.after_cache_stage {
            hook();
        }
    }
}

fn map_idle_error(error: VideoThumbnailError) -> FrameExtractionError {
    match error {
        VideoThumbnailError::Cancelled => FrameExtractionError::Cancelled,
        _ => FrameExtractionError::Unavailable,
    }
}

async fn wait_for_playback_active(mut playback: watch::Receiver<bool>) {
    loop {
        if *playback.borrow_and_update() {
            return;
        }
        if playback.changed().await.is_err() {
            std::future::pending().await
        }
    }
}

fn validate_source(
    source: &VideoSourceIdentity,
    expected_identity: &MediaFileIdentity,
) -> Result<(), VideoThumbnailError> {
    let path = source.canonical_path();
    if !path.is_absolute() {
        return Err(VideoThumbnailError::SourceChanged);
    }
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| VideoThumbnailError::SourceChanged)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() != source.size()
        || modified_ns(&metadata) != source.modified_ns()
        || path
            .canonicalize()
            .map_or(true, |canonical| canonical != path)
        || MediaFileIdentity::from_metadata(&metadata) != *expected_identity
    {
        return Err(VideoThumbnailError::SourceChanged);
    }
    Ok(())
}

fn modified_ns(metadata: &std::fs::Metadata) -> i128 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec())
    }
    #[cfg(not(unix))]
    {
        metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |duration| duration.as_nanos() as i128)
    }
}

fn score_gray_frame(bytes: &[u8]) -> FrameSample {
    if bytes.is_empty() {
        return FrameSample::decode_failed();
    }
    let count = bytes.len() as f64;
    let luminance = bytes
        .iter()
        .map(|byte| f64::from(*byte) / 255.0)
        .sum::<f64>()
        / count;
    let variance = bytes
        .iter()
        .map(|byte| {
            let difference = f64::from(*byte) / 255.0 - luminance;
            difference * difference
        })
        .sum::<f64>()
        / count;
    FrameSample::decoded(luminance, variance)
}

fn artifact(
    path: PathBuf,
    session_id: SessionId,
    generation: Generation,
    request_id: Option<VideoThumbnailRequestId>,
) -> VideoThumbnailArtifact {
    VideoThumbnailArtifact {
        path,
        session_id,
        generation,
        request_id,
    }
}

fn map_extraction_error(error: FrameExtractionError) -> VideoThumbnailError {
    match error {
        FrameExtractionError::DecodeFailed => VideoThumbnailError::Unavailable,
        FrameExtractionError::Cancelled => VideoThumbnailError::Cancelled,
        FrameExtractionError::SourceChanged => VideoThumbnailError::SourceChanged,
        FrameExtractionError::Unavailable => VideoThumbnailError::Unavailable,
    }
}

fn map_cache_error(_error: CacheError) -> VideoThumbnailError {
    VideoThumbnailError::Cache
}

#[cfg(test)]
mod publication_boundary_tests {
    use super::*;
    use async_trait::async_trait;
    use std::{fs, sync::Arc};

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nthumbnail";

    struct ImmediateExtractor;

    #[async_trait]
    impl VideoFrameExtractor for ImmediateExtractor {
        async fn extract_frame(
            &self,
            _canonical_path: &Path,
            _expected_identity: &MediaFileIdentity,
            _time_us: u64,
            _output: VideoFrameOutput,
            _cancellation: CancellationToken,
        ) -> Result<Vec<u8>, FrameExtractionError> {
            Ok(PNG.to_vec())
        }
    }

    struct Fixture {
        _source_directory: tempfile::TempDir,
        _cache_directory: tempfile::TempDir,
        source: VideoSourceIdentity,
        identity: MediaFileIdentity,
        cache: Arc<VideoCache>,
        coordinator: Arc<TaskCoordinator>,
        service: VideoThumbnailService,
    }

    impl Fixture {
        fn new() -> Self {
            let source_directory = tempfile::tempdir().unwrap();
            let path = source_directory.path().join("clip.mp4");
            fs::write(&path, b"fixture").unwrap();
            let path = path.canonicalize().unwrap();
            let metadata = fs::metadata(&path).unwrap();
            let source = VideoSourceIdentity::new(&path, metadata.len(), modified_ns(&metadata));
            let identity = MediaFileIdentity::from_metadata(&metadata);
            let cache_directory = tempfile::tempdir().unwrap();
            let cache = Arc::new(VideoCache::initialize(cache_directory.path()).unwrap());
            let coordinator = Arc::new(TaskCoordinator::default());
            let (_playback_tx, playback_rx) = watch::channel(false);
            let service = VideoThumbnailService::new(
                Arc::new(ImmediateExtractor),
                Arc::clone(&cache),
                Arc::clone(&coordinator),
                playback_rx,
            );
            Self {
                _source_directory: source_directory,
                _cache_directory: cache_directory,
                source,
                identity,
                cache,
                coordinator,
                service,
            }
        }
    }

    #[tokio::test]
    async fn cache_hit_revalidates_cancellation_after_cache_io() {
        let mut fixture = Fixture::new();
        let session = SessionId::new();
        let generation = fixture.coordinator.begin_session(session);
        let key = VideoCacheKey::timeline(
            fixture.source.clone(),
            1_000_000,
            VIDEO_THUMBNAIL_ALGORITHM_VERSION,
        );
        fixture.cache.put_atomic(&key, PNG).unwrap();
        let cancellation = CancellationToken::new();
        let cancel_after_hit = cancellation.clone();
        fixture
            .service
            .set_after_cache_hit_for_test(move || cancel_after_hit.cancel());

        let result = fixture
            .service
            .timeline(TimelineThumbnailRequest::new(
                VideoThumbnailContext::new(
                    session,
                    generation,
                    fixture.source,
                    fixture.identity,
                    cancellation,
                ),
                VideoThumbnailRequestId::new(),
                1_000_000,
                5_000_000,
            ))
            .await;

        assert_eq!(result, Err(VideoThumbnailError::Cancelled));
    }

    #[tokio::test]
    async fn cancellation_after_staging_prevents_atomic_cache_commit() {
        let mut fixture = Fixture::new();
        let session = SessionId::new();
        let generation = fixture.coordinator.begin_session(session);
        let cancellation = CancellationToken::new();
        let cancel_before_commit = cancellation.clone();
        fixture
            .service
            .set_after_cache_stage_for_test(move || cancel_before_commit.cancel());

        let result = fixture
            .service
            .timeline(TimelineThumbnailRequest::new(
                VideoThumbnailContext::new(
                    session,
                    generation,
                    fixture.source,
                    fixture.identity,
                    cancellation,
                ),
                VideoThumbnailRequestId::new(),
                1_000_000,
                5_000_000,
            ))
            .await;

        assert_eq!(result, Err(VideoThumbnailError::Cancelled));
        assert_eq!(fixture.cache.stats().unwrap().entry_count, 0);
    }

    #[tokio::test]
    async fn cache_hit_revalidates_generation_after_cache_io() {
        let mut fixture = Fixture::new();
        let session = SessionId::new();
        let generation = fixture.coordinator.begin_session(session);
        let key = VideoCacheKey::timeline(
            fixture.source.clone(),
            1_000_000,
            VIDEO_THUMBNAIL_ALGORITHM_VERSION,
        );
        fixture.cache.put_atomic(&key, PNG).unwrap();
        let coordinator = Arc::clone(&fixture.coordinator);
        fixture.service.set_after_cache_hit_for_test(move || {
            coordinator.bump_generation(session).unwrap();
        });

        let result = fixture
            .service
            .timeline(TimelineThumbnailRequest::new(
                VideoThumbnailContext::new(
                    session,
                    generation,
                    fixture.source,
                    fixture.identity,
                    CancellationToken::new(),
                ),
                VideoThumbnailRequestId::new(),
                1_000_000,
                5_000_000,
            ))
            .await;

        assert_eq!(result, Err(VideoThumbnailError::Stale));
    }

    #[tokio::test]
    async fn generation_change_after_staging_prevents_atomic_cache_commit() {
        let mut fixture = Fixture::new();
        let session = SessionId::new();
        let generation = fixture.coordinator.begin_session(session);
        let coordinator = Arc::clone(&fixture.coordinator);
        fixture.service.set_after_cache_stage_for_test(move || {
            coordinator.bump_generation(session).unwrap();
        });

        let result = fixture
            .service
            .timeline(TimelineThumbnailRequest::new(
                VideoThumbnailContext::new(
                    session,
                    generation,
                    fixture.source,
                    fixture.identity,
                    CancellationToken::new(),
                ),
                VideoThumbnailRequestId::new(),
                1_000_000,
                5_000_000,
            ))
            .await;

        assert_eq!(result, Err(VideoThumbnailError::Stale));
        assert_eq!(fixture.cache.stats().unwrap().entry_count, 0);
    }
}
