use crate::{
    dto::{VideoCacheStatsDto, VideoEventDto, VideoMediaDto, VideoSessionDto, VideoStateDto},
    error::{CommandError, ErrorCategory},
    state::{AuthorizedVideoSource, RuntimeError, VideoClosePort},
};
use std::{
    collections::{HashSet, VecDeque},
    future::Future,
    path::Path,
    str::FromStr,
    sync::{Arc, Mutex, MutexGuard},
};
use tokio_util::sync::CancellationToken;
use viewer_application::scheduler::TaskCoordinator;
use viewer_application::{
    EngineEvent, FrameDirection, PlaybackRate, SurfaceRect, VideoCommand, VideoCommandKind,
    VideoEngine, VideoPlaybackState, VideoPreviewService, VideoServiceError, VideoSource,
};
use viewer_domain::{SessionId, VideoThumbnailRequestId, search::Generation};
use viewer_infrastructure::{
    image_cache::ImageArtifactRegistry,
    video_cache::{CacheError, VideoCache, VideoSourceIdentity},
    video_thumbnail::{
        CoverThumbnailRequest, TimelineThumbnailRequest, VideoThumbnailContext,
        VideoThumbnailError, VideoThumbnailService as NativeThumbnailService,
        quantize_timeline_time,
    },
};
use viewer_video_mpv::{BundledMediaTools, MediaFileIdentity};

pub struct VideoRuntime<E: VideoEngine> {
    engine: Arc<E>,
    preview: VideoPreviewService<E>,
    cache: Option<VideoCache>,
    events: Option<Arc<dyn VideoEventPort>>,
    thumbnails: Option<Arc<dyn TimelineThumbnailPort>>,
    playback_activity: Option<Arc<dyn PlaybackActivityPort>>,
    active_source: Mutex<Option<AuthorizedVideoSource>>,
    thumbnail_request: Mutex<Option<(u64, String, CancellationToken)>>,
    open_attempts: Mutex<VideoOpenAttemptState>,
    transition: tokio::sync::Mutex<()>,
}

#[derive(Clone, Debug)]
pub struct VideoOpenAttempt {
    id: String,
    cancellation: CancellationToken,
}

impl VideoOpenAttempt {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn cancellation(&self) -> &CancellationToken {
        &self.cancellation
    }
}

#[derive(Default)]
struct VideoOpenAttemptState {
    current: Option<VideoOpenAttempt>,
    terminal: HashSet<String>,
    terminal_order: VecDeque<String>,
}

const VIDEO_OPEN_ATTEMPT_TERMINAL_LIMIT: usize = 1_024;

impl VideoOpenAttemptState {
    fn record_terminal(&mut self, attempt_id: String) {
        if !self.terminal.insert(attempt_id.clone()) {
            return;
        }
        self.terminal_order.push_back(attempt_id);
        while self.terminal_order.len() > VIDEO_OPEN_ATTEMPT_TERMINAL_LIMIT {
            if let Some(expired) = self.terminal_order.pop_front() {
                self.terminal.remove(&expired);
            }
        }
    }
}

pub trait VideoEventPort: Send + Sync {
    fn publish(&self, event: VideoEventDto);
}

pub trait PlaybackActivityPort: Send + Sync {
    fn set_active(&self, generation: u64, active: bool);
}

#[derive(Clone)]
pub struct TimelineThumbnailBridgeRequest {
    pub source: AuthorizedVideoSource,
    pub generation: u64,
    pub request_id: String,
    pub time_us: u64,
    pub cancellation: CancellationToken,
}

#[derive(Clone)]
pub struct CoverThumbnailBridgeRequest {
    pub source: AuthorizedVideoSource,
    pub cancellation: CancellationToken,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelineThumbnailBridgeResult {
    pub request_id: String,
    pub bucket_us: u64,
    pub artifact_url: String,
}

#[async_trait::async_trait]
pub trait TimelineThumbnailPort: Send + Sync {
    async fn request_cover(
        &self,
        request: CoverThumbnailBridgeRequest,
    ) -> Result<String, VideoCommandError>;

    async fn request(
        &self,
        request: TimelineThumbnailBridgeRequest,
    ) -> Result<TimelineThumbnailBridgeResult, VideoCommandError>;

    fn revoke_session(&self, _session_id: viewer_domain::SessionId) {}
}

pub struct NativeTimelineThumbnailBridge {
    service: NativeThumbnailService,
    cache: VideoCache,
    registry: Arc<ImageArtifactRegistry>,
    coordinator: Arc<TaskCoordinator>,
    playback: tokio::sync::watch::Sender<bool>,
    playback_generation: Mutex<Option<u64>>,
    thumbnail_session: Mutex<Option<(SessionId, Generation)>>,
}

impl NativeTimelineThumbnailBridge {
    pub fn new(
        tools: BundledMediaTools,
        cache: VideoCache,
        registry: Arc<ImageArtifactRegistry>,
    ) -> Self {
        let coordinator = Arc::new(TaskCoordinator::default());
        let (playback, playback_rx) = tokio::sync::watch::channel(false);
        let service = NativeThumbnailService::new(
            Arc::new(tools),
            Arc::new(cache.clone()),
            Arc::clone(&coordinator),
            playback_rx,
        );
        Self {
            service,
            cache,
            registry,
            coordinator,
            playback,
            playback_generation: Mutex::new(None),
            thumbnail_session: Mutex::new(None),
        }
    }

    fn thumbnail_generation(&self, session_id: SessionId) -> Generation {
        let mut current = lock(&self.thumbnail_session);
        if let Some((active_session, generation)) = *current
            && active_session == session_id
            && self.coordinator.is_publishable(session_id, generation)
        {
            return generation;
        }
        let generation = self.coordinator.begin_session(session_id);
        *current = Some((session_id, generation));
        generation
    }
}

impl PlaybackActivityPort for NativeTimelineThumbnailBridge {
    fn set_active(&self, generation: u64, active: bool) {
        let mut current = lock(&self.playback_generation);
        if current.is_some_and(|current| current > generation) {
            return;
        }
        *current = Some(generation);
        self.playback.send_replace(active);
    }
}

#[async_trait::async_trait]
impl TimelineThumbnailPort for NativeTimelineThumbnailBridge {
    async fn request_cover(
        &self,
        request: CoverThumbnailBridgeRequest,
    ) -> Result<String, VideoCommandError> {
        let duration_us = request
            .source
            .metadata
            .duration_us
            .ok_or(VideoCommandError::ThumbnailUnavailable)?;
        let metadata = std::fs::symlink_metadata(&request.source.canonical_path)
            .map_err(|_| VideoCommandError::ThumbnailUnavailable)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(VideoCommandError::ThumbnailUnavailable);
        }
        let source = VideoSourceIdentity::new(
            &request.source.canonical_path,
            metadata.len(),
            metadata_modified_ns(&metadata),
        );
        let identity = MediaFileIdentity::from_metadata(&metadata);
        let thumbnail_generation = self.thumbnail_generation(request.source.session_id);
        let artifact = self
            .service
            .cover(CoverThumbnailRequest::new(
                VideoThumbnailContext::new(
                    request.source.session_id,
                    thumbnail_generation,
                    source,
                    identity,
                    request.cancellation.clone(),
                ),
                duration_us,
            ))
            .await
            .map_err(map_thumbnail_error)?;
        if request.cancellation.is_cancelled() {
            return Err(VideoCommandError::ThumbnailCancelled);
        }
        crate::state::register_video_png_url(
            &self.registry,
            &self.coordinator,
            &self.cache,
            &artifact,
        )
        .map_err(|_| VideoCommandError::ThumbnailCancelled)
    }

    async fn request(
        &self,
        request: TimelineThumbnailBridgeRequest,
    ) -> Result<TimelineThumbnailBridgeResult, VideoCommandError> {
        let request_id = VideoThumbnailRequestId::from_str(&request.request_id)
            .map_err(|_| VideoCommandError::InvalidThumbnailRequest)?;
        let duration_us = request
            .source
            .metadata
            .duration_us
            .ok_or(VideoCommandError::ThumbnailUnavailable)?;
        let metadata = std::fs::symlink_metadata(&request.source.canonical_path)
            .map_err(|_| VideoCommandError::ThumbnailUnavailable)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(VideoCommandError::ThumbnailUnavailable);
        }
        let source = VideoSourceIdentity::new(
            &request.source.canonical_path,
            metadata.len(),
            metadata_modified_ns(&metadata),
        );
        let identity = MediaFileIdentity::from_metadata(&metadata);
        let thumbnail_generation = self.thumbnail_generation(request.source.session_id);
        let artifact = self
            .service
            .timeline(TimelineThumbnailRequest::new(
                VideoThumbnailContext::new(
                    request.source.session_id,
                    thumbnail_generation,
                    source,
                    identity,
                    request.cancellation.clone(),
                ),
                request_id,
                request.time_us,
                duration_us,
            ))
            .await
            .map_err(map_thumbnail_error)?;
        if request.cancellation.is_cancelled() {
            return Err(VideoCommandError::ThumbnailCancelled);
        }
        let artifact_url = crate::state::register_video_png_url(
            &self.registry,
            &self.coordinator,
            &self.cache,
            &artifact,
        )
        .map_err(|_| VideoCommandError::ThumbnailCancelled)?;
        Ok(TimelineThumbnailBridgeResult {
            request_id: request.request_id,
            bucket_us: quantize_timeline_time(request.time_us, duration_us),
            artifact_url,
        })
    }

    fn revoke_session(&self, session_id: viewer_domain::SessionId) {
        self.coordinator.cancel_session(session_id);
        let mut current = lock(&self.thumbnail_session);
        if current.is_some_and(|(active_session, _)| active_session == session_id) {
            *current = None;
        }
        self.registry.remove_timeline_video_artifacts(session_id);
    }
}

impl<E: VideoEngine> VideoRuntime<E> {
    pub fn new(engine: Arc<E>) -> Self {
        Self {
            preview: VideoPreviewService::new(Arc::clone(&engine)),
            engine,
            cache: None,
            events: None,
            thumbnails: None,
            playback_activity: None,
            active_source: Mutex::new(None),
            thumbnail_request: Mutex::new(None),
            open_attempts: Mutex::new(VideoOpenAttemptState::default()),
            transition: tokio::sync::Mutex::new(()),
        }
    }

    pub fn with_cache(engine: Arc<E>, app_cache_root: &Path) -> Result<Self, CacheError> {
        Ok(Self {
            preview: VideoPreviewService::new(Arc::clone(&engine)),
            engine,
            cache: Some(VideoCache::initialize(app_cache_root)?),
            events: None,
            thumbnails: None,
            playback_activity: None,
            active_source: Mutex::new(None),
            thumbnail_request: Mutex::new(None),
            open_attempts: Mutex::new(VideoOpenAttemptState::default()),
            transition: tokio::sync::Mutex::new(()),
        })
    }

    pub fn with_cache_and_events(
        engine: Arc<E>,
        app_cache_root: &Path,
        events: Arc<dyn VideoEventPort>,
    ) -> Result<Self, CacheError> {
        Ok(Self {
            preview: VideoPreviewService::new(Arc::clone(&engine)),
            engine,
            cache: Some(VideoCache::initialize(app_cache_root)?),
            events: Some(events),
            thumbnails: None,
            playback_activity: None,
            active_source: Mutex::new(None),
            thumbnail_request: Mutex::new(None),
            open_attempts: Mutex::new(VideoOpenAttemptState::default()),
            transition: tokio::sync::Mutex::new(()),
        })
    }

    pub fn with_bridges(
        engine: Arc<E>,
        events: Arc<dyn VideoEventPort>,
        thumbnails: Arc<dyn TimelineThumbnailPort>,
    ) -> Self {
        Self {
            preview: VideoPreviewService::new(Arc::clone(&engine)),
            engine,
            cache: None,
            events: Some(events),
            thumbnails: Some(thumbnails),
            playback_activity: None,
            active_source: Mutex::new(None),
            thumbnail_request: Mutex::new(None),
            open_attempts: Mutex::new(VideoOpenAttemptState::default()),
            transition: tokio::sync::Mutex::new(()),
        }
    }

    pub fn with_native_bridges(
        engine: Arc<E>,
        cache: VideoCache,
        events: Arc<dyn VideoEventPort>,
        thumbnails: Arc<dyn TimelineThumbnailPort>,
        playback_activity: Arc<dyn PlaybackActivityPort>,
    ) -> Self {
        Self {
            preview: VideoPreviewService::new(Arc::clone(&engine)),
            engine,
            cache: Some(cache),
            events: Some(events),
            thumbnails: Some(thumbnails),
            playback_activity: Some(playback_activity),
            active_source: Mutex::new(None),
            thumbnail_request: Mutex::new(None),
            open_attempts: Mutex::new(VideoOpenAttemptState::default()),
            transition: tokio::sync::Mutex::new(()),
        }
    }

    pub fn with_playback_activity(
        engine: Arc<E>,
        playback_activity: Arc<dyn PlaybackActivityPort>,
    ) -> Self {
        Self {
            preview: VideoPreviewService::new(Arc::clone(&engine)),
            engine,
            cache: None,
            events: None,
            thumbnails: None,
            playback_activity: Some(playback_activity),
            active_source: Mutex::new(None),
            thumbnail_request: Mutex::new(None),
            open_attempts: Mutex::new(VideoOpenAttemptState::default()),
            transition: tokio::sync::Mutex::new(()),
        }
    }

    pub fn engine(&self) -> &Arc<E> {
        &self.engine
    }

    pub fn begin_open_attempt(
        &self,
        attempt_id: impl Into<String>,
    ) -> Result<VideoOpenAttempt, VideoCommandError> {
        let attempt_id = attempt_id.into();
        let mut attempts = lock(&self.open_attempts);
        if attempts.terminal.contains(&attempt_id)
            || attempts
                .current
                .as_ref()
                .is_some_and(|attempt| attempt.id == attempt_id)
        {
            return Err(VideoCommandError::StaleOpenAttempt);
        }
        if let Some(current) = attempts.current.take() {
            current.cancellation.cancel();
            attempts.record_terminal(current.id);
        }
        let attempt = VideoOpenAttempt {
            id: attempt_id,
            cancellation: CancellationToken::new(),
        };
        attempts.current = Some(attempt.clone());
        Ok(attempt)
    }

    pub fn open_attempt(&self, attempt_id: &str) -> Result<VideoOpenAttempt, VideoCommandError> {
        let attempts = lock(&self.open_attempts);
        attempts
            .current
            .as_ref()
            .filter(|attempt| attempt.id == attempt_id && !attempt.cancellation.is_cancelled())
            .cloned()
            .ok_or(VideoCommandError::StaleOpenAttempt)
    }

    pub fn ensure_open_attempt(&self, attempt: &VideoOpenAttempt) -> Result<(), VideoCommandError> {
        self.open_attempt(&attempt.id).map(|_| ())
    }

    pub fn cancel_open_attempt(&self, attempt_id: &str) -> bool {
        let mut attempts = lock(&self.open_attempts);
        if let Some(current) = attempts.current.take() {
            if current.id == attempt_id {
                current.cancellation.cancel();
                attempts.record_terminal(current.id);
                return true;
            }
            attempts.current = Some(current);
        }
        let was_terminal = attempts.terminal.contains(attempt_id);
        attempts.record_terminal(attempt_id.to_owned());
        !was_terminal
    }

    pub fn finish_open_attempt(&self, attempt_id: &str) {
        let mut attempts = lock(&self.open_attempts);
        if attempts
            .current
            .as_ref()
            .is_some_and(|attempt| attempt.id == attempt_id)
        {
            let completed = attempts
                .current
                .take()
                .expect("matching open attempt exists");
            attempts.record_terminal(completed.id);
        }
    }

    pub async fn open_authorized(
        &self,
        source: AuthorizedVideoSource,
    ) -> Result<VideoSessionDto, VideoCommandError> {
        let _transition = self.transition.lock().await;
        let result = self.open_authorized_inner(source).await;
        if result.is_err() {
            let _ = self.close_active_inner().await;
        }
        result
    }

    pub async fn replace_authorized<Prepare, Prepared>(
        &self,
        source: AuthorizedVideoSource,
        prepare: Prepare,
    ) -> Result<VideoSessionDto, VideoCommandError>
    where
        Prepare: FnOnce() -> Prepared,
        Prepared: Future<Output = Result<(), VideoCommandError>>,
    {
        let _transition = self.transition.lock().await;
        self.close_active_inner().await?;
        if let Err(error) = prepare().await {
            let _ = self.engine.close(0).await;
            return Err(error);
        }
        let result = self.open_authorized_inner(source).await;
        if result.is_err() {
            let _ = self.close_active_inner().await;
        }
        result
    }

    pub async fn replace_authorized_for_attempt<Prepare, Prepared>(
        &self,
        attempt: &VideoOpenAttempt,
        source: AuthorizedVideoSource,
        prepare: Prepare,
    ) -> Result<VideoSessionDto, VideoCommandError>
    where
        Prepare: FnOnce() -> Prepared,
        Prepared: Future<Output = Result<(), VideoCommandError>>,
    {
        let _transition = self.transition.lock().await;
        self.ensure_open_attempt(attempt)?;
        self.close_active_inner().await?;
        self.ensure_open_attempt(attempt)?;
        if let Err(error) = prepare().await {
            let _ = self.engine.close(0).await;
            return Err(error);
        }
        if let Err(error) = self.ensure_open_attempt(attempt) {
            let _ = self.engine.close(0).await;
            return Err(error);
        }
        let result = self.open_authorized_inner(source).await;
        if result.is_err() {
            let _ = self.close_active_inner().await;
            return result;
        }
        if let Err(error) = self.ensure_open_attempt(attempt) {
            let _ = self.close_active_inner().await;
            return Err(error);
        }
        result
    }

    async fn open_authorized_inner(
        &self,
        source: AuthorizedVideoSource,
    ) -> Result<VideoSessionDto, VideoCommandError> {
        let media = VideoMediaDto::from(&source.metadata);
        let retained_source = source.clone();
        let generation = self
            .preview
            .open(VideoSource {
                entity_id: source.entity_id,
                canonical_path: source.canonical_path,
            })
            .await
            .map_err(VideoCommandError::from)?;
        self.preview
            .handle_engine_event(
                generation,
                EngineEvent::Prepared(viewer_application::VideoMedia {
                    duration_us: source.metadata.duration_us,
                    display_width: source.metadata.display_width,
                    display_height: source.metadata.display_height,
                    rotation_degrees: source.metadata.rotation_degrees,
                }),
            )
            .await
            .map_err(VideoCommandError::from)?;
        let session_id = self
            .preview
            .snapshot()
            .session_id
            .expect("successful open creates a video session")
            .to_string();
        let session = VideoSessionDto {
            generation,
            session_id,
            media,
        };
        *lock(&self.active_source) = Some(retained_source);
        self.publish_playback_activity(generation);
        self.publish(VideoEventDto::Prepared {
            generation,
            media: session.media,
        });
        self.publish_state(generation);
        Ok(session)
    }

    pub fn active_generation(&self) -> Option<u64> {
        let snapshot = self.preview.snapshot();
        snapshot.session_id.map(|_| snapshot.generation)
    }

    pub fn playback_state(&self) -> VideoPlaybackState {
        self.preview.snapshot().state
    }

    pub fn ensure_generation(&self, generation: u64) -> Result<(), VideoCommandError> {
        if self.active_generation() == Some(generation) {
            Ok(())
        } else {
            Err(VideoCommandError::StaleGeneration)
        }
    }

    pub async fn close(&self, generation: u64) -> Result<(), VideoCommandError> {
        let _transition = self.transition.lock().await;
        self.close_inner(generation).await
    }

    async fn close_inner(&self, generation: u64) -> Result<(), VideoCommandError> {
        self.ensure_generation(generation)?;
        let source = lock(&self.active_source).clone();
        self.cancel_thumbnail_request();
        let result = self
            .preview
            .execute(VideoCommand {
                generation,
                kind: VideoCommandKind::Close,
            })
            .await;
        self.revoke_thumbnail_artifacts(source.as_ref());
        *lock(&self.active_source) = None;
        self.set_playback_activity(generation, false);
        self.publish(VideoEventDto::Closed { generation });
        result.map_err(VideoCommandError::from)
    }

    pub async fn close_active(&self) -> Result<(), VideoCommandError> {
        let _transition = self.transition.lock().await;
        self.close_active_inner().await
    }

    async fn close_active_inner(&self) -> Result<(), VideoCommandError> {
        let generation = self.active_generation();
        let source = lock(&self.active_source).clone();
        self.cancel_thumbnail_request();
        let result = self.preview.close().await;
        self.revoke_thumbnail_artifacts(source.as_ref());
        *lock(&self.active_source) = None;
        if let Some(generation) = generation {
            self.set_playback_activity(generation, false);
            self.publish(VideoEventDto::Closed { generation });
        }
        result.map_err(VideoCommandError::from)
    }

    pub async fn play(&self, generation: u64) -> Result<(), VideoCommandError> {
        self.execute(generation, VideoCommandKind::Play).await
    }

    pub async fn pause(&self, generation: u64) -> Result<(), VideoCommandError> {
        self.execute(generation, VideoCommandKind::Pause).await
    }

    pub async fn seek(&self, generation: u64, time_us: u64) -> Result<(), VideoCommandError> {
        self.execute(generation, VideoCommandKind::Seek(time_us))
            .await
    }

    pub async fn step(
        &self,
        generation: u64,
        direction: FrameDirection,
    ) -> Result<(), VideoCommandError> {
        self.execute(generation, VideoCommandKind::Step(direction))
            .await
    }

    pub async fn set_volume(&self, generation: u64, percent: u8) -> Result<(), VideoCommandError> {
        self.execute(generation, VideoCommandKind::SetVolume(percent))
            .await
    }

    pub async fn set_muted(&self, generation: u64, muted: bool) -> Result<(), VideoCommandError> {
        self.execute(generation, VideoCommandKind::SetMuted(muted))
            .await
    }

    pub async fn set_rate(
        &self,
        generation: u64,
        rate: PlaybackRate,
    ) -> Result<(), VideoCommandError> {
        self.execute(generation, VideoCommandKind::SetRate(rate))
            .await
    }

    pub async fn set_surface_rect(
        &self,
        generation: u64,
        rect: SurfaceRect,
    ) -> Result<(), VideoCommandError> {
        self.execute(generation, VideoCommandKind::SetSurfaceRect(rect))
            .await
    }

    pub fn cache_stats(&self) -> Result<VideoCacheStatsDto, VideoCommandError> {
        self.cache
            .as_ref()
            .ok_or(VideoCommandError::CacheUnavailable)?
            .stats()
            .map(Into::into)
            .map_err(|_| VideoCommandError::CacheUnavailable)
    }

    pub async fn cache_clear(&self) -> Result<VideoCacheStatsDto, VideoCommandError> {
        self.cache
            .as_ref()
            .ok_or(VideoCommandError::CacheUnavailable)?
            .clear()
            .await
            .map(Into::into)
            .map_err(|_| VideoCommandError::CacheUnavailable)
    }

    pub async fn request_thumbnail(
        &self,
        generation: u64,
        request_id: String,
        time_us: u64,
    ) -> Result<(), VideoCommandError> {
        self.ensure_generation(generation)?;
        let source = lock(&self.active_source)
            .clone()
            .ok_or(VideoCommandError::NoActiveSession)?;
        let thumbnails = self
            .thumbnails
            .as_ref()
            .ok_or(VideoCommandError::ThumbnailUnavailable)?
            .clone();
        let cancellation = CancellationToken::new();
        {
            let mut pending = lock(&self.thumbnail_request);
            if let Some((_, _, old)) = pending.take() {
                old.cancel();
            }
            *pending = Some((generation, request_id.clone(), cancellation.clone()));
        }
        let result = thumbnails
            .request(TimelineThumbnailBridgeRequest {
                source,
                generation,
                request_id: request_id.clone(),
                time_us,
                cancellation,
            })
            .await?;
        self.ensure_generation(generation)?;
        let publishable = {
            let mut pending = lock(&self.thumbnail_request);
            let matches =
                pending
                    .as_ref()
                    .is_some_and(|(pending_generation, pending_id, token)| {
                        *pending_generation == generation
                            && pending_id == &result.request_id
                            && !token.is_cancelled()
                    });
            if matches {
                pending.take();
            }
            matches
        };
        if !publishable {
            return Err(VideoCommandError::ThumbnailCancelled);
        }
        self.publish(VideoEventDto::TimelineThumbnailReady {
            generation,
            request_id: result.request_id,
            bucket_us: result.bucket_us,
            artifact_url: result.artifact_url,
        });
        Ok(())
    }

    pub async fn request_cover(
        &self,
        source: AuthorizedVideoSource,
    ) -> Result<String, VideoCommandError> {
        let thumbnails = self
            .thumbnails
            .as_ref()
            .ok_or(VideoCommandError::ThumbnailUnavailable)?
            .clone();
        thumbnails
            .request_cover(CoverThumbnailBridgeRequest {
                source,
                cancellation: CancellationToken::new(),
            })
            .await
    }

    pub async fn handle_engine_event(&self, generation: u64, event: EngineEvent) {
        let _transition = self.transition.lock().await;
        if self.ensure_generation(generation).is_err()
            || self
                .preview
                .handle_engine_event(generation, event.clone())
                .await
                .is_err()
            || self.ensure_generation(generation).is_err()
        {
            return;
        }
        match event {
            EngineEvent::Prepared(media) => self.publish(VideoEventDto::Prepared {
                generation,
                media: media.into(),
            }),
            EngineEvent::FirstFrameReady => {
                self.publish(VideoEventDto::FirstFrameReady { generation });
            }
            EngineEvent::TimeChanged {
                time_us,
                duration_us,
            } => {
                self.publish(VideoEventDto::Progress {
                    generation,
                    time_us,
                    duration_us: duration_us.or(self.preview.snapshot().duration_us),
                });
            }
            EngineEvent::SeekCompleted { time_us } | EngineEvent::FrameStepped { time_us } => {
                self.publish(VideoEventDto::Progress {
                    generation,
                    time_us,
                    duration_us: self.preview.snapshot().duration_us,
                });
            }
            EngineEvent::Ended => self.publish(VideoEventDto::Ended { generation }),
            EngineEvent::Failed(failure) => self.publish(VideoEventDto::Failed {
                generation,
                error: failure.into(),
            }),
        }
        self.publish_playback_activity(generation);
        self.publish_state(generation);
    }

    async fn execute(
        &self,
        generation: u64,
        kind: VideoCommandKind,
    ) -> Result<(), VideoCommandError> {
        let _transition = self.transition.lock().await;
        self.ensure_generation(generation)?;
        let result = self
            .preview
            .execute(VideoCommand { generation, kind })
            .await;
        self.publish_playback_activity(generation);
        result.map_err(VideoCommandError::from)?;
        match kind {
            VideoCommandKind::SetVolume(_)
            | VideoCommandKind::SetMuted(_)
            | VideoCommandKind::SetRate(_) => {
                let snapshot = self.preview.snapshot();
                self.publish(VideoEventDto::SettingsChanged {
                    generation,
                    volume_percent: snapshot.volume_percent,
                    muted: snapshot.muted,
                    rate: playback_rate_f32(snapshot.rate),
                });
            }
            VideoCommandKind::Close => {
                unreachable!("close uses its cleanup-preserving path")
            }
            VideoCommandKind::Play
            | VideoCommandKind::Pause
            | VideoCommandKind::Seek(_)
            | VideoCommandKind::Step(_)
            | VideoCommandKind::SetSurfaceRect(_) => {}
        }
        self.publish_state(generation);
        Ok(())
    }

    pub fn publish_fullscreen(&self, generation: u64, fullscreen: bool) {
        self.publish(VideoEventDto::FullscreenChanged {
            generation,
            fullscreen,
        });
    }

    pub async fn set_fullscreen_guarded(
        &self,
        generation: u64,
        fullscreen: bool,
        mutate_and_confirm: impl FnOnce(bool) -> Result<bool, CommandError>,
    ) -> Result<(), CommandError> {
        let _transition = self.transition.lock().await;
        self.ensure_generation(generation)
            .map_err(CommandError::from)?;
        let confirmed = mutate_and_confirm(fullscreen)?;
        if confirmed != fullscreen {
            return Err(CommandError::new(
                "video_fullscreen_unavailable",
                ErrorCategory::Environment,
                "无法确认视频全屏状态。",
                true,
            ));
        }
        self.ensure_generation(generation)
            .map_err(CommandError::from)?;
        self.publish(VideoEventDto::FullscreenChanged {
            generation,
            fullscreen: confirmed,
        });
        Ok(())
    }

    fn publish_state(&self, generation: u64) {
        self.publish(VideoEventDto::StateChanged {
            generation,
            state: VideoStateDto::from(&self.preview.snapshot().state),
        });
    }

    fn publish_playback_activity(&self, generation: u64) {
        self.set_playback_activity(
            generation,
            matches!(self.preview.snapshot().state, VideoPlaybackState::Playing),
        );
    }

    fn set_playback_activity(&self, generation: u64, active: bool) {
        if let Some(playback_activity) = self.playback_activity.as_ref() {
            playback_activity.set_active(generation, active);
        }
    }

    fn publish(&self, event: VideoEventDto) {
        if let Some(events) = self.events.as_ref() {
            events.publish(event);
        }
    }

    fn cancel_thumbnail_request(&self) {
        if let Some((_, _, cancellation)) = lock(&self.thumbnail_request).take() {
            cancellation.cancel();
        }
    }

    fn revoke_thumbnail_artifacts(&self, source: Option<&AuthorizedVideoSource>) {
        if let (Some(thumbnails), Some(source)) = (self.thumbnails.as_ref(), source) {
            thumbnails.revoke_session(source.session_id);
        }
    }
}

#[async_trait::async_trait]
impl<E: VideoEngine> VideoClosePort for VideoRuntime<E> {
    async fn close_video(&self) -> Result<(), RuntimeError> {
        self.close_active()
            .await
            .map_err(|_| RuntimeError::CloseFailed)
    }
}

fn playback_rate_f32(rate: PlaybackRate) -> f32 {
    match rate {
        PlaybackRate::Half => 0.5,
        PlaybackRate::ThreeQuarters => 0.75,
        PlaybackRate::Normal => 1.0,
        PlaybackRate::OneAndQuarter => 1.25,
        PlaybackRate::OneAndHalf => 1.5,
        PlaybackRate::Double => 2.0,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum VideoCommandError {
    #[error("video open attempt is stale")]
    StaleOpenAttempt,
    #[error("video command belongs to a stale generation")]
    StaleGeneration,
    #[error("there is no active video session")]
    NoActiveSession,
    #[error("video command is invalid in the current state")]
    InvalidState,
    #[error("video volume is invalid")]
    InvalidVolume,
    #[error("the video engine is unavailable")]
    EngineUnavailable,
    #[error("the video cache is unavailable")]
    CacheUnavailable,
    #[error("the video thumbnail request was cancelled")]
    ThumbnailCancelled,
    #[error("the video thumbnail service is unavailable")]
    ThumbnailUnavailable,
    #[error("the video thumbnail request identifier is invalid")]
    InvalidThumbnailRequest,
}

impl VideoCommandError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::StaleOpenAttempt => "stale_video_open_attempt",
            Self::StaleGeneration => "stale_video_generation",
            Self::NoActiveSession => "video_not_open",
            Self::InvalidState => "invalid_video_state",
            Self::InvalidVolume => "invalid_video_volume",
            Self::EngineUnavailable => "video_engine_unavailable",
            Self::CacheUnavailable => "video_cache_unavailable",
            Self::ThumbnailCancelled => "video_thumbnail_cancelled",
            Self::ThumbnailUnavailable => "video_thumbnail_unavailable",
            Self::InvalidThumbnailRequest => "invalid_video_thumbnail_request",
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn map_thumbnail_error(error: VideoThumbnailError) -> VideoCommandError {
    match error {
        VideoThumbnailError::Cancelled | VideoThumbnailError::Stale => {
            VideoCommandError::ThumbnailCancelled
        }
        VideoThumbnailError::SourceChanged
        | VideoThumbnailError::Unavailable
        | VideoThumbnailError::Cache => VideoCommandError::ThumbnailUnavailable,
    }
}

fn metadata_modified_ns(metadata: &std::fs::Metadata) -> i128 {
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

impl From<VideoServiceError> for VideoCommandError {
    fn from(error: VideoServiceError) -> Self {
        match error {
            VideoServiceError::StaleGeneration => Self::StaleGeneration,
            VideoServiceError::NoActiveSession => Self::NoActiveSession,
            VideoServiceError::InvalidState => Self::InvalidState,
            VideoServiceError::InvalidVolume => Self::InvalidVolume,
            VideoServiceError::GenerationExhausted | VideoServiceError::Engine(_) => {
                Self::EngineUnavailable
            }
        }
    }
}

pub fn state_accepts_fullscreen(state: &VideoPlaybackState) -> bool {
    !matches!(
        state,
        VideoPlaybackState::Idle | VideoPlaybackState::Closing | VideoPlaybackState::Failed(_)
    )
}
