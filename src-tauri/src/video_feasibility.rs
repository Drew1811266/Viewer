use crate::error::{CommandError, ErrorCategory};
use dispatch2::DispatchQueue;
use objc2::{msg_send, runtime::AnyObject};
use objc2_app_kit::NSColor;
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow, webview::Color};
use viewer_platform_macos::video::{
    MacVideoRenderSession, MacVideoSurface, SurfaceRect, VideoRenderDiagnostics,
};
use viewer_video_mpv::{FrameDirection, MpvClient, MpvLibrary, runtime_manifest::RuntimeLayout};

const FRAME_EVENT: &str = "viewer://video-feasibility-frame";

thread_local! {
    static ACTIVE_SESSION: RefCell<Option<ActiveFeasibilitySession>> = const { RefCell::new(None) };
    static NEXT_GENERATION: Cell<u64> = const { Cell::new(0) };
}

struct ActiveFeasibilitySession {
    fixture_id: String,
    rect: SurfaceRectDto,
    session: MacVideoRenderSession,
    initial_rendered_frames: u64,
    forward_step: bool,
    backward_step: bool,
    command_serial: u64,
    last_command_latency_us: u64,
    observed_hwdec: String,
    observed_video_output: String,
    generation_probe: Arc<FeasibilityGenerationProbe>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct FeasibilityGenerationDiagnostics {
    generation: u64,
    mount_returned: bool,
    update_callbacks: u64,
    draw_entries: u64,
    frame_updates: u64,
    picture_frames: u64,
    reveals: u64,
    event_emits: u64,
}

#[derive(Debug)]
struct FeasibilityGenerationProbe {
    generation: u64,
    mount_returned: AtomicBool,
    update_callbacks: AtomicU64,
    draw_entries: AtomicU64,
    frame_updates: AtomicU64,
    picture_frames: AtomicU64,
    reveals: AtomicU64,
    event_emits: AtomicU64,
}

impl FeasibilityGenerationProbe {
    fn new(generation: u64) -> Self {
        Self {
            generation,
            mount_returned: AtomicBool::new(false),
            update_callbacks: AtomicU64::new(0),
            draw_entries: AtomicU64::new(0),
            frame_updates: AtomicU64::new(0),
            picture_frames: AtomicU64::new(0),
            reveals: AtomicU64::new(0),
            event_emits: AtomicU64::new(0),
        }
    }

    fn record_mount_returned(&self) {
        self.mount_returned.store(true, Ordering::Release);
    }

    fn record_update_callback(&self) {
        self.update_callbacks.fetch_add(1, Ordering::AcqRel);
    }

    fn record_draw(&self, frame_update: bool, picture_frame: bool, revealed: bool) {
        self.draw_entries.fetch_add(1, Ordering::AcqRel);
        if frame_update {
            self.frame_updates.fetch_add(1, Ordering::AcqRel);
        }
        if picture_frame {
            self.picture_frames.fetch_add(1, Ordering::AcqRel);
        }
        if revealed {
            self.reveals.fetch_add(1, Ordering::AcqRel);
        }
    }

    fn record_event_emit(&self) {
        self.event_emits.fetch_add(1, Ordering::AcqRel);
    }

    fn snapshot(&self) -> FeasibilityGenerationDiagnostics {
        FeasibilityGenerationDiagnostics {
            generation: self.generation,
            mount_returned: self.mount_returned.load(Ordering::Acquire),
            update_callbacks: self.update_callbacks.load(Ordering::Acquire),
            draw_entries: self.draw_entries.load(Ordering::Acquire),
            frame_updates: self.frame_updates.load(Ordering::Acquire),
            picture_frames: self.picture_frames.load(Ordering::Acquire),
            reveals: self.reveals.load(Ordering::Acquire),
            event_emits: self.event_emits.load(Ordering::Acquire),
        }
    }
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VideoFeasibilityAction {
    Mount,
    UpdateGeometry,
    FrameStepForward,
    FrameStepBackward,
    Play,
    Pause,
    Status,
    Close,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoFeasibilityRequest {
    fixture_id: String,
    action: VideoFeasibilityAction,
    rect: SurfaceRectDto,
    expected_generation: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoFeasibilityReport {
    fixture_id: String,
    rect: SurfaceRectDto,
    first_frame_ready: bool,
    decoded_picture_type: Option<String>,
    forward_step: bool,
    backward_step: bool,
    playback_time_us: Option<u64>,
    command_serial: u64,
    last_command_latency_us: u64,
    generation_diagnostics: FeasibilityGenerationDiagnostics,
    diagnostics: VideoRenderDiagnostics,
}

#[tauri::command]
pub async fn run_video_feasibility(
    app: AppHandle,
    window: WebviewWindow,
    request: VideoFeasibilityRequest,
) -> Result<VideoFeasibilityReport, CommandError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let cancelled_action = request.action;
    let cancelled_generation = request.expected_generation;
    DispatchQueue::main().exec_async(move || {
        let result = handle_on_main_thread(app, window, request);
        if sender.send(result).is_err() {
            ACTIVE_SESSION.with(|slot| {
                let mut slot = slot.borrow_mut();
                let active_generation = slot
                    .as_ref()
                    .map(|active| active.generation_probe.snapshot().generation);
                if cancelled_command_owns_session(
                    cancelled_action,
                    cancelled_generation,
                    active_generation,
                ) {
                    slot.take();
                }
            });
        }
    });
    receiver.await.map_err(|_| {
        feasibility_error(
            "video_feasibility_cancelled",
            "原生视频测试在返回结果前被取消。",
        )
    })?
}

fn handle_on_main_thread(
    app: AppHandle,
    window: WebviewWindow,
    request: VideoFeasibilityRequest,
) -> Result<VideoFeasibilityReport, CommandError> {
    if request.action == VideoFeasibilityAction::Close {
        return close(
            &request.fixture_id,
            request.expected_generation,
            request.rect,
        );
    }
    run_command_with_cleanup(
        || {
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
                VideoFeasibilityAction::Play => {
                    with_matching_session(&request.fixture_id, |active| {
                        let started = Instant::now();
                        active.session.play().map_err(render_error)?;
                        active.last_command_latency_us = elapsed_micros(started);
                        active.command_serial += 1;
                        active.report()
                    })
                }
                VideoFeasibilityAction::Pause => {
                    with_matching_session(&request.fixture_id, |active| {
                        let started = Instant::now();
                        active.session.pause().map_err(render_error)?;
                        active.last_command_latency_us = elapsed_micros(started);
                        active.command_serial += 1;
                        active.report()
                    })
                }
                VideoFeasibilityAction::Status => {
                    with_matching_session(&request.fixture_id, |active| active.report())
                }
                VideoFeasibilityAction::Close => unreachable!("close is handled before setup"),
            }
        },
        || {
            ACTIVE_SESSION.with(|slot| slot.borrow_mut().take());
        },
    )
}

fn mount(
    window: WebviewWindow,
    fixture_id: String,
    rect: SurfaceRectDto,
    fixture: &Path,
    bundle_resources: &Path,
) -> Result<VideoFeasibilityReport, CommandError> {
    ACTIVE_SESSION.with(|slot| slot.borrow_mut().take());
    let generation = NEXT_GENERATION.with(|next| {
        let generation = next.get() + 1;
        next.set(generation);
        generation
    });
    let generation_probe = Arc::new(FeasibilityGenerationProbe::new(generation));
    let layout = RuntimeLayout::from_bundle_root(bundle_resources).map_err(render_error)?;
    let library = MpvLibrary::load(&layout).map_err(render_error)?;
    let mut client = MpvClient::new(&library).map_err(render_error)?;
    client.initialize_for_rendering().map_err(render_error)?;
    let surface = MacVideoSurface::mount(&window, rect.into()).map_err(render_error)?;
    let scheduling_window = window.clone();
    let callback_probe = Arc::clone(&generation_probe);
    let schedule_draw: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        callback_probe.record_update_callback();
        let event_window = scheduling_window.clone();
        DispatchQueue::main().exec_async(move || {
            let report = ACTIVE_SESSION.with(|slot| {
                let mut slot = slot.borrow_mut();
                slot.as_ref()?;
                let result = run_registered_action(&mut slot, |active| {
                    active.draw_and_record().map_err(render_error)?;
                    active.generation_probe.record_event_emit();
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
            if let Some(report) = report
                && let Err(error) = event_window.emit(FRAME_EVENT, report)
            {
                eprintln!("Viewer video feasibility frame event failed: {error}");
                ACTIVE_SESSION.with(|slot| slot.borrow_mut().take());
            }
        });
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
        command_serial: 0,
        last_command_latency_us: 0,
        observed_hwdec: diagnostics.hwdec,
        observed_video_output: diagnostics.video_output,
        generation_probe,
    };
    ACTIVE_SESSION.with(|slot| {
        let mut slot = slot.borrow_mut();
        *slot = Some(active);
        let result = (|| {
            let active = slot.as_mut().expect("active session was just registered");
            active.draw_and_record().map_err(render_error)?;
            active.initial_rendered_frames = active
                .session
                .diagnostics()
                .map_err(render_error)?
                .rendered_frames;
            active
                .session
                .open_local_file_paused(fixture)
                .map_err(render_error)?;
            active.generation_probe.record_mount_returned();
            active.report()
        })();
        clear_session_on_error(&mut slot, result)
    })
}

fn close(
    fixture_id: &str,
    expected_generation: Option<u64>,
    rect: SurfaceRectDto,
) -> Result<VideoFeasibilityReport, CommandError> {
    let closed = ACTIVE_SESSION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let active = slot.as_mut().ok_or_else(no_active_session)?;
        if active.fixture_id != fixture_id {
            return Err(fixture_mismatch());
        }
        if !generation_owns_session(
            active.generation_probe.snapshot().generation,
            expected_generation,
        ) {
            return Ok(stale_close_report(active, rect));
        }
        close_registered_session(&mut slot, ActiveFeasibilitySession::report)
    })?;
    let diagnostics = VideoRenderDiagnostics::snapshot("", "");
    Ok(VideoFeasibilityReport {
        fixture_id: closed.fixture_id,
        rect,
        first_frame_ready: closed.first_frame_ready,
        decoded_picture_type: closed.decoded_picture_type,
        forward_step: closed.forward_step,
        backward_step: closed.backward_step,
        playback_time_us: closed.playback_time_us,
        command_serial: closed.command_serial,
        last_command_latency_us: closed.last_command_latency_us,
        generation_diagnostics: closed.generation_diagnostics,
        diagnostics,
    })
}

fn stale_close_report(
    active: &ActiveFeasibilitySession,
    rect: SurfaceRectDto,
) -> VideoFeasibilityReport {
    VideoFeasibilityReport {
        fixture_id: active.fixture_id.clone(),
        rect,
        first_frame_ready: false,
        decoded_picture_type: None,
        forward_step: active.forward_step,
        backward_step: active.backward_step,
        playback_time_us: None,
        command_serial: active.command_serial,
        last_command_latency_us: active.last_command_latency_us,
        generation_diagnostics: active.generation_probe.snapshot(),
        diagnostics: VideoRenderDiagnostics::snapshot(
            &active.observed_hwdec,
            &active.observed_video_output,
        ),
    }
}

fn generation_owns_session(active_generation: u64, expected_generation: Option<u64>) -> bool {
    match expected_generation {
        Some(expected) => active_generation == expected,
        None => true,
    }
}

fn cancelled_command_owns_session(
    action: VideoFeasibilityAction,
    expected_generation: Option<u64>,
    active_generation: Option<u64>,
) -> bool {
    if action != VideoFeasibilityAction::Close {
        return true;
    }
    match (expected_generation, active_generation) {
        (Some(expected), Some(active)) => expected == active,
        (Some(_), None) => false,
        (None, _) => true,
    }
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
    fn draw_and_record(&mut self) -> Result<bool, viewer_platform_macos::video::RenderLoopError> {
        let revealed_before = self.session.first_decoded_frame_revealed();
        let rendered = self.session.draw_if_needed()?;
        if let Some(rendered) = rendered {
            let picture_type = self.session.sample_decoded_picture_type()?;
            self.session
                .confirm_first_decoded_frame(rendered.serial, picture_type)?;
        }
        let frame_update = rendered.is_some();
        let picture_frame = frame_update && self.session.decoded_picture_type().is_some();
        let revealed = !revealed_before && self.session.first_decoded_frame_revealed();
        self.generation_probe
            .record_draw(frame_update, picture_frame, revealed);
        Ok(frame_update)
    }

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
                self.session.first_decoded_frame_revealed(),
            ),
            decoded_picture_type: self.session.decoded_picture_type().map(str::to_owned),
            forward_step: self.forward_step,
            backward_step: self.backward_step,
            playback_time_us,
            command_serial: self.command_serial,
            last_command_latency_us: self.last_command_latency_us,
            generation_diagnostics: self.generation_probe.snapshot(),
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

fn run_command_with_cleanup<R, E>(
    command: impl FnOnce() -> Result<R, E>,
    cleanup: impl FnOnce(),
) -> Result<R, E> {
    let result = command();
    if result.is_err() {
        cleanup();
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

fn fixture_frame_ready(
    rendered_frames: u64,
    fixture_baseline: u64,
    decoded_frame_revealed: bool,
) -> bool {
    rendered_frames > fixture_baseline && decoded_frame_revealed
}

fn elapsed_micros(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
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
        "h264-1080p60" => "../../../target/video-performance/h264-1080p60.mp4",
        "hevc-4k30" => "../../../target/video-performance/hevc-4k30.mov",
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
        FeasibilityGenerationProbe, VideoFeasibilityAction, cancelled_command_owns_session,
        clear_session_on_error, close_registered_session, fixture_frame_ready, fixture_path,
        generation_owns_session, retain_observed_backend, run_command_with_cleanup,
        run_registered_action,
    };
    use std::{cell::Cell, rc::Rc};
    use viewer_platform_macos::video::VideoRenderDiagnostics;

    #[test]
    fn feasibility_fixture_ids_are_an_exact_allowlist() {
        for id in [
            "h264-1080p",
            "hevc-portrait",
            "vfr-step",
            "h264-1080p60",
            "hevc-4k30",
        ] {
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
    fn failed_command_precondition_drops_an_existing_native_session() {
        struct DropProbe(Rc<Cell<bool>>);
        impl Drop for DropProbe {
            fn drop(&mut self) {
                self.0.set(true);
            }
        }

        let dropped = Rc::new(Cell::new(false));
        let mut slot = Some(DropProbe(Rc::clone(&dropped)));
        let result: Result<(), &str> = run_command_with_cleanup(
            || Err("fixture rejected"),
            || {
                slot.take();
            },
        );

        assert_eq!(result, Err("fixture rejected"));
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
        assert!(!fixture_frame_ready(8, baseline_after_context_wake, false));
        assert!(!fixture_frame_ready(9, baseline_after_context_wake, false));
        assert!(fixture_frame_ready(9, baseline_after_context_wake, true));
    }

    #[test]
    fn feasibility_pipeline_counters_are_bound_to_their_mount_generation() {
        let first = FeasibilityGenerationProbe::new(41);
        let second = FeasibilityGenerationProbe::new(42);

        first.record_mount_returned();
        first.record_update_callback();
        first.record_draw(true, true, true);
        first.record_event_emit();
        second.record_mount_returned();
        second.record_update_callback();
        second.record_draw(false, false, false);

        assert_eq!(
            first.snapshot(),
            super::FeasibilityGenerationDiagnostics {
                generation: 41,
                mount_returned: true,
                update_callbacks: 1,
                draw_entries: 1,
                frame_updates: 1,
                picture_frames: 1,
                reveals: 1,
                event_emits: 1,
            }
        );
        assert_eq!(
            second.snapshot(),
            super::FeasibilityGenerationDiagnostics {
                generation: 42,
                mount_returned: true,
                update_callbacks: 1,
                draw_entries: 1,
                frame_updates: 0,
                picture_frames: 0,
                reveals: 0,
                event_emits: 0,
            }
        );
    }

    #[test]
    fn a_late_close_cannot_own_a_newer_same_fixture_generation() {
        assert!(generation_owns_session(8, Some(8)));
        assert!(!generation_owns_session(9, Some(8)));
        assert!(generation_owns_session(9, None));
        assert!(!cancelled_command_owns_session(
            VideoFeasibilityAction::Close,
            Some(8),
            Some(9),
        ));
        assert!(cancelled_command_owns_session(
            VideoFeasibilityAction::Close,
            Some(9),
            Some(9),
        ));
    }

    #[test]
    fn dispatch_and_bundle_source_have_no_fallible_or_override_escape_hatches() {
        let source = include_str!("video_feasibility.rs");
        let fallible_dispatch = ["run", "on", "main", "thread"].join("_");
        let bundle_override = ["VIEWER", "VIDEO", "BUNDLE", "RESOURCES"].join("_");
        assert!(source.contains("DispatchQueue::main().exec_async"));
        assert!(!source.contains(&fallible_dispatch));
        assert!(!source.contains(&bundle_override));
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

        assert!(cargo_manifest.contains("video-feasibility = ["));
        assert!(cargo_manifest.contains("dispatch2.workspace = true"));
        assert!(cargo_manifest.contains("viewer-video-mpv = { path ="));
        assert!(!cargo_manifest.contains("\"dep:viewer-video-mpv\""));
        assert!(cargo_manifest.contains("\"tauri/macos-private-api\""));
        assert!(cargo_manifest.contains("tauri = { version = \"2\", features = [] }"));
        assert!(!base_config.contains("macOSPrivateApi"));
        assert!(!matrix_runner.contains("macOSPrivateApi"));
    }
}
