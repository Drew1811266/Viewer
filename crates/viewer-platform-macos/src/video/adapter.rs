#![allow(deprecated)]

use super::{
    MacVideoRenderSession, MacVideoSurface, SurfaceRect as MacSurfaceRect,
    VideoDiagnosticsCounters, VideoRenderDiagnostics,
    interaction::{LatestGeometryMailbox, LatestSeekMailbox},
};
use async_trait::async_trait;
use dispatch2::DispatchQueue;
use objc2::MainThreadMarker;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard, OnceLock, Weak,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{Runtime, WebviewWindow};
use viewer_application::{
    EngineEvent, EngineOpenRequest, FrameDirection, PlaybackRate, SeekIntent, SeekRequest,
    SurfaceRect, VideoEngine, VideoEngineError,
};
use viewer_video_mpv::{
    FrameDirection as MpvFrameDirection, MpvClient, MpvLibrary, PlaybackRate as MpvPlaybackRate,
    SeekMode, runtime_manifest::RuntimeLayout,
};

type EngineEventSink = dyn Fn(u64, EngineEvent) + Send + Sync;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingCompletion {
    Seek {
        request_id: u64,
        target_time_us: u64,
    },
    Step,
}

const COMMIT_COMPLETION_TOLERANCE_US: u64 = 50_000;

impl PendingCompletion {
    const fn event_if_ready(self, time_us: u64) -> Option<EngineEvent> {
        match self {
            Self::Seek {
                request_id,
                target_time_us,
            } => {
                if time_us.abs_diff(target_time_us) > COMMIT_COMPLETION_TOLERANCE_US {
                    return None;
                }
                Some(EngineEvent::SeekCompleted {
                    request_id,
                    time_us,
                })
            }
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
    seek_mailbox: Mutex<LatestSeekMailbox>,
    geometry_mailbox: Mutex<LatestGeometryMailbox>,
    self_reference: OnceLock<Weak<Self>>,
    first_frame_published: AtomicBool,
    ended_published: AtomicBool,
}

// SAFETY: AppKit, OpenGL, and render-context values inside `session` are
// created, accessed, and destroyed only through `on_main`. The mutexes make
// outer scheduling/state access serialized; they do not grant native access
// away from AppKit's main thread.
unsafe impl Send for MacOsLibmpvAdapter {}
// SAFETY: see the `Send` invariant. Every native operation synchronously hops
// to the main queue before locking or touching the native session.
unsafe impl Sync for MacOsLibmpvAdapter {}

impl MacOsLibmpvAdapter {
    pub fn new(bundle_root: PathBuf) -> Self {
        Self {
            session: Mutex::new(None),
            generation: Mutex::new(None),
            bundle_root,
            diagnostics: Arc::new(VideoDiagnosticsCounters),
            event_sink: Mutex::new(None),
            pending_completion: Mutex::new(None),
            seek_mailbox: Mutex::new(LatestSeekMailbox::default()),
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
            let surface = MacVideoSurface::mount(&window, mac_rect(rect))
                .map_err(|_| VideoEngineError::RenderSurface)?;
            let schedule_draw: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
                let weak = Weak::clone(&weak);
                DispatchQueue::main().exec_async(move || {
                    if let Some(adapter) = weak.upgrade() {
                        adapter.draw_on_main();
                    }
                });
            });
            let session =
                MacVideoRenderSession::new_first_frame_gated(client, surface, schedule_draw)
                    .map_err(|_| VideoEngineError::RenderSurface)?;
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
            match session.as_ref() {
                Some(session) => session
                    .diagnostics()
                    .map_err(|_| VideoEngineError::Unavailable),
                None => Ok(adapter.diagnostics.snapshot("", "")),
            }
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

    fn schedule_seek_drain(&self) -> Result<(), VideoEngineError> {
        let weak = self
            .self_reference
            .get()
            .cloned()
            .ok_or(VideoEngineError::Unavailable)?;
        DispatchQueue::main().exec_async(move || {
            if let Some(adapter) = weak.upgrade() {
                adapter.drain_seek_on_main();
            }
        });
        Ok(())
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

    fn drain_seek_on_main(&self) {
        debug_assert!(MainThreadMarker::new().is_some());
        let request = lock(&self.seek_mailbox).take_for_drain();
        if let Some(request) = request {
            let generation = *lock(&self.generation);
            let result = generation
                .ok_or(VideoEngineError::StaleGeneration)
                .and_then(|generation| {
                    self.with_generation(generation, |session| {
                        session
                            .seek(
                                request.time_us,
                                match request.intent {
                                    SeekIntent::Preview => SeekMode::PreviewKeyframe,
                                    SeekIntent::Commit => SeekMode::CommitExact,
                                },
                            )
                            .map_err(|_| VideoEngineError::Decode)
                    })
                });
            if result.is_ok() && request.intent == SeekIntent::Commit {
                *lock(&self.pending_completion) = Some(PendingCompletion::Seek {
                    request_id: request.request_id,
                    target_time_us: request.time_us,
                });
            } else if result.is_err()
                && request.intent == SeekIntent::Commit
                && let Some(generation) = generation
            {
                self.emit(
                    generation,
                    EngineEvent::Failed(
                        viewer_domain::video::VideoFailureKind::DecodeFallbackFailed,
                    ),
                );
            }
        }
        if lock(&self.seek_mailbox).finish_drain() {
            let _ = self.schedule_seek_drain();
        }
    }

    fn drain_geometry_on_main(&self) {
        debug_assert!(MainThreadMarker::new().is_some());
        let update = lock(&self.geometry_mailbox).take_for_drain();
        if let Some(update) = update {
            let _ = self.with_generation(update.generation, |session| {
                session
                    .update_geometry_and_redraw(mac_rect(update.rect))
                    .map_err(|_| VideoEngineError::RenderSurface)
            });
        }
        if lock(&self.geometry_mailbox).finish_drain() {
            let _ = self.schedule_geometry_drain();
        }
    }

    fn draw_on_main(&self) {
        debug_assert!(MainThreadMarker::new().is_some());
        let generation = *lock(&self.generation);
        let (first_frame, progress, ended) = {
            let mut session = lock(&self.session);
            let Some(session) = session.as_mut() else {
                return;
            };
            let drew = session.draw_if_needed().unwrap_or(false);
            (
                drew && session.first_decoded_frame_ready()
                    && !self.first_frame_published.swap(true, Ordering::AcqRel),
                drew.then(|| session.playback_time_us().ok().flatten())
                    .flatten(),
                drew && session.eof_reached().unwrap_or(false)
                    && !self.ended_published.swap(true, Ordering::AcqRel),
            )
        };
        let Some(generation) = generation else {
            return;
        };
        if first_frame {
            self.emit(generation, EngineEvent::FirstFrameReady);
        }
        if let Some(time_us) = progress {
            self.emit(
                generation,
                EngineEvent::TimeChanged {
                    time_us,
                    duration_us: None,
                },
            );
            let completion = {
                let mut pending = lock(&self.pending_completion);
                let event = pending.and_then(|completion| completion.event_if_ready(time_us));
                if event.is_some() {
                    pending.take();
                }
                event
            };
            if let Some(completion) = completion {
                self.emit(generation, completion);
            }
        }
        if ended {
            self.emit(generation, EngineEvent::Ended);
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
        lock(&self.session).take();
        *lock(&self.generation) = None;
        self.first_frame_published.store(false, Ordering::Release);
        self.ended_published.store(false, Ordering::Release);
        *lock(&self.pending_completion) = None;
        lock(&self.seek_mailbox).invalidate();
        lock(&self.geometry_mailbox).invalidate();
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
            *lock(&adapter.generation) = Some(request.generation);
            lock(&adapter.seek_mailbox).activate(request.generation);
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
        let schedule = lock(&self.seek_mailbox).publish(generation, request)?;
        if schedule {
            self.schedule_seek_drain()?;
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

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::PendingCompletion;
    use viewer_application::EngineEvent;

    #[test]
    fn pending_seek_and_step_complete_with_the_drawn_frame_time() {
        assert_eq!(
            PendingCompletion::Seek {
                request_id: 12,
                target_time_us: 750_000,
            }
            .event_if_ready(750_000),
            Some(EngineEvent::SeekCompleted {
                request_id: 12,
                time_us: 750_000,
            })
        );
        assert_eq!(
            PendingCompletion::Step.event_if_ready(800_000),
            Some(EngineEvent::FrameStepped { time_us: 800_000 })
        );
    }

    #[test]
    fn pending_commit_completes_only_near_its_own_target() {
        let pending = PendingCompletion::Seek {
            request_id: 11,
            target_time_us: 2_000_000,
        };
        assert_eq!(pending.event_if_ready(1_000_000), None);
        assert_eq!(
            pending.event_if_ready(2_049_999),
            Some(EngineEvent::SeekCompleted {
                request_id: 11,
                time_us: 2_049_999,
            })
        );
    }
}
