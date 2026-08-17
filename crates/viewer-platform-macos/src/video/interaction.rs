use viewer_application::{SurfaceRect, VideoEngineError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct GeometryUpdate {
    pub generation: u64,
    pub sequence: u64,
    pub rect: SurfaceRect,
}

#[derive(Default)]
pub(super) struct LatestGeometryMailbox {
    active_generation: Option<u64>,
    latest_sequence: u64,
    pending: Option<GeometryUpdate>,
    drain_scheduled: bool,
    pending_settled_redraw: Option<u64>,
    settle_timer_scheduled: bool,
}

impl LatestGeometryMailbox {
    pub fn activate(&mut self, generation: u64) {
        self.active_generation = Some(generation);
        self.latest_sequence = 0;
        self.pending = None;
        self.drain_scheduled = false;
        self.pending_settled_redraw = None;
        self.settle_timer_scheduled = false;
    }

    pub fn invalidate(&mut self) {
        self.active_generation = None;
        self.latest_sequence = 0;
        self.pending = None;
        self.drain_scheduled = false;
        self.pending_settled_redraw = None;
        self.settle_timer_scheduled = false;
    }

    pub fn publish(
        &mut self,
        generation: u64,
        sequence: u64,
        rect: SurfaceRect,
    ) -> Result<bool, VideoEngineError> {
        if self.active_generation != Some(generation) {
            return Err(VideoEngineError::StaleGeneration);
        }
        if sequence <= self.latest_sequence {
            return Ok(false);
        }
        self.latest_sequence = sequence;
        self.pending = Some(GeometryUpdate {
            generation,
            sequence,
            rect,
        });
        if self.drain_scheduled {
            return Ok(false);
        }
        self.drain_scheduled = true;
        Ok(true)
    }

    pub fn take_for_drain(&mut self) -> Option<GeometryUpdate> {
        self.pending.take()
    }

    pub fn finish_drain(&mut self) -> bool {
        if self.pending.is_some() {
            true
        } else {
            self.drain_scheduled = false;
            false
        }
    }

    pub fn mark_applied(&mut self, generation: u64, sequence: u64, playback_active: bool) -> bool {
        if self.active_generation != Some(generation) {
            return false;
        }
        if playback_active {
            self.pending_settled_redraw = None;
            self.settle_timer_scheduled = false;
            return false;
        }
        self.pending_settled_redraw = Some(sequence);
        if self.settle_timer_scheduled {
            return false;
        }
        self.settle_timer_scheduled = true;
        true
    }

    pub fn take_settled_redraw(&mut self, generation: u64, scheduled_sequence: u64) -> Option<u64> {
        if self.active_generation != Some(generation) {
            return None;
        }
        if self.pending_settled_redraw != Some(scheduled_sequence) {
            return None;
        }
        self.pending_settled_redraw = None;
        self.settle_timer_scheduled = false;
        Some(scheduled_sequence)
    }

    pub fn pending_settled_redraw(&self, generation: u64) -> Option<u64> {
        if self.active_generation != Some(generation) {
            return None;
        }
        self.pending_settled_redraw
    }

    pub fn cancel_settled_redraw(&mut self, generation: u64) {
        if self.active_generation == Some(generation) {
            self.pending_settled_redraw = None;
            self.settle_timer_scheduled = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LatestGeometryMailbox;
    use viewer_application::SurfaceRect;

    #[test]
    fn resize_burst_keeps_one_pending_rect_and_one_drain_owner() {
        let mut mailbox = LatestGeometryMailbox::default();
        mailbox.activate(5);
        let mut schedules = 0;
        for sequence in 1..=120 {
            if mailbox.publish(5, sequence, geometry(sequence)).unwrap() {
                schedules += 1;
            }
        }

        assert_eq!(schedules, 1);
        let latest = mailbox.take_for_drain().unwrap();
        assert_eq!(latest.sequence, 120);
        assert_eq!(latest.rect, geometry(120));
        assert!(!mailbox.finish_drain());
    }

    #[test]
    fn paused_resize_burst_redraws_only_the_latest_applied_rect_after_settle() {
        let mut mailbox = LatestGeometryMailbox::default();
        mailbox.activate(5);
        let mut redraw_schedules = 0;

        for sequence in 1..=120 {
            mailbox.publish(5, sequence, geometry(sequence)).unwrap();
            let update = mailbox.take_for_drain().unwrap();
            if mailbox.mark_applied(5, update.sequence, false) {
                redraw_schedules += 1;
            }
        }

        assert_eq!(redraw_schedules, 1);
        assert_eq!(mailbox.take_settled_redraw(5, 1), None);
        assert_eq!(mailbox.pending_settled_redraw(5), Some(120));
        assert_eq!(mailbox.take_settled_redraw(5, 120), Some(120));
        assert_eq!(mailbox.pending_settled_redraw(5), None);
    }

    #[test]
    fn playing_resize_uses_natural_frames_without_a_forced_redraw() {
        let mut mailbox = LatestGeometryMailbox::default();
        mailbox.activate(5);
        mailbox.publish(5, 1, geometry(1)).unwrap();
        let update = mailbox.take_for_drain().unwrap();

        assert!(!mailbox.mark_applied(5, update.sequence, true));
        assert_eq!(mailbox.pending_settled_redraw(5), None);
    }

    #[test]
    fn delayed_paused_redraw_from_an_old_generation_cannot_consume_new_generation_work() {
        let mut mailbox = LatestGeometryMailbox::default();
        mailbox.activate(5);
        assert!(mailbox.mark_applied(5, 1, false));

        mailbox.activate(6);
        assert!(mailbox.mark_applied(6, 2, false));
        assert_eq!(mailbox.take_settled_redraw(5, 1), None);
        assert_eq!(mailbox.pending_settled_redraw(5), None);

        assert_eq!(mailbox.take_settled_redraw(6, 2), Some(2));
    }

    const fn geometry(sequence: u64) -> SurfaceRect {
        SurfaceRect {
            x: sequence as i32,
            y: 2,
            width: 640,
            height: 360,
        }
    }
}
