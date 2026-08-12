use crate::dto::VideoEventDto;
use crate::video_runtime::VideoEventPort;
use std::{
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};
use tauri::Emitter;

const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Default)]
pub struct ProgressCoalescer {
    last_emitted_at: Option<Duration>,
    pending: Option<VideoEventDto>,
}

pub const VIDEO_EVENT_NAME: &str = "viewer://video-event";

pub struct TauriVideoEventEmitter {
    app: tauri::AppHandle,
    started: Instant,
    coalescer: Mutex<ProgressCoalescer>,
}

impl TauriVideoEventEmitter {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self {
            app,
            started: Instant::now(),
            coalescer: Mutex::new(ProgressCoalescer::default()),
        }
    }
}

impl VideoEventPort for TauriVideoEventEmitter {
    fn publish(&self, event: VideoEventDto) {
        let events = lock(&self.coalescer).push_at(self.started.elapsed(), event);
        for event in events {
            if let Err(error) = self.app.emit(VIDEO_EVENT_NAME, event) {
                eprintln!("Viewer video event delivery failed: {error}");
            }
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl ProgressCoalescer {
    pub fn push_at(&mut self, elapsed: Duration, event: VideoEventDto) -> Vec<VideoEventDto> {
        if event.is_progress() {
            let may_emit = self
                .last_emitted_at
                .is_none_or(|last| elapsed.saturating_sub(last) >= PROGRESS_INTERVAL);
            if may_emit {
                self.last_emitted_at = Some(elapsed);
                self.pending = None;
                return vec![event];
            }
            self.pending = Some(event);
            return Vec::new();
        }

        let mut emitted = Vec::with_capacity(2);
        if event.is_terminal()
            && self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.generation() == event.generation())
            && let Some(pending) = self.pending.take()
        {
            self.last_emitted_at = Some(elapsed);
            emitted.push(pending);
        }
        if event.is_terminal() {
            self.pending = None;
        }
        emitted.push(event);
        emitted
    }
}
