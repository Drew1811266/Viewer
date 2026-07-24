use std::sync::Mutex;
use tokio::sync::watch;
use viewer_application::file_commands::{BatchProgress, BatchSummary, FileCommandServiceError};

pub(super) struct OperationRecord {
    pub(super) progress: Mutex<BatchProgress>,
    pub(super) summary: Mutex<Option<BatchSummary>>,
    pub(super) failure: Mutex<Option<FileCommandServiceError>>,
    pub(super) complete: watch::Sender<bool>,
}

impl OperationRecord {
    pub(super) fn new(progress: BatchProgress) -> Self {
        let (complete, _) = watch::channel(false);
        Self {
            progress: Mutex::new(progress),
            summary: Mutex::new(None),
            failure: Mutex::new(None),
            complete,
        }
    }

    pub(super) fn progress(&self) -> BatchProgress {
        self.progress
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub(super) fn publish(&self, progress: BatchProgress) {
        *self
            .progress
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = progress;
    }

    pub(super) fn store_result(&self, result: Result<BatchSummary, FileCommandServiceError>) {
        match result {
            Ok(summary) => {
                *self
                    .summary
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(summary);
            }
            Err(error) => {
                *self
                    .failure
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(error);
            }
        }
    }

    pub(super) fn mark_complete(&self) {
        self.complete.send_replace(true);
    }

    pub(super) fn is_complete(&self) -> bool {
        *self.complete.borrow()
    }

    pub(super) fn subscribe_completion(&self) -> watch::Receiver<bool> {
        self.complete.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Arc, time::Duration};
    use viewer_application::file_commands::BatchId;
    use viewer_domain::operation::BatchLifecycle;

    #[tokio::test]
    async fn completion_latch_remembers_finish_for_late_and_multiple_waiters() {
        let initial = BatchProgress {
            batch_id: BatchId::new(),
            lifecycle: BatchLifecycle::Queued,
            requested: 1,
            completed: 0,
            failed: 0,
            skipped: 0,
            cancelled: 0,
            active_entity_id: None,
        };
        let record = Arc::new(OperationRecord::new(initial));
        let mut before = record.subscribe_completion();
        record.store_result(Ok(BatchSummary::try_from_results(
            BatchId::new(),
            0,
            Vec::new(),
        )
        .unwrap()));
        record.mark_complete();

        tokio::time::timeout(Duration::from_millis(100), before.changed())
            .await
            .expect("an already-registered waiter must be woken")
            .expect("completion channel remains open");
        assert!(*before.borrow_and_update());

        let mut late = record.subscribe_completion();
        assert!(
            *late.borrow_and_update(),
            "late waiter observes the latched value"
        );
        let waiters = (0..8)
            .map(|_| {
                let mut completion = record.subscribe_completion();
                tokio::spawn(async move {
                    if !*completion.borrow_and_update() {
                        completion.changed().await.unwrap();
                    }
                    assert!(*completion.borrow_and_update());
                })
            })
            .collect::<Vec<_>>();
        for waiter in waiters {
            tokio::time::timeout(Duration::from_millis(100), waiter)
                .await
                .expect("every completion waiter finishes")
                .unwrap();
        }
    }
}
