use crate::{
    dto::{
        GenerationDto, VideoCacheStatsDto, VideoFullscreenDto, VideoMutedDto, VideoOpenAttemptDto,
        VideoOpenRequestDto, VideoRateDto, VideoSeekDto, VideoSessionDto, VideoStepDto,
        VideoThumbnailRequestDto, VideoVolumeDto,
    },
    error::{CommandError, ErrorCategory},
    state::{AuthorizedVideoSource, DesktopRuntime},
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
        let media_geometry = native_video_geometry(&source);
        let media_duration_us = source.metadata.duration_us;
        let engine = Arc::clone(video.engine());
        video
            .replace_authorized_for_attempt(&attempt, source, || async move {
                engine
                    .prepare_surface(window, media_geometry, media_duration_us)
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

fn native_video_geometry(
    source: &AuthorizedVideoSource,
) -> viewer_platform_macos::video::VideoDisplayGeometry {
    viewer_platform_macos::video::VideoDisplayGeometry {
        width: source.metadata.display_width.unwrap_or(1).max(1),
        height: source.metadata.display_height.unwrap_or(1).max(1),
        rotation_degrees: i32::from(source.metadata.rotation_degrees),
    }
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
        .seek(request.generation, request.request())
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
    use super::native_video_geometry;
    use crate::state::AuthorizedVideoSource;
    use std::path::PathBuf;
    use viewer_domain::{
        EntityId, SessionId,
        video::{VideoMetadata, VideoProbeStatus},
    };
    use viewer_platform_macos::video::VideoDisplayGeometry;

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
    fn native_theater_uses_authoritative_media_geometry_instead_of_a_browser_rect() {
        assert_eq!(
            native_video_geometry(&source(true)),
            VideoDisplayGeometry {
                width: 1_920,
                height: 1_080,
                rotation_degrees: 0,
            }
        );
        assert_eq!(native_video_geometry(&source(false)).width, 1_920);
    }
}
