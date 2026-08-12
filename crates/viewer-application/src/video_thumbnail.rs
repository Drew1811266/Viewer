use std::sync::{Mutex, MutexGuard};
use viewer_domain::{VideoSessionId, VideoThumbnailRequestId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimelineThumbnailRequest {
    pub session_id: VideoSessionId,
    pub generation: u64,
    pub request_id: VideoThumbnailRequestId,
    pub bucket_us: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelineThumbnailResult<T> {
    pub session_id: VideoSessionId,
    pub generation: u64,
    pub request_id: VideoThumbnailRequestId,
    pub bucket_us: u64,
    pub artifact: T,
}

#[derive(Default)]
struct ThumbnailState {
    active: Option<(VideoSessionId, u64)>,
    pending: Option<TimelineThumbnailRequest>,
}

#[derive(Default)]
pub struct VideoThumbnailService {
    state: Mutex<ThumbnailState>,
}

impl VideoThumbnailService {
    pub fn activate(&self, session_id: VideoSessionId, generation: u64) {
        let mut state = self.lock_state();
        state.active = Some((session_id, generation));
        state.pending = None;
    }

    pub fn close(&self) {
        let mut state = self.lock_state();
        state.active = None;
        state.pending = None;
    }

    pub fn request(&self, bucket_us: u64) -> Option<TimelineThumbnailRequest> {
        let mut state = self.lock_state();
        let (session_id, generation) = state.active?;
        let request = TimelineThumbnailRequest {
            session_id,
            generation,
            request_id: VideoThumbnailRequestId::new(),
            bucket_us,
        };
        state.pending = Some(request);
        Some(request)
    }

    pub fn pending(&self) -> Option<TimelineThumbnailRequest> {
        self.lock_state().pending
    }

    pub fn publish<T>(&self, result: TimelineThumbnailResult<T>) -> Option<T> {
        let mut state = self.lock_state();
        let pending = state.pending?;
        let matches = state.active == Some((result.session_id, result.generation))
            && pending.session_id == result.session_id
            && pending.generation == result.generation
            && pending.request_id == result.request_id
            && pending.bucket_us == result.bucket_us;
        if !matches {
            return None;
        }
        state.pending = None;
        Some(result.artifact)
    }

    fn lock_state(&self) -> MutexGuard<'_, ThumbnailState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
