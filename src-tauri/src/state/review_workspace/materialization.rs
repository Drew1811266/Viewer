use std::{sync::Arc, time::Duration};
use tokio::{sync::Mutex, task::JoinHandle};
use viewer_application::{
    ClockPort, ReviewTaskCancellation,
    review_workspace::{ReviewMaterializationOutcome, ReviewMaterializationService},
};

/// Owns the one background publication loop for a project review session. `Notify` is used as a
/// coalescing wake signal: any number of foreground commits only need one pending permit.
pub(super) struct ReviewMaterializationRuntime {
    cancellation: ReviewTaskCancellation,
    wake: Arc<tokio::sync::Notify>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Default for ReviewMaterializationRuntime {
    fn default() -> Self {
        Self {
            cancellation: ReviewTaskCancellation::default(),
            wake: Arc::new(tokio::sync::Notify::new()),
            worker: Mutex::new(None),
        }
    }
}

impl ReviewMaterializationRuntime {
    pub async fn start(
        &self,
        service: Arc<ReviewMaterializationService>,
        clock: Arc<dyn ClockPort>,
    ) {
        let mut worker = self.worker.lock().await;
        if worker.is_some() || self.cancellation.is_cancelled() {
            return;
        }
        let cancellation = self.cancellation.clone();
        let wake = self.wake.clone();
        *worker = Some(tokio::spawn(async move {
            run_worker(service, clock, cancellation, wake).await;
        }));
    }

    pub fn wake(&self) {
        self.wake.notify_one();
    }

    pub fn revoke(&self) {
        self.cancellation.cancel();
        // `notify_one` retains a permit if the worker is between its cancellation check and wait.
        self.wake.notify_one();
    }

    pub async fn close(&self) {
        self.revoke();
        if let Some(worker) = self.worker.lock().await.take() {
            let _ = worker.await;
        }
    }
}

async fn run_worker(
    service: Arc<ReviewMaterializationService>,
    clock: Arc<dyn ClockPort>,
    cancellation: ReviewTaskCancellation,
    wake: Arc<tokio::sync::Notify>,
) {
    let mut retry_at_ms = None;
    loop {
        if cancellation.is_cancelled() {
            return;
        }
        if let Some(deadline) = retry_at_ms.take() {
            wait_for_wake_or_deadline(&wake, &cancellation, &clock, deadline).await;
            continue;
        }

        match service.run_one(cancellation.clone()).await {
            Ok(ReviewMaterializationOutcome::Published { .. }) => tokio::task::yield_now().await,
            Ok(ReviewMaterializationOutcome::Retrying {
                next_attempt_at_ms, ..
            }) => retry_at_ms = Some(next_attempt_at_ms),
            Ok(
                ReviewMaterializationOutcome::Idle | ReviewMaterializationOutcome::Blocked { .. },
            ) => {
                wake.notified().await;
            }
            Err(_) => {
                wait_for_wake_or_deadline(
                    &wake,
                    &cancellation,
                    &clock,
                    clock.unix_millis().saturating_add(500),
                )
                .await;
            }
        }
    }
}

async fn wait_for_wake_or_deadline(
    wake: &tokio::sync::Notify,
    cancellation: &ReviewTaskCancellation,
    clock: &Arc<dyn ClockPort>,
    deadline_ms: i64,
) {
    loop {
        if cancellation.is_cancelled() {
            return;
        }
        let remaining_ms = deadline_ms.saturating_sub(clock.unix_millis()).max(0) as u64;
        if remaining_ms == 0 {
            return;
        }
        tokio::select! {
            _ = wake.notified() => {}
            _ = tokio::time::sleep(Duration::from_millis(remaining_ms)) => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Mutex as StdMutex;
    use tokio::sync::{Notify, oneshot};
    use viewer_application::review_workspace::*;
    use viewer_domain::{
        FeedbackId, ProjectId, ReviewArchiveId, ReviewCommandId, ReviewSnapshotId, ReviewStreamId,
        ReviewTextRevisionId, review::continuous::ContinuousReviewState,
    };

    struct SystemTestClock;
    impl ClockPort for SystemTestClock {
        fn unix_millis(&self) -> i64 {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64
        }
    }

    struct Queue(StdMutex<Option<ClaimedReviewMaterialization>>);
    impl ReviewMaterializationQueuePort for Queue {
        fn next(&self, _: i64) -> Result<Option<ClaimedReviewMaterialization>, ReviewCommitError> {
            Ok(self.0.lock().unwrap().take())
        }
        fn retry(
            &self,
            _: &ClaimedReviewMaterialization,
            _: ReviewMaterializationFailure,
            _: i64,
        ) -> Result<(), ReviewCommitError> {
            Ok(())
        }
        fn block(
            &self,
            _: &ClaimedReviewMaterialization,
            _: ReviewMaterializationFailure,
        ) -> Result<(), ReviewCommitError> {
            Ok(())
        }
        fn mark_published(
            &self,
            _: &ClaimedReviewMaterialization,
            _: ReviewPublicationReceipt,
        ) -> Result<(), ReviewCommitError> {
            Ok(())
        }
        fn status(&self, _: ReviewStreamId) -> Result<ReviewPublicationStatus, ReviewCommitError> {
            Ok(ReviewPublicationStatus::Pending {
                pending_revisions: 1,
            })
        }
        fn requeue_expired(&self, _: i64) -> Result<u32, ReviewCommitError> {
            Ok(0)
        }
    }

    struct GatedPublication {
        entered: StdMutex<Option<oneshot::Sender<ReviewTaskCancellation>>>,
        release: Arc<Notify>,
    }
    #[async_trait]
    impl ReviewPublicationPort for GatedPublication {
        async fn materialize(
            &self,
            _: StoredAuthoringState,
            cancellation: ReviewTaskCancellation,
        ) -> Result<PreparedReviewPublication, ReviewWorkspaceError> {
            self.entered
                .lock()
                .unwrap()
                .take()
                .unwrap()
                .send(cancellation.clone())
                .unwrap();
            self.release.notified().await;
            Err(ReviewWorkspaceError::Cancelled)
        }
        async fn publish(
            &self,
            _: PreparedReviewPublication,
        ) -> Result<ReviewPublicationReceipt, ReviewWorkspaceError> {
            unreachable!()
        }
        async fn verify_publication(
            &self,
            _: &StoredAuthoringState,
        ) -> Result<Option<ReviewPublicationReceipt>, ReviewWorkspaceError> {
            Ok(None)
        }
    }

    fn target() -> StoredAuthoringState {
        let project = ProjectId::from_u128(1);
        let stream = ReviewStreamId::from_u128(2);
        let snapshot_id = ReviewSnapshotId::from_u128(3);
        StoredAuthoringState {
            head: ReviewAuthoringHead {
                sequence: 1,
                snapshot_id,
            },
            publication_protocol: ReviewPublicationProtocol::V3,
            production: None,
            state: ContinuousReviewState::empty(project, stream, snapshot_id),
            command_id: ReviewCommandId::from_u128(4),
            payload_digest: [5; 32],
            generated: GeneratedReviewIds {
                snapshot_id,
                feedback_id: FeedbackId::from_u128(6),
                text_revision_id: ReviewTextRevisionId::from_u128(7),
                archive_id: ReviewArchiveId::from_u128(8),
                targets: vec![],
                migration: vec![],
                created_at_ms: 1,
            },
            changes: vec![],
            archives: vec![],
            adopted_usage: vec![],
            barrier: ReviewBarrierKind::None,
        }
    }

    #[tokio::test]
    async fn close_cancels_and_awaits_an_in_flight_materialization() {
        let (entered_tx, entered_rx) = oneshot::channel();
        let release = Arc::new(Notify::new());
        let clock: Arc<dyn ClockPort> = Arc::new(SystemTestClock);
        let service = Arc::new(ReviewMaterializationService::new(
            Arc::new(Queue(StdMutex::new(Some(ClaimedReviewMaterialization {
                stream_id: ReviewStreamId::from_u128(2),
                target: target(),
                lease_epoch: 1,
                attempt_count: 1,
            })))),
            Arc::new(GatedPublication {
                entered: StdMutex::new(Some(entered_tx)),
                release: release.clone(),
            }),
            clock.clone(),
        ));
        let runtime = Arc::new(ReviewMaterializationRuntime::default());
        runtime.start(service, clock).await;
        let cancellation = entered_rx.await.unwrap();
        let owner = runtime.clone();
        let closing = tokio::spawn(async move { owner.close().await });
        tokio::task::yield_now().await;

        assert!(cancellation.is_cancelled());
        assert!(!closing.is_finished());
        release.notify_one();
        closing.await.unwrap();
    }
}
