use std::{
    sync::{Arc, Condvar, Mutex, MutexGuard},
    thread::{self, JoinHandle},
};

use viewer_application::{SeekIntent, SeekRequest, VideoEngineError};
use viewer_video_mpv::{MpvCommandClient, MpvPlaybackSnapshot, SeekMode};

const SEEK_COMPLETION_TOLERANCE_US: u64 = 50_000;

pub(super) trait MediaCommandBackend: Send + 'static {
    fn seek_absolute_us(&self, time_us: u64, mode: SeekMode) -> Result<(), VideoEngineError>;
    fn playback_snapshot(&self) -> Result<MpvPlaybackSnapshot, VideoEngineError>;
}

impl MediaCommandBackend for MpvCommandClient {
    fn seek_absolute_us(&self, time_us: u64, mode: SeekMode) -> Result<(), VideoEngineError> {
        MpvCommandClient::seek_absolute_us(self, time_us, mode)
            .map_err(|_| VideoEngineError::Decode)
    }

    fn playback_snapshot(&self) -> Result<MpvPlaybackSnapshot, VideoEngineError> {
        MpvCommandClient::playback_snapshot(self).map_err(|_| VideoEngineError::Decode)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum MediaWorkerEvent {
    FrameSnapshot {
        generation: u64,
        frame_serial: u64,
        snapshot: MpvPlaybackSnapshot,
    },
    SeekCompleted {
        generation: u64,
        request_id: u64,
        time_us: u64,
    },
    Failed {
        generation: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PreviewFrameGate {
    generation: u64,
    request_id: u64,
    frame_serial_before_seek: u64,
}

#[derive(Default)]
struct WorkerState {
    active_generation: Option<u64>,
    latest_request_id: u64,
    latest_preview_request_id: u64,
    latest_frame_serial: u64,
    pending_preview: Option<SeekRequest>,
    pending_commit: Option<SeekRequest>,
    pending_frame: Option<(u64, u64)>,
    pending_completion: Option<SeekRequest>,
    awaiting_preview_frame: Option<PreviewFrameGate>,
    shutdown: bool,
}

struct SharedState {
    state: Mutex<WorkerState>,
    changed: Condvar,
}

pub(super) struct MediaCommandWorker {
    shared: Arc<SharedState>,
    join: Option<JoinHandle<()>>,
}

impl MediaCommandWorker {
    pub(super) fn start<B>(
        backend: B,
        event_sink: impl Fn(MediaWorkerEvent) + Send + 'static,
    ) -> Result<Self, VideoEngineError>
    where
        B: MediaCommandBackend,
    {
        let shared = Arc::new(SharedState {
            state: Mutex::new(WorkerState::default()),
            changed: Condvar::new(),
        });
        let state_for_thread = Arc::clone(&shared);
        let join = thread::Builder::new()
            .name("viewer-media-command".into())
            .spawn(move || run_worker(state_for_thread, backend, event_sink))
            .map_err(|_| VideoEngineError::Unavailable)?;
        Ok(Self {
            shared,
            join: Some(join),
        })
    }

    pub(super) fn activate(&self, generation: u64) -> Result<(), VideoEngineError> {
        let mut state = lock(&self.shared.state);
        if state.shutdown {
            return Err(VideoEngineError::Unavailable);
        }
        *state = WorkerState {
            active_generation: Some(generation),
            ..WorkerState::default()
        };
        self.shared.changed.notify_all();
        Ok(())
    }

    pub(super) fn publish_seek(
        &self,
        generation: u64,
        request: SeekRequest,
    ) -> Result<(), VideoEngineError> {
        let mut state = lock(&self.shared.state);
        if state.shutdown {
            return Err(VideoEngineError::Unavailable);
        }
        if state.active_generation != Some(generation) {
            return Err(VideoEngineError::StaleGeneration);
        }
        if request.request_id <= state.latest_request_id {
            return Ok(());
        }
        state.latest_request_id = request.request_id;
        match request.intent {
            SeekIntent::Preview => {
                state.latest_preview_request_id = request.request_id;
                state.pending_preview = Some(request);
                state.pending_commit = None;
                state.pending_completion = None;
                state.awaiting_preview_frame = None;
            }
            SeekIntent::Commit => state.pending_commit = Some(request),
        }
        self.shared.changed.notify_all();
        Ok(())
    }

    pub(super) fn frame_rendered(
        &self,
        generation: u64,
        frame_serial: u64,
    ) -> Result<(), VideoEngineError> {
        let mut state = lock(&self.shared.state);
        if state.shutdown {
            return Err(VideoEngineError::Unavailable);
        }
        if state.active_generation != Some(generation) {
            return Err(VideoEngineError::StaleGeneration);
        }
        if frame_serial <= state.latest_frame_serial {
            return Ok(());
        }
        state.latest_frame_serial = frame_serial;
        state.pending_frame = Some((generation, frame_serial));
        self.shared.changed.notify_all();
        Ok(())
    }

    pub(super) fn shutdown_and_join(&mut self) {
        {
            let mut state = lock(&self.shared.state);
            state.shutdown = true;
            state.pending_preview = None;
            state.pending_commit = None;
            state.pending_frame = None;
            state.pending_completion = None;
            state.awaiting_preview_frame = None;
            self.shared.changed.notify_all();
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for MediaCommandWorker {
    fn drop(&mut self) {
        self.shutdown_and_join();
    }
}

enum Work {
    Preview {
        generation: u64,
        request: SeekRequest,
        frame_serial_before_seek: u64,
    },
    Commit {
        generation: u64,
        request: SeekRequest,
    },
    Snapshot {
        generation: u64,
        frame_serial: u64,
    },
    Shutdown,
}

fn run_worker<B>(shared: Arc<SharedState>, backend: B, event_sink: impl Fn(MediaWorkerEvent))
where
    B: MediaCommandBackend,
{
    loop {
        let work = next_work(&shared);
        match work {
            Work::Shutdown => break,
            Work::Preview {
                generation,
                request,
                frame_serial_before_seek,
            } => {
                let result = backend.seek_absolute_us(request.time_us, SeekMode::PreviewKeyframe);
                let mut state = lock(&shared.state);
                if result.is_ok()
                    && state.active_generation == Some(generation)
                    && state.latest_preview_request_id == request.request_id
                {
                    state.awaiting_preview_frame = Some(PreviewFrameGate {
                        generation,
                        request_id: request.request_id,
                        frame_serial_before_seek,
                    });
                } else if result.is_err() && state.active_generation == Some(generation) {
                    drop(state);
                    event_sink(MediaWorkerEvent::Failed { generation });
                }
            }
            Work::Commit {
                generation,
                request,
            } => {
                let result = backend.seek_absolute_us(request.time_us, SeekMode::CommitExact);
                let mut state = lock(&shared.state);
                if result.is_ok() && state.active_generation == Some(generation) {
                    state.pending_completion = Some(request);
                } else if result.is_err() && state.active_generation == Some(generation) {
                    drop(state);
                    event_sink(MediaWorkerEvent::Failed { generation });
                }
            }
            Work::Snapshot {
                generation,
                frame_serial,
            } => match backend.playback_snapshot() {
                Ok(snapshot) => {
                    let completion = {
                        let mut state = lock(&shared.state);
                        if state.active_generation != Some(generation) {
                            None
                        } else {
                            if state.awaiting_preview_frame.is_some_and(|gate| {
                                gate.generation == generation
                                    && gate.request_id == state.latest_preview_request_id
                                    && frame_serial > gate.frame_serial_before_seek
                            }) {
                                state.awaiting_preview_frame = None;
                            }
                            state.pending_completion.and_then(|request| {
                                snapshot.time_us.and_then(|time_us| {
                                    (time_us.abs_diff(request.time_us)
                                        <= SEEK_COMPLETION_TOLERANCE_US)
                                        .then_some((request.request_id, time_us))
                                })
                            })
                        }
                    };
                    event_sink(MediaWorkerEvent::FrameSnapshot {
                        generation,
                        frame_serial,
                        snapshot,
                    });
                    if let Some((request_id, time_us)) = completion {
                        let mut state = lock(&shared.state);
                        if state.active_generation == Some(generation)
                            && state
                                .pending_completion
                                .is_some_and(|request| request.request_id == request_id)
                        {
                            state.pending_completion = None;
                            drop(state);
                            event_sink(MediaWorkerEvent::SeekCompleted {
                                generation,
                                request_id,
                                time_us,
                            });
                        }
                    }
                    shared.changed.notify_all();
                }
                Err(_) => event_sink(MediaWorkerEvent::Failed { generation }),
            },
        }
    }
}

fn next_work(shared: &SharedState) -> Work {
    let mut state = lock(&shared.state);
    loop {
        if state.shutdown {
            return Work::Shutdown;
        }
        let Some(generation) = state.active_generation else {
            state = shared
                .changed
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
            continue;
        };
        if let Some(request) = state.pending_preview.take() {
            return Work::Preview {
                generation,
                request,
                frame_serial_before_seek: state.latest_frame_serial,
            };
        }
        if state.awaiting_preview_frame.is_none()
            && let Some(request) = state.pending_commit.take()
        {
            return Work::Commit {
                generation,
                request,
            };
        }
        if let Some((frame_generation, frame_serial)) = state.pending_frame.take() {
            if frame_generation == generation {
                return Work::Snapshot {
                    generation,
                    frame_serial,
                };
            }
        }
        state = shared
            .changed
            .wait(state)
            .unwrap_or_else(|error| error.into_inner());
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Condvar, Mutex},
        thread::{self, ThreadId},
        time::Duration,
    };

    use viewer_application::{SeekIntent, SeekRequest, VideoEngineError};
    use viewer_video_mpv::{MpvPlaybackSnapshot, SeekMode};

    use super::{MediaCommandBackend, MediaCommandWorker, MediaWorkerEvent};

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum Call {
        Seek {
            thread: ThreadId,
            time_us: u64,
            mode: SeekMode,
        },
        Snapshot {
            thread: ThreadId,
        },
    }

    #[derive(Clone)]
    struct RecordingBackend {
        calls: Arc<(Mutex<Vec<Call>>, Condvar)>,
        snapshot: MpvPlaybackSnapshot,
    }

    impl RecordingBackend {
        fn new(snapshot: MpvPlaybackSnapshot) -> Self {
            Self {
                calls: Arc::new((Mutex::new(Vec::new()), Condvar::new())),
                snapshot,
            }
        }

        fn wait_for_calls(&self, count: usize) -> Vec<Call> {
            let (calls, changed) = &*self.calls;
            let calls = calls.lock().unwrap();
            let (calls, timeout) = changed
                .wait_timeout_while(calls, Duration::from_secs(1), |calls| calls.len() < count)
                .unwrap();
            assert!(
                !timeout.timed_out(),
                "media worker did not publish {count} calls"
            );
            calls.clone()
        }

        fn record(&self, call: Call) {
            let (calls, changed) = &*self.calls;
            calls.lock().unwrap().push(call);
            changed.notify_all();
        }
    }

    impl MediaCommandBackend for RecordingBackend {
        fn seek_absolute_us(&self, time_us: u64, mode: SeekMode) -> Result<(), VideoEngineError> {
            self.record(Call::Seek {
                thread: thread::current().id(),
                time_us,
                mode,
            });
            Ok(())
        }

        fn playback_snapshot(&self) -> Result<MpvPlaybackSnapshot, VideoEngineError> {
            self.record(Call::Snapshot {
                thread: thread::current().id(),
            });
            Ok(self.snapshot.clone())
        }
    }

    #[test]
    fn exact_commit_waits_for_a_new_frame_from_the_preview_seek() {
        let caller = thread::current().id();
        let backend = RecordingBackend::new(snapshot(4_000_000));
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_for_sink = Arc::clone(&events);
        let mut worker = MediaCommandWorker::start(backend.clone(), move |event| {
            events_for_sink.lock().unwrap().push(event);
        })
        .unwrap();
        worker.activate(7).unwrap();

        worker.publish_seek(7, preview(10, 4_000_000)).unwrap();
        worker.publish_seek(7, commit(11, 4_000_000)).unwrap();
        let calls = backend.wait_for_calls(1);
        assert_eq!(calls.len(), 1);
        assert!(matches!(
            calls[0],
            Call::Seek {
                mode: SeekMode::PreviewKeyframe,
                ..
            }
        ));

        worker.frame_rendered(7, 41).unwrap();
        let calls = backend.wait_for_calls(3);
        assert_eq!(
            calls.iter().map(call_kind).collect::<Vec<_>>(),
            ["preview", "snapshot", "commit"]
        );
        assert!(calls.iter().all(|call| call_thread(call) != caller));
        worker.shutdown_and_join();
    }

    #[test]
    fn a_new_preview_discards_the_old_unacknowledged_commit() {
        let backend = RecordingBackend::new(snapshot(8_000_000));
        let mut worker = MediaCommandWorker::start(backend.clone(), |_| {}).unwrap();
        worker.activate(4).unwrap();
        worker.publish_seek(4, preview(1, 2_000_000)).unwrap();
        worker.publish_seek(4, commit(2, 2_000_000)).unwrap();
        backend.wait_for_calls(1);

        worker.publish_seek(4, preview(3, 8_000_000)).unwrap();
        worker.publish_seek(4, commit(4, 8_000_000)).unwrap();
        backend.wait_for_calls(2);
        worker.frame_rendered(4, 9).unwrap();
        let calls = backend.wait_for_calls(4);
        assert_eq!(
            calls.iter().map(call_kind).collect::<Vec<_>>(),
            ["preview", "preview", "snapshot", "commit"]
        );
        assert!(matches!(
            calls[3],
            Call::Seek {
                time_us: 8_000_000,
                ..
            }
        ));
        worker.shutdown_and_join();
    }

    #[test]
    fn commit_without_a_preview_executes_immediately_and_stale_generation_is_rejected() {
        let backend = RecordingBackend::new(snapshot(1_000_000));
        let mut worker = MediaCommandWorker::start(backend.clone(), |_| {}).unwrap();
        worker.activate(5).unwrap();

        assert_eq!(
            worker.publish_seek(4, commit(1, 1_000_000)),
            Err(VideoEngineError::StaleGeneration)
        );
        worker.publish_seek(5, commit(2, 1_000_000)).unwrap();
        let calls = backend.wait_for_calls(1);
        assert!(matches!(
            calls[0],
            Call::Seek {
                mode: SeekMode::CommitExact,
                ..
            }
        ));
        worker.shutdown_and_join();
    }

    #[test]
    fn frame_snapshot_emits_progress_and_seek_completion_for_the_active_request() {
        let backend = RecordingBackend::new(snapshot(3_000_000));
        let events = Arc::new((Mutex::new(Vec::new()), Condvar::new()));
        let events_for_sink = Arc::clone(&events);
        let mut worker = MediaCommandWorker::start(backend, move |event| {
            let (events, changed) = &*events_for_sink;
            events.lock().unwrap().push(event);
            changed.notify_all();
        })
        .unwrap();
        worker.activate(2).unwrap();
        worker.publish_seek(2, commit(7, 3_000_000)).unwrap();
        worker.frame_rendered(2, 12).unwrap();

        let (values, changed) = &*events;
        let values = values.lock().unwrap();
        let (values, timeout) = changed
            .wait_timeout_while(values, Duration::from_secs(1), |values| values.len() < 2)
            .unwrap();
        assert!(!timeout.timed_out());
        assert!(values.contains(&MediaWorkerEvent::FrameSnapshot {
            generation: 2,
            frame_serial: 12,
            snapshot: snapshot(3_000_000),
        }));
        assert!(values.contains(&MediaWorkerEvent::SeekCompleted {
            generation: 2,
            request_id: 7,
            time_us: 3_000_000,
        }));
        drop(values);
        worker.shutdown_and_join();
    }

    fn snapshot(time_us: u64) -> MpvPlaybackSnapshot {
        MpvPlaybackSnapshot {
            time_us: Some(time_us),
            eof_reached: false,
            picture_type: Some("I".into()),
        }
    }

    const fn preview(request_id: u64, time_us: u64) -> SeekRequest {
        SeekRequest {
            request_id,
            time_us,
            intent: SeekIntent::Preview,
        }
    }

    const fn commit(request_id: u64, time_us: u64) -> SeekRequest {
        SeekRequest {
            request_id,
            time_us,
            intent: SeekIntent::Commit,
        }
    }

    const fn call_kind(call: &Call) -> &'static str {
        match call {
            Call::Seek {
                mode: SeekMode::PreviewKeyframe,
                ..
            } => "preview",
            Call::Seek {
                mode: SeekMode::CommitExact,
                ..
            } => "commit",
            Call::Snapshot { .. } => "snapshot",
        }
    }

    const fn call_thread(call: &Call) -> ThreadId {
        match call {
            Call::Seek { thread, .. } | Call::Snapshot { thread } => *thread,
        }
    }
}
