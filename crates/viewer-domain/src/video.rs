#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoMetadata {
    pub duration_us: Option<u64>,
    pub display_width: Option<u32>,
    pub display_height: Option<u32>,
    pub rotation_degrees: i16,
    pub frame_rate_millihertz: Option<u32>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub probe_status: VideoProbeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VideoProbeStatus {
    Pending,
    Ready,
    Failed(VideoFailureKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFailureKind {
    Unsupported,
    Damaged,
    Unreadable,
    Missing,
    EngineInitialization,
    DecodeFallbackFailed,
    RenderSurface,
    ThumbnailUnavailable,
}
