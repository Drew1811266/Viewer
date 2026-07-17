use async_trait::async_trait;
use rusqlite::Connection;
use std::{
    fs,
    sync::{Arc, Mutex},
};
use viewer_application::{
    CommitStage, FileOperationError, OperationCommit, OperationCommitError, OperationCommitPort,
};
use viewer_domain::{
    EntityId, OperationId, RelativePath,
    operation::{ConflictPolicy, OperationItemPlan, OperationKind, OperationPlan, OperationState},
};
use viewer_infrastructure::operation::{
    copy::LocalFileMutation,
    executor::{CopyError, CopyExecutor},
    journal::{BatchState, JournalError, OperationJournal},
};
use viewer_test_support::{FixedClock, project_fixture::ProjectFixture};

const MIGRATION_V1: &str =
    include_str!("../crates/viewer-infrastructure/migrations/portable/0001_initial.sql");
const MIGRATION_V2: &str =
    include_str!("../crates/viewer-infrastructure/migrations/portable/0002_markers.sql");

fn schema_versions(path: &std::path::Path) -> Vec<i64> {
    let connection = Connection::open(path).unwrap();
    let mut statement = connection
        .prepare("SELECT version FROM schema_migrations ORDER BY version")
        .unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

fn item(batch_id: OperationId, source: &str, destination: &str) -> OperationItemPlan {
    OperationItemPlan {
        batch_id,
        operation_id: OperationId::new(),
        entity_id: EntityId::new(),
        kind: OperationKind::Copy,
        source: RelativePath::parse(source).unwrap(),
        destination: Some(RelativePath::parse(destination).unwrap()),
        conflict_policy: ConflictPolicy::Skip,
    }
}

#[test]
fn complete_plan_registration_is_atomic_even_when_a_late_item_conflicts() {
    let directory = tempfile::tempdir().unwrap();
    let journal = OperationJournal::open(directory.path().join("metadata.sqlite")).unwrap();
    let existing_batch = OperationId::new();
    let existing = item(existing_batch, "existing.jpg", "out/existing.jpg");
    journal
        .begin_batch(existing_batch, OperationKind::Copy, 1, 1)
        .unwrap();
    journal.record_item(&existing, 2).unwrap();

    let attempted_batch = OperationId::new();
    let first = item(attempted_batch, "first.jpg", "out/first.jpg");
    let mut late_conflict = item(attempted_batch, "second.jpg", "out/second.jpg");
    late_conflict.operation_id = existing.operation_id;
    let plan = OperationPlan {
        batch_id: attempted_batch,
        kind: OperationKind::Copy,
        items: vec![first.clone(), late_conflict],
    };

    assert!(matches!(
        journal.begin_plan(&plan, 3),
        Err(JournalError::Database(_))
    ));
    assert!(journal.batch(attempted_batch).unwrap().is_none());
    assert!(journal.item(first.operation_id).unwrap().is_none());
    assert_eq!(
        journal
            .item(existing.operation_id)
            .unwrap()
            .unwrap()
            .batch_id,
        existing_batch
    );
}

#[test]
fn schema_v3_is_exact_and_enforces_lifecycle_constraints() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("metadata.sqlite");
    drop(OperationJournal::open(&database).unwrap());

    assert_eq!(schema_versions(&database), vec![1, 2, 3]);
    let connection = Connection::open(&database).unwrap();
    assert!(
        connection
            .execute(
                "INSERT INTO operation_batches(
                batch_id, kind, created_at_ms, state, requested_count,
                completed_count, failed_count, skipped_count, started_at_ms
             ) VALUES ('bad', 'copy', 1, 'invented', 1, 0, 0, 0, 1)",
                [],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "INSERT INTO operation_batches(
                batch_id, kind, created_at_ms, state, requested_count,
                completed_count, failed_count, skipped_count, started_at_ms
             ) VALUES ('bad-count', 'copy', 1, 'running', -1, 0, 0, 0, 1)",
                [],
            )
            .is_err()
    );
}

#[test]
fn version_two_migrates_once_with_an_immutable_v2_backup() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("metadata.sqlite");
    Connection::open(&database)
        .unwrap()
        .execute_batch(&format!("{MIGRATION_V1}\n{MIGRATION_V2}"))
        .unwrap();
    let legacy_batch = OperationId::new();
    let legacy_operation = OperationId::new();
    let legacy_entity = EntityId::new();
    Connection::open(&database)
        .unwrap()
        .execute_batch(&format!(
            "INSERT INTO operation_batches(batch_id, kind, created_at_ms)
             VALUES ('{legacy_batch}', 'copy', 1);
             INSERT INTO operation_items(
               operation_id, batch_id, entity_id, kind, state, source_path,
               destination_path, conflict_policy, error_code, updated_at_ms
             ) VALUES (
               '{legacy_operation}', '{legacy_batch}', '{legacy_entity}', 'copy', 'failed',
               'source.jpg', 'target.jpg', 'skip', '../../private/path raw OS error', 2
             );"
        ))
        .unwrap();
    let v2_bytes = fs::read(&database).unwrap();

    drop(OperationJournal::open(&database).unwrap());

    assert_eq!(schema_versions(&database), vec![1, 2, 3]);
    let migrated = Connection::open(&database).unwrap();
    let (result_code, error_code): (String, String) = migrated
        .query_row(
            "SELECT result_code, error_code FROM operation_items WHERE operation_id = ?1",
            [legacy_operation.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(result_code, "legacy_failure");
    assert_eq!(error_code, "legacy_failure");
    drop(migrated);
    let backup = directory.path().join("metadata.sqlite.v2.bak");
    assert_eq!(fs::read(&backup).unwrap(), v2_bytes);
    let backup_before_reopen = fs::read(&backup).unwrap();
    drop(OperationJournal::open(&database).unwrap());
    assert_eq!(fs::read(backup).unwrap(), backup_before_reopen);
}

#[test]
fn non_contiguous_schema_history_is_rejected_without_mutation() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("metadata.sqlite");
    let connection = Connection::open(&database).unwrap();
    connection.execute_batch(MIGRATION_V1).unwrap();
    connection
        .execute(
            "INSERT INTO schema_migrations(version, applied_at_ms) VALUES (3, 0)",
            [],
        )
        .unwrap();
    drop(connection);
    let before = fs::read(&database).unwrap();

    assert!(matches!(
        OperationJournal::open(&database),
        Err(JournalError::Schema(_))
    ));
    assert_eq!(fs::read(database).unwrap(), before);
}

#[test]
fn journal_counts_each_terminal_result_once_and_pages_stable_codes() {
    let directory = tempfile::tempdir().unwrap();
    let journal = OperationJournal::open(directory.path().join("metadata.sqlite")).unwrap();
    let batch_id = OperationId::new();
    let completed = item(batch_id, "a.jpg", "out/a.jpg");
    let failed = item(batch_id, "b.jpg", "out/b.jpg");
    let skipped = item(batch_id, "c.jpg", "out/c.jpg");
    journal
        .begin_batch(batch_id, OperationKind::Copy, 3, 100)
        .unwrap();
    for candidate in [&completed, &failed, &skipped] {
        journal.record_item(candidate, 101).unwrap();
    }

    for (current, next) in [
        (OperationState::Prepared, OperationState::Staged),
        (OperationState::Staged, OperationState::FsApplied),
        (OperationState::FsApplied, OperationState::Verified),
        (OperationState::Verified, OperationState::MetaCommitted),
        (OperationState::MetaCommitted, OperationState::IndexSynced),
    ] {
        journal
            .advance(completed.operation_id, current, next, 102)
            .unwrap();
    }
    journal
        .complete_item(
            completed.operation_id,
            OperationState::IndexSynced,
            "completed",
            103,
        )
        .unwrap();
    journal
        .fail_item(
            failed.operation_id,
            OperationState::Prepared,
            "permission_denied",
            104,
        )
        .unwrap();
    journal
        .skip_item(
            skipped.operation_id,
            OperationState::Prepared,
            "conflict_skipped",
            105,
        )
        .unwrap();
    assert!(
        journal
            .complete_item(
                completed.operation_id,
                OperationState::IndexSynced,
                "completed",
                106,
            )
            .is_err()
    );

    journal.finish_batch(batch_id, 107).unwrap();
    let batch = journal.batch(batch_id).unwrap().unwrap();
    assert_eq!(batch.state, BatchState::Completed);
    assert_eq!(batch.requested_count, 3);
    assert_eq!(batch.completed_count, 1);
    assert_eq!(batch.failed_count, 1);
    assert_eq!(batch.skipped_count, 1);
    assert_eq!(batch.completed_at_ms, Some(107));

    let first_page = journal.results_page(batch_id, 0, 2).unwrap();
    let second_page = journal.results_page(batch_id, 2, 200).unwrap();
    assert_eq!(first_page.len(), 2);
    assert_eq!(second_page.len(), 1);
    let mut codes = first_page
        .into_iter()
        .chain(second_page)
        .map(|row| row.result_code.unwrap())
        .collect::<Vec<_>>();
    codes.sort();
    assert_eq!(
        codes,
        ["completed", "conflict_skipped", "permission_denied"]
    );
    assert!(journal.results_page(batch_id, 0, 201).is_err());
}

#[derive(Default)]
struct RecordingCommitPort {
    events: Mutex<Vec<CommitStage>>,
    fail_at: Mutex<Option<CommitStage>>,
}

impl RecordingCommitPort {
    fn failing(stage: CommitStage) -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            fail_at: Mutex::new(Some(stage)),
        }
    }

    fn events(&self) -> Vec<CommitStage> {
        self.events.lock().unwrap().clone()
    }

    fn record(&self, stage: CommitStage) -> Result<(), OperationCommitError> {
        self.events.lock().unwrap().push(stage);
        if *self.fail_at.lock().unwrap() == Some(stage) {
            Err(OperationCommitError::new(stage, "injected_commit_failure"))
        } else {
            Ok(())
        }
    }
}

#[async_trait]
impl OperationCommitPort for RecordingCommitPort {
    async fn commit_metadata(&self, _commit: &OperationCommit) -> Result<(), OperationCommitError> {
        self.record(CommitStage::Metadata)
    }

    async fn sync_index(&self, _commit: &OperationCommit) -> Result<(), OperationCommitError> {
        self.record(CommitStage::Index)
    }
}

fn prepared_copy(project: &ProjectFixture) -> (Arc<OperationJournal>, OperationItemPlan, Vec<u8>) {
    let contents = b"truthful barriers".repeat(4_096);
    let source = project.create_file("products/a.jpg", &contents);
    project.create_directory("exports");
    let batch_id = OperationId::new();
    let item = item(batch_id, source.as_str(), "exports/a.jpg");
    let journal = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
    journal
        .begin_batch(batch_id, OperationKind::Copy, 1, 1_000)
        .unwrap();
    journal.record_item(&item, 1_001).unwrap();
    (journal, item, contents)
}

#[tokio::test]
async fn copy_crosses_barriers_only_after_real_callbacks_succeed() {
    for (failure, expected_state, expected_events) in [
        (
            Some(CommitStage::Metadata),
            OperationState::Verified,
            vec![CommitStage::Metadata],
        ),
        (
            Some(CommitStage::Index),
            OperationState::MetaCommitted,
            vec![CommitStage::Metadata, CommitStage::Index],
        ),
        (
            None,
            OperationState::Completed,
            vec![CommitStage::Metadata, CommitStage::Index],
        ),
    ] {
        let project = ProjectFixture::new();
        let (journal, item, contents) = prepared_copy(&project);
        let commits = Arc::new(match failure {
            Some(stage) => RecordingCommitPort::failing(stage),
            None => RecordingCommitPort::default(),
        });
        let executor = CopyExecutor::new(
            project.root(),
            Arc::clone(&journal),
            Arc::new(LocalFileMutation),
            Arc::new(FixedClock::new(2_000)),
            commits.clone(),
        )
        .unwrap();

        let outcome = executor.execute(&item).await;
        if failure.is_some() {
            assert!(matches!(outcome, Err(CopyError::Commit(_))));
        } else {
            outcome.unwrap();
        }
        assert_eq!(
            fs::read(project.root().join("exports/a.jpg")).unwrap(),
            contents
        );
        assert_eq!(
            journal.item(item.operation_id).unwrap().unwrap().state,
            expected_state
        );
        assert_eq!(commits.events(), expected_events);
    }
}

#[test]
fn stable_result_codes_reject_paths_and_unbounded_messages() {
    let directory = tempfile::tempdir().unwrap();
    let journal = OperationJournal::open(directory.path().join("metadata.sqlite")).unwrap();
    let batch_id = OperationId::new();
    let candidate = item(batch_id, "a.jpg", "b.jpg");
    journal
        .begin_batch(batch_id, OperationKind::Copy, 1, 1)
        .unwrap();
    journal.record_item(&candidate, 2).unwrap();
    assert!(matches!(
        journal.fail_item(
            candidate.operation_id,
            OperationState::Prepared,
            "../../private/path\nraw OS error",
            3,
        ),
        Err(JournalError::InvalidResultCode)
    ));
    assert_eq!(
        journal.item(candidate.operation_id).unwrap().unwrap().state,
        OperationState::Prepared
    );
}

#[test]
fn batch_rejects_an_item_whose_kind_differs_from_the_batch() {
    let directory = tempfile::tempdir().unwrap();
    let journal = OperationJournal::open(directory.path().join("metadata.sqlite")).unwrap();
    let batch_id = OperationId::new();
    let mut candidate = item(batch_id, "a.jpg", "b.jpg");
    candidate.kind = OperationKind::Move;
    journal
        .begin_batch(batch_id, OperationKind::Copy, 1, 1)
        .unwrap();

    assert!(matches!(
        journal.record_item(&candidate, 2),
        Err(JournalError::BatchNotWritable(id)) if id == batch_id
    ));
    assert!(journal.item(candidate.operation_id).unwrap().is_none());
}

#[allow(dead_code)]
fn assert_file_error_is_send_sync(error: FileOperationError) {
    fn require<T: Send + Sync>(_: T) {}
    require(error);
}
