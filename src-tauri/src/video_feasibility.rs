use crate::error::{CommandError, ErrorCategory};
use objc2::{msg_send, runtime::AnyObject};
use objc2_app_kit::NSColor;
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    sync::Arc,
};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow, webview::Color};
use viewer_platform_macos::video::{
    MacVideoRenderSession, MacVideoSurface, SurfaceRect, VideoRenderDiagnostics,
};
use viewer_video_mpv::{FrameDirection, MpvClient, MpvLibrary, runtime_manifest::RuntimeLayout};

const FRAME_EVENT: &str = "viewer://video-feasibility-frame";

thread_local! {
    static ACTIVE_SESSION: RefCell<Option<ActiveFeasibilitySession>> = const { RefCell::new(None) };
}

struct ActiveFeasibilitySession {
    fixture_id: String,
    rect: SurfaceRectDto,
    session: MacVideoRenderSession,
    initial_rendered_frames: u64,
    forward_step: bool,
    backward_step: bool,
    observed_hwdec: String,
    observed_video_output: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SurfaceRectDto {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl From<SurfaceRectDto> for SurfaceRect {
    fn from(value: SurfaceRectDto) -> Self {
        Self {
            x: value.x,
            y: value.y,
            width: value.width,
            height: value.height,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoFeasibilityAction {
    Mount,
    UpdateGeometry,
    FrameStepForward,
    FrameStepBackward,
    Status,
    Close,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoFeasibilityRequest {
    fixture_id: String,
    action: VideoFeasibilityAction,
    rect: SurfaceRectDto,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoFeasibilityReport {
    fixture_id: String,
    rect: SurfaceRectDto,
    first_frame_ready: bool,
    forward_step: bool,
    backward_step: bool,
    playback_time_us: Option<u64>,
    diagnostics: VideoRenderDiagnostics,
}

#[tauri::command]
pub async fn run_video_feasibility(
    app: AppHandle,
    window: WebviewWindow,
    request: VideoFeasibilityRequest,
) -> Result<VideoFeasibilityReport, CommandError> {
    let fixture = fixture_path(&request.fixture_id)?;
    let bundle_resources = bundle_resources(&app)?;
    window
        .set_background_color(Some(Color(0, 0, 0, 0)))
        .map_err(|_| {
            feasibility_error(
                "video_feasibility_transparency",
                "无法将网页视图切换为透明背景。",
            )
        })?;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let command_window = window.clone();
    window
        .run_on_main_thread(move || {
            let result = handle_on_main_thread(command_window, request, fixture, bundle_resources);
            let _ = sender.send(result);
        })
        .map_err(|_| {
            feasibility_error("video_feasibility_main_thread", "无法调度原生视频测试。")
        })?;
    receiver.await.map_err(|_| {
        feasibility_error(
            "video_feasibility_cancelled",
            "原生视频测试在返回结果前被取消。",
        )
    })?
}

fn handle_on_main_thread(
    window: WebviewWindow,
    request: VideoFeasibilityRequest,
    fixture: PathBuf,
    bundle_resources: PathBuf,
) -> Result<VideoFeasibilityReport, CommandError> {
    configure_transparent_webview(&window)?;
    match request.action {
        VideoFeasibilityAction::Mount => mount(
            window,
            request.fixture_id,
            request.rect,
            &fixture,
            &bundle_resources,
        ),
        VideoFeasibilityAction::UpdateGeometry => {
            with_matching_session(&request.fixture_id, |active| {
                active
                    .session
                    .update_geometry(request.rect.into())
                    .map_err(render_error)?;
                active.rect = request.rect;
                active.report()
            })
        }
        VideoFeasibilityAction::FrameStepForward => {
            with_matching_session(&request.fixture_id, |active| {
                active
                    .session
                    .frame_step(FrameDirection::Forward)
                    .map_err(render_error)?;
                active.forward_step = true;
                active.report()
            })
        }
        VideoFeasibilityAction::FrameStepBackward => {
            with_matching_session(&request.fixture_id, |active| {
                active
                    .session
                    .frame_step(FrameDirection::Backward)
                    .map_err(render_error)?;
                active.backward_step = true;
                active.report()
            })
        }
        VideoFeasibilityAction::Status => {
            with_matching_session(&request.fixture_id, |active| active.report())
        }
        VideoFeasibilityAction::Close => close(&request.fixture_id, request.rect),
    }
}

fn mount(
    window: WebviewWindow,
    fixture_id: String,
    rect: SurfaceRectDto,
    fixture: &Path,
    bundle_resources: &Path,
) -> Result<VideoFeasibilityReport, CommandError> {
    ACTIVE_SESSION.with(|slot| slot.borrow_mut().take());
    let layout = RuntimeLayout::from_bundle_root(bundle_resources).map_err(render_error)?;
    let library = MpvLibrary::load(&layout).map_err(render_error)?;
    let mut client = MpvClient::new(&library).map_err(render_error)?;
    client.initialize_for_rendering().map_err(render_error)?;
    let surface = MacVideoSurface::mount(&window, rect.into()).map_err(render_error)?;
    let scheduling_window = window.clone();
    let schedule_draw: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        let main_window = scheduling_window.clone();
        let event_window = scheduling_window.clone();
        let scheduled = main_window.run_on_main_thread(move || {
            let report = ACTIVE_SESSION.with(|slot| {
                let mut slot = slot.borrow_mut();
                slot.as_ref()?;
                let result = run_registered_action(&mut slot, |active| {
                    active.session.draw_if_needed().map_err(render_error)?;
                    active.report()
                });
                match result {
                    Ok(report) => Some(report),
                    Err(error) => {
                        eprintln!("Viewer video feasibility draw failed: {error:?}");
                        None
                    }
                }
            });
            if let Some(report) = report {
                let _ = event_window.emit(FRAME_EVENT, report);
            }
        });
        if let Err(error) = scheduled {
            eprintln!("Viewer video feasibility: main-thread draw scheduling failed: {error}");
        }
    });
    let session =
        MacVideoRenderSession::new(client, surface, schedule_draw).map_err(render_error)?;
    let diagnostics = session.diagnostics().map_err(render_error)?;
    let active = ActiveFeasibilitySession {
        fixture_id,
        rect,
        session,
        initial_rendered_frames: diagnostics.rendered_frames,
        forward_step: false,
        backward_step: false,
        observed_hwdec: diagnostics.hwdec,
        observed_video_output: diagnostics.video_output,
    };
    ACTIVE_SESSION.with(|slot| {
        let mut slot = slot.borrow_mut();
        *slot = Some(active);
        let result = (|| {
            let active = slot.as_mut().expect("active session was just registered");
            active.session.draw_if_needed().map_err(render_error)?;
            active.initial_rendered_frames = active
                .session
                .diagnostics()
                .map_err(render_error)?
                .rendered_frames;
            active
                .session
                .open_local_file(fixture)
                .map_err(render_error)?;
            active.session.pause().map_err(render_error)?;
            active.report()
        })();
        clear_session_on_error(&mut slot, result)
    })
}

fn close(fixture_id: &str, rect: SurfaceRectDto) -> Result<VideoFeasibilityReport, CommandError> {
    let closed = ACTIVE_SESSION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let active = slot.as_ref().ok_or_else(no_active_session)?;
        if active.fixture_id != fixture_id {
            return Err(fixture_mismatch());
        }
        close_registered_session(&mut slot, ActiveFeasibilitySession::report)
    })?;
    let diagnostics = VideoRenderDiagnostics::snapshot("", "");
    Ok(VideoFeasibilityReport {
        fixture_id: closed.fixture_id,
        rect,
        first_frame_ready: closed.first_frame_ready,
        forward_step: closed.forward_step,
        backward_step: closed.backward_step,
        playback_time_us: closed.playback_time_us,
        diagnostics,
    })
}

fn with_matching_session(
    fixture_id: &str,
    action: impl FnOnce(&mut ActiveFeasibilitySession) -> Result<VideoFeasibilityReport, CommandError>,
) -> Result<VideoFeasibilityReport, CommandError> {
    ACTIVE_SESSION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let active = slot.as_mut().ok_or_else(no_active_session)?;
        if active.fixture_id != fixture_id {
            return Err(fixture_mismatch());
        }
        run_registered_action(&mut slot, action)
    })
}

impl ActiveFeasibilitySession {
    fn report(&mut self) -> Result<VideoFeasibilityReport, CommandError> {
        let mut diagnostics = self.session.diagnostics().map_err(render_error)?;
        let playback_time_us = self.session.playback_time_us().map_err(render_error)?;
        retain_observed_backend(
            &mut diagnostics,
            &mut self.observed_hwdec,
            &mut self.observed_video_output,
        );
        Ok(VideoFeasibilityReport {
            fixture_id: self.fixture_id.clone(),
            rect: self.rect,
            first_frame_ready: fixture_frame_ready(
                diagnostics.rendered_frames,
                self.initial_rendered_frames,
            ),
            forward_step: self.forward_step,
            backward_step: self.backward_step,
            playback_time_us,
            diagnostics,
        })
    }
}

fn clear_session_on_error<T, R, E>(slot: &mut Option<T>, result: Result<R, E>) -> Result<R, E> {
    if result.is_err() {
        slot.take();
    }
    result
}

fn close_registered_session<T, R, E>(
    slot: &mut Option<T>,
    close: impl FnOnce(&mut T) -> Result<R, E>,
) -> Result<R, E> {
    let mut active = slot
        .take()
        .expect("close_registered_session requires an active session");
    close(&mut active)
}

fn run_registered_action<T, R, E>(
    slot: &mut Option<T>,
    action: impl FnOnce(&mut T) -> Result<R, E>,
) -> Result<R, E> {
    let result = action(
        slot.as_mut()
            .expect("run_registered_action requires an active session"),
    );
    clear_session_on_error(slot, result)
}

fn fixture_frame_ready(rendered_frames: u64, fixture_baseline: u64) -> bool {
    rendered_frames > fixture_baseline
}

fn retain_observed_backend(
    diagnostics: &mut VideoRenderDiagnostics,
    observed_hwdec: &mut String,
    observed_video_output: &mut String,
) {
    if diagnostics.hwdec.is_empty() {
        diagnostics.hwdec.clone_from(observed_hwdec);
    } else {
        observed_hwdec.clone_from(&diagnostics.hwdec);
    }
    if diagnostics.video_output.is_empty() {
        diagnostics.video_output.clone_from(observed_video_output);
    } else {
        observed_video_output.clone_from(&diagnostics.video_output);
    }
}

fn fixture_path(fixture_id: &str) -> Result<PathBuf, CommandError> {
    let file = match fixture_id {
        "h264-1080p" => "h264-1080p.mp4",
        "hevc-portrait" => "hevc-portrait.mp4",
        "vfr-step" => "vfr-step.mp4",
        _ => {
            return Err(feasibility_error(
                "video_feasibility_fixture_rejected",
                "原生视频测试样本不在允许列表中。",
            ));
        }
    };
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/videos")
        .join(file)
        .canonicalize()
        .map_err(|_| {
            feasibility_error(
                "video_feasibility_fixture_missing",
                "原生视频测试样本不存在。",
            )
        })
}

fn bundle_resources(app: &AppHandle) -> Result<PathBuf, CommandError> {
    if let Some(path) = std::env::var_os("VIEWER_VIDEO_BUNDLE_RESOURCES") {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            return Ok(path);
        }
    }
    app.path().resource_dir().map_err(|_| {
        feasibility_error(
            "video_feasibility_runtime_missing",
            "无法定位随 Viewer 打包的视频运行时。",
        )
    })
}

fn configure_transparent_webview(window: &WebviewWindow) -> Result<(), CommandError> {
    window
        .with_webview(|webview| {
            let clear = NSColor::clearColor();
            // SAFETY: Tauri's macOS inner view is WKWebView. This public
            // selector is available on the minimum supported macOS 13 and
            // this feature-gated route runs it on AppKit's main thread.
            let webview: &AnyObject = unsafe { &*webview.inner().cast() };
            unsafe {
                let _: () = msg_send![webview, setUnderPageBackgroundColor: &*clear];
            }
        })
        .map_err(|_| {
            feasibility_error(
                "video_feasibility_transparency",
                "无法将网页视图切换为透明背景。",
            )
        })
}

fn no_active_session() -> CommandError {
    feasibility_error(
        "video_feasibility_not_mounted",
        "原生视频测试表面尚未挂载。",
    )
}

fn fixture_mismatch() -> CommandError {
    feasibility_error(
        "video_feasibility_fixture_mismatch",
        "原生视频测试会话与样本不匹配。",
    )
}

fn render_error(error: impl std::fmt::Display) -> CommandError {
    eprintln!("Viewer video feasibility error: {error}");
    feasibility_error("video_feasibility_native_error", "原生视频渲染门禁失败。")
}

fn feasibility_error(code: &'static str, message: &'static str) -> CommandError {
    CommandError::new(code, ErrorCategory::Environment, message, true)
}

#[cfg(test)]
mod tests {
    use super::{
        clear_session_on_error, close_registered_session, fixture_frame_ready, fixture_path,
        retain_observed_backend, run_registered_action,
    };
    use std::{cell::Cell, rc::Rc};
    use viewer_platform_macos::video::VideoRenderDiagnostics;

    #[test]
    fn feasibility_fixture_ids_are_an_exact_allowlist() {
        for id in ["h264-1080p", "hevc-portrait", "vfr-step"] {
            assert!(fixture_path(id).is_ok(), "missing approved fixture {id}");
        }
        assert!(fixture_path("../h264-1080p").is_err());
        assert!(fixture_path("https://example.com/video.mp4").is_err());
    }

    #[test]
    fn observed_backend_survives_end_of_file_property_unavailability() {
        let mut observed_hwdec = String::new();
        let mut observed_video_output = String::new();
        let mut active = VideoRenderDiagnostics::snapshot("videotoolbox", "libmpv");
        retain_observed_backend(&mut active, &mut observed_hwdec, &mut observed_video_output);

        let mut ended = VideoRenderDiagnostics::snapshot("", "");
        retain_observed_backend(&mut ended, &mut observed_hwdec, &mut observed_video_output);

        assert_eq!(ended.hwdec, "videotoolbox");
        assert_eq!(ended.video_output, "libmpv");
    }

    #[test]
    fn failed_mount_drops_the_registered_native_session() {
        struct DropProbe(Rc<Cell<bool>>);
        impl Drop for DropProbe {
            fn drop(&mut self) {
                self.0.set(true);
            }
        }

        let dropped = Rc::new(Cell::new(false));
        let mut slot = Some(DropProbe(Rc::clone(&dropped)));
        let result: Result<(), &str> = clear_session_on_error(&mut slot, Err("mount failed"));

        assert_eq!(result, Err("mount failed"));
        assert!(slot.is_none());
        assert!(dropped.get());
    }

    #[test]
    fn failed_close_drops_the_registered_native_session() {
        struct DropProbe(Rc<Cell<bool>>);
        impl Drop for DropProbe {
            fn drop(&mut self) {
                self.0.set(true);
            }
        }

        let dropped = Rc::new(Cell::new(false));
        let mut slot = Some(DropProbe(Rc::clone(&dropped)));
        let result: Result<(), &str> =
            close_registered_session(&mut slot, |_session| Err("diagnostics failed"));

        assert_eq!(result, Err("diagnostics failed"));
        assert!(slot.is_none());
        assert!(dropped.get());
    }

    #[test]
    fn failed_runtime_action_drops_the_registered_native_session() {
        struct DropProbe(Rc<Cell<bool>>);
        impl Drop for DropProbe {
            fn drop(&mut self) {
                self.0.set(true);
            }
        }

        let dropped = Rc::new(Cell::new(false));
        let mut slot = Some(DropProbe(Rc::clone(&dropped)));
        let result: Result<(), &str> =
            run_registered_action(&mut slot, |_session| Err("draw failed"));

        assert_eq!(result, Err("draw failed"));
        assert!(slot.is_none());
        assert!(dropped.get());
    }

    #[test]
    fn render_context_wake_does_not_count_as_a_fixture_frame() {
        let baseline_after_context_wake = 8;
        assert!(!fixture_frame_ready(8, baseline_after_context_wake));
        assert!(fixture_frame_ready(9, baseline_after_context_wake));
    }

    #[test]
    fn macos_private_api_is_limited_to_the_feasibility_feature() {
        let manifest_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let cargo_manifest = std::fs::read_to_string(manifest_root.join("Cargo.toml"))
            .expect("read desktop Cargo.toml");
        let base_config = std::fs::read_to_string(manifest_root.join("tauri.conf.json"))
            .expect("read base Tauri config");
        let matrix_runner =
            std::fs::read_to_string(manifest_root.join("../scripts/video/render-feasibility.mjs"))
                .expect("read feasibility matrix runner");

        assert!(cargo_manifest.contains(
            "video-feasibility = [\"dep:viewer-video-mpv\", \"tauri/macos-private-api\"]"
        ));
        assert!(cargo_manifest.contains("tauri = { version = \"2\", features = [] }"));
        assert!(!base_config.contains("macOSPrivateApi"));
        assert!(!matrix_runner.contains("macOSPrivateApi"));
    }
}
