#[path = "support/continuous_review.rs"]
mod continuous_review;

use async_trait::async_trait;
use continuous_review::Fixture;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use viewer_application::{
    ClockPort, ReviewArtifactError, ReviewAssetError, ReviewTaskCancellation, review_workspace::*,
};
use viewer_domain::{
    EntityId, FeedbackId, ProjectId, ReviewArchiveId, ReviewCommandId, ReviewSnapshotId,
    ReviewStreamId, ReviewTextRevisionId,
    review::{FeedbackAnchor, continuous::ContinuousReviewState},
};

struct Clock;
impl ClockPort for Clock {
    fn unix_millis(&self) -> i64 {
        1_000
    }
}

struct Queue {
    claim: Mutex<Option<ClaimedReviewMaterialization>>,
    published: Mutex<Option<ReviewAuthoringHead>>,
    retried: Mutex<Option<(ReviewMaterializationFailure, i64)>>,
    blocked: Mutex<Option<ReviewMaterializationFailure>>,
}
impl ReviewMaterializationQueuePort for Queue {
    fn next(&self, _: i64) -> Result<Option<ClaimedReviewMaterialization>, ReviewCommitError> {
        Ok(self.claim.lock().unwrap().take())
    }
    fn retry(
        &self,
        _: &ClaimedReviewMaterialization,
        code: ReviewMaterializationFailure,
        next_attempt_at_ms: i64,
    ) -> Result<(), ReviewCommitError> {
        *self.retried.lock().unwrap() = Some((code, next_attempt_at_ms));
        Ok(())
    }
    fn block(
        &self,
        _: &ClaimedReviewMaterialization,
        code: ReviewMaterializationFailure,
    ) -> Result<(), ReviewCommitError> {
        *self.blocked.lock().unwrap() = Some(code);
        Ok(())
    }
    fn mark_published(
        &self,
        claim: &ClaimedReviewMaterialization,
        receipt: ReviewPublicationReceipt,
    ) -> Result<(), ReviewCommitError> {
        assert_eq!(receipt.target, claim.target.head);
        *self.published.lock().unwrap() = Some(receipt.target);
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

struct Publication {
    published: Mutex<Option<ReviewPublicationReceipt>>,
    materialize_error: Mutex<Option<ReviewWorkspaceError>>,
    materialize_calls: AtomicUsize,
    publish_calls: AtomicUsize,
}

struct RejectingPublication;
#[async_trait]
impl ReviewPublicationPort for RejectingPublication {
    async fn materialize(
        &self,
        target: StoredAuthoringState,
        _: ReviewTaskCancellation,
    ) -> Result<PreparedReviewPublication, ReviewWorkspaceError> {
        Ok(PreparedReviewPublication {
            target: target.head,
            request: ReviewCommitRequest {
                expected: target.state.parent,
                production: target.production,
                next: PreparedContinuousSnapshot {
                    state: target.state,
                    command_id: target.command_id,
                    payload_digest: target.payload_digest,
                    changes: target.changes,
                    evidence: vec![],
                },
                archives: target.archives,
                adopted_usage: target.adopted_usage,
                staged_evidence: vec![],
            },
        })
    }

    async fn publish(
        &self,
        _: PreparedReviewPublication,
    ) -> Result<ReviewPublicationReceipt, ReviewWorkspaceError> {
        Err(ReviewCommitError::Integrity.into())
    }

    async fn verify_publication(
        &self,
        _: &StoredAuthoringState,
    ) -> Result<Option<ReviewPublicationReceipt>, ReviewWorkspaceError> {
        Ok(None)
    }
}

fn target() -> StoredAuthoringState {
    let snapshot_id = ReviewSnapshotId::from_u128(300);
    StoredAuthoringState {
        head: ReviewAuthoringHead {
            sequence: 1,
            snapshot_id,
        },
        production: None,
        state: ContinuousReviewState::empty(
            ProjectId::from_u128(1),
            ReviewStreamId::from_u128(2),
            snapshot_id,
        ),
        command_id: ReviewCommandId::from_u128(301),
        payload_digest: [7; 32],
        generated: GeneratedReviewIds {
            snapshot_id,
            feedback_id: FeedbackId::from_u128(302),
            text_revision_id: ReviewTextRevisionId::from_u128(303),
            archive_id: ReviewArchiveId::from_u128(304),
            targets: vec![],
            migration: vec![],
            created_at_ms: 900,
        },
        changes: vec![],
        archives: vec![],
        adopted_usage: vec![],
        barrier: ReviewBarrierKind::None,
    }
}

fn pending_queue(target: StoredAuthoringState, attempt_count: u32) -> Arc<Queue> {
    Arc::new(Queue {
        claim: Mutex::new(Some(ClaimedReviewMaterialization {
            stream_id: target.state.stream_id,
            target,
            lease_epoch: 1,
            attempt_count,
        })),
        published: Mutex::new(None),
        retried: Mutex::new(None),
        blocked: Mutex::new(None),
    })
}

fn publication(error: Option<ReviewWorkspaceError>) -> Arc<Publication> {
    Arc::new(Publication {
        published: Mutex::new(None),
        materialize_error: Mutex::new(error),
        materialize_calls: AtomicUsize::new(0),
        publish_calls: AtomicUsize::new(0),
    })
}
#[async_trait]
impl ReviewPublicationPort for Publication {
    async fn materialize(
        &self,
        target: StoredAuthoringState,
        _: ReviewTaskCancellation,
    ) -> Result<PreparedReviewPublication, ReviewWorkspaceError> {
        self.materialize_calls.fetch_add(1, Ordering::AcqRel);
        if let Some(error) = self.materialize_error.lock().unwrap().take() {
            return Err(error);
        }
        Ok(PreparedReviewPublication {
            target: target.head,
            request: ReviewCommitRequest {
                expected: target.state.parent,
                production: target.production,
                next: PreparedContinuousSnapshot {
                    state: target.state,
                    command_id: target.command_id,
                    payload_digest: target.payload_digest,
                    changes: target.changes,
                    evidence: vec![],
                },
                archives: target.archives,
                adopted_usage: target.adopted_usage,
                staged_evidence: vec![],
            },
        })
    }
    async fn publish(
        &self,
        prepared: PreparedReviewPublication,
    ) -> Result<ReviewPublicationReceipt, ReviewWorkspaceError> {
        self.publish_calls.fetch_add(1, Ordering::AcqRel);
        let receipt = ReviewPublicationReceipt {
            target: prepared.target,
            snapshot: viewer_domain::review::continuous::SnapshotRef {
                snapshot_id: prepared.target.snapshot_id,
                blake3: prepared.request.next.payload_digest,
            },
        };
        *self.published.lock().unwrap() = Some(receipt);
        Ok(receipt)
    }
    async fn verify_publication(
        &self,
        target: &StoredAuthoringState,
    ) -> Result<Option<ReviewPublicationReceipt>, ReviewWorkspaceError> {
        Ok(self
            .published
            .lock()
            .unwrap()
            .filter(|receipt| receipt.target == target.head))
    }
}

#[tokio::test]
async fn one_job_materializes_exact_target_and_advances_published_head_last() {
    let fixture = Fixture::new();
    let service = fixture.authoring_service();
    let asset = service
        .prepare_assets(
            &[EntityId::from_u128(10)],
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap()
        .remove(0);
    let envelope = service
        .prepare(
            ReviewCommandId::from_u128(90),
            None,
            ReviewWorkspaceCommand::SaveFeedback {
                feedback_id: None,
                text: "收紧袖口".into(),
                targets: vec![TargetEdit::Add {
                    asset_version_id: asset.id,
                    anchor: FeedbackAnchor::Asset,
                }],
            },
        )
        .await
        .unwrap();
    service
        .apply_authoring_with_cancellation(envelope, ReviewTaskCancellation::default())
        .await
        .unwrap();
    let target = fixture
        .authoring
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    let queue = Arc::new(Queue {
        claim: Mutex::new(Some(ClaimedReviewMaterialization {
            stream_id: ReviewStreamId::from_u128(2),
            target: target.clone(),
            lease_epoch: 1,
            attempt_count: 1,
        })),
        published: Mutex::new(None),
        retried: Mutex::new(None),
        blocked: Mutex::new(None),
    });
    let materializer = ReviewMaterializationService::new(
        queue.clone(),
        Arc::new(Publication {
            published: Mutex::new(None),
            materialize_error: Mutex::new(None),
            materialize_calls: AtomicUsize::new(0),
            publish_calls: AtomicUsize::new(0),
        }),
        Arc::new(Clock),
    );

    let outcome = materializer
        .run_one(ReviewTaskCancellation::default())
        .await
        .unwrap();

    assert!(matches!(
        outcome,
        ReviewMaterializationOutcome::Published { .. }
    ));
    assert_eq!(*queue.published.lock().unwrap(), Some(target.head));
}

#[tokio::test]
async fn permanent_source_failure_blocks_with_a_stable_code() {
    let target = target();
    let queue = pending_queue(target.clone(), 1);
    let publication = publication(Some(ReviewAssetError::SourceChanged.into()));
    let service = ReviewMaterializationService::new(queue.clone(), publication, Arc::new(Clock));

    let outcome = service
        .run_one(ReviewTaskCancellation::default())
        .await
        .unwrap();

    assert_eq!(
        outcome,
        ReviewMaterializationOutcome::Blocked {
            target: target.head,
            code: ReviewMaterializationFailure::SourceChanged,
        }
    );
    assert_eq!(
        *queue.blocked.lock().unwrap(),
        Some(ReviewMaterializationFailure::SourceChanged)
    );
    assert_eq!(*queue.retried.lock().unwrap(), None);
}

#[tokio::test]
async fn transient_renderer_failure_uses_bounded_retry_then_blocks() {
    let target = target();
    let queue = pending_queue(target.clone(), 1);
    let service = ReviewMaterializationService::new(
        queue.clone(),
        publication(Some(ReviewArtifactError::Unavailable.into())),
        Arc::new(Clock),
    );
    let first = service
        .run_one(ReviewTaskCancellation::default())
        .await
        .unwrap();
    assert_eq!(
        first,
        ReviewMaterializationOutcome::Retrying {
            target: target.head,
            attempt_count: 1,
        }
    );
    assert_eq!(
        *queue.retried.lock().unwrap(),
        Some((ReviewMaterializationFailure::RenderFailed, 1_100))
    );

    let exhausted = pending_queue(target.clone(), 6);
    let service = ReviewMaterializationService::new(
        exhausted.clone(),
        publication(Some(ReviewArtifactError::Unavailable.into())),
        Arc::new(Clock),
    );
    assert_eq!(
        service
            .run_one(ReviewTaskCancellation::default())
            .await
            .unwrap(),
        ReviewMaterializationOutcome::Blocked {
            target: target.head,
            code: ReviewMaterializationFailure::RenderFailed,
        }
    );
    assert_eq!(
        *exhausted.blocked.lock().unwrap(),
        Some(ReviewMaterializationFailure::RenderFailed)
    );
}

#[tokio::test]
async fn restart_reconciliation_advances_a_verified_target_without_rematerializing() {
    let target = target();
    let queue = pending_queue(target.clone(), 2);
    let publication = publication(None);
    *publication.published.lock().unwrap() = Some(ReviewPublicationReceipt {
        target: target.head,
        snapshot: viewer_domain::review::continuous::SnapshotRef {
            snapshot_id: target.head.snapshot_id,
            blake3: target.payload_digest,
        },
    });
    let service =
        ReviewMaterializationService::new(queue.clone(), publication.clone(), Arc::new(Clock));

    assert!(matches!(
        service
            .run_one(ReviewTaskCancellation::default())
            .await
            .unwrap(),
        ReviewMaterializationOutcome::Published { .. }
    ));
    assert_eq!(publication.materialize_calls.load(Ordering::Acquire), 0);
    assert_eq!(publication.publish_calls.load(Ordering::Acquire), 0);
    assert_eq!(*queue.published.lock().unwrap(), Some(target.head));
}

#[tokio::test]
async fn permanent_publish_failure_is_not_downgraded_to_an_io_retry() {
    let target = target();
    let queue = pending_queue(target.clone(), 1);
    let service = ReviewMaterializationService::new(
        queue.clone(),
        Arc::new(RejectingPublication),
        Arc::new(Clock),
    );

    assert_eq!(
        service
            .run_one(ReviewTaskCancellation::default())
            .await
            .unwrap(),
        ReviewMaterializationOutcome::Blocked {
            target: target.head,
            code: ReviewMaterializationFailure::Integrity,
        }
    );
    assert_eq!(
        *queue.blocked.lock().unwrap(),
        Some(ReviewMaterializationFailure::Integrity)
    );
    assert_eq!(*queue.retried.lock().unwrap(), None);
}
