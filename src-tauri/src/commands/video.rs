use crate::{
    dto::{
        GenerationDto, VideoCacheStatsDto, VideoFullscreenDto, VideoMutedDto, VideoOpenRequestDto,
        VideoRateDto, VideoSeekDto, VideoSessionDto, VideoStepDto, VideoSurfaceRectDto,
        VideoThumbnailRequestDto, VideoVolumeDto,
    },
    error::{CommandError, ErrorCategory},
    state::DesktopRuntime,
    video_runtime::{VideoCommandError, VideoRuntime},
};
use std::{str::FromStr, sync::Arc};
use tauri::{State, WebviewWindow};
use viewer_application::VideoEngine;
use viewer_domain::EntityId;

pub type NativeVideoRuntime = VideoRuntime<viewer_platform_macos::video::MacOsLibmpvAdapter>;

#[tauri::command]
pub async fn video_open(
    request: VideoOpenRequestDto,
    window: WebviewWindow,
    desktop: State<'_, Arc<DesktopRuntime>>,
    video: State<'_, Arc<NativeVideoRuntime>>,
) -> Result<VideoSessionDto, CommandError> {
    let entity_id = EntityId::from_str(&request.entity_id).map_err(|_| invalid_entity_id())?;
    let _project_lease = desktop.video_open_project_lease().await;
    let source = desktop
        .resolve_video_entity(entity_id)
        .await
        .map_err(CommandError::from)?;
    let engine = Arc::clone(video.engine());
    video
        .replace_authorized(source, || async move {
            engine
                .prepare_surface(window, request.surface_rect.into())
                .await
                .map_err(|_| VideoCommandError::EngineUnavailable)
        })
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn video_close(
    request: GenerationDto,
    video: State<'_, Arc<NativeVideoRuntime>>,
) -> Result<(), CommandError> {
    video.close(request.generation).await.map_err(Into::into)
}

#[tauri::command]
pub async fn video_play(
    request: GenerationDto,
    video: State<'_, Arc<NativeVideoRuntime>>,
) -> Result<(), CommandError> {
    video.play(request.generation).await.map_err(Into::into)
}

#[tauri::command]
pub async fn video_pause(
    request: GenerationDto,
    video: State<'_, Arc<NativeVideoRuntime>>,
) -> Result<(), CommandError> {
    video.pause(request.generation).await.map_err(Into::into)
}

#[tauri::command]
pub async fn video_seek(
    request: VideoSeekDto,
    video: State<'_, Arc<NativeVideoRuntime>>,
) -> Result<(), CommandError> {
    video
        .seek(request.generation, request.time_us)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn video_step(
    request: VideoStepDto,
    video: State<'_, Arc<NativeVideoRuntime>>,
) -> Result<(), CommandError> {
    video
        .step(request.generation, request.direction.into())
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn video_set_volume(
    request: VideoVolumeDto,
    video: State<'_, Arc<NativeVideoRuntime>>,
) -> Result<(), CommandError> {
    video
        .set_volume(request.generation, request.volume_percent)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn video_set_muted(
    request: VideoMutedDto,
    video: State<'_, Arc<NativeVideoRuntime>>,
) -> Result<(), CommandError> {
    video
        .set_muted(request.generation, request.muted)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn video_set_rate(
    request: VideoRateDto,
    video: State<'_, Arc<NativeVideoRuntime>>,
) -> Result<(), CommandError> {
    video
        .set_rate(request.generation, request.rate.into())
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn video_set_surface_rect(
    request: VideoSurfaceRectDto,
    video: State<'_, Arc<NativeVideoRuntime>>,
) -> Result<(), CommandError> {
    video
        .set_surface_rect(request.generation, request.rect())
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn video_set_fullscreen(
    request: VideoFullscreenDto,
    window: WebviewWindow,
    video: State<'_, Arc<NativeVideoRuntime>>,
) -> Result<(), CommandError> {
    video
        .set_fullscreen_guarded(request.generation, request.fullscreen, |fullscreen| {
            window
                .set_fullscreen(fullscreen)
                .map_err(|_| fullscreen_unavailable())?;
            window.is_fullscreen().map_err(|_| fullscreen_unavailable())
        })
        .await
}

pub fn apply_fullscreen_after_generation_gate<E: VideoEngine>(
    video: &VideoRuntime<E>,
    generation: u64,
    fullscreen: bool,
    mutate_window: impl FnOnce(bool) -> Result<(), CommandError>,
) -> Result<(), CommandError> {
    video.ensure_generation(generation)?;
    mutate_window(fullscreen)
}

#[tauri::command]
pub async fn video_request_thumbnail(
    request: VideoThumbnailRequestDto,
    video: State<'_, Arc<NativeVideoRuntime>>,
) -> Result<(), CommandError> {
    video
        .request_thumbnail(request.generation, request.request_id, request.time_us)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn video_cache_stats(
    video: State<'_, Arc<NativeVideoRuntime>>,
) -> Result<VideoCacheStatsDto, CommandError> {
    video.cache_stats().map_err(Into::into)
}

#[tauri::command]
pub async fn video_cache_clear(
    video: State<'_, Arc<NativeVideoRuntime>>,
) -> Result<VideoCacheStatsDto, CommandError> {
    video.cache_clear().await.map_err(Into::into)
}

fn invalid_entity_id() -> CommandError {
    CommandError::new(
        "invalid_entity_id",
        ErrorCategory::Validation,
        "文件标识无效，请刷新项目后重试。",
        false,
    )
}

fn fullscreen_unavailable() -> CommandError {
    CommandError::new(
        "video_fullscreen_unavailable",
        ErrorCategory::Environment,
        "无法切换视频全屏状态。",
        true,
    )
}
