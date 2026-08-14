use viewer_application::{SeekIntent, SeekRequest, VideoEngineError};

#[derive(Default)]
pub(super) struct LatestSeekMailbox {
    active_generation: Option<u64>,
    latest_request_id: u64,
    pending: Option<SeekRequest>,
    drain_scheduled: bool,
}

impl LatestSeekMailbox {
    pub fn activate(&mut self, generation: u64) {
        self.active_generation = Some(generation);
        self.latest_request_id = 0;
        self.pending = None;
        self.drain_scheduled = false;
    }

    pub fn invalidate(&mut self) {
        self.active_generation = None;
        self.latest_request_id = 0;
        self.pending = None;
        self.drain_scheduled = false;
    }

    pub fn publish(
        &mut self,
        generation: u64,
        request: SeekRequest,
    ) -> Result<bool, VideoEngineError> {
        if self.active_generation != Some(generation) {
            return Err(VideoEngineError::StaleGeneration);
        }
        if request.request_id <= self.latest_request_id {
            return Ok(false);
        }
        if self
            .pending
            .is_some_and(|pending| pending.intent == SeekIntent::Commit)
            && request.intent == SeekIntent::Preview
        {
            return Ok(false);
        }
        self.latest_request_id = request.request_id;
        self.pending = Some(request);
        if self.drain_scheduled {
            return Ok(false);
        }
        self.drain_scheduled = true;
        Ok(true)
    }

    pub fn take_for_drain(&mut self) -> Option<SeekRequest> {
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
}

#[cfg(test)]
mod tests {
    use super::LatestSeekMailbox;
    use viewer_application::{SeekIntent, SeekRequest, VideoEngineError};

    #[test]
    fn one_hundred_twenty_previews_keep_only_the_latest_request() {
        let mut mailbox = LatestSeekMailbox::default();
        mailbox.activate(7);
        let mut schedules = 0;
        for request_id in 1..=120 {
            if mailbox
                .publish(
                    7,
                    SeekRequest {
                        request_id,
                        time_us: request_id * 10_000,
                        intent: SeekIntent::Preview,
                    },
                )
                .unwrap()
            {
                schedules += 1;
            }
        }

        assert_eq!(schedules, 1);
        assert_eq!(mailbox.take_for_drain().unwrap().request_id, 120);
        assert!(!mailbox.finish_drain());
        assert!(mailbox.take_for_drain().is_none());
    }

    #[test]
    fn commit_replaces_preview_and_stale_generation_is_rejected() {
        let mut mailbox = LatestSeekMailbox::default();
        mailbox.activate(9);
        assert_eq!(
            mailbox.publish(8, preview(1)),
            Err(VideoEngineError::StaleGeneration)
        );
        assert_eq!(mailbox.publish(9, preview(2)), Ok(true));
        assert_eq!(mailbox.publish(9, commit(3)), Ok(false));
        assert_eq!(mailbox.take_for_drain(), Some(commit(3)));
    }

    #[test]
    fn newer_work_arriving_during_a_drain_retains_the_single_owner() {
        let mut mailbox = LatestSeekMailbox::default();
        mailbox.activate(4);
        assert_eq!(mailbox.publish(4, preview(1)), Ok(true));
        assert_eq!(mailbox.take_for_drain(), Some(preview(1)));
        assert_eq!(mailbox.publish(4, preview(2)), Ok(false));
        assert!(mailbox.finish_drain());
        assert_eq!(mailbox.take_for_drain(), Some(preview(2)));
        assert!(!mailbox.finish_drain());
    }

    const fn preview(request_id: u64) -> SeekRequest {
        SeekRequest {
            request_id,
            time_us: request_id * 10_000,
            intent: SeekIntent::Preview,
        }
    }

    const fn commit(request_id: u64) -> SeekRequest {
        SeekRequest {
            intent: SeekIntent::Commit,
            ..preview(request_id)
        }
    }
}
