use rusqlite::{Connection, params};
use std::fs;
use tempfile::TempDir;
use viewer_application::{
    ProjectAccess,
    review_workspace::{
        ContinuousReviewAuthoringStorePort, GeneratedReviewIds, ReviewAuthoringCommitRequest,
        ReviewAuthoringHead, ReviewBarrierKind, ReviewCommitError, ReviewMaterializationFailure,
        ReviewMaterializationQueuePort, ReviewPublicationReceipt, ReviewPublicationStatus,
        ReviewWorkspaceError, StoredAuthoringState,
    },
};
use viewer_domain::{
    FeedbackId, ProjectId, ReviewArchiveId, ReviewCommandId, ReviewSnapshotId, ReviewStreamId,
    ReviewTextRevisionId,
    review::continuous::{ContinuousReviewState, SnapshotRef},
};
use viewer_infrastructure::{
    portable::{PortablePersistenceMode, PortableProjectMetadata},
    review::SqliteContinuousReviewAuthoringStore,
};

struct AuthoringFixture {
    root: TempDir,
    project_id: ProjectId,
    stream: ReviewStreamId,
    store: SqliteContinuousReviewAuthoringStore,
}

impl AuthoringFixture {
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

    fn commit(
        &self,
        command: u128,
        expected_snapshot_id: Option<ReviewSnapshotId>,
        text: &str,
    ) -> Result<viewer_application::review_workspace::ReviewAuthoringReceipt, ReviewWorkspaceError>
    {
        let command_id = ReviewCommandId::from_u128(command);
        let payload_digest = *blake3::hash(text.as_bytes()).as_bytes();
        let mut prepare = |current: Option<&StoredAuthoringState>| {
            let sequence = current.map_or(1, |state| state.head.sequence + 1);
            let snapshot_id = ReviewSnapshotId::from_u128(100 + sequence as u128);
            Ok(ReviewAuthoringCommitRequest {
                expected_snapshot_id,
                next: StoredAuthoringState {
                    head: ReviewAuthoringHead {
                        sequence,
                        snapshot_id,
                    },
                    production: None,
                    state: ContinuousReviewState::empty(self.project_id, self.stream, snapshot_id),
                    command_id,
                    payload_digest,
                    generated: GeneratedReviewIds {
                        snapshot_id,
                        feedback_id: FeedbackId::from_u128(200 + sequence as u128),
                        text_revision_id: ReviewTextRevisionId::from_u128(300 + sequence as u128),
                        archive_id: ReviewArchiveId::from_u128(400 + sequence as u128),
                        targets: vec![],
                        migration: vec![],
                        created_at_ms: 1_000 + sequence as i64,
                    },
                    changes: vec![],
                    archives: vec![],
                    adopted_usage: vec![],
                    barrier: ReviewBarrierKind::None,
                },
            })
        };
        self.store
            .commit(self.stream, command_id, payload_digest, &mut prepare)
    }

    fn queued_targets(&self) -> Vec<u64> {
        let connection =
            Connection::open(self.root.path().join(".viewer/metadata.sqlite")).unwrap();
        let mut statement = connection
            .prepare(
                "SELECT target_seq
                 FROM review_materialization_jobs
                 WHERE stream_id = ?1
                 ORDER BY target_seq",
            )
            .unwrap();
        statement
            .query_map([self.stream.to_string()], |row| {
                row.get::<_, i64>(0).map(|value| value as u64)
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }
}

#[test]
fn commit_advances_head_and_enqueues_exactly_once() {
    let fixture = AuthoringFixture::new();
    assert_eq!(
        fixture.store.persistence_mode(),
        PortablePersistenceMode::Wal
    );
    let receipt = fixture.commit(7, None, "第一条").unwrap();
    let duplicate = fixture.commit(7, None, "第一条").unwrap();

    assert_eq!(duplicate, receipt);
    assert_eq!(
        fixture.store.load_heads(fixture.stream).unwrap().authoring,
        Some(receipt.head)
    );
    assert_eq!(fixture.queued_targets(), vec![receipt.head.sequence]);
    let current = fixture.store.load_current(fixture.stream).unwrap().unwrap();
    assert_eq!(current.head, receipt.head);
    assert_eq!(current.command_id, receipt.command_id);
    assert_eq!(current.payload_digest, receipt.payload_digest);
}

#[test]
fn same_command_with_different_digest_is_a_conflict() {
    let fixture = AuthoringFixture::new();
    fixture.commit(7, None, "第一条").unwrap();

    assert!(matches!(
        fixture.commit(7, None, "不同负载"),
        Err(ReviewWorkspaceError::Repository(
            ReviewCommitError::CommandConflict
        ))
    ));
}

#[test]
fn callback_failure_rolls_back_head_snapshot_and_outbox_together() {
    let fixture = AuthoringFixture::new();
    let command = ReviewCommandId::from_u128(9);
    let mut prepare =
        |_current: Option<&StoredAuthoringState>| Err(ReviewWorkspaceError::NoChanges);

    assert_eq!(
        fixture
            .store
            .commit(fixture.stream, command, [9; 32], &mut prepare),
        Err(ReviewWorkspaceError::NoChanges)
    );
    assert_eq!(
        fixture.store.load_heads(fixture.stream).unwrap().authoring,
        None
    );
    assert!(fixture.queued_targets().is_empty());
}

#[test]
fn stale_basis_rolls_back_without_advancing_or_enqueuing() {
    let fixture = AuthoringFixture::new();
    let first = fixture.commit(7, None, "第一条").unwrap();

    assert!(matches!(
        fixture.commit(8, Some(ReviewSnapshotId::from_u128(999)), "第二条"),
        Err(ReviewWorkspaceError::Repository(
            ReviewCommitError::StaleSnapshot
        ))
    ));
    assert_eq!(
        fixture.store.load_heads(fixture.stream).unwrap().authoring,
        Some(first.head)
    );
    assert_eq!(fixture.queued_targets(), vec![first.head.sequence]);
}

#[test]
fn materialization_claims_are_leased_retryable_and_restart_safe() {
    let fixture = AuthoringFixture::new();
    fixture.commit(7, None, "第一条").unwrap();

    let first = fixture.store.next(2_000).unwrap().unwrap();
    assert_eq!(first.attempt_count, 1);
    assert_eq!(first.lease_epoch, 1);
    assert_eq!(fixture.store.requeue_expired(31_999).unwrap(), 0);
    assert_eq!(fixture.store.requeue_expired(32_000).unwrap(), 1);
    let second = fixture.store.next(32_000).unwrap().unwrap();
    assert_eq!(second.attempt_count, 2);
    assert_eq!(second.lease_epoch, 2);
    fixture
        .store
        .retry(&second, ReviewMaterializationFailure::RenderFailed, 40_000)
        .unwrap();
    assert!(fixture.store.next(39_999).unwrap().is_none());
    let third = fixture.store.next(40_000).unwrap().unwrap();
    fixture
        .store
        .block(&third, ReviewMaterializationFailure::SourceChanged)
        .unwrap();
    assert_eq!(
        fixture.store.status(fixture.stream).unwrap(),
        ReviewPublicationStatus::Blocked {
            code: ReviewMaterializationFailure::SourceChanged
        }
    );
}

#[test]
fn publishing_a_claim_advances_only_the_published_head_and_clears_its_job() {
    let fixture = AuthoringFixture::new();
    let receipt = fixture.commit(7, None, "第一条").unwrap();
    assert_eq!(
        fixture.store.status(fixture.stream).unwrap(),
        ReviewPublicationStatus::Pending {
            pending_revisions: 1
        }
    );
    let claim = fixture.store.next(2_000).unwrap().unwrap();

    fixture
        .store
        .mark_published(
            &claim,
            ReviewPublicationReceipt {
                target: receipt.head,
                snapshot: SnapshotRef {
                    snapshot_id: receipt.head.snapshot_id,
                    blake3: [5; 32],
                },
            },
        )
        .unwrap();

    let heads = fixture.store.load_heads(fixture.stream).unwrap();
    assert_eq!(heads.authoring, Some(receipt.head));
    assert_eq!(heads.published, Some(receipt.head));
    assert_eq!(
        fixture.store.status(fixture.stream).unwrap(),
        ReviewPublicationStatus::Ready
    );
    assert!(fixture.queued_targets().is_empty());
}

#[test]
fn schema_rejects_unbounded_status_barrier_and_error_values() {
    let fixture = AuthoringFixture::new();
    fixture.commit(7, None, "第一条").unwrap();
    let connection = Connection::open(fixture.root.path().join(".viewer/metadata.sqlite")).unwrap();
    connection
        .execute_batch("PRAGMA foreign_keys = ON")
        .unwrap();

    assert!(
        connection
            .execute(
                "UPDATE review_authoring_snapshots SET barrier_kind = 'invented'",
                [],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE review_materialization_jobs SET status = 'finished'",
                [],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE review_materialization_jobs SET error_code = '../../private/path'",
                [],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE review_authoring_streams SET authoring_snapshot_id = ?1",
                [ReviewSnapshotId::from_u128(999).to_string()],
            )
            .is_err()
    );
}

#[test]
fn reads_fail_closed_when_indexed_transition_and_logical_state_disagree() {
    let fixture = AuthoringFixture::new();
    fixture.commit(7, None, "第一条").unwrap();
    let connection = Connection::open(fixture.root.path().join(".viewer/metadata.sqlite")).unwrap();
    connection
        .execute(
            "UPDATE review_authoring_snapshots SET transition = ?1",
            [b"{}".as_slice()],
        )
        .unwrap();

    assert_eq!(
        fixture.store.load_current(fixture.stream),
        Err(ReviewCommitError::Integrity)
    );
    assert_eq!(
        fixture.commit(7, None, "第一条"),
        Err(ReviewWorkspaceError::Repository(
            ReviewCommitError::Integrity
        ))
    );
}

#[test]
fn v3_database_migrates_to_v4_without_changing_operation_rows() {
    let project = TempDir::new().unwrap();
    let viewer = project.path().join(".viewer");
    fs::create_dir(&viewer).unwrap();
    let project_id = ProjectId::from_u128(11);
    fs::write(
        viewer.join("project.json"),
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1,
            "projectId": project_id.to_string(),
            "createdAtMs": 1,
        }))
        .unwrap(),
    )
    .unwrap();
    let database = viewer.join("metadata.sqlite");
    let connection = Connection::open(&database).unwrap();
    connection
        .execute_batch(concat!(
            include_str!("../migrations/portable/0001_initial.sql"),
            include_str!("../migrations/portable/0002_markers.sql"),
            include_str!("../migrations/portable/0003_operation_results.sql")
        ))
        .unwrap();
    connection
        .execute(
            "INSERT INTO project_metadata(singleton, project_id, created_at_ms)
             VALUES (1, ?1, 1)",
            [project_id.to_string()],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO operation_batches(
                batch_id, kind, created_at_ms, completed_at_ms, state, requested_count,
                completed_count, failed_count, skipped_count, started_at_ms
             ) VALUES (?1, 'copy', 2, NULL, 'running', 1, 0, 0, 0, 2)",
            [ReviewCommandId::from_u128(12).to_string()],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO operation_items(
                operation_id, batch_id, entity_id, kind, state, source_path,
                destination_path, temporary_path, expected_size, expected_hash,
                conflict_policy, error_code, updated_at_ms, result_code
             ) VALUES (?1, ?2, ?3, 'copy', 'prepared', 'a.jpg', 'b.jpg', NULL,
                       3, NULL, 'skip', NULL, 2, NULL)",
            params![
                ReviewCommandId::from_u128(13).to_string(),
                ReviewCommandId::from_u128(12).to_string(),
                ReviewCommandId::from_u128(14).to_string(),
            ],
        )
        .unwrap();
    let before: Vec<(String, String, String, i64)> = connection
        .prepare(
            "SELECT operation_id, batch_id, state, updated_at_ms
             FROM operation_items ORDER BY operation_id",
        )
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    drop(connection);

    let metadata = PortableProjectMetadata::open(project.path(), ProjectAccess::ReadWrite, 3)
        .expect("migrate v3 metadata");
    drop(metadata);
    let connection = Connection::open(&database).unwrap();
    let versions = connection
        .prepare("SELECT version FROM schema_migrations ORDER BY version")
        .unwrap()
        .query_map([], |row| row.get::<_, i64>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let after: Vec<(String, String, String, i64)> = connection
        .prepare(
            "SELECT operation_id, batch_id, state, updated_at_ms
             FROM operation_items ORDER BY operation_id",
        )
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();

    assert_eq!(versions, vec![1, 2, 3, 4]);
    assert_eq!(after, before);
    assert!(viewer.join("metadata.sqlite.v3.bak").is_file());
}
