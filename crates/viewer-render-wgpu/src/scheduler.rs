use crate::{FrameRequest, FrameState};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FrameScheduler {
    state: FrameState,
    last_display_tick_ns: Option<u64>,
}

impl FrameScheduler {
    pub fn frame_state(&mut self) -> &mut FrameState {
        &mut self.state
    }

    pub fn on_display_tick(&mut self, timestamp_ns: u64) -> Option<FrameRequest> {
        if self
            .last_display_tick_ns
            .is_some_and(|previous| timestamp_ns <= previous)
        {
            return None;
        }
        self.last_display_tick_ns = Some(timestamp_ns);
        self.state.take_request()
    }

    pub const fn last_display_tick_ns(&self) -> Option<u64> {
        self.last_display_tick_ns
    }
}
