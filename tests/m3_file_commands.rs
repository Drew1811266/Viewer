use async_trait::async_trait;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::Notify;
use viewer_application::{
    CommitStage, FileMutationPort, FileOperationError, FileSnapshot, OperationCommit,
    OperationCommitError, OperationCommitPort, ProjectAccess, TrashPort, VolumePort,
    file_commands::{
        BatchResultCode, ConflictResolution, FileCommand, FileCommandAction,
        FileCommandCancellation, FileCommandItem, FileCommandKind, FileCommandPreflightState,
        FileCommandService,
    },
    scheduler::TaskCoordinator,
};
use viewer_domain::{
    EntityId, RelativePath, SessionId,
    file::{FileKind, FileNode},
    operation::{ConflictPolicy, OperationState},
    search::Generation,
};
use viewer_infrastructure::{
    operation::{
        copy::LocalFileMutation,
        journal::{BatchState, OperationJournal},
        recovery::{RecoveryActionKind, RecoveryService},
        service::LocalFileCommandAdapter,
    },
    scan::reconcile::ExpectedChangeLedger,
    search::index::SessionIndex,
};
use viewer_test_support::{
    FixedClock, operation_commits::InMemoryOperationCommitPort, project_fixture::ProjectFixture,
};

#[derive(Default)]
struct FakeVolume {
    cross_volume: bool,
    queries: Mutex<Vec<PathBuf>>,
}

impl VolumePort for FakeVolume {
    fn volume_id(&self, path: &Path) -> Result<u64, FileOperationError> {
        self.queries.lock().unwrap().push(path.to_path_buf());
        Ok(if self.cross_volume && path.is_dir() {
            2
        } else {
            1
        })
    }

    fn is_case_sensitive(&self, _path: &Path) -> Result<bool, FileOperationError> {
        Ok(true)
    }

    fn name_max(&self, _path: &Path) -> Result<usize, FileOperationError> {
        Ok(255)
    }
}

struct FakeTrash {
    directory: tempfile::TempDir,
    trashed: Mutex<Vec<PathBuf>>,
}

impl FakeTrash {
    fn new() -> Self {
        Self {
            directory: tempfile::tempdir().unwrap(),
            trashed: Mutex::new(Vec::new()),
        }
    }

    fn count(&self) -> usize {
        self.trashed.lock().unwrap().len()
    }
}

#[async_trait]
impl TrashPort for FakeTrash {
    async fn trash(&self, path: &Path) -> Result<(), FileOperationError> {
        let destination = self
            .directory
            .path()
            .join(format!("{}-trashed", self.count()));
        fs::rename(path, &destination)
            .map_err(|error| FileOperationError::io("fake Trash", path, &error))?;
        self.trashed.lock().unwrap().push(destination);
        Ok(())
    }
}

struct Fixture {
    project: ProjectFixture,
    index: Arc<SessionIndex>,
    journal: Arc<OperationJournal>,
    trash: Arc<FakeTrash>,
    volume: Arc<FakeVolume>,
    adapter: Arc<LocalFileCommandAdapter>,
    expected_changes: ExpectedChangeLedger,
    session_id: SessionId,
    generation: Generation,
    service: Arc<FileCommandService>,
}

impl Fixture {
    fn new(cross_volume: bool) -> Self {
        Self::with_mutation(cross_volume, Arc::new(LocalFileMutation))
    }

    fn with_mutation(cross_volume: bool, mutation: Arc<dyn FileMutationPort>) -> Self {
        Self::with_ports(
            cross_volume,
            mutation,
            Arc::new(InMemoryOperationCommitPort::default()),
        )
    }

    fn with_ports(
        cross_volume: bool,
        mutation: Arc<dyn FileMutationPort>,
        commits: Arc<dyn OperationCommitPort>,
    ) -> Self {
        let project = ProjectFixture::new();
        let index =
            Arc::new(SessionIndex::open(project.root().join(".viewer/session.sqlite")).unwrap());
        let journal = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
        let trash = Arc::new(FakeTrash::new());
        let volume = Arc::new(FakeVolume {
            cross_volume,
            ..FakeVolume::default()
        });
        let adapter = Arc::new(
            LocalFileCommandAdapter::new(
                project.root(),
                index.clone(),
                Arc::clone(&journal),
                mutation,
                trash.clone(),
                volume.clone(),
                Arc::new(FixedClock::new(10_000)),
                commits,
            )
            .unwrap(),
        );
        let expected_changes = adapter.expected_change_ledger();
        let session_id = SessionId::new();
        let coordinator = Arc::new(TaskCoordinator::default());
        let generation = coordinator.begin_session(session_id);
        let service = Arc::new(FileCommandService::new(
            session_id,
            ProjectAccess::ReadWrite,
            coordinator,
            adapter.clone(),
        ));
        Self {
            project,
            index,
            journal,
            trash,
            volume,
            adapter,
            expected_changes,
            session_id,
            generation,
            service,
        }
    }

    fn directory(&self, relative: &str) -> EntityId {
        self.project.create_directory(relative);
        self.index_node(relative, FileKind::Directory)
    }

    fn file(&self, relative: &str, contents: &[u8]) -> EntityId {
        self.project.create_file(relative, contents);
        let kind = if relative.ends_with(".txt") {
            FileKind::Text
        } else {
            FileKind::Png
        };
        self.index_node(relative, kind)
    }

    fn index_node(&self, relative: &str, kind: FileKind) -> EntityId {
        let path = self.project.root().join(relative);
        let metadata = fs::metadata(&path).unwrap();
        let entity_id = EntityId::new();
        self.index
            .upsert_batch(&[FileNode {
                entity_id,
                relative_path: RelativePath::parse(relative).unwrap(),
                kind,
                size: metadata.len(),
                modified_ns: 1,
            }])
            .unwrap();
        entity_id
    }

    fn command(&self, kind: FileCommandKind, items: Vec<FileCommandItem>) -> FileCommand {
        FileCommand {
            session_id: self.session_id,
            generation: self.generation,
            kind,
            items,
        }
    }
}

#[tokio::test]
async fn repeated_display_preflights_leave_no_prepared_batch_registered() {
    let fixture = Fixture::new(false);
    fixture.directory("source");
    let destination = fixture.directory("exports");
    let source = fixture.file("source/item.png", b"source");
    let command = fixture.command(
        FileCommandKind::Copy,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Copy {
                destination_folder: destination,
            },
        }],
    );

    for _ in 0..32 {
        let preview = fixture.service.preview(command.clone()).await.unwrap();
        assert!(preview.is_executable());
    }

    assert_eq!(fixture.adapter.prepared_batch_count(), 0);
}

#[tokio::test]
async fn rename_cycle_is_planned_as_one_atomic_batch_and_preserves_contents() {
    let fixture = Fixture::new(false);
    fixture.directory("products");
    let a = fixture.file("products/a.png", b"A");
    let b = fixture.file("products/b.png", b"B");
    let command = fixture.command(
        FileCommandKind::Rename,
        vec![
            FileCommandItem {
                entity_id: a,
                action: FileCommandAction::Rename {
                    proposed_name: "b.png".into(),
                    edit_extension: true,
                },
            },
            FileCommandItem {
                entity_id: b,
                action: FileCommandAction::Rename {
                    proposed_name: "a.png".into(),
                    edit_extension: true,
                },
            },
        ],
    );

    let preflight = fixture.service.preflight(command).await.unwrap();
    assert!(preflight.is_executable());
    assert!(
        preflight
            .rows()
            .iter()
            .all(|row| row.state == FileCommandPreflightState::Ready)
    );
    let batch_id = preflight.batch_id();
    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();

    assert_eq!(summary.completed(), 2);
    assert_eq!(
        fs::read(fixture.project.root().join("products/a.png")).unwrap(),
        b"B"
    );
    assert_eq!(
        fs::read(fixture.project.root().join("products/b.png")).unwrap(),
        b"A"
    );
    assert_eq!(
        fixture
            .journal
            .batch(batch_id)
            .unwrap()
            .unwrap()
            .completed_count,
        2
    );
}

#[tokio::test]
async fn copy_keep_both_and_replace_never_silently_overwrite() {
    let fixture = Fixture::new(false);
    fixture.directory("source");
    let destination = fixture.directory("exports");
    let source = fixture.file("source/item.png", b"new");
    fixture.file("exports/item.png", b"old");

    let keep = fixture.command(
        FileCommandKind::Copy,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Copy {
                destination_folder: destination,
            },
        }],
    );
    let preflight = fixture.service.preflight(keep).await.unwrap();
    assert_eq!(
        preflight.rows()[0].state,
        FileCommandPreflightState::Conflict
    );
    let summary = fixture
        .service
        .execute(
            preflight,
            &[ConflictResolution {
                entity_id: source,
                policy: ConflictPolicy::KeepBoth,
                apply_to_remaining: false,
            }],
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        summary.result_page(0, 1).items[0].code,
        BatchResultCode::Copied
    );
    assert_eq!(
        fs::read(fixture.project.root().join("exports/item.png")).unwrap(),
        b"old"
    );
    assert_eq!(
        fs::read(fixture.project.root().join("exports/item copy.png")).unwrap(),
        b"new"
    );

    let replace = fixture.command(
        FileCommandKind::Copy,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Copy {
                destination_folder: destination,
            },
        }],
    );
    let preflight = fixture.service.preflight(replace).await.unwrap();
    fixture
        .service
        .execute(
            preflight,
            &[ConflictResolution {
                entity_id: source,
                policy: ConflictPolicy::Replace,
                apply_to_remaining: false,
            }],
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        fs::read(fixture.project.root().join("exports/item.png")).unwrap(),
        b"new"
    );
    assert_eq!(fixture.trash.count(), 1);
}

#[tokio::test]
async fn cross_volume_move_copies_verifies_then_trashes_source() {
    let fixture = Fixture::new(true);
    fixture.directory("source");
    fixture.directory("deep");
    let destination = fixture.directory("deep/exports");
    let source = fixture.file("source/item.png", b"arbitrary corrupt image bytes");
    let command = fixture.command(
        FileCommandKind::Move,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Move {
                destination_folder: destination,
            },
        }],
    );

    let preflight = fixture.service.preflight(command).await.unwrap();
    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();

    assert_eq!(
        summary.result_page(0, 1).items[0].code,
        BatchResultCode::Moved
    );
    assert!(!fixture.project.root().join("source/item.png").exists());
    assert_eq!(
        fs::read(fixture.project.root().join("deep/exports/item.png")).unwrap(),
        b"arbitrary corrupt image bytes"
    );
    assert_eq!(fixture.trash.count(), 1);
    assert!(fixture.volume.queries.lock().unwrap().len() >= 2);
    assert_eq!(fixture.expected_changes.pending_count(), 2);
}

struct FailMetadataCommit;

#[async_trait]
impl OperationCommitPort for FailMetadataCommit {
    async fn commit_metadata(&self, _commit: &OperationCommit) -> Result<(), OperationCommitError> {
        Err(OperationCommitError::new(
            CommitStage::Metadata,
            "injected_failure",
        ))
    }

    async fn sync_index(&self, _commit: &OperationCommit) -> Result<(), OperationCommitError> {
        unreachable!("metadata failure prevents index synchronization")
    }
}

#[tokio::test]
async fn cross_volume_move_recovers_from_a_projection_barrier_failure() {
    let fixture = Fixture::with_ports(
        true,
        Arc::new(LocalFileMutation),
        Arc::new(FailMetadataCommit),
    );
    fixture.directory("source");
    let destination = fixture.directory("exports");
    let source = fixture.file("source/item.png", b"recoverable");
    let command = fixture.command(
        FileCommandKind::Move,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Move {
                destination_folder: destination,
            },
        }],
    );
    let preflight = fixture.service.preflight(command).await.unwrap();
    let batch_id = preflight.batch_id();

    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();

    assert_eq!(summary.failed(), 1);
    let incomplete = fixture.journal.incomplete_items().unwrap();
    assert_eq!(incomplete.len(), 1);
    assert_eq!(incomplete[0].state, OperationState::Verified);
    assert!(!fixture.project.root().join("source/item.png").exists());
    assert!(fixture.project.root().join("exports/item.png").is_file());

    let recovery = RecoveryService::new(
        fixture.project.root(),
        Arc::clone(&fixture.journal),
        Arc::new(LocalFileMutation),
        fixture.trash.clone(),
        Arc::new(FixedClock::new(20_000)),
        Arc::new(InMemoryOperationCommitPort::default()),
    )
    .unwrap();
    let report = recovery.recover_project().await.unwrap();

    assert_eq!(report.needs_user_review, Vec::new());
    assert_eq!(report.actions.len(), 1);
    assert_eq!(report.actions[0].kind, RecoveryActionKind::Completed);
    assert_eq!(fixture.journal.incomplete_items().unwrap(), Vec::new());
    assert_eq!(
        fixture.journal.batch(batch_id).unwrap().unwrap().state,
        BatchState::Completed
    );
}

#[tokio::test]
async fn same_volume_move_is_an_identity_preserving_rename_without_trash() {
    use std::os::unix::fs::MetadataExt;

    let fixture = Fixture::new(false);
    fixture.directory("source");
    let destination = fixture.directory("exports");
    let source = fixture.file("source/item.png", b"same volume");
    let source_path = fixture.project.root().join("source/item.png");
    let inode = fs::metadata(&source_path).unwrap().ino();
    let command = fixture.command(
        FileCommandKind::Move,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Move {
                destination_folder: destination,
            },
        }],
    );

    let preflight = fixture.service.preflight(command).await.unwrap();
    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();

    assert_eq!(summary.completed(), 1);
    assert!(!source_path.exists());
    assert_eq!(
        fs::metadata(fixture.project.root().join("exports/item.png"))
            .unwrap()
            .ino(),
        inode
    );
    assert_eq!(fixture.trash.count(), 0);
}

#[tokio::test]
async fn destination_race_fails_safely_and_preserves_both_files() {
    let fixture = Fixture::new(false);
    fixture.directory("source");
    let destination = fixture.directory("exports");
    let source = fixture.file("source/item.png", b"source");
    let command = fixture.command(
        FileCommandKind::Copy,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Copy {
                destination_folder: destination,
            },
        }],
    );
    let preflight = fixture.service.preflight(command).await.unwrap();
    fs::write(fixture.project.root().join("exports/item.png"), b"racer").unwrap();

    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();

    assert_eq!(summary.failed(), 1);
    assert_eq!(
        summary.result_page(0, 1).items[0].code,
        BatchResultCode::DestinationOccupied
    );
    assert_eq!(
        fs::read(fixture.project.root().join("source/item.png")).unwrap(),
        b"source"
    );
    assert_eq!(
        fs::read(fixture.project.root().join("exports/item.png")).unwrap(),
        b"racer"
    );
}

#[tokio::test]
async fn source_identity_replacement_after_preflight_is_never_operated_on() {
    let fixture = Fixture::new(false);
    fixture.directory("source");
    let destination = fixture.directory("exports");
    let source = fixture.file("source/item.png", b"selected");
    let command = fixture.command(
        FileCommandKind::Copy,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Copy {
                destination_folder: destination,
            },
        }],
    );
    let preflight = fixture.service.preflight(command).await.unwrap();
    let source_path = fixture.project.root().join("source/item.png");
    fs::remove_file(&source_path).unwrap();
    fs::write(&source_path, b"replacement").unwrap();

    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();

    assert_eq!(summary.failed(), 1);
    assert_eq!(
        summary.result_page(0, 1).items[0].code,
        BatchResultCode::VerificationFailed
    );
    assert_eq!(fs::read(source_path).unwrap(), b"replacement");
    assert!(!fixture.project.root().join("exports/item.png").exists());
}

#[tokio::test]
async fn same_size_in_place_source_rewrite_after_preflight_is_never_copied() {
    let fixture = Fixture::new(false);
    fixture.directory("source");
    let destination = fixture.directory("exports");
    let source = fixture.file("source/item.png", b"AAAA");
    let command = fixture.command(
        FileCommandKind::Copy,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Copy {
                destination_folder: destination,
            },
        }],
    );
    let preflight = fixture.service.preflight(command).await.unwrap();
    let source_path = fixture.project.root().join("source/item.png");
    fs::write(&source_path, b"BBBB").unwrap();

    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();

    assert_eq!(summary.failed(), 1);
    assert_eq!(
        summary.result_page(0, 1).items[0].code,
        BatchResultCode::VerificationFailed
    );
    assert_eq!(fs::read(source_path).unwrap(), b"BBBB");
    assert!(!fixture.project.root().join("exports/item.png").exists());
}

#[tokio::test]
async fn replace_revalidates_the_exact_destination_before_using_trash() {
    let fixture = Fixture::new(false);
    fixture.directory("source");
    let destination = fixture.directory("exports");
    let source = fixture.file("source/item.png", b"source");
    fixture.file("exports/item.png", b"selected destination");
    let command = fixture.command(
        FileCommandKind::Copy,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Copy {
                destination_folder: destination,
            },
        }],
    );
    let preflight = fixture.service.preflight(command).await.unwrap();
    let destination_path = fixture.project.root().join("exports/item.png");
    fs::remove_file(&destination_path).unwrap();
    fs::write(&destination_path, b"replacement destination").unwrap();

    let summary = fixture
        .service
        .execute(
            preflight,
            &[ConflictResolution {
                entity_id: source,
                policy: ConflictPolicy::Replace,
                apply_to_remaining: false,
            }],
            None,
        )
        .await
        .unwrap();

    assert_eq!(summary.failed(), 1);
    assert_eq!(
        summary.result_page(0, 1).items[0].code,
        BatchResultCode::VerificationFailed
    );
    assert_eq!(
        fs::read(destination_path).unwrap(),
        b"replacement destination"
    );
    assert_eq!(
        fs::read(fixture.project.root().join("source/item.png")).unwrap(),
        b"source"
    );
    assert_eq!(fixture.trash.count(), 0);
}

#[tokio::test]
async fn replace_rejects_same_size_in_place_destination_rewrite_before_trash() {
    let fixture = Fixture::new(false);
    fixture.directory("source");
    let destination = fixture.directory("exports");
    let source = fixture.file("source/item.png", b"source");
    fixture.file("exports/item.png", b"AAAA");
    let command = fixture.command(
        FileCommandKind::Copy,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Copy {
                destination_folder: destination,
            },
        }],
    );
    let preflight = fixture.service.preflight(command).await.unwrap();
    let destination_path = fixture.project.root().join("exports/item.png");
    fs::write(&destination_path, b"BBBB").unwrap();

    let summary = fixture
        .service
        .execute(
            preflight,
            &[ConflictResolution {
                entity_id: source,
                policy: ConflictPolicy::Replace,
                apply_to_remaining: false,
            }],
            None,
        )
        .await
        .unwrap();

    assert_eq!(summary.failed(), 1);
    assert_eq!(
        summary.result_page(0, 1).items[0].code,
        BatchResultCode::VerificationFailed
    );
    assert_eq!(fs::read(destination_path).unwrap(), b"BBBB");
    assert_eq!(
        fs::read(fixture.project.root().join("source/item.png")).unwrap(),
        b"source"
    );
    assert_eq!(fixture.trash.count(), 0);
}

#[tokio::test]
async fn destination_permission_loss_after_preflight_fails_before_mutation() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new(false);
    fixture.directory("source");
    let destination = fixture.directory("exports");
    let source = fixture.file("source/item.png", b"source");
    let command = fixture.command(
        FileCommandKind::Copy,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Copy {
                destination_folder: destination,
            },
        }],
    );
    let preflight = fixture.service.preflight(command).await.unwrap();
    let destination_path = fixture.project.root().join("exports");
    fs::set_permissions(&destination_path, fs::Permissions::from_mode(0o555)).unwrap();

    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();
    fs::set_permissions(&destination_path, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(summary.failed(), 1);
    assert_eq!(
        summary.result_page(0, 1).items[0].code,
        BatchResultCode::PermissionDenied
    );
    assert!(fixture.project.root().join("source/item.png").is_file());
    assert!(!fixture.project.root().join("exports/item.png").exists());
}

#[tokio::test]
async fn trash_records_intent_and_only_accepts_supported_regular_files() {
    let fixture = Fixture::new(false);
    fixture.directory("source");
    let source = fixture.file("source/note.txt", b"note");
    let command = fixture.command(
        FileCommandKind::Trash,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Trash,
        }],
    );

    let preflight = fixture.service.preflight(command).await.unwrap();
    let batch_id = preflight.batch_id();
    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();

    assert_eq!(
        summary.result_page(0, 1).items[0].code,
        BatchResultCode::MovedToTrash
    );
    assert!(!fixture.project.root().join("source/note.txt").exists());
    assert_eq!(fixture.trash.count(), 1);
    assert_eq!(
        fixture
            .journal
            .batch(batch_id)
            .unwrap()
            .unwrap()
            .completed_count,
        1
    );
}

#[tokio::test]
async fn conflict_skip_is_durable_and_does_not_touch_either_file() {
    let fixture = Fixture::new(false);
    fixture.directory("source");
    let destination = fixture.directory("exports");
    let source = fixture.file("source/item.png", b"source");
    fixture.file("exports/item.png", b"destination");
    let command = fixture.command(
        FileCommandKind::Copy,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Copy {
                destination_folder: destination,
            },
        }],
    );
    let preflight = fixture.service.preflight(command).await.unwrap();
    let batch_id = preflight.batch_id();

    let summary = fixture
        .service
        .execute(
            preflight,
            &[ConflictResolution {
                entity_id: source,
                policy: ConflictPolicy::Skip,
                apply_to_remaining: false,
            }],
            None,
        )
        .await
        .unwrap();

    assert_eq!(summary.skipped(), 1);
    assert_eq!(
        fs::read(fixture.project.root().join("source/item.png")).unwrap(),
        b"source"
    );
    assert_eq!(
        fs::read(fixture.project.root().join("exports/item.png")).unwrap(),
        b"destination"
    );
    let persisted = fixture.journal.batch(batch_id).unwrap().unwrap();
    assert_eq!(persisted.skipped_count, 1);
}

#[tokio::test]
async fn duplicate_batch_destinations_are_resolved_in_visible_order() {
    let fixture = Fixture::new(false);
    fixture.directory("one");
    fixture.directory("two");
    let destination = fixture.directory("exports");
    let first = fixture.file("one/item.png", b"first");
    let second = fixture.file("two/item.png", b"second");
    let command = fixture.command(
        FileCommandKind::Copy,
        [first, second]
            .into_iter()
            .map(|entity_id| FileCommandItem {
                entity_id,
                action: FileCommandAction::Copy {
                    destination_folder: destination,
                },
            })
            .collect(),
    );

    let preflight = fixture.service.preflight(command).await.unwrap();
    assert_eq!(preflight.rows()[0].state, FileCommandPreflightState::Ready);
    assert_eq!(
        preflight.rows()[1].state,
        FileCommandPreflightState::Conflict
    );
    let summary = fixture
        .service
        .execute(
            preflight,
            &[ConflictResolution {
                entity_id: second,
                policy: ConflictPolicy::KeepBoth,
                apply_to_remaining: false,
            }],
            None,
        )
        .await
        .unwrap();

    assert_eq!(summary.completed(), 2);
    assert_eq!(
        fs::read(fixture.project.root().join("exports/item.png")).unwrap(),
        b"first"
    );
    assert_eq!(
        fs::read(fixture.project.root().join("exports/item copy.png")).unwrap(),
        b"second"
    );
}

#[tokio::test]
async fn copy_faults_are_isolated_and_batch_reports_exact_partial_success() {
    let fixture = Fixture::with_mutation(
        false,
        Arc::new(FailOneCopy {
            delegate: LocalFileMutation,
        }),
    );
    fixture.directory("source");
    let destination = fixture.directory("exports");
    let good = fixture.file("source/good.png", b"good");
    let bad = fixture.file("source/bad.png", b"bad");
    let denied = fixture.file("source/denied.png", b"denied");
    let command = fixture.command(
        FileCommandKind::Copy,
        [good, bad, denied]
            .into_iter()
            .map(|entity_id| FileCommandItem {
                entity_id,
                action: FileCommandAction::Copy {
                    destination_folder: destination,
                },
            })
            .collect(),
    );
    let preflight = fixture.service.preflight(command).await.unwrap();
    let batch_id = preflight.batch_id();

    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();

    assert_eq!((summary.completed(), summary.failed()), (1, 2));
    let results = summary.result_page(0, 3).items;
    assert_eq!(results[0].code, BatchResultCode::Copied);
    assert_eq!(results[1].code, BatchResultCode::BackendUnavailable);
    assert_eq!(results[2].code, BatchResultCode::PermissionDenied);
    assert_eq!(
        fs::read(fixture.project.root().join("exports/good.png")).unwrap(),
        b"good"
    );
    assert!(!fixture.project.root().join("exports/bad.png").exists());
    assert!(!fixture.project.root().join("exports/denied.png").exists());
    assert_eq!(
        fs::read(fixture.project.root().join("source/bad.png")).unwrap(),
        b"bad"
    );
    let persisted = fixture.journal.batch(batch_id).unwrap().unwrap();
    assert_eq!((persisted.completed_count, persisted.failed_count), (1, 2));
}

struct RewriteSourceAfterCopy {
    delegate: LocalFileMutation,
}

struct SwapSourceBeforeMutationSnapshot {
    delegate: LocalFileMutation,
    swap: Mutex<Option<(PathBuf, PathBuf)>>,
}

#[async_trait]
impl FileMutationPort for SwapSourceBeforeMutationSnapshot {
    async fn snapshot(&self, path: &Path) -> Result<FileSnapshot, FileOperationError> {
        let paths = {
            let mut swap = self.swap.lock().unwrap();
            if swap
                .as_ref()
                .is_some_and(|(source, _)| source.as_path() == path)
            {
                swap.take()
            } else {
                None
            }
        };
        if let Some((source, parked_original)) = paths {
            fs::rename(&source, &parked_original)
                .map_err(|error| FileOperationError::io("park selected source", &source, &error))?;
            fs::write(&source, b"replacement").map_err(|error| {
                FileOperationError::io("install replacement source", &source, &error)
            })?;
        }
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
        self.delegate.rename(source, destination).await
    }

    async fn remove_registered_temporary(&self, path: &Path) -> Result<(), FileOperationError> {
        self.delegate.remove_registered_temporary(path).await
    }
}

#[tokio::test]
async fn rename_rejects_a_source_entity_swapped_after_dispatch_validation() {
    let mutation = Arc::new(SwapSourceBeforeMutationSnapshot {
        delegate: LocalFileMutation,
        swap: Mutex::new(None),
    });
    let fixture = Fixture::with_mutation(false, mutation.clone());
    fixture.directory("products");
    let source = fixture.file("products/source.png", b"selected");
    let source_path = fixture.project.root().join("products/source.png");
    let canonical_source = fs::canonicalize(&source_path).unwrap();
    let parked_original = canonical_source
        .parent()
        .unwrap()
        .join("original-selected.png");
    *mutation.swap.lock().unwrap() = Some((canonical_source, parked_original.clone()));
    let command = fixture.command(
        FileCommandKind::Rename,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Rename {
                proposed_name: "renamed.png".into(),
                edit_extension: true,
            },
        }],
    );
    let preflight = fixture.service.preflight(command).await.unwrap();

    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();

    assert_eq!(summary.failed(), 1);
    assert_eq!(
        summary.result_page(0, 1).items[0].code,
        BatchResultCode::VerificationFailed
    );
    assert_eq!(fs::read(&source_path).unwrap(), b"replacement");
    assert_eq!(fs::read(&parked_original).unwrap(), b"selected");
    assert!(!fixture.project.root().join("products/renamed.png").exists());
}

#[async_trait]
impl FileMutationPort for RewriteSourceAfterCopy {
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

    async fn copy_and_hash_cancellable(
        &self,
        source: &Path,
        temporary: &Path,
        cancellation: &FileCommandCancellation,
    ) -> Result<(u64, [u8; 32]), FileOperationError> {
        let copied = self
            .delegate
            .copy_and_hash_cancellable(source, temporary, cancellation)
            .await?;
        fs::write(source, b"mutated!")
            .map_err(|error| FileOperationError::io("rewrite copy source", source, &error))?;
        Ok(copied)
    }

    async fn rename(&self, source: &Path, destination: &Path) -> Result<(), FileOperationError> {
        self.delegate.rename(source, destination).await
    }

    async fn remove_registered_temporary(&self, path: &Path) -> Result<(), FileOperationError> {
        self.delegate.remove_registered_temporary(path).await
    }
}

#[tokio::test]
async fn cross_volume_move_never_trashes_a_source_rewritten_during_copy() {
    let fixture = Fixture::with_mutation(
        true,
        Arc::new(RewriteSourceAfterCopy {
            delegate: LocalFileMutation,
        }),
    );
    fixture.directory("source");
    let destination = fixture.directory("exports");
    let source = fixture.file("source/item.png", b"original");
    let command = fixture.command(
        FileCommandKind::Move,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Move {
                destination_folder: destination,
            },
        }],
    );
    let preflight = fixture.service.preflight(command).await.unwrap();

    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();

    assert_eq!(summary.failed(), 1);
    assert_eq!(
        summary.result_page(0, 1).items[0].code,
        BatchResultCode::VerificationFailed
    );
    assert_eq!(
        fs::read(fixture.project.root().join("source/item.png")).unwrap(),
        b"mutated!"
    );
    assert!(!fixture.project.root().join("exports/item.png").exists());
    assert_eq!(fixture.trash.count(), 0);
}

struct RewriteDestinationAfterSourceStage {
    delegate: LocalFileMutation,
    destination: Mutex<Option<PathBuf>>,
}

#[async_trait]
impl FileMutationPort for RewriteDestinationAfterSourceStage {
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
        self.delegate.rename(source, destination).await?;
        let staged_replace = destination
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(".viewer-replace-") && name.ends_with(".part"));
        if staged_replace && let Some(target) = self.destination.lock().unwrap().take() {
            fs::write(&target, b"BBBB").map_err(|error| {
                FileOperationError::io("rewrite replace destination", &target, &error)
            })?;
        }
        Ok(())
    }

    async fn remove_registered_temporary(&self, path: &Path) -> Result<(), FileOperationError> {
        self.delegate.remove_registered_temporary(path).await
    }
}

#[tokio::test]
async fn rename_replace_revalidates_destination_after_staging_source() {
    let mutation = Arc::new(RewriteDestinationAfterSourceStage {
        delegate: LocalFileMutation,
        destination: Mutex::new(None),
    });
    let fixture = Fixture::with_mutation(false, mutation.clone());
    fixture.directory("products");
    let source = fixture.file("products/source.png", b"source");
    fixture.file("products/destination.png", b"AAAA");
    let destination_path = fixture.project.root().join("products/destination.png");
    *mutation.destination.lock().unwrap() = Some(destination_path.clone());
    let command = fixture.command(
        FileCommandKind::Rename,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Rename {
                proposed_name: "destination.png".into(),
                edit_extension: true,
            },
        }],
    );
    let preflight = fixture.service.preflight(command).await.unwrap();
    assert_eq!(
        preflight.rows()[0].state,
        FileCommandPreflightState::Conflict
    );

    let summary = fixture
        .service
        .execute(
            preflight,
            &[ConflictResolution {
                entity_id: source,
                policy: ConflictPolicy::Replace,
                apply_to_remaining: false,
            }],
            None,
        )
        .await
        .unwrap();

    assert_eq!(summary.failed(), 1);
    assert_eq!(
        summary.result_page(0, 1).items[0].code,
        BatchResultCode::VerificationFailed
    );
    assert_eq!(fs::read(destination_path).unwrap(), b"BBBB");
    assert_eq!(
        fs::read(fixture.project.root().join("products/source.png")).unwrap(),
        b"source"
    );
    assert_eq!(fixture.trash.count(), 0);
}

struct BlockingCopy {
    delegate: LocalFileMutation,
    started: Notify,
}

struct BlockingFirstRename {
    delegate: LocalFileMutation,
    started: Notify,
    release: Notify,
    rename_count: Mutex<usize>,
}

#[async_trait]
impl FileMutationPort for BlockingFirstRename {
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
        let first = {
            let mut count = self.rename_count.lock().unwrap();
            *count += 1;
            *count == 1
        };
        if first {
            self.started.notify_one();
            self.release.notified().await;
        }
        self.delegate.rename(source, destination).await
    }

    async fn remove_registered_temporary(&self, path: &Path) -> Result<(), FileOperationError> {
        self.delegate.remove_registered_temporary(path).await
    }
}

#[tokio::test]
async fn rename_cancellation_stops_not_started_independent_components() {
    let mutation = Arc::new(BlockingFirstRename {
        delegate: LocalFileMutation,
        started: Notify::new(),
        release: Notify::new(),
        rename_count: Mutex::new(0),
    });
    let fixture = Fixture::with_mutation(false, mutation.clone());
    fixture.directory("products");
    let first = fixture.file("products/first.png", b"first");
    let second = fixture.file("products/second.png", b"second");
    let command = fixture.command(
        FileCommandKind::Rename,
        vec![
            FileCommandItem {
                entity_id: first,
                action: FileCommandAction::Rename {
                    proposed_name: "first-renamed.png".into(),
                    edit_extension: true,
                },
            },
            FileCommandItem {
                entity_id: second,
                action: FileCommandAction::Rename {
                    proposed_name: "second-renamed.png".into(),
                    edit_extension: true,
                },
            },
        ],
    );
    let preflight = fixture.service.preflight(command).await.unwrap();
    let batch_id = preflight.batch_id();
    let service = Arc::clone(&fixture.service);
    let execution = tokio::spawn(async move { service.execute(preflight, &[], None).await });
    mutation.started.notified().await;

    assert!(fixture.service.cancel_pending(batch_id));
    mutation.release.notify_one();
    let summary = execution.await.unwrap().unwrap();

    assert_eq!((summary.completed(), summary.cancelled()), (1, 1));
    assert_eq!(
        fs::read(fixture.project.root().join("products/first-renamed.png")).unwrap(),
        b"first"
    );
    assert_eq!(
        fs::read(fixture.project.root().join("products/second.png")).unwrap(),
        b"second"
    );
    assert!(
        !fixture
            .project
            .root()
            .join("products/second-renamed.png")
            .exists(),
        "cancellation must be observed between independent rename components"
    );
    assert_eq!(*mutation.rename_count.lock().unwrap(), 1);
}

#[async_trait]
impl FileMutationPort for BlockingCopy {
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

    async fn copy_and_hash_cancellable(
        &self,
        _source: &Path,
        _temporary: &Path,
        cancellation: &FileCommandCancellation,
    ) -> Result<(u64, [u8; 32]), FileOperationError> {
        self.started.notify_one();
        while !cancellation.is_cancelled() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        Err(FileOperationError::Cancelled)
    }

    async fn rename(&self, source: &Path, destination: &Path) -> Result<(), FileOperationError> {
        self.delegate.rename(source, destination).await
    }

    async fn remove_registered_temporary(&self, path: &Path) -> Result<(), FileOperationError> {
        self.delegate.remove_registered_temporary(path).await
    }
}

#[tokio::test]
async fn cancelling_an_active_stream_copy_cleans_temporary_and_settles_the_batch() {
    let mutation = Arc::new(BlockingCopy {
        delegate: LocalFileMutation,
        started: Notify::new(),
    });
    let fixture = Fixture::with_mutation(false, mutation.clone());
    fixture.directory("source");
    let destination = fixture.directory("exports");
    let source = fixture.file("source/item.png", &[7_u8; 4096]);
    let command = fixture.command(
        FileCommandKind::Copy,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Copy {
                destination_folder: destination,
            },
        }],
    );
    let preflight = fixture.service.preflight(command).await.unwrap();
    let batch_id = preflight.batch_id();
    let service = Arc::clone(&fixture.service);
    let execution = tokio::spawn(async move { service.execute(preflight, &[], None).await });
    mutation.started.notified().await;

    assert!(fixture.service.cancel_pending(batch_id));
    let summary = execution.await.unwrap().unwrap();

    assert_eq!(summary.cancelled(), 1);
    assert!(fixture.project.root().join("source/item.png").is_file());
    assert!(!fixture.project.root().join("exports/item.png").exists());
    assert!(
        fs::read_dir(fixture.project.root().join("exports"))
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".viewer-copy-"))
    );
    assert_eq!(
        fixture
            .journal
            .batch(batch_id)
            .unwrap()
            .unwrap()
            .skipped_count,
        1
    );
}

#[tokio::test]
async fn symlinked_source_is_blocked_before_any_durable_intent() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new(false);
    fixture.directory("source");
    let outside = tempfile::NamedTempFile::new().unwrap();
    let link = fixture.project.root().join("source/link.png");
    symlink(outside.path(), &link).unwrap();
    let source = fixture.index_node("source/link.png", FileKind::Png);
    let command = fixture.command(
        FileCommandKind::Trash,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Trash,
        }],
    );

    let preflight = fixture.service.preflight(command).await.unwrap();

    assert!(!preflight.is_executable());
    assert_eq!(
        preflight.rows()[0].state,
        FileCommandPreflightState::Blocked(BatchResultCode::InvalidTarget)
    );
    assert!(link.is_symlink());
    assert!(
        fixture
            .journal
            .batch(preflight.batch_id())
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn destination_parent_replaced_by_symlink_cannot_escape_the_project() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new(false);
    fixture.directory("source");
    let destination = fixture.directory("exports");
    let source = fixture.file("source/item.png", b"source");
    let command = fixture.command(
        FileCommandKind::Copy,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Copy {
                destination_folder: destination,
            },
        }],
    );
    let preflight = fixture.service.preflight(command).await.unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::rename(
        fixture.project.root().join("exports"),
        fixture.project.root().join("exports-original"),
    )
    .unwrap();
    symlink(outside.path(), fixture.project.root().join("exports")).unwrap();

    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();

    assert_eq!(summary.failed(), 1);
    assert_eq!(
        summary.result_page(0, 1).items[0].code,
        BatchResultCode::InvalidTarget
    );
    assert!(outside.path().read_dir().unwrap().next().is_none());
    assert!(fixture.project.root().join("source/item.png").is_file());
}

struct FailOneCopy {
    delegate: LocalFileMutation,
}

#[async_trait]
impl FileMutationPort for FailOneCopy {
    async fn snapshot(&self, path: &Path) -> Result<FileSnapshot, FileOperationError> {
        self.delegate.snapshot(path).await
    }

    async fn copy_and_hash(
        &self,
        source: &Path,
        temporary: &Path,
    ) -> Result<(u64, [u8; 32]), FileOperationError> {
        let name = source.file_name().and_then(|name| name.to_str());
        if name == Some("bad.png") {
            return Err(FileOperationError::Io {
                action: "injected disk failure",
                path: temporary.to_path_buf(),
                message: "disk full".into(),
            });
        }
        if name == Some("denied.png") {
            return Err(FileOperationError::Io {
                action: "injected permission loss",
                path: temporary.to_path_buf(),
                message: "Permission denied".into(),
            });
        }
        self.delegate.copy_and_hash(source, temporary).await
    }

    async fn rename(&self, source: &Path, destination: &Path) -> Result<(), FileOperationError> {
        self.delegate.rename(source, destination).await
    }

    async fn remove_registered_temporary(&self, path: &Path) -> Result<(), FileOperationError> {
        self.delegate.remove_registered_temporary(path).await
    }
}
