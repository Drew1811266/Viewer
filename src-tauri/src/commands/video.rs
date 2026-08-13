use crate::{
    dto::{
        GenerationDto, VideoCacheStatsDto, VideoFullscreenDto, VideoMutedDto, VideoOpenAttemptDto,
        VideoOpenRequestDto, VideoOpenSurfaceRectDto, VideoRateDto, VideoSeekDto, VideoSessionDto,
        VideoStepDto, VideoSurfaceRectDto, VideoThumbnailRequestDto, VideoVolumeDto,
    },
    error::{CommandError, ErrorCategory},
    state::{AuthorizedVideoSource, DesktopRuntime},
    video_runtime::{VideoCommandError, VideoRuntime},
};
use std::{str::FromStr, sync::Arc};
use tauri::{State, WebviewWindow};
use viewer_application::{SurfaceRect, VideoEngine};
use viewer_domain::EntityId;

pub type NativeVideoRuntime = VideoRuntime<viewer_platform_macos::video::MacOsLibmpvAdapter>;

#[tauri::command]
pub async fn video_open(
    request: VideoOpenRequestDto,
    window: WebviewWindow,
    desktop: State<'_, Arc<DesktopRuntime>>,
    video: State<'_, Arc<NativeVideoRuntime>>,
) -> Result<VideoSessionDto, CommandError> {
    validate_open_attempt_id(&request.attempt_id)?;
    let entity_id = EntityId::from_str(&request.entity_id).map_err(|_| invalid_entity_id())?;
    let attempt = video
        .begin_open_attempt(request.attempt_id.clone())
        .map_err(CommandError::from)?;
    let result = async {
        let _project_lease = desktop.video_open_project_lease().await;
        video.ensure_open_attempt(&attempt)?;
        let source = desktop
            .resolve_video_entity_for_open(entity_id, attempt.cancellation().clone())
            .await
            .map_err(CommandError::from)?;
        video.ensure_open_attempt(&attempt)?;
        let surface_rect = initial_video_surface_rect(&source, request.surface_rect);
        let engine = Arc::clone(video.engine());
        video
            .replace_authorized_for_attempt(&attempt, source, || async move {
                engine
                    .prepare_surface(window, surface_rect)
                    .await
                    .map_err(|_| VideoCommandError::EngineUnavailable)
            })
            .await
            .map_err(CommandError::from)
    }
    .await;
    video.finish_open_attempt(attempt.id());
    result
}

#[tauri::command]
pub fn video_cancel_open(
    request: VideoOpenAttemptDto,
    video: State<'_, Arc<NativeVideoRuntime>>,
) -> Result<bool, CommandError> {
    validate_open_attempt_id(&request.attempt_id)?;
    Ok(video.cancel_open_attempt(&request.attempt_id))
}

#[tauri::command]
pub async fn video_request_cover(
    entity_id: String,
    desktop: State<'_, Arc<DesktopRuntime>>,
    video: State<'_, Arc<NativeVideoRuntime>>,
) -> Result<String, CommandError> {
    let entity_id = EntityId::from_str(&entity_id).map_err(|_| invalid_entity_id())?;
    let _project_lease = desktop.video_open_project_lease().await;
    let source = desktop
        .resolve_video_entity(entity_id)
        .await
        .map_err(CommandError::from)?;
    video
        .request_cover(source)
        .await
        .map_err(CommandError::from)
}

fn validate_open_attempt_id(attempt_id: &str) -> Result<(), CommandError> {
    if attempt_id.len() == 36
        && attempt_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
    {
        Ok(())
    } else {
        Err(CommandError::new(
            "invalid_video_open_attempt",
            ErrorCategory::Validation,
            "视频打开请求标识无效。",
            false,
        ))
    }
}

fn initial_video_surface_rect(
    source: &AuthorizedVideoSource,
    requested: VideoOpenSurfaceRectDto,
) -> SurfaceRect {
    let stage = SurfaceRect::from(requested);
    if !source.metadata_refreshed_for_retry {
        return stage;
    }
    let Some(display_width) = source.metadata.display_width.filter(|width| *width > 0) else {
        return stage;
    };
    let Some(display_height) = source.metadata.display_height.filter(|height| *height > 0) else {
        return stage;
    };
    let rotated = source.metadata.rotation_degrees.unsigned_abs() % 180 == 90;
    let (source_width, source_height) = if rotated {
        (display_height, display_width)
    } else {
        (display_width, display_height)
    };
    let scale = f64::min(
        f64::from(stage.width) / f64::from(source_width),
        f64::from(stage.height) / f64::from(source_height),
    );
    let fitted_width = f64::from(source_width) * scale;
    let fitted_height = f64::from(source_height) * scale;
    SurfaceRect {
        x: js_round_i32(f64::from(stage.x) + (f64::from(stage.width) - fitted_width) / 2.0),
        y: js_round_i32(f64::from(stage.y) + (f64::from(stage.height) - fitted_height) / 2.0),
        width: js_round_u32(fitted_width),
        height: js_round_u32(fitted_height),
    }
}

fn js_round_i32(value: f64) -> i32 {
    (value + 0.5)
        .floor()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

fn js_round_u32(value: f64) -> u32 {
    (value + 0.5).floor().clamp(0.0, f64::from(u32::MAX)) as u32
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

#[cfg(test)]
mod tests {
    use super::initial_video_surface_rect;
    use crate::{dto::VideoOpenSurfaceRectDto, state::AuthorizedVideoSource};
    use std::path::PathBuf;
    use viewer_application::SurfaceRect;
    use viewer_domain::{
        EntityId, SessionId,
        video::{VideoMetadata, VideoProbeStatus},
    };

    fn source(metadata_refreshed_for_retry: bool) -> AuthorizedVideoSource {
        AuthorizedVideoSource {
            entity_id: EntityId::from_u128(7),
            session_id: SessionId::from_u128(8),
            canonical_path: PathBuf::from("/project/clip.mp4"),
            metadata: VideoMetadata {
                duration_us: Some(2_000_000),
                display_width: Some(1_920),
                display_height: Some(1_080),
                rotation_degrees: 0,
                frame_rate_millihertz: Some(24_000),
                video_codec: Some("h264".into()),
                audio_codec: None,
                probe_status: VideoProbeStatus::Ready,
            },
            metadata_refreshed_for_retry,
        }
    }

    #[test]
    fn retry_open_uses_authoritative_fit_before_the_native_surface_is_prepared() {
        let stage = VideoOpenSurfaceRectDto {
            x: 100,
            y: 50,
            width: 800,
            height: 600,
        };

        assert_eq!(
            initial_video_surface_rect(&source(true), stage),
            SurfaceRect {
                x: 100,
                y: 125,
                width: 800,
                height: 450,
            }
        );
        assert_eq!(
            initial_video_surface_rect(&source(false), stage),
            SurfaceRect::from(stage)
        );
    }
}
