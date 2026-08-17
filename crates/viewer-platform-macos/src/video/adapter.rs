#![allow(deprecated)]

use super::{
    MacVideoRenderSession, MacVideoSurface, SurfaceRect as MacSurfaceRect,
    VideoDiagnosticsCounters, VideoRenderDiagnostics,
    interaction::LatestGeometryMailbox,
    media_worker::{MediaCommandWorker, MediaWorkerEvent},
};
use async_trait::async_trait;
use dispatch2::{DispatchQueue, DispatchTime};
use objc2::MainThreadMarker;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard, OnceLock, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tauri::{Runtime, WebviewWindow};
use viewer_application::{
    EngineEvent, EngineOpenRequest, FrameDirection, PlaybackRate, SeekIntent, SeekRequest,
    SurfaceRect, VideoEngine, VideoEngineError,
};
use viewer_video_mpv::{
    FrameDirection as MpvFrameDirection, MpvClient, MpvLibrary, PlaybackRate as MpvPlaybackRate,
    runtime_manifest::RuntimeLayout,
};

type EngineEventSink = dyn Fn(u64, EngineEvent) + Send + Sync;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingCompletion {
    Step,
}

const GEOMETRY_REDRAW_INTERVAL: Duration = Duration::from_millis(33);

impl PendingCompletion {
    const fn event_if_ready(self, time_us: u64) -> Option<EngineEvent> {
        match self {
            Self::Step => Some(EngineEvent::FrameStepped { time_us }),
        }
    }
}

pub struct MacOsLibmpvAdapter {
    session: Mutex<Option<MacVideoRenderSession>>,
    generation: Mutex<Option<u64>>,
    bundle_root: PathBuf,
    diagnostics: Arc<VideoDiagnosticsCounters>,
    event_sink: Mutex<Option<Arc<EngineEventSink>>>,
    pending_completion: Mutex<Option<PendingCompletion>>,
    media_worker: Mutex<Option<MediaCommandWorker>>,
    geometry_mailbox: Mutex<LatestGeometryMailbox>,
    self_reference: OnceLock<Weak<Self>>,
    first_frame_published: AtomicBool,
    ended_published: AtomicBool,
}

// SAFETY: AppKit, OpenGL, and render-context values inside `session` are
// created, accessed, and destroyed only through `on_main`. The media worker
// owns only libmpv's documented thread-safe command handle; it never receives
// an AppKit or render-context value.
unsafe impl Send for MacOsLibmpvAdapter {}
// SAFETY: see the `Send` invariant. Every surface/render operation hops to the
// main queue, while worker state and libmpv command calls use their dedicated
// synchronized ownership boundaries.
unsafe impl Sync for MacOsLibmpvAdapter {}

impl MacOsLibmpvAdapter {
    pub fn new(bundle_root: PathBuf) -> Self {
        Self {
            session: Mutex::new(None),
            generation: Mutex::new(None),
            bundle_root,
            diagnostics: Arc::new(VideoDiagnosticsCounters::default()),
            event_sink: Mutex::new(None),
            pending_completion: Mutex::new(None),
            media_worker: Mutex::new(None),
            geometry_mailbox: Mutex::new(LatestGeometryMailbox::default()),
            self_reference: OnceLock::new(),
            first_frame_published: AtomicBool::new(false),
            ended_published: AtomicBool::new(false),
        }
    }

    pub fn set_event_sink(&self, sink: Arc<EngineEventSink>) {
        *lock(&self.event_sink) = Some(sink);
    }

    pub async fn prepare_surface<R: Runtime>(
        self: &Arc<Self>,
        window: WebviewWindow<R>,
        rect: SurfaceRect,
    ) -> Result<(), VideoEngineError> {
        self.self_reference.get_or_init(|| Arc::downgrade(self));
        let weak = Arc::downgrade(self);
        self.on_main(move |adapter| {
            adapter.close_on_main();
            let layout = RuntimeLayout::from_bundle_root(&adapter.bundle_root)
                .map_err(|_| VideoEngineError::Initialization)?;
            let library =
                MpvLibrary::load(&layout).map_err(|_| VideoEngineError::Initialization)?;
            let mut client =
                MpvClient::new(&library).map_err(|_| VideoEngineError::Initialization)?;
            client
                .initialize_for_rendering()
                .map_err(|_| VideoEngineError::Initialization)?;
            let command_client = client.command_client();
            let surface = MacVideoSurface::mount(&window, mac_rect(rect))
                .map_err(|_| VideoEngineError::RenderSurface)?;
            let draw_weak = Weak::clone(&weak);
            let schedule_draw: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
                let weak = Weak::clone(&draw_weak);
                DispatchQueue::main().exec_async(move || {
                    if let Some(adapter) = weak.upgrade() {
                        adapter.draw_on_main();
                    }
                });
            });
            let session =
                MacVideoRenderSession::new_first_frame_gated(client, surface, schedule_draw)
                    .map_err(|_| VideoEngineError::RenderSurface)?;
            let event_weak = Weak::clone(&weak);
            let worker = MediaCommandWorker::start(command_client, move |event| {
                if let Some(adapter) = event_weak.upgrade() {
                    adapter.handle_media_worker_event(event);
                }
            })?;
            *lock(&adapter.media_worker) = Some(worker);
            *lock(&adapter.session) = Some(session);
            adapter
                .first_frame_published
                .store(false, Ordering::Release);
            adapter.ended_published.store(false, Ordering::Release);
            *lock(&adapter.pending_completion) = None;
            Ok(())
        })
    }

    pub fn diagnostics(&self) -> VideoRenderDiagnostics {
        self.on_main(|adapter| {
            let session = lock(&adapter.session);
            let mut diagnostics = match session.as_ref() {
                Some(session) => session
                    .diagnostics()
                    .map_err(|_| VideoEngineError::Unavailable),
                None => Ok(adapter.diagnostics.snapshot("", "")),
            }?;
            diagnostics.interaction = adapter.diagnostics.interaction_snapshot();
            Ok(diagnostics)
        })
        .unwrap_or_else(|_| self.diagnostics.snapshot("", ""))
    }

    pub fn active_generation(&self) -> Option<u64> {
        *lock(&self.generation)
    }

    fn on_main<T: Send>(
        &self,
        action: impl FnOnce(&Self) -> Result<T, VideoEngineError> + Send,
    ) -> Result<T, VideoEngineError> {
        if MainThreadMarker::new().is_some() {
            return action(self);
        }
        let output = Mutex::new(None);
        DispatchQueue::main().exec_sync(|| {
            *lock(&output) = Some(action(self));
        });
        output
            .into_inner()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .expect("main queue action always records a result")
    }

    fn with_generation(
        &self,
        generation: u64,
        action: impl FnOnce(&mut MacVideoRenderSession) -> Result<(), VideoEngineError>,
    ) -> Result<(), VideoEngineError> {
        if *lock(&self.generation) != Some(generation) {
            return Err(VideoEngineError::StaleGeneration);
        }
        let mut session = lock(&self.session);
        let session = session.as_mut().ok_or(VideoEngineError::Unavailable)?;
        action(session)
    }

    fn schedule_geometry_drain(&self) -> Result<(), VideoEngineError> {
        let weak = self
            .self_reference
            .get()
            .cloned()
            .ok_or(VideoEngineError::Unavailable)?;
        DispatchQueue::main().exec_async(move || {
            if let Some(adapter) = weak.upgrade() {
                adapter.drain_geometry_on_main();
            }
        });
        Ok(())
    }

    fn schedule_geometry_redraw(&self, generation: u64) -> Result<(), VideoEngineError> {
        let weak = self
            .self_reference
            .get()
            .cloned()
            .ok_or(VideoEngineError::Unavailable)?;
        let deadline = DispatchTime::try_from(GEOMETRY_REDRAW_INTERVAL)
            .map_err(|()| VideoEngineError::Unavailable)?;
        DispatchQueue::main()
            .after(deadline, move || {
                if let Some(adapter) = weak.upgrade() {
                    adapter.redraw_geometry_on_main(generation);
                }
            })
            .map_err(|_| VideoEngineError::Unavailable)
    }

    fn drain_geometry_on_main(&self) {
        debug_assert!(MainThreadMarker::new().is_some());
        let update = lock(&self.geometry_mailbox).take_for_drain();
        if let Some(update) = update {
            let result = self.with_generation(update.generation, |session| {
                session
                    .update_geometry(mac_rect(update.rect))
                    .map_err(|_| VideoEngineError::RenderSurface)
            });
            if result.is_ok() {
                self.diagnostics.record_geometry_application();
                if lock(&self.geometry_mailbox)
                    .mark_applied_for_redraw(update.generation, update.sequence)
                {
                    let _ = self.schedule_geometry_redraw(update.generation);
                }
            }
        }
        if lock(&self.geometry_mailbox).finish_drain() {
            let _ = self.schedule_geometry_drain();
        }
    }

    fn redraw_geometry_on_main(&self, generation: u64) {
        debug_assert!(MainThreadMarker::new().is_some());
        if lock(&self.geometry_mailbox)
            .take_for_redraw(generation)
            .is_some()
        {
            let _ = self.with_generation(generation, |session| {
                session
                    .redraw_retained_frame()
                    .map_err(|_| VideoEngineError::RenderSurface)
            });
        }
        if lock(&self.geometry_mailbox).finish_redraw(generation) {
            let _ = self.schedule_geometry_redraw(generation);
        }
    }

    fn draw_on_main(&self) {
        debug_assert!(MainThreadMarker::new().is_some());
        let rendered = {
            let mut session = lock(&self.session);
            let Some(session) = session.as_mut() else {
                return;
            };
            session.draw_if_needed().ok().flatten()
        };
        let (Some(generation), Some(rendered)) = (*lock(&self.generation), rendered) else {
            return;
        };
        if let Some(worker) = lock(&self.media_worker).as_ref() {
            let _ = worker.frame_rendered(generation, rendered.serial);
        }
    }

    fn handle_media_worker_event(&self, event: MediaWorkerEvent) {
        match event {
            MediaWorkerEvent::SeekIssued {
                generation, intent, ..
            } => {
                if *lock(&self.generation) != Some(generation) {
                    self.diagnostics.record_stale_completion_rejection();
                    return;
                }
                match intent {
                    SeekIntent::Preview => self.diagnostics.record_preview_issue(),
                    SeekIntent::Commit => self.diagnostics.record_commit_issue(),
                }
            }
            MediaWorkerEvent::FrameSnapshot {
                generation,
                frame_serial,
                snapshot,
            } => {
                if *lock(&self.generation) != Some(generation) {
                    self.diagnostics.record_stale_completion_rejection();
                    return;
                }
                self.schedule_frame_confirmation(generation, frame_serial, snapshot.picture_type);
                if let Some(time_us) = snapshot.time_us {
                    self.emit(
                        generation,
                        EngineEvent::TimeChanged {
                            time_us,
                            duration_us: None,
                        },
                    );
                    let completion = {
                        let mut pending = lock(&self.pending_completion);
                        let event =
                            pending.and_then(|completion| completion.event_if_ready(time_us));
                        if event.is_some() {
                            pending.take();
                        }
                        event
                    };
                    if let Some(completion) = completion {
                        self.emit(generation, completion);
                    }
                }
                if snapshot.eof_reached && !self.ended_published.swap(true, Ordering::AcqRel) {
                    self.emit(generation, EngineEvent::Ended);
                }
            }
            MediaWorkerEvent::SeekCompleted {
                generation,
                request_id,
                time_us,
            } => {
                if *lock(&self.generation) == Some(generation) {
                    self.emit(
                        generation,
                        EngineEvent::SeekCompleted {
                            request_id,
                            time_us,
                        },
                    );
                } else {
                    self.diagnostics.record_stale_completion_rejection();
                }
            }
            MediaWorkerEvent::Failed { generation } => {
                if *lock(&self.generation) == Some(generation) {
                    self.emit(
                        generation,
                        EngineEvent::Failed(
                            viewer_domain::video::VideoFailureKind::DecodeFallbackFailed,
                        ),
                    );
                }
            }
        }
    }

    fn schedule_frame_confirmation(
        &self,
        generation: u64,
        frame_serial: u64,
        picture_type: Option<String>,
    ) {
        let Some(weak) = self.self_reference.get().cloned() else {
            return;
        };
        DispatchQueue::main().exec_async(move || {
            if let Some(adapter) = weak.upgrade() {
                adapter.confirm_frame_on_main(generation, frame_serial, picture_type);
            }
        });
    }

    fn confirm_frame_on_main(
        &self,
        generation: u64,
        frame_serial: u64,
        picture_type: Option<String>,
    ) {
        debug_assert!(MainThreadMarker::new().is_some());
        let first_frame = {
            let mut session = lock(&self.session);
            let latest_serial = session
                .as_ref()
                .and_then(MacVideoRenderSession::latest_rendered_frame_serial);
            if !frame_snapshot_matches_active_render(
                *lock(&self.generation),
                generation,
                frame_serial,
                latest_serial,
            ) {
                self.diagnostics.record_stale_completion_rejection();
                return;
            }
            session
                .as_mut()
                .and_then(|session| {
                    session
                        .confirm_first_decoded_frame(frame_serial, picture_type)
                        .ok()
                })
                .unwrap_or(false)
                && !self.first_frame_published.swap(true, Ordering::AcqRel)
        };
        if first_frame {
            self.emit(generation, EngineEvent::FirstFrameReady);
        }
    }

    fn emit(&self, generation: u64, event: EngineEvent) {
        let sink = lock(&self.event_sink).clone();
        if let Some(sink) = sink {
            sink(generation, event);
        }
    }

    fn close_on_main(&self) {
        debug_assert!(MainThreadMarker::new().is_some());
        *lock(&self.generation) = None;
        self.first_frame_published.store(false, Ordering::Release);
        self.ended_published.store(false, Ordering::Release);
        *lock(&self.pending_completion) = None;
        lock(&self.geometry_mailbox).invalidate();
        let worker = lock(&self.media_worker).take();
        let session = lock(&self.session).take();
        drop_in_media_shutdown_order(worker, session);
    }
}

#[async_trait]
impl VideoEngine for MacOsLibmpvAdapter {
    async fn open_paused(&self, request: EngineOpenRequest) -> Result<(), VideoEngineError> {
        self.on_main(move |adapter| {
            let mut session = lock(&adapter.session);
            let session = session.as_mut().ok_or(VideoEngineError::Unavailable)?;
            session
                .open_local_file_paused(&request.source.canonical_path)
                .map_err(|_| VideoEngineError::Decode)?;
            lock(&adapter.media_worker)
                .as_ref()
                .ok_or(VideoEngineError::Unavailable)?
                .activate(request.generation)?;
            *lock(&adapter.generation) = Some(request.generation);
            lock(&adapter.geometry_mailbox).activate(request.generation);
            adapter
                .first_frame_published
                .store(false, Ordering::Release);
            adapter.ended_published.store(false, Ordering::Release);
            Ok(())
        })
    }

    async fn reveal_surface(&self, generation: u64) -> Result<(), VideoEngineError> {
        self.on_main(move |adapter| {
            adapter.with_generation(generation, |session| {
                session
                    .reveal_surface()
                    .map_err(|_| VideoEngineError::RenderSurface)
            })
        })
    }

    async fn close(&self, generation: u64) -> Result<(), VideoEngineError> {
        self.on_main(move |adapter| {
            let active = *lock(&adapter.generation);
            if active.is_some_and(|active| active != generation) {
                return Err(VideoEngineError::StaleGeneration);
            }
            adapter.close_on_main();
            Ok(())
        })
    }

    async fn play(&self, generation: u64) -> Result<(), VideoEngineError> {
        self.on_main(move |adapter| {
            adapter.with_generation(generation, |session| {
                session.play().map_err(|_| VideoEngineError::Decode)
            })
        })
    }

    async fn pause(&self, generation: u64) -> Result<(), VideoEngineError> {
        self.on_main(move |adapter| {
            adapter.with_generation(generation, |session| {
                session.pause().map_err(|_| VideoEngineError::Decode)
            })
        })
    }

    fn publish_seek(&self, generation: u64, request: SeekRequest) -> Result<(), VideoEngineError> {
        lock(&self.media_worker)
            .as_ref()
            .ok_or(VideoEngineError::Unavailable)?
            .publish_seek(generation, request)?;
        match request.intent {
            SeekIntent::Preview => self
                .diagnostics
                .record_preview_publication(request.request_id, false),
            SeekIntent::Commit => self
                .diagnostics
                .record_commit_publication(request.request_id, false),
        }
        Ok(())
    }

    async fn step(
        &self,
        generation: u64,
        direction: FrameDirection,
    ) -> Result<(), VideoEngineError> {
        self.on_main(move |adapter| {
            adapter.with_generation(generation, |session| {
                session
                    .frame_step(match direction {
                        FrameDirection::Backward => MpvFrameDirection::Backward,
                        FrameDirection::Forward => MpvFrameDirection::Forward,
                    })
                    .map_err(|_| VideoEngineError::Decode)
            })?;
            *lock(&adapter.pending_completion) = Some(PendingCompletion::Step);
            Ok(())
        })
    }

    async fn set_volume(&self, generation: u64, percent: u8) -> Result<(), VideoEngineError> {
        self.on_main(move |adapter| {
            adapter.with_generation(generation, |session| {
                session
                    .set_volume(percent)
                    .map_err(|_| VideoEngineError::Decode)
            })
        })
    }

    async fn set_muted(&self, generation: u64, muted: bool) -> Result<(), VideoEngineError> {
        self.on_main(move |adapter| {
            adapter.with_generation(generation, |session| {
                session
                    .set_muted(muted)
                    .map_err(|_| VideoEngineError::Decode)
            })
        })
    }

    async fn set_rate(&self, generation: u64, rate: PlaybackRate) -> Result<(), VideoEngineError> {
        self.on_main(move |adapter| {
            adapter.with_generation(generation, |session| {
                session
                    .set_rate(match rate {
                        PlaybackRate::Half => MpvPlaybackRate::Half,
                        PlaybackRate::ThreeQuarters => MpvPlaybackRate::ThreeQuarters,
                        PlaybackRate::Normal => MpvPlaybackRate::Normal,
                        PlaybackRate::OneAndQuarter => MpvPlaybackRate::FiveQuarters,
                        PlaybackRate::OneAndHalf => MpvPlaybackRate::OneAndHalf,
                        PlaybackRate::Double => MpvPlaybackRate::Double,
                    })
                    .map_err(|_| VideoEngineError::Decode)
            })
        })
    }

    fn publish_surface_rect(
        &self,
        generation: u64,
        sequence: u64,
        rect: SurfaceRect,
    ) -> Result<(), VideoEngineError> {
        let schedule = lock(&self.geometry_mailbox).publish(generation, sequence, rect)?;
        self.diagnostics
            .record_geometry_publication(sequence, !schedule);
        if schedule {
            self.schedule_geometry_drain()?;
        }
        Ok(())
    }
}

fn mac_rect(rect: SurfaceRect) -> MacSurfaceRect {
    MacSurfaceRect {
        x: f64::from(rect.x),
        y: f64::from(rect.y),
        width: f64::from(rect.width),
        height: f64::from(rect.height),
    }
}

fn frame_snapshot_matches_active_render(
    active_generation: Option<u64>,
    event_generation: u64,
    event_frame_serial: u64,
    latest_rendered_serial: Option<u64>,
) -> bool {
    active_generation == Some(event_generation)
        && latest_rendered_serial == Some(event_frame_serial)
}

fn drop_in_media_shutdown_order<W, S>(worker: Option<W>, session: Option<S>) {
    drop(worker);
    drop(session);
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{
        PendingCompletion, drop_in_media_shutdown_order, frame_snapshot_matches_active_render,
    };
    use viewer_application::EngineEvent;

    #[test]
    fn pending_step_completes_with_the_drawn_frame_time() {
        assert_eq!(
            PendingCompletion::Step.event_if_ready(800_000),
            Some(EngineEvent::FrameStepped { time_us: 800_000 })
        );
    }

    #[test]
    fn first_frame_snapshot_is_generation_and_frame_serial_guarded() {
        assert!(frame_snapshot_matches_active_render(
            Some(7),
            7,
            42,
            Some(42)
        ));
        assert!(!frame_snapshot_matches_active_render(
            Some(7),
            6,
            42,
            Some(42)
        ));
        assert!(!frame_snapshot_matches_active_render(
            Some(7),
            7,
            41,
            Some(42)
        ));
        assert!(!frame_snapshot_matches_active_render(None, 7, 42, Some(42)));
    }

    #[test]
    fn media_worker_is_shutdown_before_native_session_teardown() {
        #[derive(Clone)]
        struct TracedDrop {
            name: &'static str,
            trace: Arc<Mutex<Vec<&'static str>>>,
        }

        impl Drop for TracedDrop {
            fn drop(&mut self) {
                self.trace.lock().unwrap().push(self.name);
            }
        }

        let trace = Arc::new(Mutex::new(Vec::new()));
        drop_in_media_shutdown_order(
            Some(TracedDrop {
                name: "worker",
                trace: Arc::clone(&trace),
            }),
            Some(TracedDrop {
                name: "session",
                trace: Arc::clone(&trace),
            }),
        );

        assert_eq!(*trace.lock().unwrap(), ["worker", "session"]);
    }
}
