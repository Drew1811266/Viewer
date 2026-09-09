use std::{
    collections::VecDeque,
    sync::{Condvar, Mutex},
};

pub(super) const HANDOFF_BYTES: u64 = 8 * 1024 * 1024;

struct State<T> {
    queue: VecDeque<(T, u64)>,
    bytes: u64,
    epoch: u64,
    closed: bool,
}

/// Byte-limited FIFO, not a last-value slot. An oversized single item is allowed
/// only with an empty queue. The producer owns at most one additional item.
pub(super) struct ByteHandoff<T> {
    state: Mutex<State<T>>,
    changed: Condvar,
}

impl<T> Default for ByteHandoff<T> {
    fn default() -> Self {
        Self {
            state: Mutex::new(State {
                queue: VecDeque::new(),
                bytes: 0,
                epoch: 0,
                closed: false,
            }),
            changed: Condvar::new(),
        }
    }
}

impl<T> ByteHandoff<T> {
    #[cfg(test)]
    pub(super) fn queued_bytes(&self) -> u64 {
        super::lock(&self.state).bytes
    }
    pub(super) fn publish(&self, item: T, bytes: u64, epoch: u64) -> bool {
        let mut state = super::lock(&self.state);
        loop {
            if state.closed || state.epoch != epoch {
                return false;
            }
            if state.queue.is_empty()
                || (state.queue.len() < 256
                    && state
                        .bytes
                        .checked_add(bytes)
                        .is_some_and(|total| total <= HANDOFF_BYTES))
            {
                break;
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        state.bytes = state
            .bytes
            .checked_add(bytes)
            .expect("admitted handoff bytes fit");
        state.queue.push_back((item, bytes));
        true
    }

    pub(super) fn take(&self) -> Option<T> {
        let mut state = super::lock(&self.state);
        let (item, bytes) = state.queue.pop_front()?;
        state.bytes -= bytes;
        self.changed.notify_all();
        Some(item)
    }
    pub(super) fn reset(&self, epoch: u64) {
        let mut state = super::lock(&self.state);
        state.epoch = epoch;
        state.queue.clear();
        state.bytes = 0;
        self.changed.notify_all();
    }
    pub(super) fn clear(&self) {
        let mut state = super::lock(&self.state);
        state.queue.clear();
        state.bytes = 0;
        self.changed.notify_all();
    }
    pub(super) fn close(&self) {
        let mut state = super::lock(&self.state);
        state.closed = true;
        state.queue.clear();
        state.bytes = 0;
        self.changed.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Arc, mpsc},
        time::Duration,
    };

    #[test]
    fn paused_consumer_bounds_bytes_and_close_wakes_blocked_producer() {
        let handoff = Arc::new(ByteHandoff::default());
        assert!(handoff.publish(1, HANDOFF_BYTES, 0));
        let (done, finished) = mpsc::channel();
        let producer = handoff.clone();
        let worker = std::thread::spawn(move || {
            done.send(producer.publish(2, 1, 0)).unwrap();
        });
        let early = finished.recv_timeout(Duration::from_millis(30));
        handoff.close();
        worker.join().unwrap();
        assert!(
            early.is_err(),
            "producer must wait instead of exceeding byte capacity"
        );
        assert!(!finished.recv_timeout(Duration::from_secs(1)).unwrap());
    }

    #[test]
    fn oversized_single_resource_does_not_deadlock_and_fifo_keeps_manifest() {
        let handoff = Arc::new(ByteHandoff::default());
        assert!(handoff.publish("large", HANDOFF_BYTES + 1, 0));
        assert_eq!(handoff.take(), Some("large"));
        assert!(handoff.publish("tile", 16, 0));
        assert!(handoff.publish("manifest", 0, 0));
        assert_eq!(handoff.take(), Some("tile"));
        assert_eq!(handoff.take(), Some("manifest"));
    }

    #[test]
    fn new_epoch_cancels_queued_storage_and_wakes_old_producer() {
        let handoff = Arc::new(ByteHandoff::default());
        assert!(handoff.publish(1, HANDOFF_BYTES, 0));
        let producer = handoff.clone();
        let worker = std::thread::spawn(move || producer.publish(2, 1, 0));
        handoff.reset(1);
        assert!(!worker.join().unwrap());
        assert!(handoff.take().is_none());
        assert!(handoff.publish(3, 1, 1));
        assert_eq!(handoff.take(), Some(3));
    }

    #[test]
    fn close_releases_actual_pixel_owners_in_queue_and_blocked_producer() {
        use viewer_render_core::{
            AssetGeneration, ImageMemoryCoordinator, ImageMemoryPolicy, SharedPixels,
        };
        let memory = ImageMemoryCoordinator::new(ImageMemoryPolicy::baseline_8gb());
        let handoff = Arc::new(ByteHandoff::default());
        assert!(handoff.publish(
            SharedPixels::try_zeroed(&memory, AssetGeneration(1), HANDOFF_BYTES).unwrap(),
            HANDOFF_BYTES,
            0
        ));
        let current = SharedPixels::try_zeroed(&memory, AssetGeneration(2), 1024).unwrap();
        let producer = handoff.clone();
        let worker = std::thread::spawn(move || producer.publish(current, 1024, 0));
        assert_eq!(memory.snapshot().combined_bytes, HANDOFF_BYTES + 1024);
        handoff.close();
        assert!(!worker.join().unwrap());
        assert_eq!(memory.snapshot().combined_bytes, 0);
    }
}
