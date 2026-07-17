use async_trait::async_trait;
use std::{fs, path::Path, sync::Arc};
use viewer_application::{FileMutationPort, FileOperationError, FileSnapshot};
use viewer_domain::{
    EntityId, OperationId, RelativePath,
    operation::{ConflictPolicy, OperationItemPlan, OperationKind, OperationState},
};
use viewer_infrastructure::operation::{
    copy::LocalFileMutation,
    executor::{CopyExecutor, CopyResumeResult},
    journal::OperationJournal,
    rename::{RenameExecutor, RenameItemStatus, RenameMapping, RenamePlanner},
};
use viewer_test_support::{FixedClock, project_fixture::ProjectFixture};

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
        .create_batch(batch_id, OperationKind::Copy, 1_000)
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

#[async_trait]
impl FileMutationPort for CancelledCopyPort {
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
    assert!(
        !project
            .root()
            .join(persisted.temporary.unwrap().as_str())
            .exists()
    );
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
            .create_batch(batch_id, OperationKind::Copy, 1_000)
            .unwrap();
        journal.record_item(&item, 1_001).unwrap();
        let executor = CopyExecutor::new(
            project.root(),
            journal,
            Arc::new(LocalFileMutation),
            Arc::new(FixedClock::new(2_000)),
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
    journal.create_batch(batch_id, kind, 4_000).unwrap();
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
    )
    .unwrap();

    let result = executor.execute(&plan).await;
    let after = mutation
        .snapshot(&project.root().join("destination/A.jpg"))
        .await
        .unwrap();

    assert_eq!(result.items[0].status, RenameItemStatus::Completed);
    assert_eq!(before, after);
    assert!(!project.root().join("source/A.jpg").exists());
}

struct FailNamedRenamePort {
    delegate: LocalFileMutation,
    fail_source_name: &'static str,
}

#[async_trait]
impl FileMutationPort for FailNamedRenamePort {
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
        OperationState::Staged
    );
}
