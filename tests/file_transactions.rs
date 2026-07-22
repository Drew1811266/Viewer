use async_trait::async_trait;
use std::{
    fs,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use viewer_application::{
    FileContentEvidence, FileMutationPort, FileOperationError, FileSnapshot, TrashPort,
    file_commands::FileCommandCancellation,
    metadata::MarkerTarget,
    undo::{UndoAction, UndoError, UndoStack},
    watcher::FileIdentity,
};
use viewer_domain::{
    EntityId, OperationId, RelativePath,
    file::{FileKind, Marker},
    operation::{ConflictPolicy, OperationItemPlan, OperationKind, OperationState},
};
use viewer_infrastructure::operation::{
    conflict::{ConflictError, ConflictExecutor, ConflictResult, ReplaceExecutor},
    copy::LocalFileMutation,
    executor::{CopyExecutor, CopyResumeResult},
    journal::OperationJournal,
    recovery::{RecoveryActionKind, RecoveryReport, RecoveryService},
    rename::{RenameExecutor, RenameItemStatus, RenameMapping, RenamePlanner},
};
use viewer_test_support::{
    FixedClock, faults::FailAfterState, operation_commits::InMemoryOperationCommitPort,
    project_fixture::ProjectFixture,
};

fn operation_commits() -> Arc<InMemoryOperationCommitPort> {
    Arc::new(InMemoryOperationCommitPort::default())
}

async fn delegate_registered_copy(
    delegate: &LocalFileMutation,
    source: &Path,
    temporary: &Path,
    cancellation: &FileCommandCancellation,
    expected_source: &FileSnapshot,
    source_parent: FileIdentity,
    temporary_parent: FileIdentity,
) -> Result<FileContentEvidence, FileOperationError> {
    delegate
        .create_and_copy_cancellable_verified(
            source,
            temporary,
            cancellation,
            expected_source,
            source_parent,
            temporary_parent,
        )
        .await
}

fn copy_item(
    batch_id: OperationId,
    source: RelativePath,
    destination: RelativePath,
) -> OperationItemPlan {
    OperationItemPlan {
        batch_id,
        operation_id: OperationId::new(),
        entity_id: EntityId::new(),
        kind: OperationKind::Copy,
        source,
        destination: Some(destination),
        conflict_policy: ConflictPolicy::Skip,
    }
}

fn prepared_copy(project: &ProjectFixture) -> (Arc<OperationJournal>, OperationItemPlan, Vec<u8>) {
    let contents = (0..=255).cycle().take(128 * 1024).collect::<Vec<_>>();
    let source = project.create_file("products/a.jpg", &contents);
    let destination_directory = project.create_directory("exports");
    let destination =
        RelativePath::parse(&format!("{}/a.jpg", destination_directory.as_str())).unwrap();
    let batch_id = OperationId::new();
    let item = copy_item(batch_id, source, destination);
    let journal = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
    journal
        .begin_batch(batch_id, OperationKind::Copy, 1, 1_000)
        .unwrap();
    journal.record_item(&item, 1_001).unwrap();
    (journal, item, contents)
}

#[tokio::test]
async fn verified_copy_writes_identical_bytes_and_completes_journal() {
    let project = ProjectFixture::new();
    let (journal, item, contents) = prepared_copy(&project);
    let executor = CopyExecutor::new(
        project.root(),
        Arc::clone(&journal),
        Arc::new(LocalFileMutation),
        Arc::new(FixedClock::new(2_000)),
        operation_commits(),
    )
    .unwrap();

    let result = executor.execute(&item).await.unwrap();

    assert_eq!(
        fs::read(project.root().join(item.source.as_str())).unwrap(),
        contents
    );
    assert_eq!(
        fs::read(
            project
                .root()
                .join(item.destination.as_ref().unwrap().as_str())
        )
        .unwrap(),
        contents
    );
    assert_eq!(result.len, contents.len() as u64);
    assert_eq!(result.hash, *blake3::hash(&contents).as_bytes());
    assert_eq!(
        journal.item(item.operation_id).unwrap().unwrap().state,
        OperationState::Completed
    );
}

struct FailingRenamePort {
    delegate: LocalFileMutation,
}

struct CancelledCopyPort {
    delegate: LocalFileMutation,
}

struct BoundRenameOnlyPort {
    delegate: LocalFileMutation,
    called: AtomicBool,
}

#[async_trait]
impl FileMutationPort for BoundRenameOnlyPort {
    async fn create_and_copy_cancellable_verified(
        &self,
        source: &Path,
        temporary: &Path,
        cancellation: &FileCommandCancellation,
        expected_source: &FileSnapshot,
        source_parent: FileIdentity,
        temporary_parent: FileIdentity,
    ) -> Result<FileContentEvidence, FileOperationError> {
        delegate_registered_copy(
            &self.delegate,
            source,
            temporary,
            cancellation,
            expected_source,
            source_parent,
            temporary_parent,
        )
        .await
    }

    async fn snapshot(&self, path: &Path) -> Result<FileSnapshot, FileOperationError> {
        self.delegate.snapshot(path).await
    }

    async fn copy_and_hash(
        &self,
        source: &Path,
        temporary: &Path,
    ) -> Result<(u64, [u8; 32]), FileOperationError> {
        self.delegate.copy_and_hash(source, temporary).await
    }

    async fn rename(&self, _source: &Path, _destination: &Path) -> Result<(), FileOperationError> {
        Err(FileOperationError::IdentityChanged)
    }

    async fn rename_verified(
        &self,
        source: &Path,
        destination: &Path,
        expected: &FileSnapshot,
        source_parent: viewer_application::watcher::FileIdentity,
        destination_parent: viewer_application::watcher::FileIdentity,
    ) -> Result<(), FileOperationError> {
        self.called.store(true, Ordering::Release);
        self.delegate
            .rename_verified(
                source,
                destination,
                expected,
                source_parent,
                destination_parent,
            )
            .await
    }

    async fn remove_registered_temporary(&self, path: &Path) -> Result<(), FileOperationError> {
        self.delegate.remove_registered_temporary(path).await
    }
}

#[async_trait]
impl FileMutationPort for CancelledCopyPort {
    async fn create_and_copy_cancellable_verified(
        &self,
        _source: &Path,
        _temporary: &Path,
        _cancellation: &FileCommandCancellation,
        _expected_source: &FileSnapshot,
        _source_parent: FileIdentity,
        _temporary_parent: FileIdentity,
    ) -> Result<FileContentEvidence, FileOperationError> {
        Err(FileOperationError::Cancelled)
    }

    async fn snapshot(&self, path: &Path) -> Result<FileSnapshot, FileOperationError> {
        self.delegate.snapshot(path).await
    }

    async fn copy_and_hash(
        &self,
        _source: &Path,
        _temporary: &Path,
    ) -> Result<(u64, [u8; 32]), FileOperationError> {
        Err(FileOperationError::Cancelled)
    }

    async fn rename(&self, _source: &Path, _destination: &Path) -> Result<(), FileOperationError> {
        unreachable!("a cancelled copy must never reach final rename")
    }

    async fn remove_registered_temporary(&self, path: &Path) -> Result<(), FileOperationError> {
        self.delegate.remove_registered_temporary(path).await
    }
}

#[async_trait]
impl FileMutationPort for FailingRenamePort {
    async fn create_and_copy_cancellable_verified(
        &self,
        source: &Path,
        temporary: &Path,
        cancellation: &FileCommandCancellation,
        expected_source: &FileSnapshot,
        source_parent: FileIdentity,
        temporary_parent: FileIdentity,
    ) -> Result<FileContentEvidence, FileOperationError> {
        delegate_registered_copy(
            &self.delegate,
            source,
            temporary,
            cancellation,
            expected_source,
            source_parent,
            temporary_parent,
        )
        .await
    }

    async fn snapshot(&self, path: &Path) -> Result<FileSnapshot, FileOperationError> {
        self.delegate.snapshot(path).await
    }

    async fn copy_and_hash(
        &self,
        source: &Path,
        temporary: &Path,
    ) -> Result<(u64, [u8; 32]), FileOperationError> {
        self.delegate.copy_and_hash(source, temporary).await
    }

    async fn rename(&self, _source: &Path, destination: &Path) -> Result<(), FileOperationError> {
        Err(FileOperationError::Io {
            action: "injected final rename",
            path: destination.to_path_buf(),
            message: "injected failure".into(),
        })
    }

    async fn remove_registered_temporary(&self, path: &Path) -> Result<(), FileOperationError> {
        self.delegate.remove_registered_temporary(path).await
    }
}

#[tokio::test]
async fn verified_copy_failure_before_final_rename_leaves_only_registered_temporary() {
    let project = ProjectFixture::new();
    let (journal, item, contents) = prepared_copy(&project);
    let executor = CopyExecutor::new(
        project.root(),
        Arc::clone(&journal),
        Arc::new(FailingRenamePort {
            delegate: LocalFileMutation,
        }),
        Arc::new(FixedClock::new(2_000)),
        operation_commits(),
    )
    .unwrap();

    assert!(executor.execute(&item).await.is_err());

    let persisted = journal.item(item.operation_id).unwrap().unwrap();
    assert_eq!(persisted.state, OperationState::Verified);
    let temporary = persisted.temporary.expect("registered temporary path");
    assert!(project.root().join(temporary.as_str()).is_file());
    assert!(
        !project
            .root()
            .join(item.destination.as_ref().unwrap().as_str())
            .exists()
    );
    assert_eq!(
        fs::read(project.root().join(item.source.as_str())).unwrap(),
        contents
    );
}

#[tokio::test]
async fn verified_copy_retry_after_reopen_finishes_registered_temporary() {
    let project = ProjectFixture::new();
    let (journal, item, contents) = prepared_copy(&project);
    let executor = CopyExecutor::new(
        project.root(),
        Arc::clone(&journal),
        Arc::new(FailingRenamePort {
            delegate: LocalFileMutation,
        }),
        Arc::new(FixedClock::new(2_000)),
        operation_commits(),
    )
    .unwrap();
    assert!(executor.execute(&item).await.is_err());
    drop(executor);
    drop(journal);

    let reopened = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
    let retry = CopyExecutor::new(
        project.root(),
        Arc::clone(&reopened),
        Arc::new(LocalFileMutation),
        Arc::new(FixedClock::new(3_000)),
        operation_commits(),
    )
    .unwrap();
    let outcome = retry.resume(item.operation_id).await.unwrap();

    assert!(matches!(outcome, CopyResumeResult::Completed(_)));
    assert_eq!(
        fs::read(
            project
                .root()
                .join(item.destination.as_ref().unwrap().as_str())
        )
        .unwrap(),
        contents
    );
    assert_eq!(
        reopened.item(item.operation_id).unwrap().unwrap().state,
        OperationState::Completed
    );
}

#[tokio::test]
async fn verified_copy_recovery_places_only_through_the_bound_rename_primitive() {
    let project = ProjectFixture::new();
    let (journal, item, contents) = prepared_copy(&project);
    let executor = CopyExecutor::new(
        project.root(),
        Arc::clone(&journal),
        Arc::new(FailingRenamePort {
            delegate: LocalFileMutation,
        }),
        Arc::new(FixedClock::new(2_000)),
        operation_commits(),
    )
    .unwrap();
    assert!(executor.execute(&item).await.is_err());
    drop(executor);

    let mutation = Arc::new(BoundRenameOnlyPort {
        delegate: LocalFileMutation,
        called: AtomicBool::new(false),
    });
    let retry = CopyExecutor::new(
        project.root(),
        Arc::clone(&journal),
        mutation.clone(),
        Arc::new(FixedClock::new(3_000)),
        operation_commits(),
    )
    .unwrap();

    let outcome = retry.resume(item.operation_id).await.unwrap();

    assert!(matches!(outcome, CopyResumeResult::Completed(_)));
    assert!(mutation.called.load(Ordering::Acquire));
    assert_eq!(
        fs::read(
            project
                .root()
                .join(item.destination.as_ref().unwrap().as_str())
        )
        .unwrap(),
        contents
    );
}

#[tokio::test]
async fn recovery_does_not_delete_an_unknown_registered_copy_temporary_occupant() {
    let project = ProjectFixture::new();
    let (journal, item, _) = prepared_copy(&project);
    let destination = Path::new(item.destination.as_ref().unwrap().as_str());
    let temporary = RelativePath::parse(
        destination
            .parent()
            .unwrap()
            .join(format!(".viewer-copy-{}.part", item.operation_id))
            .to_str()
            .unwrap(),
    )
    .unwrap();
    journal
        .register_temporary(
            item.operation_id,
            OperationState::Prepared,
            &temporary,
            1_002,
        )
        .unwrap();
    let unknown = project.root().join(temporary.as_str());
    fs::write(&unknown, b"unknown occupant").unwrap();
    let recovery = RecoveryService::new(
        project.root(),
        Arc::clone(&journal),
        Arc::new(LocalFileMutation),
        durable_trash(&project),
        Arc::new(FixedClock::new(9_000)),
        operation_commits(),
    )
    .unwrap();

    let report = recovery.recover_project().await.unwrap();

    assert!(report.actions.is_empty());
    assert_eq!(report.needs_user_review.len(), 1);
    assert_eq!(fs::read(&unknown).unwrap(), b"unknown occupant");
    assert_eq!(
        journal.item(item.operation_id).unwrap().unwrap().state,
        OperationState::Prepared
    );
}

#[tokio::test]
async fn cross_volume_recovery_does_not_delete_an_unbound_temporary_occupant() {
    let project = ProjectFixture::new();
    let source = project.create_file("source.jpg", b"source identity");
    let destination = RelativePath::parse("exports/moved.jpg").unwrap();
    fs::create_dir_all(project.root().join("exports")).unwrap();
    let batch_id = OperationId::new();
    let operation_id = OperationId::new();
    let item = OperationItemPlan {
        batch_id,
        operation_id,
        entity_id: EntityId::new(),
        kind: OperationKind::Move,
        source,
        destination: Some(destination),
        conflict_policy: ConflictPolicy::Skip,
    };
    let temporary =
        RelativePath::parse(&format!("exports/.viewer-copy-{operation_id}.part")).unwrap();
    let journal = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
    journal
        .begin_batch(batch_id, OperationKind::Move, 1, 1_000)
        .unwrap();
    journal.record_item(&item, 1_001).unwrap();
    journal
        .register_temporary(operation_id, OperationState::Prepared, &temporary, 1_002)
        .unwrap();
    let unknown = project.root().join(temporary.as_str());
    fs::write(&unknown, b"unknown occupant").unwrap();
    let recovery = RecoveryService::new(
        project.root(),
        Arc::clone(&journal),
        Arc::new(LocalFileMutation),
        durable_trash(&project),
        Arc::new(FixedClock::new(9_000)),
        operation_commits(),
    )
    .unwrap();

    let report = recovery.recover_project().await.unwrap();

    assert!(report.actions.is_empty());
    assert_eq!(report.needs_user_review.len(), 1);
    assert_eq!(fs::read(&unknown).unwrap(), b"unknown occupant");
    assert_eq!(
        journal.item(operation_id).unwrap().unwrap().state,
        OperationState::Prepared
    );
}

#[tokio::test]
async fn verified_copy_cancellation_removes_only_registered_temporary() {
    let project = ProjectFixture::new();
    let (journal, item, contents) = prepared_copy(&project);
    let executor = CopyExecutor::new(
        project.root(),
        Arc::clone(&journal),
        Arc::new(CancelledCopyPort {
            delegate: LocalFileMutation,
        }),
        Arc::new(FixedClock::new(2_000)),
        operation_commits(),
    )
    .unwrap();

    assert!(matches!(
        executor.execute(&item).await,
        Err(viewer_infrastructure::operation::executor::CopyError::File(
            FileOperationError::Cancelled
        ))
    ));

    let persisted = journal.item(item.operation_id).unwrap().unwrap();
    assert_eq!(persisted.state, OperationState::Failed);
    assert_eq!(persisted.error_code.as_deref(), Some("cancelled"));
    assert_eq!(persisted.temporary, None);
    let temporary = format!("exports/.viewer-copy-{}.part", item.operation_id);
    assert!(!project.root().join(temporary).exists());
    assert_eq!(
        fs::read(project.root().join(item.source.as_str())).unwrap(),
        contents
    );
}

#[cfg(unix)]
#[tokio::test]
async fn verified_copy_rejects_source_and_destination_symlink_escapes() {
    use std::os::unix::fs::symlink;

    let project = ProjectFixture::new();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("outside.jpg"), b"outside").unwrap();
    symlink(
        outside.path().join("outside.jpg"),
        project.root().join("escaped-source.jpg"),
    )
    .unwrap();
    symlink(outside.path(), project.root().join("escaped-destination")).unwrap();

    for (source, destination) in [
        ("escaped-source.jpg", "inside.jpg"),
        ("inside-source.jpg", "escaped-destination/output.jpg"),
    ] {
        if source == "inside-source.jpg" {
            fs::write(project.root().join(source), b"inside").unwrap();
        }
        let batch_id = OperationId::new();
        let item = copy_item(
            batch_id,
            RelativePath::parse(source).unwrap(),
            RelativePath::parse(destination).unwrap(),
        );
        let journal = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
        journal
            .begin_batch(batch_id, OperationKind::Copy, 1, 1_000)
            .unwrap();
        journal.record_item(&item, 1_001).unwrap();
        let executor = CopyExecutor::new(
            project.root(),
            journal,
            Arc::new(LocalFileMutation),
            Arc::new(FixedClock::new(2_000)),
            operation_commits(),
        )
        .unwrap();

        assert!(matches!(
            executor.execute(&item).await,
            Err(viewer_infrastructure::operation::executor::CopyError::File(
                FileOperationError::OutsideProject
            ))
        ));
    }
}

fn prepare_rename_items(
    journal: &OperationJournal,
    batch_id: OperationId,
    mappings: &[RenameMapping],
    kind: OperationKind,
) {
    journal
        .begin_batch(batch_id, kind, mappings.len() as u32, 4_000)
        .unwrap();
    for mapping in mappings {
        journal
            .record_item(
                &OperationItemPlan {
                    batch_id,
                    operation_id: mapping.operation_id,
                    entity_id: mapping.entity_id,
                    kind,
                    source: RelativePath::parse(mapping.source.to_str().unwrap()).unwrap(),
                    destination: Some(
                        RelativePath::parse(mapping.destination.to_str().unwrap()).unwrap(),
                    ),
                    conflict_policy: ConflictPolicy::Skip,
                },
                4_001,
            )
            .unwrap();
    }
}

fn rename_mapping(source: &str, destination: &str) -> RenameMapping {
    RenameMapping {
        operation_id: OperationId::new(),
        entity_id: EntityId::new(),
        source: source.into(),
        destination: destination.into(),
    }
}

#[tokio::test]
async fn rename_cycle_swaps_files_without_data_loss() {
    let project = ProjectFixture::new();
    project.create_file("A.jpg", b"contents-A");
    project.create_file("B.jpg", b"contents-B");
    let mappings = [
        rename_mapping("A.jpg", "B.jpg"),
        rename_mapping("B.jpg", "A.jpg"),
    ];
    let plan = RenamePlanner::plan(project.root(), true, &mappings).unwrap();
    let journal = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
    prepare_rename_items(
        &journal,
        OperationId::new(),
        &mappings,
        OperationKind::Rename,
    );
    let executor = RenameExecutor::new(
        project.root(),
        Arc::clone(&journal),
        Arc::new(LocalFileMutation),
        Arc::new(FixedClock::new(5_000)),
        operation_commits(),
    )
    .unwrap();

    let result = executor.execute(&plan).await;

    assert!(
        result
            .items
            .iter()
            .all(|item| item.status == RenameItemStatus::Completed)
    );
    assert_eq!(
        fs::read(project.root().join("A.jpg")).unwrap(),
        b"contents-B"
    );
    assert_eq!(
        fs::read(project.root().join("B.jpg")).unwrap(),
        b"contents-A"
    );
    for mapping in mappings {
        assert_eq!(
            journal.item(mapping.operation_id).unwrap().unwrap().state,
            OperationState::Completed
        );
    }
}

#[tokio::test]
async fn rename_case_only_uses_temporary_stage() {
    let project = ProjectFixture::new();
    project.create_file("A.jpg", b"case-only");
    let mappings = [rename_mapping("A.jpg", "a.jpg")];
    let plan = RenamePlanner::plan(project.root(), false, &mappings).unwrap();
    let journal = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
    prepare_rename_items(
        &journal,
        OperationId::new(),
        &mappings,
        OperationKind::Rename,
    );
    let executor = RenameExecutor::new(
        project.root(),
        journal,
        Arc::new(LocalFileMutation),
        Arc::new(FixedClock::new(5_000)),
        operation_commits(),
    )
    .unwrap();

    let result = executor.execute(&plan).await;

    assert_eq!(result.items[0].status, RenameItemStatus::Completed);
    assert_eq!(
        fs::read(project.root().join("a.jpg")).unwrap(),
        b"case-only"
    );
}

#[tokio::test]
async fn rename_in_project_move_preserves_file_identity() {
    let project = ProjectFixture::new();
    project.create_file("source/A.jpg", b"move-me");
    project.create_directory("destination");
    let mappings = [rename_mapping("source/A.jpg", "destination/A.jpg")];
    let plan = RenamePlanner::plan(project.root(), true, &mappings).unwrap();
    let journal = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
    prepare_rename_items(&journal, OperationId::new(), &mappings, OperationKind::Move);
    let mutation = Arc::new(LocalFileMutation);
    let before = mutation
        .snapshot(&project.root().join("source/A.jpg"))
        .await
        .unwrap();
    let executor = RenameExecutor::new(
        project.root(),
        journal,
        mutation.clone(),
        Arc::new(FixedClock::new(5_000)),
        operation_commits(),
    )
    .unwrap();

    let result = executor.execute(&plan).await;
    let after = mutation
        .snapshot(&project.root().join("destination/A.jpg"))
        .await
        .unwrap();

    assert_eq!(result.items[0].status, RenameItemStatus::Completed);
    assert_eq!(before.volume_id, after.volume_id);
    assert_eq!(before.file_id, after.file_id);
    assert_eq!(before.len, after.len);
    assert_eq!(before.modified_ns, after.modified_ns);
    assert!(!project.root().join("source/A.jpg").exists());
}

struct FailNamedRenamePort {
    delegate: LocalFileMutation,
    fail_source_name: &'static str,
}

#[async_trait]
impl FileMutationPort for FailNamedRenamePort {
    async fn create_and_copy_cancellable_verified(
        &self,
        source: &Path,
        temporary: &Path,
        cancellation: &FileCommandCancellation,
        expected_source: &FileSnapshot,
        source_parent: FileIdentity,
        temporary_parent: FileIdentity,
    ) -> Result<FileContentEvidence, FileOperationError> {
        delegate_registered_copy(
            &self.delegate,
            source,
            temporary,
            cancellation,
            expected_source,
            source_parent,
            temporary_parent,
        )
        .await
    }

    async fn snapshot(&self, path: &Path) -> Result<FileSnapshot, FileOperationError> {
        self.delegate.snapshot(path).await
    }

    async fn copy_and_hash(
        &self,
        source: &Path,
        temporary: &Path,
    ) -> Result<(u64, [u8; 32]), FileOperationError> {
        self.delegate.copy_and_hash(source, temporary).await
    }

    async fn rename(&self, source: &Path, destination: &Path) -> Result<(), FileOperationError> {
        if source.file_name().and_then(|value| value.to_str()) == Some(self.fail_source_name) {
            return Err(FileOperationError::Io {
                action: "injected batch rename",
                path: source.to_path_buf(),
                message: "injected partial failure".into(),
            });
        }
        self.delegate.rename(source, destination).await
    }

    async fn remove_registered_temporary(&self, path: &Path) -> Result<(), FileOperationError> {
        self.delegate.remove_registered_temporary(path).await
    }
}

#[tokio::test]
async fn rename_partial_failure_keeps_completed_items() {
    let project = ProjectFixture::new();
    project.create_file("A.jpg", b"A");
    project.create_file("B.jpg", b"B");
    let mappings = [
        rename_mapping("A.jpg", "C.jpg"),
        rename_mapping("B.jpg", "D.jpg"),
    ];
    let plan = RenamePlanner::plan(project.root(), true, &mappings).unwrap();
    let journal = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
    prepare_rename_items(
        &journal,
        OperationId::new(),
        &mappings,
        OperationKind::Rename,
    );
    let executor = RenameExecutor::new(
        project.root(),
        Arc::clone(&journal),
        Arc::new(FailNamedRenamePort {
            delegate: LocalFileMutation,
            fail_source_name: "B.jpg",
        }),
        Arc::new(FixedClock::new(5_000)),
        operation_commits(),
    )
    .unwrap();

    let result = executor.execute(&plan).await;

    assert_eq!(result.items[0].status, RenameItemStatus::Completed);
    assert!(matches!(
        result.items[1].status,
        RenameItemStatus::Failed(_)
    ));
    assert_eq!(fs::read(project.root().join("C.jpg")).unwrap(), b"A");
    assert_eq!(fs::read(project.root().join("B.jpg")).unwrap(), b"B");
    assert!(!project.root().join("D.jpg").exists());
    assert_eq!(
        journal
            .item(mappings[0].operation_id)
            .unwrap()
            .unwrap()
            .state,
        OperationState::Completed
    );
    assert_eq!(
        journal
            .item(mappings[1].operation_id)
            .unwrap()
            .unwrap()
            .state,
        OperationState::Prepared,
        "failed source isolation leaves the journal before its staged boundary"
    );
}

#[derive(Default)]
struct FakeTrashPort {
    events: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl TrashPort for FakeTrashPort {
    async fn trash(&self, path: &Path) -> Result<(), FileOperationError> {
        self.events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(format!(
                "trash:{}",
                path.file_name().unwrap().to_string_lossy()
            ));
        fs::remove_file(path).map_err(|error| FileOperationError::io("fake trash", path, &error))
    }
}

struct RecordingMutationPort {
    delegate: LocalFileMutation,
    events: Arc<Mutex<Vec<String>>>,
}

struct FailingPlacementPort {
    delegate: LocalFileMutation,
    events: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl FileMutationPort for FailingPlacementPort {
    async fn create_and_copy_cancellable_verified(
        &self,
        source: &Path,
        temporary: &Path,
        cancellation: &FileCommandCancellation,
        expected_source: &FileSnapshot,
        source_parent: FileIdentity,
        temporary_parent: FileIdentity,
    ) -> Result<FileContentEvidence, FileOperationError> {
        delegate_registered_copy(
            &self.delegate,
            source,
            temporary,
            cancellation,
            expected_source,
            source_parent,
            temporary_parent,
        )
        .await
    }

    async fn snapshot(&self, path: &Path) -> Result<FileSnapshot, FileOperationError> {
        self.delegate.snapshot(path).await
    }

    async fn copy_and_hash(
        &self,
        source: &Path,
        temporary: &Path,
    ) -> Result<(u64, [u8; 32]), FileOperationError> {
        self.delegate.copy_and_hash(source, temporary).await
    }

    async fn rename(&self, source: &Path, destination: &Path) -> Result<(), FileOperationError> {
        self.events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(format!(
                "rename:{}->{}",
                source.file_name().unwrap().to_string_lossy(),
                destination.file_name().unwrap().to_string_lossy()
            ));
        Err(FileOperationError::Io {
            action: "injected replacement placement",
            path: destination.to_path_buf(),
            message: "injected failure".into(),
        })
    }

    async fn remove_registered_temporary(&self, path: &Path) -> Result<(), FileOperationError> {
        self.delegate.remove_registered_temporary(path).await
    }
}

#[async_trait]
impl FileMutationPort for RecordingMutationPort {
    async fn create_and_copy_cancellable_verified(
        &self,
        source: &Path,
        temporary: &Path,
        cancellation: &FileCommandCancellation,
        expected_source: &FileSnapshot,
        source_parent: FileIdentity,
        temporary_parent: FileIdentity,
    ) -> Result<FileContentEvidence, FileOperationError> {
        delegate_registered_copy(
            &self.delegate,
            source,
            temporary,
            cancellation,
            expected_source,
            source_parent,
            temporary_parent,
        )
        .await
    }

    async fn snapshot(&self, path: &Path) -> Result<FileSnapshot, FileOperationError> {
        self.delegate.snapshot(path).await
    }

    async fn copy_and_hash(
        &self,
        source: &Path,
        temporary: &Path,
    ) -> Result<(u64, [u8; 32]), FileOperationError> {
        self.delegate.copy_and_hash(source, temporary).await
    }

    async fn rename(&self, source: &Path, destination: &Path) -> Result<(), FileOperationError> {
        self.events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(format!(
                "rename:{}->{}",
                source.file_name().unwrap().to_string_lossy(),
                destination.file_name().unwrap().to_string_lossy()
            ));
        self.delegate.rename(source, destination).await
    }

    async fn remove_registered_temporary(&self, path: &Path) -> Result<(), FileOperationError> {
        self.delegate.remove_registered_temporary(path).await
    }
}

#[tokio::test]
async fn conflict_skip_keep_both_and_replace_are_deterministic() {
    let project = ProjectFixture::new();
    let events = Arc::new(Mutex::new(Vec::new()));
    let mutation: Arc<dyn FileMutationPort> = Arc::new(RecordingMutationPort {
        delegate: LocalFileMutation,
        events: Arc::clone(&events),
    });
    let trash: Arc<dyn TrashPort> = Arc::new(FakeTrashPort {
        events: Arc::clone(&events),
    });
    let executor =
        ConflictExecutor::new(project.root(), Arc::clone(&mutation), Arc::clone(&trash)).unwrap();

    let skipped_source = project.root().join("skip.part");
    let skipped_destination = project.root().join("skip.jpg");
    fs::write(&skipped_source, b"new").unwrap();
    fs::write(&skipped_destination, b"existing").unwrap();
    assert_eq!(
        executor
            .place(&skipped_source, &skipped_destination, ConflictPolicy::Skip,)
            .await
            .unwrap(),
        ConflictResult::Skipped
    );
    assert!(
        events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_empty()
    );

    let keep_source = project.root().join("keep.part");
    let keep_destination = project.root().join("name.jpg");
    fs::write(&keep_source, b"new").unwrap();
    fs::write(&keep_destination, b"existing").unwrap();
    fs::write(project.root().join("name copy.jpg"), b"existing-copy").unwrap();
    assert_eq!(
        executor
            .place(&keep_source, &keep_destination, ConflictPolicy::KeepBoth)
            .await
            .unwrap(),
        ConflictResult::Placed(
            fs::canonicalize(project.root())
                .unwrap()
                .join("name copy 2.jpg")
        )
    );

    events
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
    let replace_source = project.root().join("replace.part");
    let replace_destination = project.root().join("replace.jpg");
    fs::write(&replace_source, b"replacement").unwrap();
    fs::write(&replace_destination, b"previous").unwrap();
    executor
        .place(
            &replace_source,
            &replace_destination,
            ConflictPolicy::Replace,
        )
        .await
        .unwrap();
    assert_eq!(
        *events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
        ["trash:replace.jpg", "rename:replace.part->replace.jpg"]
    );
    assert_eq!(fs::read(replace_destination).unwrap(), b"replacement");
}

#[tokio::test]
async fn replace_placement_failure_reports_that_previous_destination_is_in_trash() {
    let project = ProjectFixture::new();
    let events = Arc::new(Mutex::new(Vec::new()));
    let mutation: Arc<dyn FileMutationPort> = Arc::new(FailingPlacementPort {
        delegate: LocalFileMutation,
        events: Arc::clone(&events),
    });
    let trash: Arc<dyn TrashPort> = Arc::new(FakeTrashPort {
        events: Arc::clone(&events),
    });
    let executor = ConflictExecutor::new(project.root(), mutation, trash).unwrap();
    let source = project.root().join("replacement.part");
    let destination = project.root().join("replacement.jpg");
    fs::write(&source, b"replacement").unwrap();
    fs::write(&destination, b"previous").unwrap();

    let error = executor
        .place(&source, &destination, ConflictPolicy::Replace)
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        ConflictError::PlacementAfterTrash {
            source: failed_source,
            destination: failed_destination,
            ..
        } if failed_source == fs::canonicalize(&source).unwrap()
            && failed_destination == fs::canonicalize(project.root()).unwrap().join("replacement.jpg")
    ));
    assert_eq!(
        *events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
        [
            "trash:replacement.jpg",
            "rename:replacement.part->replacement.jpg"
        ]
    );
    assert!(source.is_file());
    assert!(!destination.exists());
}

#[tokio::test]
async fn conflict_executor_rejects_a_source_outside_the_project_before_mutation() {
    let project = ProjectFixture::new();
    let outside = tempfile::tempdir().unwrap();
    let source = outside.path().join("outside.part");
    fs::write(&source, b"outside").unwrap();
    let destination = project.root().join("inside.jpg");
    let events = Arc::new(Mutex::new(Vec::new()));
    let mutation: Arc<dyn FileMutationPort> = Arc::new(RecordingMutationPort {
        delegate: LocalFileMutation,
        events: Arc::clone(&events),
    });
    let trash: Arc<dyn TrashPort> = Arc::new(FakeTrashPort {
        events: Arc::clone(&events),
    });
    let executor = ConflictExecutor::new(project.root(), mutation, trash).unwrap();

    assert!(matches!(
        executor
            .place(&source, &destination, ConflictPolicy::Replace)
            .await,
        Err(ConflictError::File(FileOperationError::OutsideProject))
    ));
    assert!(source.is_file());
    assert!(!destination.exists());
    assert!(events.lock().unwrap().is_empty());

    let inside_source = project.root().join("inside.part");
    let outside_destination = outside.path().join("outside.jpg");
    fs::write(&inside_source, b"inside").unwrap();
    fs::write(&outside_destination, b"outside destination").unwrap();
    assert!(matches!(
        executor
            .place(
                &inside_source,
                &outside_destination,
                ConflictPolicy::Replace,
            )
            .await,
        Err(ConflictError::File(FileOperationError::OutsideProject))
    ));
    assert!(inside_source.is_file());
    assert_eq!(
        fs::read(outside_destination).unwrap(),
        b"outside destination"
    );
    assert!(events.lock().unwrap().is_empty());
}

#[tokio::test]
async fn undo_stack_accepts_only_session_undoable_batches_and_revalidates_identity() {
    let project = ProjectFixture::new();
    project.create_file("renamed.jpg", b"rename");
    let mutation = LocalFileMutation;
    let expected = mutation
        .snapshot(&project.root().join("renamed.jpg"))
        .await
        .unwrap();
    let session_id = viewer_domain::SessionId::new();
    let mut stack = UndoStack::new(session_id);
    let action = UndoAction::File {
        entity_id: EntityId::new(),
        current: RelativePath::parse("renamed.jpg").unwrap(),
        restore: RelativePath::parse("original.jpg").unwrap(),
        expected,
    };

    assert!(!stack.record_batch(
        OperationId::new(),
        OperationKind::Copy,
        vec![action.clone()]
    ));
    assert!(!stack.record_batch(
        OperationId::new(),
        OperationKind::Trash,
        vec![action.clone()]
    ));
    assert!(stack.record_batch(OperationId::new(), OperationKind::Rename, vec![action]));
    assert_eq!(stack.len(), 1);
    let undo = stack
        .pop_validated(project.root(), &mutation)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(undo.kind, OperationKind::Rename);

    let changed_snapshot = mutation
        .snapshot(&project.root().join("renamed.jpg"))
        .await
        .unwrap();
    assert!(stack.record_batch(
        OperationId::new(),
        OperationKind::Move,
        vec![UndoAction::File {
            entity_id: EntityId::new(),
            current: RelativePath::parse("renamed.jpg").unwrap(),
            restore: RelativePath::parse("elsewhere.jpg").unwrap(),
            expected: changed_snapshot,
        }],
    ));
    fs::remove_file(project.root().join("renamed.jpg")).unwrap();
    fs::write(project.root().join("renamed.jpg"), b"different identity").unwrap();
    assert!(
        stack
            .pop_validated(project.root(), &mutation)
            .await
            .is_err()
    );
    assert_eq!(stack.len(), 1);
    stack.close_session(session_id);
    assert!(stack.is_empty());

    assert!(stack.record_batch(
        OperationId::new(),
        OperationKind::SetReviewState,
        vec![UndoAction::ReviewState {
            target: MarkerTarget {
                entity_id: EntityId::new(),
                relative_path: RelativePath::parse("review.jpg").unwrap(),
                kind: FileKind::Jpeg,
                size: 1,
                modified_ns: 1,
            },
            previous: Marker::default(),
            expected: Marker::default(),
        }],
    ));
    assert!(stack.record_batch(
        OperationId::new(),
        OperationKind::SetFavorite,
        vec![UndoAction::Favorite {
            target: MarkerTarget {
                entity_id: EntityId::new(),
                relative_path: RelativePath::parse("favorite.jpg").unwrap(),
                kind: FileKind::Jpeg,
                size: 1,
                modified_ns: 1,
            },
            previous: Marker::default(),
            expected: Marker::default(),
        }],
    ));
    stack.close_session(viewer_domain::SessionId::new());
    assert_eq!(stack.len(), 2);
    stack.close_session(session_id);
    assert!(stack.is_empty());
}

#[tokio::test]
async fn undo_refuses_an_occupied_or_escaped_restore_destination() {
    let project = ProjectFixture::new();
    project.create_file("current.jpg", b"current");
    project.create_file("occupied.jpg", b"occupied");
    let mutation = LocalFileMutation;
    let expected = mutation
        .snapshot(&project.root().join("current.jpg"))
        .await
        .unwrap();
    let session_id = viewer_domain::SessionId::new();
    let mut stack = UndoStack::new(session_id);
    assert!(stack.record_batch(
        OperationId::new(),
        OperationKind::Rename,
        vec![UndoAction::File {
            entity_id: EntityId::new(),
            current: RelativePath::parse("current.jpg").unwrap(),
            restore: RelativePath::parse("occupied.jpg").unwrap(),
            expected: expected.clone(),
        }],
    ));
    assert!(matches!(
        stack.pop_validated(project.root(), &mutation).await,
        Err(UndoError::DestinationOccupied)
    ));
    assert_eq!(stack.len(), 1);

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        stack.close_session(session_id);
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), project.root().join("escaped-parent")).unwrap();
        assert!(stack.record_batch(
            OperationId::new(),
            OperationKind::Move,
            vec![UndoAction::File {
                entity_id: EntityId::new(),
                current: RelativePath::parse("current.jpg").unwrap(),
                restore: RelativePath::parse("escaped-parent/restored.jpg").unwrap(),
                expected,
            }],
        ));
        assert!(matches!(
            stack.pop_validated(project.root(), &mutation).await,
            Err(UndoError::OutsideProject)
        ));
        assert_eq!(stack.len(), 1);
    }
}

const RECOVERY_STATES: [OperationState; 7] = [
    OperationState::Prepared,
    OperationState::Staged,
    OperationState::FsApplied,
    OperationState::Verified,
    OperationState::MetaCommitted,
    OperationState::IndexSynced,
    OperationState::Completed,
];

struct DurableFakeTrashPort {
    directory: std::path::PathBuf,
}

#[async_trait]
impl TrashPort for DurableFakeTrashPort {
    async fn trash(&self, path: &Path) -> Result<(), FileOperationError> {
        fs::create_dir_all(&self.directory).map_err(|error| {
            FileOperationError::io("create fake Trash", &self.directory, &error)
        })?;
        let destination = self
            .directory
            .join(path.file_name().ok_or(FileOperationError::OutsideProject)?);
        if destination.exists() {
            return Err(FileOperationError::DestinationExists);
        }
        fs::rename(path, &destination)
            .map_err(|error| FileOperationError::io("move to fake Trash", path, &error))
    }
}

fn durable_trash(project: &ProjectFixture) -> Arc<dyn TrashPort> {
    Arc::new(DurableFakeTrashPort {
        directory: project.root().join(".test-trash"),
    })
}

async fn recover_twice(
    project: &ProjectFixture,
    journal: Arc<OperationJournal>,
    trash: Arc<dyn TrashPort>,
) -> (RecoveryReport, RecoveryReport) {
    let recovery = RecoveryService::new(
        project.root(),
        journal,
        Arc::new(LocalFileMutation),
        trash,
        Arc::new(FixedClock::new(9_000)),
        operation_commits(),
    )
    .unwrap();
    let first = recovery.recover_project().await.unwrap();
    let second = recovery.recover_project().await.unwrap();
    assert!(
        second.actions.is_empty(),
        "recovery must be idempotent on the second pass"
    );
    (first, second)
}

async fn copy_recovery_case(fail_after: OperationState) {
    let project = ProjectFixture::new();
    let (journal, item, contents) = prepared_copy(&project);
    let executor = CopyExecutor::with_faults(
        project.root(),
        Arc::clone(&journal),
        Arc::new(LocalFileMutation),
        Arc::new(FixedClock::new(2_000)),
        operation_commits(),
        Arc::new(FailAfterState(fail_after)),
    )
    .unwrap();
    let error = executor.execute(&item).await.unwrap_err();
    assert_eq!(error.injected_state(), Some(fail_after));
    drop(executor);
    drop(journal);

    let reopened = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
    let (first_recovery, second_recovery) =
        recover_twice(&project, Arc::clone(&reopened), durable_trash(&project)).await;

    assert_eq!(
        fs::read(project.root().join(item.source.as_str())).unwrap(),
        contents
    );
    let destination = project
        .root()
        .join(item.destination.as_ref().unwrap().as_str());
    if destination.exists() {
        assert_eq!(fs::read(destination).unwrap(), contents);
    }
    let persisted = reopened.item(item.operation_id).unwrap().unwrap();
    if matches!(
        fail_after,
        OperationState::Prepared | OperationState::Staged
    ) {
        assert!(first_recovery.actions.is_empty());
        assert_eq!(first_recovery.needs_user_review.len(), 1);
        assert!(second_recovery.actions.is_empty());
        assert_eq!(second_recovery.needs_user_review.len(), 1);
        assert_eq!(persisted.state, fail_after);
        assert!(
            project
                .root()
                .join(persisted.temporary.unwrap().as_str())
                .is_file()
        );
        return;
    }
    if let Some(temporary) = persisted.temporary {
        assert!(!project.root().join(temporary.as_str()).exists());
    }
    assert!(matches!(
        persisted.state,
        OperationState::Completed | OperationState::Failed
    ));
}

async fn rename_or_move_recovery_case(fail_after: OperationState, kind: OperationKind) {
    let project = ProjectFixture::new();
    project.create_file("source.jpg", b"rename-payload");
    let mapping = rename_mapping("source.jpg", "destination.jpg");
    let plan = RenamePlanner::plan(project.root(), true, std::slice::from_ref(&mapping)).unwrap();
    let journal = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
    prepare_rename_items(
        &journal,
        OperationId::new(),
        std::slice::from_ref(&mapping),
        kind,
    );
    let executor = RenameExecutor::with_faults(
        project.root(),
        Arc::clone(&journal),
        Arc::new(LocalFileMutation),
        Arc::new(FixedClock::new(3_000)),
        operation_commits(),
        Arc::new(FailAfterState(fail_after)),
    )
    .unwrap();
    let error = executor.execute_interruptible(&plan).await.unwrap_err();
    assert_eq!(error.state(), fail_after);
    drop(executor);
    drop(journal);

    let reopened = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
    recover_twice(&project, Arc::clone(&reopened), durable_trash(&project)).await;

    let candidates = [
        project.root().join("source.jpg"),
        project.root().join("destination.jpg"),
    ];
    let existing = candidates
        .iter()
        .filter(|candidate| candidate.exists())
        .collect::<Vec<_>>();
    assert_eq!(existing.len(), 1);
    assert_eq!(fs::read(existing[0]).unwrap(), b"rename-payload");
    let persisted = reopened.item(mapping.operation_id).unwrap().unwrap();
    if let Some(temporary) = persisted.temporary {
        assert!(!project.root().join(temporary.as_str()).exists());
    }
    assert!(matches!(
        persisted.state,
        OperationState::Completed | OperationState::Failed
    ));
}

async fn replace_recovery_case(fail_after: OperationState) {
    let project = ProjectFixture::new();
    let source = project.create_file("replacement.jpg", b"replacement-payload");
    let destination = project.create_file("existing.jpg", b"previous-payload");
    let batch_id = OperationId::new();
    let item = OperationItemPlan {
        batch_id,
        operation_id: OperationId::new(),
        entity_id: EntityId::new(),
        kind: OperationKind::Move,
        source,
        destination: Some(destination),
        conflict_policy: ConflictPolicy::Replace,
    };
    let journal = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
    journal
        .begin_batch(batch_id, OperationKind::Move, 1, 1_000)
        .unwrap();
    journal.record_item(&item, 1_001).unwrap();
    let trash = durable_trash(&project);
    let executor = ReplaceExecutor::with_faults(
        project.root(),
        Arc::clone(&journal),
        Arc::new(LocalFileMutation),
        Arc::clone(&trash),
        Arc::new(FixedClock::new(4_000)),
        operation_commits(),
        Arc::new(FailAfterState(fail_after)),
    )
    .unwrap();
    let error = executor.execute(&item).await.unwrap_err();
    assert_eq!(error.injected_state(), Some(fail_after));
    drop(executor);
    drop(journal);

    let reopened = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
    recover_twice(&project, Arc::clone(&reopened), trash).await;

    let new_candidates = [
        project.root().join("replacement.jpg"),
        project.root().join("existing.jpg"),
    ];
    assert_eq!(
        new_candidates
            .iter()
            .filter(|candidate| {
                candidate.exists()
                    && fs::read(candidate).unwrap().as_slice() == b"replacement-payload"
            })
            .count(),
        1
    );
    let old_candidates = [
        project.root().join("existing.jpg"),
        project.root().join(".test-trash/existing.jpg"),
    ];
    assert_eq!(
        old_candidates
            .iter()
            .filter(|candidate| {
                candidate.exists() && fs::read(candidate).unwrap().as_slice() == b"previous-payload"
            })
            .count(),
        1
    );
    let persisted = reopened.item(item.operation_id).unwrap().unwrap();
    if let Some(temporary) = persisted.temporary {
        assert!(!project.root().join(temporary.as_str()).exists());
    }
    assert!(matches!(
        persisted.state,
        OperationState::Completed | OperationState::Failed
    ));
}

#[tokio::test]
async fn recovery_matrix_is_data_safe_and_idempotent_after_every_persisted_state() {
    for state in RECOVERY_STATES {
        copy_recovery_case(state).await;
        rename_or_move_recovery_case(state, OperationKind::Rename).await;
        rename_or_move_recovery_case(state, OperationKind::Move).await;
        replace_recovery_case(state).await;
    }
}

#[tokio::test]
async fn staged_trash_recovery_restores_the_registered_identity_and_is_idempotent() {
    let project = ProjectFixture::new();
    let contents = b"recover staged trash";
    let source = project.create_file("source.jpg", contents);
    let batch_id = OperationId::new();
    let operation_id = OperationId::new();
    let item = OperationItemPlan {
        batch_id,
        operation_id,
        entity_id: EntityId::new(),
        kind: OperationKind::Trash,
        source: source.clone(),
        destination: None,
        conflict_policy: ConflictPolicy::Skip,
    };
    let temporary = RelativePath::parse(&format!(".viewer-trash-{operation_id}.part")).unwrap();
    let journal = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
    journal
        .begin_batch(batch_id, OperationKind::Trash, 1, 1_000)
        .unwrap();
    journal.record_item(&item, 1_001).unwrap();
    journal
        .record_prepared_evidence(
            operation_id,
            Some(&temporary),
            contents.len() as u64,
            *blake3::hash(contents).as_bytes(),
            1_002,
        )
        .unwrap();
    fs::rename(
        project.root().join(source.as_str()),
        project.root().join(temporary.as_str()),
    )
    .unwrap();
    journal
        .advance(
            operation_id,
            OperationState::Prepared,
            OperationState::Staged,
            1_003,
        )
        .unwrap();

    recover_twice(&project, Arc::clone(&journal), durable_trash(&project)).await;

    assert_eq!(
        fs::read(project.root().join(source.as_str())).unwrap(),
        contents
    );
    assert!(!project.root().join(temporary.as_str()).exists());
    assert_eq!(
        journal.item(operation_id).unwrap().unwrap().state,
        OperationState::Failed
    );
}

async fn interrupted_verified_copy(
    project: &ProjectFixture,
) -> (Arc<OperationJournal>, OperationItemPlan) {
    let (journal, item, _) = prepared_copy(project);
    let executor = CopyExecutor::with_faults(
        project.root(),
        Arc::clone(&journal),
        Arc::new(LocalFileMutation),
        Arc::new(FixedClock::new(2_000)),
        operation_commits(),
        Arc::new(FailAfterState(OperationState::Verified)),
    )
    .unwrap();
    assert_eq!(
        executor.execute(&item).await.unwrap_err().injected_state(),
        Some(OperationState::Verified)
    );
    (journal, item)
}

#[tokio::test]
async fn recovery_requires_review_for_unknown_or_ambiguous_candidates() {
    for duplicate_verified_contents in [false, true] {
        let project = ProjectFixture::new();
        let (journal, item) = interrupted_verified_copy(&project).await;
        let persisted = journal.item(item.operation_id).unwrap().unwrap();
        let temporary = project
            .root()
            .join(persisted.temporary.as_ref().unwrap().as_str());
        let destination = project
            .root()
            .join(item.destination.as_ref().unwrap().as_str());
        if duplicate_verified_contents {
            fs::copy(&temporary, &destination).unwrap();
        } else {
            fs::write(&destination, b"unknown destination occupant").unwrap();
        }
        let recovery = RecoveryService::new(
            project.root(),
            Arc::clone(&journal),
            Arc::new(LocalFileMutation),
            durable_trash(&project),
            Arc::new(FixedClock::new(9_000)),
            operation_commits(),
        )
        .unwrap();

        let first = recovery.recover_project().await.unwrap();
        let second = recovery.recover_project().await.unwrap();

        assert!(first.actions.is_empty());
        assert_eq!(first.needs_user_review.len(), 1);
        assert!(second.actions.is_empty());
        assert!(temporary.is_file());
        assert!(destination.is_file());
        assert_eq!(
            journal.item(item.operation_id).unwrap().unwrap().state,
            OperationState::Verified
        );
    }

    let project = ProjectFixture::new();
    project.create_file("source.jpg", b"rename-payload");
    let mapping = rename_mapping("source.jpg", "destination.jpg");
    let plan = RenamePlanner::plan(project.root(), true, std::slice::from_ref(&mapping)).unwrap();
    let journal = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
    prepare_rename_items(
        &journal,
        OperationId::new(),
        std::slice::from_ref(&mapping),
        OperationKind::Rename,
    );
    let executor = RenameExecutor::with_faults(
        project.root(),
        Arc::clone(&journal),
        Arc::new(LocalFileMutation),
        Arc::new(FixedClock::new(3_000)),
        operation_commits(),
        Arc::new(FailAfterState(OperationState::Staged)),
    )
    .unwrap();
    executor.execute_interruptible(&plan).await.unwrap_err();
    fs::write(project.root().join("destination.jpg"), b"unknown").unwrap();
    let recovery = RecoveryService::new(
        project.root(),
        Arc::clone(&journal),
        Arc::new(LocalFileMutation),
        durable_trash(&project),
        Arc::new(FixedClock::new(9_000)),
        operation_commits(),
    )
    .unwrap();
    let report = recovery.recover_project().await.unwrap();
    assert_eq!(report.actions.len(), 1);
    assert_eq!(
        report.actions[0].kind,
        RecoveryActionKind::RestoredSource,
        "the evidence-backed staged source is restored without touching the unknown destination"
    );
    assert!(report.needs_user_review.is_empty());
    assert_eq!(
        fs::read(project.root().join("source.jpg")).unwrap(),
        b"rename-payload"
    );
    assert_eq!(
        fs::read(project.root().join("destination.jpg")).unwrap(),
        b"unknown"
    );
}

#[tokio::test]
async fn cross_volume_recovery_never_deletes_when_source_and_destination_both_match() {
    let project = ProjectFixture::new();
    let contents = b"duplicated move payload";
    let source = project.create_file("source.jpg", contents);
    project.create_directory("exports");
    let destination = project.create_file("exports/source.jpg", contents);
    let batch_id = OperationId::new();
    let item = OperationItemPlan {
        batch_id,
        operation_id: OperationId::new(),
        entity_id: EntityId::new(),
        kind: OperationKind::Move,
        source,
        destination: Some(destination.clone()),
        conflict_policy: ConflictPolicy::Skip,
    };
    let temporary =
        RelativePath::parse(&format!("exports/.viewer-copy-{}.part", item.operation_id)).unwrap();
    let journal = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
    journal
        .begin_batch(batch_id, OperationKind::Move, 1, 1_000)
        .unwrap();
    journal.record_item(&item, 1_001).unwrap();
    journal
        .record_prepared_evidence(
            item.operation_id,
            Some(&temporary),
            contents.len() as u64,
            *blake3::hash(contents).as_bytes(),
            1_002,
        )
        .unwrap();
    let recovery = RecoveryService::new(
        project.root(),
        Arc::clone(&journal),
        Arc::new(LocalFileMutation),
        durable_trash(&project),
        Arc::new(FixedClock::new(9_000)),
        operation_commits(),
    )
    .unwrap();

    let report = recovery.recover_project().await.unwrap();

    assert!(report.actions.is_empty());
    assert_eq!(report.needs_user_review.len(), 1);
    assert_eq!(report.needs_user_review[0].operation_id, item.operation_id);
    assert_eq!(
        fs::read(project.root().join("source.jpg")).unwrap(),
        contents
    );
    assert_eq!(
        fs::read(project.root().join("exports/source.jpg")).unwrap(),
        contents
    );
    assert_eq!(
        journal.item(item.operation_id).unwrap().unwrap().state,
        OperationState::Prepared
    );
}

#[tokio::test]
async fn recovery_never_deletes_a_non_deterministic_registered_temporary_path() {
    let project = ProjectFixture::new();
    let source = project.create_file("source.jpg", b"source");
    project.create_directory("exports");
    project.create_file("do-not-delete.jpg", b"user-file");
    let batch_id = OperationId::new();
    let item = copy_item(
        batch_id,
        source,
        RelativePath::parse("exports/source.jpg").unwrap(),
    );
    let journal = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
    journal
        .begin_batch(batch_id, OperationKind::Copy, 1, 1_000)
        .unwrap();
    journal.record_item(&item, 1_001).unwrap();
    journal
        .register_temporary(
            item.operation_id,
            OperationState::Prepared,
            &RelativePath::parse("do-not-delete.jpg").unwrap(),
            1_002,
        )
        .unwrap();
    let recovery = RecoveryService::new(
        project.root(),
        Arc::clone(&journal),
        Arc::new(LocalFileMutation),
        durable_trash(&project),
        Arc::new(FixedClock::new(9_000)),
        operation_commits(),
    )
    .unwrap();

    let report = recovery.recover_project().await.unwrap();

    assert!(report.actions.is_empty());
    assert_eq!(report.needs_user_review.len(), 1);
    assert_eq!(
        fs::read(project.root().join("do-not-delete.jpg")).unwrap(),
        b"user-file"
    );
    assert_eq!(
        journal.item(item.operation_id).unwrap().unwrap().state,
        OperationState::Prepared
    );

    let project = ProjectFixture::new();
    let (journal, item, _) = prepared_copy(&project);
    let colliding_path = project
        .root()
        .join(format!("exports/.viewer-copy-{}.part", item.operation_id));
    fs::write(&colliding_path, b"pre-existing-user-file").unwrap();
    let executor = CopyExecutor::new(
        project.root(),
        Arc::clone(&journal),
        Arc::new(LocalFileMutation),
        Arc::new(FixedClock::new(2_000)),
        operation_commits(),
    )
    .unwrap();
    assert!(executor.execute(&item).await.is_err());
    assert!(
        journal
            .item(item.operation_id)
            .unwrap()
            .unwrap()
            .temporary
            .is_none()
    );
    let recovery = RecoveryService::new(
        project.root(),
        Arc::clone(&journal),
        Arc::new(LocalFileMutation),
        durable_trash(&project),
        Arc::new(FixedClock::new(9_000)),
        operation_commits(),
    )
    .unwrap();
    let report = recovery.recover_project().await.unwrap();
    assert!(report.actions.is_empty());
    assert_eq!(report.needs_user_review.len(), 1);
    assert_eq!(fs::read(colliding_path).unwrap(), b"pre-existing-user-file");
}

#[tokio::test]
async fn recovery_finishes_when_filesystem_truth_is_one_step_ahead_of_journal() {
    let project = ProjectFixture::new();
    let (journal, item) = interrupted_verified_copy(&project).await;
    let persisted = journal.item(item.operation_id).unwrap().unwrap();
    let temporary = project
        .root()
        .join(persisted.temporary.as_ref().unwrap().as_str());
    let destination = project
        .root()
        .join(item.destination.as_ref().unwrap().as_str());
    fs::rename(&temporary, &destination).unwrap();
    drop(journal);
    let reopened = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());

    recover_twice(&project, Arc::clone(&reopened), durable_trash(&project)).await;

    assert_eq!(
        reopened.item(item.operation_id).unwrap().unwrap().state,
        OperationState::Completed
    );
    assert!(destination.is_file());

    let project = ProjectFixture::new();
    project.create_file("source.jpg", b"rename-payload");
    let mapping = rename_mapping("source.jpg", "destination.jpg");
    let plan = RenamePlanner::plan(project.root(), true, std::slice::from_ref(&mapping)).unwrap();
    let journal = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
    prepare_rename_items(
        &journal,
        OperationId::new(),
        std::slice::from_ref(&mapping),
        OperationKind::Rename,
    );
    let executor = RenameExecutor::with_faults(
        project.root(),
        Arc::clone(&journal),
        Arc::new(LocalFileMutation),
        Arc::new(FixedClock::new(3_000)),
        operation_commits(),
        Arc::new(FailAfterState(OperationState::Staged)),
    )
    .unwrap();
    executor.execute_interruptible(&plan).await.unwrap_err();
    let persisted = journal.item(mapping.operation_id).unwrap().unwrap();
    let temporary = project
        .root()
        .join(persisted.temporary.as_ref().unwrap().as_str());
    fs::rename(temporary, project.root().join("destination.jpg")).unwrap();
    drop(executor);
    drop(journal);
    let reopened = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());

    recover_twice(&project, Arc::clone(&reopened), durable_trash(&project)).await;

    assert_eq!(
        reopened.item(mapping.operation_id).unwrap().unwrap().state,
        OperationState::Completed
    );
    assert_eq!(
        fs::read(project.root().join("destination.jpg")).unwrap(),
        b"rename-payload"
    );
}

struct FailFinalReplacementPort {
    delegate: LocalFileMutation,
}

#[async_trait]
impl FileMutationPort for FailFinalReplacementPort {
    async fn create_and_copy_cancellable_verified(
        &self,
        source: &Path,
        temporary: &Path,
        cancellation: &FileCommandCancellation,
        expected_source: &FileSnapshot,
        source_parent: FileIdentity,
        temporary_parent: FileIdentity,
    ) -> Result<FileContentEvidence, FileOperationError> {
        delegate_registered_copy(
            &self.delegate,
            source,
            temporary,
            cancellation,
            expected_source,
            source_parent,
            temporary_parent,
        )
        .await
    }

    async fn snapshot(&self, path: &Path) -> Result<FileSnapshot, FileOperationError> {
        self.delegate.snapshot(path).await
    }

    async fn copy_and_hash(
        &self,
        source: &Path,
        temporary: &Path,
    ) -> Result<(u64, [u8; 32]), FileOperationError> {
        self.delegate.copy_and_hash(source, temporary).await
    }

    async fn rename(&self, source: &Path, destination: &Path) -> Result<(), FileOperationError> {
        if destination.file_name().and_then(|name| name.to_str()) == Some("existing.jpg") {
            return Err(FileOperationError::Io {
                action: "injected final replacement placement",
                path: destination.to_path_buf(),
                message: "injected failure".into(),
            });
        }
        self.delegate.rename(source, destination).await
    }

    async fn remove_registered_temporary(&self, path: &Path) -> Result<(), FileOperationError> {
        self.delegate.remove_registered_temporary(path).await
    }
}

#[tokio::test]
async fn recovery_restores_replacement_source_when_previous_destination_is_already_in_trash() {
    let project = ProjectFixture::new();
    let source = project.create_file("replacement.jpg", b"replacement-payload");
    let destination = project.create_file("existing.jpg", b"previous-payload");
    let batch_id = OperationId::new();
    let item = OperationItemPlan {
        batch_id,
        operation_id: OperationId::new(),
        entity_id: EntityId::new(),
        kind: OperationKind::Move,
        source,
        destination: Some(destination),
        conflict_policy: ConflictPolicy::Replace,
    };
    let journal = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
    journal
        .begin_batch(batch_id, OperationKind::Move, 1, 1_000)
        .unwrap();
    journal.record_item(&item, 1_001).unwrap();
    let trash = durable_trash(&project);
    let executor = ReplaceExecutor::new(
        project.root(),
        Arc::clone(&journal),
        Arc::new(FailFinalReplacementPort {
            delegate: LocalFileMutation,
        }),
        Arc::clone(&trash),
        Arc::new(FixedClock::new(4_000)),
        operation_commits(),
    )
    .unwrap();
    assert!(matches!(
        executor.execute(&item).await,
        Err(viewer_infrastructure::operation::conflict::ReplaceError::PlacementAfterTrash(_))
    ));
    assert_eq!(
        journal.item(item.operation_id).unwrap().unwrap().state,
        OperationState::Staged
    );

    let recovery = RecoveryService::new(
        project.root(),
        Arc::clone(&journal),
        Arc::new(LocalFileMutation),
        trash,
        Arc::new(FixedClock::new(9_000)),
        operation_commits(),
    )
    .unwrap();
    let report = recovery.recover_project().await.unwrap();

    assert_eq!(report.actions.len(), 1);
    assert!(report.needs_user_review.is_empty());
    assert_eq!(
        fs::read(project.root().join("replacement.jpg")).unwrap(),
        b"replacement-payload"
    );
    assert_eq!(
        fs::read(project.root().join(".test-trash/existing.jpg")).unwrap(),
        b"previous-payload"
    );
    assert!(!project.root().join("existing.jpg").exists());
    assert_eq!(
        journal.item(item.operation_id).unwrap().unwrap().state,
        OperationState::Failed
    );
}
