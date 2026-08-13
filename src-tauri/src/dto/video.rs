use serde::{Deserialize, Serialize};
use viewer_application::{
    FrameDirection, PlaybackRate, SurfaceRect, VideoMedia, VideoPlaybackState, VideoPreviewSnapshot,
};
use viewer_domain::video::{VideoFailureKind, VideoMetadata};
use viewer_infrastructure::video_cache::VideoCacheStats;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoSurfaceRectDto {
    pub generation: u64,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl VideoSurfaceRectDto {
    pub const fn rect(self) -> SurfaceRect {
        SurfaceRect {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoOpenSurfaceRectDto {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl From<VideoOpenSurfaceRectDto> for SurfaceRect {
    fn from(value: VideoOpenSurfaceRectDto) -> Self {
        Self {
            x: value.x,
            y: value.y,
            width: value.width,
            height: value.height,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoOpenRequestDto {
    pub attempt_id: String,
    pub entity_id: String,
    pub surface_rect: VideoOpenSurfaceRectDto,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoOpenAttemptDto {
    pub attempt_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerationDto {
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoSeekDto {
    pub generation: u64,
    pub time_us: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VideoStepDirectionDto {
    Backward,
    Forward,
}

impl From<VideoStepDirectionDto> for FrameDirection {
    fn from(value: VideoStepDirectionDto) -> Self {
        match value {
            VideoStepDirectionDto::Backward => Self::Backward,
            VideoStepDirectionDto::Forward => Self::Forward,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoStepDto {
    pub generation: u64,
    pub direction: VideoStepDirectionDto,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoVolumeDto {
    pub generation: u64,
    pub volume_percent: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoMutedDto {
    pub generation: u64,
    pub muted: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VideoRateValueDto {
    Half,
    ThreeQuarters,
    Normal,
    OneAndQuarter,
    OneAndHalf,
    Double,
}

impl From<VideoRateValueDto> for PlaybackRate {
    fn from(value: VideoRateValueDto) -> Self {
        match value {
            VideoRateValueDto::Half => Self::Half,
            VideoRateValueDto::ThreeQuarters => Self::ThreeQuarters,
            VideoRateValueDto::Normal => Self::Normal,
            VideoRateValueDto::OneAndQuarter => Self::OneAndQuarter,
            VideoRateValueDto::OneAndHalf => Self::OneAndHalf,
            VideoRateValueDto::Double => Self::Double,
        }
    }
}

impl From<PlaybackRate> for VideoRateValueDto {
    fn from(value: PlaybackRate) -> Self {
        match value {
            PlaybackRate::Half => Self::Half,
            PlaybackRate::ThreeQuarters => Self::ThreeQuarters,
            PlaybackRate::Normal => Self::Normal,
            PlaybackRate::OneAndQuarter => Self::OneAndQuarter,
            PlaybackRate::OneAndHalf => Self::OneAndHalf,
            PlaybackRate::Double => Self::Double,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoRateDto {
    pub generation: u64,
    pub rate: VideoRateValueDto,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoFullscreenDto {
    pub generation: u64,
    pub fullscreen: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoThumbnailRequestDto {
    pub generation: u64,
    pub request_id: String,
    pub time_us: u64,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VideoMediaDto {
    pub duration_us: Option<u64>,
    pub display_width: Option<u32>,
    pub display_height: Option<u32>,
    pub rotation_degrees: i16,
}

impl From<&VideoMetadata> for VideoMediaDto {
    fn from(value: &VideoMetadata) -> Self {
        Self {
            duration_us: value.duration_us,
            display_width: value.display_width,
            display_height: value.display_height,
            rotation_degrees: value.rotation_degrees,
        }
    }
}

impl From<VideoMedia> for VideoMediaDto {
    fn from(value: VideoMedia) -> Self {
        Self {
            duration_us: value.duration_us,
            display_width: value.display_width,
            display_height: value.display_height,
            rotation_degrees: value.rotation_degrees,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VideoSessionDto {
    pub generation: u64,
    pub session_id: String,
    pub media: VideoMediaDto,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VideoStateDto {
    Idle,
    Preparing,
    Ready,
    Playing,
    Paused,
    Seeking,
    FrameStepping,
    Ended,
    Failed,
    Closing,
}

impl From<&VideoPlaybackState> for VideoStateDto {
    fn from(value: &VideoPlaybackState) -> Self {
        match value {
            VideoPlaybackState::Idle => Self::Idle,
            VideoPlaybackState::Preparing => Self::Preparing,
            VideoPlaybackState::Ready => Self::Ready,
            VideoPlaybackState::Playing => Self::Playing,
            VideoPlaybackState::Paused => Self::Paused,
            VideoPlaybackState::Seeking => Self::Seeking,
            VideoPlaybackState::FrameStepping => Self::FrameStepping,
            VideoPlaybackState::Ended => Self::Ended,
            VideoPlaybackState::Failed(_) => Self::Failed,
            VideoPlaybackState::Closing => Self::Closing,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VideoSettingsDto {
    pub volume_percent: u8,
    pub muted: bool,
    pub rate: VideoRateValueDto,
}

impl From<&VideoPreviewSnapshot> for VideoSettingsDto {
    fn from(value: &VideoPreviewSnapshot) -> Self {
        Self {
            volume_percent: value.volume_percent,
            muted: value.muted,
            rate: value.rate.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VideoErrorDto {
    pub code: &'static str,
    pub retryable: bool,
}

impl From<VideoFailureKind> for VideoErrorDto {
    fn from(value: VideoFailureKind) -> Self {
        let code = match value {
            VideoFailureKind::Unsupported => "unsupported",
            VideoFailureKind::Damaged => "damaged",
            VideoFailureKind::Unreadable => "unreadable",
            VideoFailureKind::Missing => "missing",
            VideoFailureKind::EngineInitialization => "engine_initialization",
            VideoFailureKind::DecodeFallbackFailed => "decode_fallback_failed",
            VideoFailureKind::RenderSurface => "render_surface",
            VideoFailureKind::ThumbnailUnavailable => "thumbnail_unavailable",
        };
        Self {
            code,
            retryable: matches!(
                value,
                VideoFailureKind::Unreadable
                    | VideoFailureKind::Missing
                    | VideoFailureKind::EngineInitialization
                    | VideoFailureKind::RenderSurface
                    | VideoFailureKind::ThumbnailUnavailable
            ),
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum VideoEventDto {
    Prepared {
        generation: u64,
        media: VideoMediaDto,
    },
    FirstFrameReady {
        generation: u64,
    },
    StateChanged {
        generation: u64,
        state: VideoStateDto,
    },
    Progress {
        generation: u64,
        time_us: u64,
        duration_us: Option<u64>,
    },
    SettingsChanged {
        generation: u64,
        volume_percent: u8,
        muted: bool,
        rate: f32,
    },
    FullscreenChanged {
        generation: u64,
        fullscreen: bool,
    },
    TimelineThumbnailReady {
        generation: u64,
        request_id: String,
        bucket_us: u64,
        artifact_url: String,
    },
    Ended {
        generation: u64,
    },
    Failed {
        generation: u64,
        error: VideoErrorDto,
    },
    Closed {
        generation: u64,
    },
}

impl VideoEventDto {
    pub const fn generation(&self) -> u64 {
        match self {
            Self::Prepared { generation, .. }
            | Self::FirstFrameReady { generation }
            | Self::StateChanged { generation, .. }
            | Self::Progress { generation, .. }
            | Self::SettingsChanged { generation, .. }
            | Self::FullscreenChanged { generation, .. }
            | Self::TimelineThumbnailReady { generation, .. }
            | Self::Ended { generation }
            | Self::Failed { generation, .. }
            | Self::Closed { generation } => *generation,
        }
    }

    pub const fn is_progress(&self) -> bool {
        matches!(self, Self::Progress { .. })
    }

    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Ended { .. } | Self::Failed { .. } | Self::Closed { .. }
        )
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VideoCacheStatsDto {
    pub bytes_used: u64,
    pub entry_count: u64,
    pub budget_bytes: u64,
}

impl From<VideoCacheStats> for VideoCacheStatsDto {
    fn from(value: VideoCacheStats) -> Self {
        Self {
            bytes_used: value.bytes_used,
            entry_count: value.entry_count,
            budget_bytes: value.budget_bytes,
        }
    }
}
