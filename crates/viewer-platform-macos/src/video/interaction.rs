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
    latest_applied_sequence: u64,
    redraw_pending: bool,
    redraw_scheduled: bool,
}

impl LatestGeometryMailbox {
    pub fn activate(&mut self, generation: u64) {
        self.active_generation = Some(generation);
        self.latest_sequence = 0;
        self.pending = None;
        self.drain_scheduled = false;
        self.latest_applied_sequence = 0;
        self.redraw_pending = false;
        self.redraw_scheduled = false;
    }

    pub fn invalidate(&mut self) {
        self.active_generation = None;
        self.latest_sequence = 0;
        self.pending = None;
        self.drain_scheduled = false;
        self.latest_applied_sequence = 0;
        self.redraw_pending = false;
        self.redraw_scheduled = false;
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

    pub fn mark_applied_for_redraw(&mut self, generation: u64, sequence: u64) -> bool {
        if self.active_generation != Some(generation) {
            return false;
        }
        self.latest_applied_sequence = self.latest_applied_sequence.max(sequence);
        self.redraw_pending = true;
        if self.redraw_scheduled {
            return false;
        }
        self.redraw_scheduled = true;
        true
    }

    pub fn take_for_redraw(&mut self, generation: u64) -> Option<u64> {
        if self.active_generation != Some(generation) {
            return None;
        }
        if !self.redraw_pending {
            return None;
        }
        self.redraw_pending = false;
        Some(self.latest_applied_sequence)
    }

    pub fn finish_redraw(&mut self, generation: u64) -> bool {
        if self.active_generation != Some(generation) {
            return false;
        }
        if self.redraw_pending {
            true
        } else {
            self.redraw_scheduled = false;
            false
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
    fn geometry_burst_schedules_one_expensive_redraw_for_the_latest_applied_rect() {
        let mut mailbox = LatestGeometryMailbox::default();
        mailbox.activate(5);
        let mut redraw_schedules = 0;

        for sequence in 1..=120 {
            mailbox.publish(5, sequence, geometry(sequence)).unwrap();
            let update = mailbox.take_for_drain().unwrap();
            if mailbox.mark_applied_for_redraw(5, update.sequence) {
                redraw_schedules += 1;
            }
        }

        assert_eq!(redraw_schedules, 1);
        assert_eq!(mailbox.take_for_redraw(5), Some(120));
        assert!(!mailbox.finish_redraw(5));
    }

    #[test]
    fn delayed_redraw_from_an_old_generation_cannot_consume_new_generation_work() {
        let mut mailbox = LatestGeometryMailbox::default();
        mailbox.activate(5);
        assert!(mailbox.mark_applied_for_redraw(5, 1));

        mailbox.activate(6);
        assert!(mailbox.mark_applied_for_redraw(6, 2));
        assert_eq!(mailbox.take_for_redraw(5), None);
        assert!(!mailbox.finish_redraw(5));

        assert_eq!(mailbox.take_for_redraw(6), Some(2));
        assert!(!mailbox.finish_redraw(6));
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
