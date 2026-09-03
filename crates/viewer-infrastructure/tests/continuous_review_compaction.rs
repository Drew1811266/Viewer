use rusqlite::Connection;
use tempfile::TempDir;
use viewer_application::{
    ProjectAccess,
    review_workspace::{
        ContinuousReviewAuthoringStorePort, GeneratedReviewIds, ReviewAuthoringCommitRequest,
        ReviewAuthoringHead, ReviewBarrierKind, ReviewMaterializationFailure,
        ReviewMaterializationQueuePort, ReviewPublicationProtocol, ReviewPublicationReceipt,
        ReviewPublicationStatus, StoredAuthoringState,
    },
};
use viewer_domain::{
    FeedbackId, ProjectId, ReviewArchiveId, ReviewCommandId, ReviewSnapshotId, ReviewStreamId,
    ReviewTextRevisionId,
    review::continuous::{ContinuousReviewState, SnapshotRef},
};
use viewer_infrastructure::{
    portable::PortableProjectMetadata, review::SqliteContinuousReviewAuthoringStore,
};

struct Fixture {
    root: TempDir,
    project_id: ProjectId,
    stream: ReviewStreamId,
    store: SqliteContinuousReviewAuthoringStore,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().unwrap();
        let metadata = PortableProjectMetadata::open(root.path(), ProjectAccess::ReadWrite, 1)
            .expect("create portable project metadata");
        let project_id = metadata.project_id();
        drop(metadata);
        let store = SqliteContinuousReviewAuthoringStore::open(
            root.path(),
            project_id,
            ProjectAccess::ReadWrite,
        )
        .expect("open authoring store");
        Self {
            root,
            project_id,
            stream: ReviewStreamId::from_u128(2),
            store,
        }
    }

    fn commit(&self, barrier: ReviewBarrierKind) -> ReviewAuthoringHead {
        let current = self.store.load_current(self.stream).unwrap();
        let sequence = current.as_ref().map_or(1, |state| state.head.sequence + 1);
        let snapshot_id = ReviewSnapshotId::from_u128(100 + u128::from(sequence));
        let command = ReviewCommandId::from_u128(200 + u128::from(sequence));
        let payload_digest = [sequence as u8; 32];
        let mut state = ContinuousReviewState::empty(self.project_id, self.stream, snapshot_id);
        state.parent = current.as_ref().map(|parent| SnapshotRef {
            snapshot_id: parent.head.snapshot_id,
            blake3: parent.payload_digest,
        });
        let next = StoredAuthoringState {
            publication_protocol: ReviewPublicationProtocol::V3,
            head: ReviewAuthoringHead {
                sequence,
                snapshot_id,
            },
            production: None,
            state,
            command_id: command,
            payload_digest,
            generated: GeneratedReviewIds {
                snapshot_id,
                feedback_id: FeedbackId::from_u128(300 + u128::from(sequence)),
                text_revision_id: ReviewTextRevisionId::from_u128(400 + u128::from(sequence)),
                archive_id: ReviewArchiveId::from_u128(500 + u128::from(sequence)),
                targets: vec![],
                migration: vec![],
                created_at_ms: 1_000 + sequence as i64,
            },
            changes: vec![],
            archives: vec![],
            adopted_usage: vec![],
            barrier,
        };
        let expected_snapshot_id = current.as_ref().map(|state| state.head.snapshot_id);
        self.store
            .commit(self.stream, command, payload_digest, &mut |_| {
                Ok(ReviewAuthoringCommitRequest {
                    expected_snapshot_id,
                    next: next.clone(),
                })
            })
            .unwrap()
            .head
    }

    fn publish_next(&self, now_ms: i64) -> u64 {
        let claim = self.store.next(now_ms).unwrap().expect("queued job");
        let sequence = claim.target.head.sequence;
        self.store
            .mark_published(
                &claim,
                ReviewPublicationReceipt {
                    target: claim.target.head,
                    snapshot: SnapshotRef {
                        snapshot_id: claim.target.head.snapshot_id,
                        blake3: [sequence as u8; 32],
                    },
                },
            )
            .unwrap();
        sequence
    }

    fn snapshot_count(&self) -> u64 {
        let connection =
            Connection::open(self.root.path().join(".viewer/metadata.sqlite")).unwrap();
        connection
            .query_row(
                "SELECT COUNT(*) FROM review_authoring_snapshots",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap() as u64
    }
}

#[test]
fn compaction_stops_before_and_after_every_barrier() {
    let fixture = Fixture::new();
    for barrier in [
        ReviewBarrierKind::None,
        ReviewBarrierKind::None,
        ReviewBarrierKind::Archive,
        ReviewBarrierKind::None,
        ReviewBarrierKind::Restore,
        ReviewBarrierKind::None,
        ReviewBarrierKind::Migration,
        ReviewBarrierKind::None,
    ] {
        fixture.commit(barrier);
    }

    let mut published = Vec::new();
    while fixture.store.status(fixture.stream).unwrap() != ReviewPublicationStatus::Ready {
        published.push(fixture.publish_next(10_000));
    }

    assert_eq!(published, vec![2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(fixture.snapshot_count(), 8);
}

#[test]
fn compacted_jobs_are_claimed_together_and_retry_together() {
    let fixture = Fixture::new();
    for _ in 0..3 {
        fixture.commit(ReviewBarrierKind::None);
    }

    let first = fixture.store.next(10_000).unwrap().unwrap();
    assert_eq!(first.target.head.sequence, 3);
    assert_eq!(fixture.store.next(10_000).unwrap(), None);

    fixture
        .store
        .retry(&first, ReviewMaterializationFailure::Io, 20_000)
        .unwrap();
    assert_eq!(fixture.store.next(19_999).unwrap(), None);
    let retry = fixture.store.next(20_000).unwrap().unwrap();
    assert_eq!(retry.target.head.sequence, 3);
    assert_eq!(retry.attempt_count, 2);

    fixture
        .store
        .mark_published(
            &retry,
            ReviewPublicationReceipt {
                target: retry.target.head,
                snapshot: SnapshotRef {
                    snapshot_id: retry.target.head.snapshot_id,
                    blake3: [9; 32],
                },
            },
        )
        .unwrap();
    assert_eq!(
        fixture.store.status(fixture.stream).unwrap(),
        ReviewPublicationStatus::Ready
    );
    assert_eq!(fixture.snapshot_count(), 3);
}

#[test]
fn blocking_a_compacted_target_releases_its_predecessors_without_losing_snapshots() {
    let fixture = Fixture::new();
    for _ in 0..3 {
        fixture.commit(ReviewBarrierKind::None);
    }

    let claim = fixture.store.next(10_000).unwrap().unwrap();
    assert_eq!(claim.target.head.sequence, 3);
    fixture
        .store
        .block(&claim, ReviewMaterializationFailure::SourceChanged)
        .unwrap();

    assert_eq!(
        fixture.store.status(fixture.stream).unwrap(),
        ReviewPublicationStatus::Blocked {
            code: ReviewMaterializationFailure::SourceChanged,
        }
    );
    assert_eq!(fixture.snapshot_count(), 3);
    assert_eq!(
        fixture
            .store
            .next(10_000)
            .unwrap()
            .unwrap()
            .target
            .head
            .sequence,
        2
    );
}
