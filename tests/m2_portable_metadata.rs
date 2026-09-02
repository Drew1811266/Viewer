use rusqlite::Connection;
use std::{collections::BTreeMap, fs};
use tempfile::TempDir;
use viewer_application::{
    ProjectAccess,
    metadata::{
        FavoritePatch, Marker, MarkerChange, MarkerPatch, MarkerProjectionError,
        MarkerProjectionPort, MarkerService, MarkerServiceError, MarkerTarget,
        PortableMetadataPort, ReviewPatch,
    },
};
use viewer_domain::{
    EntityId, ProjectId, RelativePath,
    file::{FileKind, ReviewState},
};
use viewer_infrastructure::{
    operation::journal::OperationJournal,
    portable::{PortableMarkerStore, PortableMetadataError, PortableProjectMetadata},
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
    assert_eq!(schema_version(&database), 4);
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
fn read_only_schema_v2_uses_marker_adapter_without_migrating_portable_bytes() {
    let project = TempDir::new().unwrap();
    let writable =
        PortableProjectMetadata::open(project.path(), ProjectAccess::ReadWrite, 1).unwrap();
    let project_id = writable.project_id();
    let database = writable.database_path().unwrap().to_path_buf();
    let target = marker_target("notes.txt", FileKind::Text);
    drop(writable);

    fs::remove_file(&database).unwrap();
    for suffix in ["-wal", "-shm", "-journal"] {
        let _ = fs::remove_file(format!("{}{}", database.display(), suffix));
    }
    let connection = Connection::open(&database).unwrap();
    connection
        .execute_batch(concat!(
            include_str!("../crates/viewer-infrastructure/migrations/portable/0001_initial.sql"),
            include_str!("../crates/viewer-infrastructure/migrations/portable/0002_markers.sql")
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
            "INSERT INTO markers(
                marker_id, relative_path, kind, review_state, favorite,
                evidence_size, evidence_modified_ns, content_hash, updated_at_ms
             ) VALUES (?1, ?2, 4, 0, 1, ?3, ?4, NULL, 2)",
            rusqlite::params![
                EntityId::new().to_string(),
                target.relative_path.as_str(),
                i64::try_from(target.size).unwrap(),
                target.modified_ns.to_string(),
            ],
        )
        .unwrap();
    let batch_columns = connection
        .prepare("PRAGMA table_info(operation_batches)")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(!batch_columns.iter().any(|column| column == "state"));
    drop(connection);
    assert_eq!(schema_version(&database), 2);
    let viewer = project.path().join(".viewer");
    let before = fs::read_dir(&viewer)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name(), fs::read(entry.path()).unwrap())
        })
        .collect::<BTreeMap<_, _>>();

    let metadata =
        PortableProjectMetadata::open(project.path(), ProjectAccess::ReadOnly, 3).unwrap();
    let readonly = PortableMarkerStore::open(metadata.database_path().unwrap(), false).unwrap();
    let markers = readonly
        .markers_for_paths(std::slice::from_ref(&target.relative_path))
        .unwrap();

    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].marker.review_state, Some(ReviewState::Keep));
    assert!(markers[0].marker.favorite);
    drop(readonly);
    drop(metadata);
    let after = fs::read_dir(&viewer)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name(), fs::read(entry.path()).unwrap())
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(after, before);
    for suffix in ["-wal", "-shm", "-journal"] {
        assert!(!std::path::PathBuf::from(format!("{}{}", database.display(), suffix)).exists());
    }
    assert_eq!(schema_version(&database), 2);
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
    assert_eq!(schema_version(&database), 4);
    let v1_backup = viewer.join("metadata.sqlite.v1.bak");
    let v2_backup = viewer.join("metadata.sqlite.v2.bak");
    let v3_backup = viewer.join("metadata.sqlite.v3.bak");
    assert!(v1_backup.is_file());
    assert!(v2_backup.is_file());
    assert!(v3_backup.is_file());
    let v1_before = fs::read(&v1_backup).unwrap();
    let v2_before = fs::read(&v2_backup).unwrap();
    let v3_before = fs::read(&v3_backup).unwrap();
    drop(migrated);

    let reopened =
        PortableProjectMetadata::open(project.path(), ProjectAccess::ReadWrite, 43).unwrap();
    assert_eq!(reopened.project_id(), project_id);
    assert_eq!(fs::read(&v1_backup).unwrap(), v1_before);
    assert_eq!(fs::read(&v2_backup).unwrap(), v2_before);
    assert_eq!(fs::read(&v3_backup).unwrap(), v3_before);
    drop(reopened);
    OperationJournal::open(&database).expect("operation journal accepts schema v4");
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

fn marker_target(path: &str, kind: FileKind) -> MarkerTarget {
    MarkerTarget {
        entity_id: EntityId::new(),
        relative_path: RelativePath::parse(path).unwrap(),
        kind,
        size: if kind == FileKind::Directory { 0 } else { 128 },
        modified_ns: 9,
    }
}

fn writable_marker_store(project: &TempDir) -> PortableMarkerStore {
    let metadata =
        PortableProjectMetadata::open(project.path(), ProjectAccess::ReadWrite, 1).unwrap();
    PortableMarkerStore::open(metadata.database_path().unwrap(), true).unwrap()
}

#[test]
fn markers_keep_review_and_favorite_independent_for_files_and_folders() {
    let project = TempDir::new().unwrap();
    let store = writable_marker_store(&project);
    let file = marker_target("products/id-1/front.jpg", FileKind::Jpeg);
    let folder = marker_target("products/id-1", FileKind::Directory);

    let changes = store
        .apply_batch(
            &[file.clone(), folder.clone()],
            MarkerPatch {
                review: ReviewPatch::Set(ReviewState::Keep),
                favorite: FavoritePatch::Unchanged,
            },
            10,
        )
        .unwrap();
    assert!(changes.iter().all(|change| {
        change.marker
            == Marker {
                review_state: Some(ReviewState::Keep),
                favorite: false,
            }
    }));

    let changes = store
        .apply_batch(
            std::slice::from_ref(&file),
            MarkerPatch {
                review: ReviewPatch::Unchanged,
                favorite: FavoritePatch::Toggle,
            },
            11,
        )
        .unwrap();
    assert_eq!(
        changes[0].marker,
        Marker {
            review_state: Some(ReviewState::Keep),
            favorite: true,
        }
    );

    let changes = store
        .apply_batch(
            std::slice::from_ref(&file),
            MarkerPatch {
                review: ReviewPatch::Clear,
                favorite: FavoritePatch::Unchanged,
            },
            12,
        )
        .unwrap();
    assert_eq!(
        changes[0].marker,
        Marker {
            review_state: None,
            favorite: true,
        }
    );
    assert_eq!(
        store
            .markers_for_paths(&[file.relative_path.clone(), folder.relative_path.clone()])
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn markers_round_trip_appended_file_kinds() {
    let project = TempDir::new().unwrap();
    let store = writable_marker_store(&project);
    let targets = [
        marker_target("assets/source.psd", FileKind::UnsupportedImage),
        marker_target("assets/license.pdf", FileKind::Other),
        marker_target("assets/clip.mp4", FileKind::Video),
    ];
    let patch = MarkerPatch {
        review: ReviewPatch::Unchanged,
        favorite: FavoritePatch::Set(true),
    };

    store.apply_batch(&targets, patch, 42).unwrap();
    let restored = store
        .markers_for_paths(
            &targets
                .iter()
                .map(|target| target.relative_path.clone())
                .collect::<Vec<_>>(),
        )
        .unwrap();

    assert_eq!(
        restored
            .iter()
            .map(|marker| marker.kind)
            .collect::<Vec<_>>(),
        [FileKind::Video, FileKind::Other, FileKind::UnsupportedImage,]
    );
}

#[test]
fn clearing_both_marker_dimensions_removes_the_portable_row() {
    let project = TempDir::new().unwrap();
    let store = writable_marker_store(&project);
    let target = marker_target("notes/prompt.md", FileKind::Markdown);
    store
        .apply_batch(
            std::slice::from_ref(&target),
            MarkerPatch {
                review: ReviewPatch::Set(ReviewState::Pending),
                favorite: FavoritePatch::Set(true),
            },
            1,
        )
        .unwrap();

    let cleared = store
        .apply_batch(
            std::slice::from_ref(&target),
            MarkerPatch {
                review: ReviewPatch::Clear,
                favorite: FavoritePatch::Set(false),
            },
            2,
        )
        .unwrap();

    assert_eq!(cleared[0].marker, Marker::default());
    assert!(
        store
            .markers_for_paths(std::slice::from_ref(&target.relative_path))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn marker_batch_rolls_back_when_any_evidence_is_out_of_range() {
    let project = TempDir::new().unwrap();
    let store = writable_marker_store(&project);
    let valid = marker_target("products/valid.png", FileKind::Png);
    let mut invalid = marker_target("products/invalid.png", FileKind::Png);
    invalid.size = u64::MAX;

    assert!(
        store
            .apply_batch(
                &[valid.clone(), invalid],
                MarkerPatch {
                    review: ReviewPatch::Set(ReviewState::Reject),
                    favorite: FavoritePatch::Unchanged,
                },
                4,
            )
            .is_err()
    );
    assert!(
        store
            .markers_for_paths(std::slice::from_ref(&valid.relative_path))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn markers_survive_reopen_and_whole_project_copy_without_touching_source_files() {
    let source = TempDir::new().unwrap();
    let source_file = source.path().join("front.jpg");
    fs::write(&source_file, b"original image fixture").unwrap();
    let before = fs::metadata(&source_file).unwrap().modified().unwrap();
    let target = marker_target("front.jpg", FileKind::Jpeg);
    {
        let store = writable_marker_store(&source);
        store
            .apply_batch(
                std::slice::from_ref(&target),
                MarkerPatch {
                    review: ReviewPatch::Set(ReviewState::Keep),
                    favorite: FavoritePatch::Set(true),
                },
                9,
            )
            .unwrap();
    }
    assert_eq!(fs::read(&source_file).unwrap(), b"original image fixture");
    assert_eq!(
        fs::metadata(&source_file).unwrap().modified().unwrap(),
        before
    );

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
        PortableProjectMetadata::open(destination.path(), ProjectAccess::ReadWrite, 10).unwrap();
    let store = PortableMarkerStore::open(copied.database_path().unwrap(), true).unwrap();
    let recovered = store
        .markers_for_paths(std::slice::from_ref(&target.relative_path))
        .unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(
        recovered[0].marker,
        Marker {
            review_state: Some(ReviewState::Keep),
            favorite: true,
        }
    );
}

#[derive(Default)]
struct Projection {
    fail: bool,
    changes: std::sync::Mutex<Vec<MarkerChange>>,
}

impl MarkerProjectionPort for Projection {
    fn sync_markers(&self, changes: &[MarkerChange]) -> Result<(), MarkerProjectionError> {
        if self.fail {
            return Err(MarkerProjectionError::Unavailable);
        }
        self.changes.lock().unwrap().extend_from_slice(changes);
        Ok(())
    }
}

#[test]
fn marker_service_rejects_duplicates_and_reports_committed_projection_failure() {
    let project = TempDir::new().unwrap();
    let store = writable_marker_store(&project);
    let target = marker_target("id-1/front.jpg", FileKind::Jpeg);
    let projection = Projection::default();
    let service = MarkerService::new(&store, &projection, true);
    let patch = MarkerPatch {
        review: ReviewPatch::Set(ReviewState::Keep),
        favorite: FavoritePatch::Unchanged,
    };
    assert!(matches!(
        service.apply(&[target.clone(), target.clone()], patch, 1),
        Err(MarkerServiceError::DuplicateTarget)
    ));
    assert!(
        store
            .markers_for_paths(std::slice::from_ref(&target.relative_path))
            .unwrap()
            .is_empty()
    );

    let failing_projection = Projection {
        fail: true,
        ..Projection::default()
    };
    let service = MarkerService::new(&store, &failing_projection, true);
    assert!(matches!(
        service.apply(std::slice::from_ref(&target), patch, 2),
        Err(MarkerServiceError::CommittedButProjectionStale)
    ));
    assert_eq!(
        store
            .markers_for_paths(std::slice::from_ref(&target.relative_path))
            .unwrap()[0]
            .marker
            .review_state,
        Some(ReviewState::Keep)
    );
}

#[test]
fn read_only_marker_service_rejects_writes_without_touching_the_database() {
    let project = TempDir::new().unwrap();
    let writable = writable_marker_store(&project);
    drop(writable);
    let metadata =
        PortableProjectMetadata::open(project.path(), ProjectAccess::ReadOnly, 2).unwrap();
    let store = PortableMarkerStore::open(metadata.database_path().unwrap(), false).unwrap();
    let target = marker_target("id-1", FileKind::Directory);
    let projection = Projection::default();
    let service = MarkerService::new(&store, &projection, false);

    assert!(matches!(
        service.apply(
            std::slice::from_ref(&target),
            MarkerPatch {
                review: ReviewPatch::Set(ReviewState::Reject),
                favorite: FavoritePatch::Unchanged,
            },
            3,
        ),
        Err(MarkerServiceError::ReadOnly)
    ));
    assert!(
        store
            .markers_for_paths(std::slice::from_ref(&target.relative_path))
            .unwrap()
            .is_empty()
    );
}
