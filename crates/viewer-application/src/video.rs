use crate::{browse::BrowserFile, ports::VideoEngine};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
};
use tokio::sync::Mutex as AsyncMutex;
use viewer_domain::{EntityId, VideoSessionId, file::FileKind, video::VideoFailureKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VideoSource {
    pub entity_id: EntityId,
    pub canonical_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineOpenRequest {
    pub generation: u64,
    pub session_id: VideoSessionId,
    pub source: VideoSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameDirection {
    Backward,
    Forward,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlaybackRate {
    Half,
    ThreeQuarters,
    #[default]
    Normal,
    OneAndQuarter,
    OneAndHalf,
    Double,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeekIntent {
    Preview,
    Commit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeekRequest {
    pub request_id: u64,
    pub time_us: u64,
    pub intent: SeekIntent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VideoMedia {
    pub duration_us: Option<u64>,
    pub display_width: Option<u32>,
    pub display_height: Option<u32>,
    pub rotation_degrees: i16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineEvent {
    Prepared(VideoMedia),
    FirstFrameReady,
    TimeChanged {
        time_us: u64,
        duration_us: Option<u64>,
    },
    Ended,
    FrameStepped {
        time_us: u64,
    },
    SeekCompleted {
        request_id: u64,
        time_us: u64,
    },
    Failed(VideoFailureKind),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VideoEvent {
    pub generation: u64,
    pub event: EngineEvent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VideoCommandKind {
    Play,
    Pause,
    Seek(SeekRequest),
    Step(FrameDirection),
    SetVolume(u8),
    SetMuted(bool),
    SetRate(PlaybackRate),
    SetSurfaceRect(SurfaceRect),
    Close,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VideoCommand {
    pub generation: u64,
    pub kind: VideoCommandKind,
}

impl VideoCommand {
    pub const fn play(generation: u64) -> Self {
        Self {
            generation,
            kind: VideoCommandKind::Play,
        }
    }

    pub const fn seek(generation: u64, request: SeekRequest) -> Self {
        Self {
            generation,
            kind: VideoCommandKind::Seek(SeekRequest {
                intent: SeekIntent::Commit,
                ..request
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum VideoEngineError {
    #[error("video engine initialization failed")]
    Initialization,
    #[error("video engine rejected a stale generation")]
    StaleGeneration,
    #[error("video decode failed")]
    Decode,
    #[error("video render surface failed")]
    RenderSurface,
    #[error("video engine is unavailable")]
    Unavailable,
}

impl VideoEngineError {
    const fn failure_kind(self) -> VideoFailureKind {
        match self {
            Self::Initialization | Self::StaleGeneration | Self::Unavailable => {
                VideoFailureKind::EngineInitialization
            }
            Self::Decode => VideoFailureKind::DecodeFallbackFailed,
            Self::RenderSurface => VideoFailureKind::RenderSurface,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VideoPlaybackState {
    Idle,
    Preparing,
    Ready,
    Playing,
    Paused,
    Seeking,
    FrameStepping,
    Ended,
    Failed(VideoFailureKind),
    Closing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VideoPreviewSnapshot {
    pub generation: u64,
    pub session_id: Option<VideoSessionId>,
    pub state: VideoPlaybackState,
    pub time_us: u64,
    pub duration_us: Option<u64>,
    pub volume_percent: u8,
    pub muted: bool,
    pub rate: PlaybackRate,
}

impl Default for VideoPreviewSnapshot {
    fn default() -> Self {
        Self {
            generation: 0,
            session_id: None,
            state: VideoPlaybackState::Idle,
            time_us: 0,
            duration_us: None,
            volume_percent: 100,
            muted: false,
            rate: PlaybackRate::Normal,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum VideoServiceError {
    #[error("video session generation was exhausted")]
    GenerationExhausted,
    #[error("no active video session")]
    NoActiveSession,
    #[error("video volume must be between 0 and 100 percent")]
    InvalidVolume,
    #[error("video command belongs to a stale generation")]
    StaleGeneration,
    #[error("video command is invalid in the current playback state")]
    InvalidState,
    #[error(transparent)]
    Engine(#[from] VideoEngineError),
}

#[derive(Default)]
struct ServiceState {
    snapshot: VideoPreviewSnapshot,
    surface_revealed: bool,
    resume_after_seek: bool,
    active_commit_request_id: Option<u64>,
}

pub struct VideoPreviewService<E> {
    engine: Arc<E>,
    lane: AsyncMutex<()>,
    state: Mutex<ServiceState>,
}

impl<E> VideoPreviewService<E>
where
    E: VideoEngine,
{
    pub fn new(engine: Arc<E>) -> Self {
        Self {
            engine,
            lane: AsyncMutex::new(()),
            state: Mutex::new(ServiceState::default()),
        }
    }

    pub fn snapshot(&self) -> VideoPreviewSnapshot {
        self.lock_state().snapshot.clone()
    }

    pub async fn open(&self, source: VideoSource) -> Result<u64, VideoServiceError> {
        let _lane = self.lane.lock().await;
        let active_generation = {
            let mut state = self.lock_state();
            let active = state.snapshot.session_id.is_some();
            if active {
                state.snapshot.state = VideoPlaybackState::Closing;
                Some(state.snapshot.generation)
            } else {
                None
            }
        };
        if let Some(generation) = active_generation
            && let Err(error) = self.engine.close(generation).await
        {
            self.reset_to_idle();
            return Err(error.into());
        }

        let request = {
            let mut state = self.lock_state();
            let generation = state
                .snapshot
                .generation
                .checked_add(1)
                .ok_or(VideoServiceError::GenerationExhausted)?;
            let session_id = VideoSessionId::new();
            state.snapshot = VideoPreviewSnapshot {
                generation,
                session_id: Some(session_id),
                state: VideoPlaybackState::Preparing,
                time_us: 0,
                duration_us: None,
                volume_percent: 100,
                muted: false,
                rate: PlaybackRate::Normal,
            };
            state.surface_revealed = false;
            state.resume_after_seek = false;
            state.active_commit_request_id = None;
            EngineOpenRequest {
                generation,
                session_id,
                source,
            }
        };
        let generation = request.generation;
        if let Err(error) = self.engine.open_paused(request).await {
            self.lock_state().snapshot.state = VideoPlaybackState::Failed(error.failure_kind());
            return Err(error.into());
        }
        Ok(generation)
    }

    pub async fn handle_engine_event(
        &self,
        generation: u64,
        event: EngineEvent,
    ) -> Result<(), VideoServiceError> {
        let _lane = self.lane.lock().await;
        {
            let state = self.lock_state();
            if state.snapshot.generation != generation
                || state.snapshot.session_id.is_none()
                || matches!(
                    state.snapshot.state,
                    VideoPlaybackState::Idle | VideoPlaybackState::Closing
                )
            {
                return Ok(());
            }
        }

        match event {
            EngineEvent::Prepared(media) => {
                let mut state = self.lock_state();
                if matches!(state.snapshot.state, VideoPlaybackState::Preparing) {
                    state.snapshot.duration_us = media.duration_us;
                    state.snapshot.state = VideoPlaybackState::Ready;
                }
            }
            EngineEvent::FirstFrameReady => {
                let may_reveal = {
                    let state = self.lock_state();
                    !state.surface_revealed
                        && matches!(state.snapshot.state, VideoPlaybackState::Ready)
                };
                if !may_reveal {
                    return Ok(());
                }
                if let Err(error) = self.engine.reveal_surface(generation).await {
                    self.lock_state().snapshot.state =
                        VideoPlaybackState::Failed(error.failure_kind());
                    return Err(error.into());
                }
                self.lock_state().surface_revealed = true;
                if let Err(error) = self.engine.play(generation).await {
                    self.lock_state().snapshot.state =
                        VideoPlaybackState::Failed(error.failure_kind());
                    return Err(error.into());
                }
                self.lock_state().snapshot.state = VideoPlaybackState::Playing;
            }
            EngineEvent::TimeChanged {
                time_us,
                duration_us,
            } => {
                let mut state = self.lock_state();
                if matches!(
                    state.snapshot.state,
                    VideoPlaybackState::Seeking
                        | VideoPlaybackState::Ended
                        | VideoPlaybackState::Failed(_)
                ) {
                    return Ok(());
                }
                if let Some(duration_us) = duration_us {
                    state.snapshot.duration_us = Some(duration_us);
                }
                state.snapshot.time_us = state
                    .snapshot
                    .duration_us
                    .map_or(time_us, |duration_us| time_us.min(duration_us));
            }
            EngineEvent::Ended => {
                let should_pause = {
                    let state = self.lock_state();
                    !matches!(
                        state.snapshot.state,
                        VideoPlaybackState::Ended | VideoPlaybackState::Failed(_)
                    )
                };
                if !should_pause {
                    return Ok(());
                }
                if let Err(error) = self.engine.pause(generation).await {
                    self.lock_state().snapshot.state =
                        VideoPlaybackState::Failed(error.failure_kind());
                    return Err(error.into());
                }
                let mut state = self.lock_state();
                if let Some(duration_us) = state.snapshot.duration_us {
                    state.snapshot.time_us = duration_us;
                }
                state.snapshot.state = VideoPlaybackState::Ended;
            }
            EngineEvent::FrameStepped { time_us } => {
                let mut state = self.lock_state();
                if matches!(state.snapshot.state, VideoPlaybackState::FrameStepping) {
                    state.snapshot.time_us = state
                        .snapshot
                        .duration_us
                        .map_or(time_us, |duration_us| time_us.min(duration_us));
                    state.snapshot.state = VideoPlaybackState::Paused;
                }
            }
            EngineEvent::SeekCompleted {
                request_id,
                time_us,
            } => {
                let mut state = self.lock_state();
                if matches!(state.snapshot.state, VideoPlaybackState::Seeking)
                    && state.active_commit_request_id == Some(request_id)
                {
                    state.snapshot.time_us = state
                        .snapshot
                        .duration_us
                        .map_or(time_us, |duration_us| time_us.min(duration_us));
                    state.snapshot.state = if state.resume_after_seek {
                        VideoPlaybackState::Playing
                    } else {
                        VideoPlaybackState::Paused
                    };
                    state.resume_after_seek = false;
                    state.active_commit_request_id = None;
                }
            }
            EngineEvent::Failed(failure) => {
                let mut state = self.lock_state();
                state.snapshot.state = VideoPlaybackState::Failed(failure);
                state.resume_after_seek = false;
                state.active_commit_request_id = None;
            }
        }
        Ok(())
    }

    pub async fn handle_event(&self, event: VideoEvent) -> Result<(), VideoServiceError> {
        self.handle_engine_event(event.generation, event.event)
            .await
    }

    pub async fn execute(&self, command: VideoCommand) -> Result<(), VideoServiceError> {
        let _lane = self.lane.lock().await;
        {
            let state = self.lock_state();
            if state.snapshot.generation != command.generation {
                return Err(VideoServiceError::StaleGeneration);
            }
            let is_close = matches!(command.kind, VideoCommandKind::Close);
            if is_close && state.snapshot.session_id.is_none() {
                return Ok(());
            }
            if state.snapshot.session_id.is_none() {
                return Err(VideoServiceError::NoActiveSession);
            }
            if !is_close
                && matches!(
                    state.snapshot.state,
                    VideoPlaybackState::Idle
                        | VideoPlaybackState::Closing
                        | VideoPlaybackState::Failed(_)
                )
            {
                return Err(VideoServiceError::NoActiveSession);
            }
        }

        match command.kind {
            VideoCommandKind::Play => {
                if !matches!(self.snapshot().state, VideoPlaybackState::Paused) {
                    return Err(VideoServiceError::InvalidState);
                }
                self.run_engine(self.engine.play(command.generation).await)?;
                self.lock_state().snapshot.state = VideoPlaybackState::Playing;
            }
            VideoCommandKind::Pause => {
                if !matches!(self.snapshot().state, VideoPlaybackState::Playing) {
                    return Err(VideoServiceError::InvalidState);
                }
                self.run_engine(self.engine.pause(command.generation).await)?;
                self.lock_state().snapshot.state = VideoPlaybackState::Paused;
            }
            VideoCommandKind::Seek(request) => {
                let current_state = self.snapshot().state;
                if !matches!(
                    current_state,
                    VideoPlaybackState::Playing
                        | VideoPlaybackState::Paused
                        | VideoPlaybackState::Ended
                ) {
                    return Err(VideoServiceError::InvalidState);
                }
                let time_us = self
                    .snapshot()
                    .duration_us
                    .map_or(request.time_us, |duration_us| {
                        request.time_us.min(duration_us)
                    });
                let request = SeekRequest {
                    time_us,
                    intent: SeekIntent::Commit,
                    ..request
                };
                {
                    let mut state = self.lock_state();
                    state.resume_after_seek = matches!(current_state, VideoPlaybackState::Playing);
                    state.active_commit_request_id = Some(request.request_id);
                    state.snapshot.state = VideoPlaybackState::Seeking;
                }
                self.run_engine(self.engine.publish_seek(command.generation, request))?;
            }
            VideoCommandKind::Step(direction) => {
                if matches!(self.snapshot().state, VideoPlaybackState::Playing) {
                    self.run_engine(self.engine.pause(command.generation).await)?;
                    self.lock_state().snapshot.state = VideoPlaybackState::Paused;
                }
                if !matches!(
                    self.snapshot().state,
                    VideoPlaybackState::Paused | VideoPlaybackState::Ended
                ) {
                    return Err(VideoServiceError::InvalidState);
                }
                self.run_engine(self.engine.step(command.generation, direction).await)?;
                self.lock_state().snapshot.state = VideoPlaybackState::FrameStepping;
            }
            VideoCommandKind::SetVolume(percent) => {
                if percent > 100 {
                    return Err(VideoServiceError::InvalidVolume);
                }
                self.run_engine(self.engine.set_volume(command.generation, percent).await)?;
                self.lock_state().snapshot.volume_percent = percent;
            }
            VideoCommandKind::SetMuted(muted) => {
                self.run_engine(self.engine.set_muted(command.generation, muted).await)?;
                self.lock_state().snapshot.muted = muted;
            }
            VideoCommandKind::SetRate(rate) => {
                self.run_engine(self.engine.set_rate(command.generation, rate).await)?;
                self.lock_state().snapshot.rate = rate;
            }
            VideoCommandKind::SetSurfaceRect(rect) => {
                self.run_engine(
                    self.engine
                        .publish_surface_rect(command.generation, 0, rect),
                )?;
            }
            VideoCommandKind::Close => {
                self.lock_state().snapshot.state = VideoPlaybackState::Closing;
                let result = self.engine.close(command.generation).await;
                self.reset_to_idle();
                result?;
            }
        }
        Ok(())
    }

    pub async fn close(&self) -> Result<(), VideoServiceError> {
        let _lane = self.lane.lock().await;
        let generation = {
            let mut state = self.lock_state();
            let Some(_) = state.snapshot.session_id else {
                return Ok(());
            };
            state.snapshot.state = VideoPlaybackState::Closing;
            state.snapshot.generation
        };
        let result = self.engine.close(generation).await;
        self.reset_to_idle();
        result.map_err(Into::into)
    }

    pub fn preview_seek(
        &self,
        generation: u64,
        request: SeekRequest,
    ) -> Result<(), VideoServiceError> {
        let request = {
            let state = self.lock_state();
            if state.snapshot.generation != generation {
                return Err(VideoServiceError::StaleGeneration);
            }
            if state.snapshot.session_id.is_none() {
                return Err(VideoServiceError::NoActiveSession);
            }
            if !matches!(
                state.snapshot.state,
                VideoPlaybackState::Playing
                    | VideoPlaybackState::Paused
                    | VideoPlaybackState::Ended
            ) {
                return Err(VideoServiceError::InvalidState);
            }
            SeekRequest {
                time_us: state
                    .snapshot
                    .duration_us
                    .map_or(request.time_us, |duration_us| {
                        request.time_us.min(duration_us)
                    }),
                intent: SeekIntent::Preview,
                ..request
            }
        };
        self.run_engine(self.engine.publish_seek(generation, request))
    }

    pub fn publish_surface_rect(
        &self,
        generation: u64,
        sequence: u64,
        rect: SurfaceRect,
    ) -> Result<(), VideoServiceError> {
        {
            let state = self.lock_state();
            if state.snapshot.generation != generation {
                return Err(VideoServiceError::StaleGeneration);
            }
            if state.snapshot.session_id.is_none() {
                return Err(VideoServiceError::NoActiveSession);
            }
        }
        self.run_engine(self.engine.publish_surface_rect(generation, sequence, rect))
    }

    pub async fn set_volume(&self, percent: u8) -> Result<(), VideoServiceError> {
        if percent > 100 {
            return Err(VideoServiceError::InvalidVolume);
        }
        self.execute(VideoCommand {
            generation: self.snapshot().generation,
            kind: VideoCommandKind::SetVolume(percent),
        })
        .await
    }

    pub async fn set_muted(&self, muted: bool) -> Result<(), VideoServiceError> {
        self.execute(VideoCommand {
            generation: self.snapshot().generation,
            kind: VideoCommandKind::SetMuted(muted),
        })
        .await
    }

    pub async fn set_rate(&self, rate: PlaybackRate) -> Result<(), VideoServiceError> {
        self.execute(VideoCommand {
            generation: self.snapshot().generation,
            kind: VideoCommandKind::SetRate(rate),
        })
        .await
    }

    pub async fn step(&self, direction: FrameDirection) -> Result<(), VideoServiceError> {
        self.execute(VideoCommand {
            generation: self.snapshot().generation,
            kind: VideoCommandKind::Step(direction),
        })
        .await
    }

    fn run_engine(&self, result: Result<(), VideoEngineError>) -> Result<(), VideoServiceError> {
        if let Err(error) = result {
            self.lock_state().snapshot.state = VideoPlaybackState::Failed(error.failure_kind());
            return Err(error.into());
        }
        Ok(())
    }

    fn reset_to_idle(&self) {
        let mut state = self.lock_state();
        state.snapshot.session_id = None;
        state.snapshot.state = VideoPlaybackState::Idle;
        state.surface_revealed = false;
        state.resume_after_seek = false;
        state.active_commit_request_id = None;
    }

    fn lock_state(&self) -> MutexGuard<'_, ServiceState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

pub fn video_neighbors(
    files: &[BrowserFile],
    current: EntityId,
) -> (Option<EntityId>, Option<EntityId>) {
    let videos = files
        .iter()
        .filter(|file| file.kind == FileKind::Video)
        .map(|file| file.entity_id)
        .collect::<Vec<_>>();
    let Some(index) = videos.iter().position(|entity_id| *entity_id == current) else {
        return (None, None);
    };
    (
        index
            .checked_sub(1)
            .and_then(|index| videos.get(index).copied()),
        videos.get(index + 1).copied(),
    )
}
