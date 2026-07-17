use rusqlite::Connection;
use std::fs;
use tempfile::TempDir;
use viewer_application::ProjectAccess;
use viewer_domain::ProjectId;
use viewer_infrastructure::{
    operation::journal::OperationJournal,
    portable::{PortableMetadataError, PortableProjectMetadata},
};

fn schema_version(database: &std::path::Path) -> i64 {
    Connection::open(database)
        .unwrap()
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap()
}

#[test]
fn identity_first_open_is_atomic_and_reopen_is_stable() {
    let project = TempDir::new().unwrap();

    let first = PortableProjectMetadata::open(project.path(), ProjectAccess::ReadWrite, 123)
        .expect("create portable metadata");
    let project_id = first.project_id();
    let database = first.database_path().unwrap().to_owned();

    assert!(first.is_persistent());
    assert!(first.is_writable());
    assert_eq!(schema_version(&database), 2);
    assert!(project.path().join(".viewer/project.json").is_file());
    assert!(database.is_file());
    assert!(!project.path().join(".viewer/project.json.tmp").exists());
    drop(first);

    let reopened = PortableProjectMetadata::open(project.path(), ProjectAccess::ReadWrite, 999)
        .expect("reopen portable metadata");
    assert_eq!(reopened.project_id(), project_id);

    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(project.path().join(".viewer/project.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["schemaVersion"], 1);
    assert_eq!(manifest["projectId"], project_id.to_string());
    assert_eq!(manifest["createdAtMs"], 123);
}

#[test]
fn copied_project_recovers_the_same_identity() {
    let source = TempDir::new().unwrap();
    let opened =
        PortableProjectMetadata::open(source.path(), ProjectAccess::ReadWrite, 77).unwrap();
    let expected = opened.project_id();
    drop(opened);

    let destination = TempDir::new().unwrap();
    fs::create_dir(destination.path().join(".viewer")).unwrap();
    for name in ["project.json", "metadata.sqlite"] {
        fs::copy(
            source.path().join(".viewer").join(name),
            destination.path().join(".viewer").join(name),
        )
        .unwrap();
    }

    let copied =
        PortableProjectMetadata::open(destination.path(), ProjectAccess::ReadWrite, 88).unwrap();
    assert_eq!(copied.project_id(), expected);
}

#[test]
fn read_only_open_reads_existing_metadata_but_never_creates_it() {
    let existing = TempDir::new().unwrap();
    let writable =
        PortableProjectMetadata::open(existing.path(), ProjectAccess::ReadWrite, 1).unwrap();
    let expected = writable.project_id();
    drop(writable);

    let readonly =
        PortableProjectMetadata::open(existing.path(), ProjectAccess::ReadOnly, 2).unwrap();
    assert_eq!(readonly.project_id(), expected);
    assert!(readonly.is_persistent());
    assert!(!readonly.is_writable());

    let absent = TempDir::new().unwrap();
    let ephemeral =
        PortableProjectMetadata::open(absent.path(), ProjectAccess::ReadOnly, 3).unwrap();
    assert!(!ephemeral.is_persistent());
    assert!(!ephemeral.is_writable());
    assert!(ephemeral.database_path().is_none());
    assert!(!absent.path().join(".viewer").exists());
}

#[test]
fn malformed_and_forward_manifests_are_rejected_without_replacement() {
    for document in [
        br#"{"schemaVersion":1,"projectId":"not-a-uuid","createdAtMs":1}"#.as_slice(),
        br#"{"schemaVersion":2,"projectId":"00000000-0000-0000-0000-000000000001","createdAtMs":1}"#.as_slice(),
    ] {
        let project = TempDir::new().unwrap();
        let viewer = project.path().join(".viewer");
        fs::create_dir(&viewer).unwrap();
        fs::write(viewer.join("project.json"), document).unwrap();

        let before = fs::read(viewer.join("project.json")).unwrap();
        let error = PortableProjectMetadata::open(project.path(), ProjectAccess::ReadWrite, 5)
            .unwrap_err();
        assert!(matches!(
            error,
            PortableMetadataError::InvalidManifest
                | PortableMetadataError::UnsupportedManifestVersion(2)
        ));
        assert_eq!(fs::read(viewer.join("project.json")).unwrap(), before);
    }
}

#[test]
fn version_one_database_migrates_once_with_a_durable_backup() {
    let project = TempDir::new().unwrap();
    let viewer = project.path().join(".viewer");
    fs::create_dir(&viewer).unwrap();
    let project_id = ProjectId::new();
    fs::write(
        viewer.join("project.json"),
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1,
            "projectId": project_id.to_string(),
            "createdAtMs": 41,
        }))
        .unwrap(),
    )
    .unwrap();
    let database = viewer.join("metadata.sqlite");
    Connection::open(&database)
        .unwrap()
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE schema_migrations (
               version INTEGER PRIMARY KEY,
               applied_at_ms INTEGER NOT NULL
             );
             CREATE TABLE operation_batches (
               batch_id TEXT PRIMARY KEY,
               kind TEXT NOT NULL,
               created_at_ms INTEGER NOT NULL,
               completed_at_ms INTEGER
             );
             CREATE TABLE operation_items (
               operation_id TEXT PRIMARY KEY,
               batch_id TEXT NOT NULL REFERENCES operation_batches(batch_id),
               entity_id TEXT NOT NULL,
               kind TEXT NOT NULL,
               state TEXT NOT NULL,
               source_path TEXT NOT NULL,
               destination_path TEXT,
               temporary_path TEXT,
               expected_size INTEGER,
               expected_hash BLOB,
               conflict_policy TEXT NOT NULL,
               error_code TEXT,
               updated_at_ms INTEGER NOT NULL
             );
             CREATE INDEX operation_items_incomplete
             ON operation_items(state)
             WHERE state NOT IN ('completed', 'failed');
             INSERT INTO schema_migrations(version, applied_at_ms) VALUES (1, 0);",
        )
        .unwrap();
    assert_eq!(schema_version(&database), 1);

    let migrated =
        PortableProjectMetadata::open(project.path(), ProjectAccess::ReadWrite, 42).unwrap();
    assert_eq!(migrated.project_id(), project_id);
    assert_eq!(schema_version(&database), 2);
    let backup = viewer.join("metadata.sqlite.v1.bak");
    assert!(backup.is_file());
    let backup_before = fs::read(&backup).unwrap();
    drop(migrated);

    let reopened =
        PortableProjectMetadata::open(project.path(), ProjectAccess::ReadWrite, 43).unwrap();
    assert_eq!(reopened.project_id(), project_id);
    assert_eq!(fs::read(&backup).unwrap(), backup_before);
    drop(reopened);
    OperationJournal::open(&database).expect("operation journal accepts schema v2");
}

#[test]
fn identity_and_database_project_ids_must_agree() {
    let project = TempDir::new().unwrap();
    let opened =
        PortableProjectMetadata::open(project.path(), ProjectAccess::ReadWrite, 1).unwrap();
    let database = opened.database_path().unwrap().to_owned();
    drop(opened);
    Connection::open(database)
        .unwrap()
        .execute(
            "UPDATE project_metadata SET project_id = ?1",
            [ProjectId::new().to_string()],
        )
        .unwrap();

    let error =
        PortableProjectMetadata::open(project.path(), ProjectAccess::ReadWrite, 2).unwrap_err();
    assert!(matches!(
        error,
        PortableMetadataError::ProjectIdentityMismatch
    ));
}
