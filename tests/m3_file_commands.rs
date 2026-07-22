use async_trait::async_trait;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Mutex as AsyncMutex, Notify};
use viewer_application::{
    BrowseIndexPort, CommitStage, FileContentEvidence, FileMutationPort, FileOperationError,
    FileSnapshot, OperationCommit, OperationCommitError, OperationCommitPort, ProjectAccess,
    TrashPort, VolumePort,
    file_commands::{
        BatchResultCode, ConflictResolution, FileCommand, FileCommandAction,
        FileCommandCancellation, FileCommandItem, FileCommandKind, FileCommandPreflightState,
        FileCommandService,
    },
    metadata::{
        FileMoveProjection, MarkerChange, MarkerProjectionPort, MarkerTarget,
        OperationProjectionPort,
    },
    scheduler::TaskCoordinator,
    undo::{UndoAction, UndoFilePort, UndoStack},
    watcher::FileIdentity,
};
use viewer_domain::{
    EntityId, RelativePath, SessionId,
    file::{FileKind, FileNode, Marker, ReviewState},
    operation::{ConflictPolicy, OperationKind, OperationState},
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
    search::{index::SessionIndex, text::TextStatus},
};
use viewer_test_support::{
    FixedClock, operation_commits::InMemoryOperationCommitPort, project_fixture::ProjectFixture,
};

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
    swap_before_verified: Mutex<Option<(PathBuf, PathBuf)>>,
    fail_next_verified: AtomicBool,
}

#[derive(Default)]
struct RekeyingCommitPort {
    index: Mutex<Option<Arc<SessionIndex>>>,
    root: Mutex<Option<PathBuf>>,
}

impl RekeyingCommitPort {
    fn attach(&self, index: Arc<SessionIndex>, root: PathBuf) {
        *self.index.lock().unwrap() = Some(index);
        *self.root.lock().unwrap() = Some(root);
    }
}

#[async_trait]
impl OperationCommitPort for RekeyingCommitPort {
    async fn commit_metadata(&self, _commit: &OperationCommit) -> Result<(), OperationCommitError> {
        Ok(())
    }

    async fn sync_index(&self, commit: &OperationCommit) -> Result<(), OperationCommitError> {
        let index =
            self.index.lock().unwrap().clone().ok_or_else(|| {
                OperationCommitError::new(CommitStage::Index, "index_unavailable")
            })?;
        let source = index
            .node(commit.entity_id)
            .map_err(|_| OperationCommitError::new(CommitStage::Index, "index_unavailable"))?
            .ok_or_else(|| OperationCommitError::new(CommitStage::Index, "projection_stale"))?;
        let destination_path = commit
            .destination
            .clone()
            .ok_or_else(|| OperationCommitError::new(CommitStage::Index, "destination_missing"))?;
        let root =
            self.root.lock().unwrap().clone().ok_or_else(|| {
                OperationCommitError::new(CommitStage::Index, "index_unavailable")
            })?;
        let metadata = fs::metadata(root.join(destination_path.as_str())).map_err(|_| {
            OperationCommitError::new(CommitStage::Index, "destination_unavailable")
        })?;
        #[cfg(unix)]
        let destination_id = {
            use std::os::unix::fs::MetadataExt;
            EntityId::from_u128((u128::from(metadata.dev()) << 64) | u128::from(metadata.ino()))
        };
        #[cfg(not(unix))]
        let destination_id = EntityId::new();
        let destination = FileNode {
            entity_id: destination_id,
            relative_path: destination_path,
            kind: source.kind,
            size: metadata.len(),
            modified_ns: 1,
        };
        index
            .apply_move(
                &[FileMoveProjection {
                    source,
                    destination,
                }],
                true,
            )
            .map_err(|_| OperationCommitError::new(CommitStage::Index, "projection_stale"))
    }
}

impl FakeTrash {
    fn new() -> Self {
        Self {
            directory: tempfile::tempdir().unwrap(),
            trashed: Mutex::new(Vec::new()),
            swap_before_verified: Mutex::new(None),
            fail_next_verified: AtomicBool::new(false),
        }
    }

    fn count(&self) -> usize {
        self.trashed.lock().unwrap().len()
    }

    fn swap_before_verified(&self, parked_original: PathBuf, replacement: PathBuf) {
        *self.swap_before_verified.lock().unwrap() = Some((parked_original, replacement));
    }

    fn fail_next_verified(&self) {
        self.fail_next_verified.store(true, Ordering::Release);
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

    async fn trash_verified(
        &self,
        path: &Path,
        expected: &FileSnapshot,
        expected_parent: FileIdentity,
    ) -> Result<(), FileOperationError> {
        if let Some((parked, replacement)) = self.swap_before_verified.lock().unwrap().take() {
            fs::rename(path, &parked).map_err(|error| {
                FileOperationError::io("park Trash candidate in race hook", path, &error)
            })?;
            fs::hard_link(&replacement, path).map_err(|error| {
                FileOperationError::io("replace Trash candidate in race hook", path, &error)
            })?;
        }
        let parent = fs::metadata(path.parent().ok_or(FileOperationError::OutsideProject)?)
            .map_err(|error| FileOperationError::io("inspect fake Trash parent", path, &error))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if parent.dev() != expected_parent.volume || parent.ino() != expected_parent.file {
                return Err(FileOperationError::IdentityChanged);
            }
        }
        let actual = LocalFileMutation.snapshot(path).await?;
        if actual.volume_id != expected.volume_id
            || actual.file_id != expected.file_id
            || actual.len != expected.len
            || actual.modified_ns != expected.modified_ns
        {
            return Err(FileOperationError::IdentityChanged);
        }
        if self.fail_next_verified.swap(false, Ordering::AcqRel) {
            return Err(FileOperationError::Io {
                action: "injected verified Trash failure",
                path: path.to_path_buf(),
                message: "injected failure".into(),
            });
        }
        self.trash(path).await
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
    undo_stack: Arc<Mutex<UndoStack>>,
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
        let undo_stack = Arc::new(Mutex::new(UndoStack::new(session_id)));
        let service = Arc::new(FileCommandService::new_with_undo(
            session_id,
            ProjectAccess::ReadWrite,
            coordinator,
            adapter.clone(),
            Arc::new(AsyncMutex::new(())),
            Arc::clone(&undo_stack),
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
            undo_stack,
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

#[tokio::test]
async fn cross_volume_move_undo_tracks_each_current_filesystem_entity_and_rekeys_projection_twice()
{
    let commits = Arc::new(RekeyingCommitPort::default());
    let fixture = Fixture::with_ports(true, Arc::new(LocalFileMutation), commits.clone());
    commits.attach(fixture.index.clone(), fixture.project.root().to_path_buf());
    fixture.directory("source");
    let destination_folder = fixture.directory("exports");
    let original_entity = fixture.file("source/item.txt", b"searchable body");
    let source_path = RelativePath::parse("source/item.txt").unwrap();
    fixture
        .index
        .replace_text(
            original_entity,
            &source_path,
            &TextStatus::Indexed("searchable body".into()),
        )
        .unwrap();
    let source_node = fixture.index.node(original_entity).unwrap().unwrap();
    fixture
        .index
        .sync_markers(&[MarkerChange {
            target: MarkerTarget {
                entity_id: original_entity,
                relative_path: source_path.clone(),
                kind: source_node.kind,
                size: source_node.size,
                modified_ns: source_node.modified_ns,
            },
            marker: Marker {
                review_state: Some(ReviewState::Keep),
                favorite: true,
            },
        }])
        .unwrap();
    let command = fixture.command(
        FileCommandKind::Move,
        vec![FileCommandItem {
            entity_id: original_entity,
            action: FileCommandAction::Move { destination_folder },
        }],
    );
    let preflight = fixture.service.preflight(command).await.unwrap();

    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();

    assert_eq!(summary.completed(), 1);
    let moved = fixture
        .index
        .node_by_relative_path(&RelativePath::parse("exports/item.txt").unwrap())
        .unwrap()
        .expect("forward destination is projected");
    assert_ne!(moved.entity_id, original_entity);
    assert!(fixture.index.node(original_entity).unwrap().is_none());
    let undo = fixture
        .undo_stack
        .lock()
        .unwrap()
        .last()
        .expect("forward move records undo");
    let UndoAction::File { entity_id, .. } = undo.actions[0] else {
        panic!("move undo must contain a file action")
    };
    assert_eq!(
        entity_id, moved.entity_id,
        "undo targets the current destination entity"
    );

    fixture
        .adapter
        .reverse_batch(fixture.project.root(), &undo)
        .await
        .unwrap();

    let restored = fixture
        .index
        .node_by_relative_path(&source_path)
        .unwrap()
        .expect("reverse destination is projected");
    assert_ne!(restored.entity_id, moved.entity_id);
    assert!(fixture.index.node(moved.entity_id).unwrap().is_none());
    assert_eq!(
        fs::read(fixture.project.root().join("source/item.txt")).unwrap(),
        b"searchable body"
    );
    assert!(!fixture.project.root().join("exports/item.txt").exists());
    let indexed = fixture
        .index
        .indexed_node(restored.entity_id)
        .unwrap()
        .expect("rekeyed node retains derived projection");
    assert!(matches!(
        indexed.text_status,
        viewer_domain::file::TextIndexStatus::Ready
    ));
    assert_eq!(
        indexed.marker,
        Marker {
            review_state: Some(ReviewState::Keep),
            favorite: true,
        },
        "marker projection follows the twice-rekeyed file"
    );
    let connection =
        rusqlite::Connection::open(fixture.project.root().join(".viewer/session.sqlite")).unwrap();
    let fts_row = connection
        .query_row(
            "SELECT entity_id, relative_path, body FROM text_fts WHERE entity_id = ?1",
            [restored.entity_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(
        fts_row,
        (
            restored.entity_id.to_string(),
            "source/item.txt".into(),
            "searchable body".into(),
        ),
        "FTS identity and path follow the twice-rekeyed file"
    );
    assert!(fixture.journal.incomplete_items().unwrap().is_empty());
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

struct ReplaceTemporaryAfterBoundCopy {
    delegate: LocalFileMutation,
    outside_victim: PathBuf,
    primary: FileOperationError,
}

#[async_trait]
impl FileMutationPort for ReplaceTemporaryAfterBoundCopy {
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

    async fn create_and_copy_cancellable_verified(
        &self,
        source: &Path,
        temporary: &Path,
        cancellation: &FileCommandCancellation,
        expected_source: &FileSnapshot,
        source_parent: FileIdentity,
        temporary_parent: FileIdentity,
    ) -> Result<FileContentEvidence, FileOperationError> {
        let _evidence = self
            .delegate
            .create_and_copy_cancellable_verified(
                source,
                temporary,
                cancellation,
                expected_source,
                source_parent,
                temporary_parent,
            )
            .await?;
        fs::remove_file(temporary).map_err(|error| {
            FileOperationError::io("remove copied temporary in race hook", temporary, &error)
        })?;
        fs::hard_link(&self.outside_victim, temporary).map_err(|error| {
            FileOperationError::io("replace copied temporary in race hook", temporary, &error)
        })?;
        Err(FileOperationError::RegisteredTemporaryCleanupRequired {
            primary: Box::new(self.primary.clone()),
            cleanup: Box::new(FileOperationError::IdentityChanged),
        })
    }

    async fn rename(&self, source: &Path, destination: &Path) -> Result<(), FileOperationError> {
        self.delegate.rename(source, destination).await
    }
}

#[cfg(unix)]
#[tokio::test]
async fn copy_leaf_replacement_retains_a_terminal_idempotent_cleanup_obligation() {
    let outside = tempfile::tempdir().unwrap();
    let outside_victim = outside.path().join("victim.bin");
    fs::write(&outside_victim, b"outside sentinel").unwrap();
    let fixture = Fixture::with_mutation(
        false,
        Arc::new(ReplaceTemporaryAfterBoundCopy {
            delegate: LocalFileMutation,
            outside_victim: outside_victim.clone(),
            primary: FileOperationError::IdentityChanged,
        }),
    );
    fixture.directory("source");
    let destination = fixture.directory("exports");
    let source = fixture.file("source/item.png", b"selected bytes");
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

    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();

    assert_eq!(summary.failed(), 1);
    assert_eq!(fs::read(&outside_victim).unwrap(), b"outside sentinel");
    let source_bytes = fs::read(fixture.project.root().join("source/item.png")).unwrap();
    assert_eq!(source_bytes, b"selected bytes");
    assert_eq!(blake3::hash(&source_bytes), blake3::hash(b"selected bytes"));
    assert!(!fixture.project.root().join("exports/item.png").exists());
    assert_eq!(fixture.trash.count(), 0);
    let batch = fixture.journal.batch(batch_id).unwrap().unwrap();
    assert_eq!(batch.state, BatchState::Completed);
    assert_eq!((batch.completed_count, batch.failed_count), (0, 1));
    let obligation = fixture
        .journal
        .incomplete_items()
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.batch_id == batch_id)
        .expect("terminal copy failure retains its cleanup obligation");
    assert_eq!(obligation.state, OperationState::Failed);
    assert!(obligation.temporary.is_some());
    assert_eq!(
        obligation.result_code.as_deref(),
        Some(BatchResultCode::VerificationFailed.as_str())
    );
    assert_eq!(
        summary.result_page(0, 1).items[0].code,
        BatchResultCode::VerificationFailed
    );

    for _ in 0..2 {
        let reopened = Arc::new(OperationJournal::open(fixture.project.metadata_path()).unwrap());
        let recovery = RecoveryService::new(
            fixture.project.root(),
            Arc::clone(&reopened),
            Arc::new(LocalFileMutation),
            fixture.trash.clone(),
            Arc::new(FixedClock::new(20_000)),
            Arc::new(InMemoryOperationCommitPort::default()),
        )
        .unwrap();
        let report = recovery.recover_project().await.unwrap();
        assert_eq!(report.actions, Vec::new());
        assert_eq!(report.needs_user_review.len(), 1);
        assert_eq!(fs::read(&outside_victim).unwrap(), b"outside sentinel");
        assert_eq!(
            reopened.batch(batch_id).unwrap().unwrap().state,
            BatchState::Completed
        );
    }

    fs::remove_file(
        fixture
            .project
            .root()
            .join(obligation.temporary.as_ref().unwrap().as_str()),
    )
    .unwrap();
    let reopened = Arc::new(OperationJournal::open(fixture.project.metadata_path()).unwrap());
    let recovery = RecoveryService::new(
        fixture.project.root(),
        Arc::clone(&reopened),
        Arc::new(LocalFileMutation),
        fixture.trash.clone(),
        Arc::new(FixedClock::new(30_000)),
        Arc::new(InMemoryOperationCommitPort::default()),
    )
    .unwrap();
    let report = recovery.recover_project().await.unwrap();
    assert_eq!(report.actions.len(), 1);
    assert_eq!(report.needs_user_review, Vec::new());
    assert!(reopened.incomplete_items().unwrap().is_empty());
    let terminal = reopened.item(obligation.operation_id).unwrap().unwrap();
    assert_eq!(terminal.state, OperationState::Failed);
    assert_eq!(
        terminal.result_code.as_deref(),
        Some(BatchResultCode::VerificationFailed.as_str())
    );
    assert_eq!(terminal.temporary, None);
    assert_eq!(reopened.batch(batch_id).unwrap().unwrap().failed_count, 1);
}

#[cfg(unix)]
#[tokio::test]
async fn cancelled_copy_cleanup_obligation_preserves_cancelled_result_and_skipped_count() {
    let outside = tempfile::tempdir().unwrap();
    let outside_victim = outside.path().join("victim.bin");
    fs::write(&outside_victim, b"outside sentinel").unwrap();
    let fixture = Fixture::with_mutation(
        false,
        Arc::new(ReplaceTemporaryAfterBoundCopy {
            delegate: LocalFileMutation,
            outside_victim: outside_victim.clone(),
            primary: FileOperationError::Cancelled,
        }),
    );
    fixture.directory("source");
    let destination = fixture.directory("exports");
    let source = fixture.file("source/item.png", b"selected bytes");
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

    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();

    assert_eq!((summary.failed(), summary.cancelled()), (0, 1));
    assert_eq!(
        summary.result_page(0, 1).items[0].code,
        BatchResultCode::Cancelled
    );
    assert_eq!(fs::read(&outside_victim).unwrap(), b"outside sentinel");
    let batch = fixture.journal.batch(batch_id).unwrap().unwrap();
    assert_eq!(batch.state, BatchState::Completed);
    assert_eq!((batch.failed_count, batch.skipped_count), (0, 1));
    let obligation = fixture
        .journal
        .incomplete_items()
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.batch_id == batch_id)
        .unwrap();
    assert_eq!(obligation.state, OperationState::Completed);
    assert_eq!(
        obligation.result_code.as_deref(),
        Some(BatchResultCode::Cancelled.as_str())
    );
    assert!(obligation.temporary.is_some());
}

struct SwapSourceBeforeMutationSnapshot {
    delegate: LocalFileMutation,
    swap: Mutex<Option<(PathBuf, PathBuf)>>,
}

#[async_trait]
impl FileMutationPort for SwapSourceBeforeMutationSnapshot {
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

    async fn rename_verified(
        &self,
        source: &Path,
        destination: &Path,
        expected: &FileSnapshot,
        source_parent: FileIdentity,
        destination_parent: FileIdentity,
    ) -> Result<(), FileOperationError> {
        let paths = {
            let mut swap = self.swap.lock().unwrap();
            if swap
                .as_ref()
                .is_some_and(|(candidate, _)| candidate.as_path() == source)
            {
                swap.take()
            } else {
                None
            }
        };
        if let Some((swapped_source, parked_original)) = paths {
            fs::rename(&swapped_source, &parked_original).map_err(|error| {
                FileOperationError::io("park selected source", &swapped_source, &error)
            })?;
            fs::write(&swapped_source, b"replacement").map_err(|error| {
                FileOperationError::io("install replacement source", &swapped_source, &error)
            })?;
        }
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

#[tokio::test]
async fn trash_rejects_a_source_replacement_before_safe_isolation() {
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
        FileCommandKind::Trash,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Trash,
        }],
    );
    let preflight = fixture.service.preflight(command).await.unwrap();

    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();

    assert_eq!(summary.failed(), 1);
    assert_eq!(fs::read(&source_path).unwrap(), b"replacement");
    assert_eq!(fs::read(&parked_original).unwrap(), b"selected");
    assert_eq!(fixture.trash.count(), 0);
}

#[cfg(unix)]
#[tokio::test]
async fn verified_trash_rejects_internal_leaf_swaps_for_direct_copy_replace_and_rename_replace() {
    for scenario in ["direct", "copy_replace", "rename_replace"] {
        let outside = tempfile::tempdir().unwrap();
        let outside_victim = outside.path().join("victim.bin");
        fs::write(&outside_victim, b"outside sentinel").unwrap();
        let fixture = Fixture::new(false);
        let products = fixture.directory("products");
        fixture.directory("source");
        let parked = fixture
            .project
            .root()
            .join(format!("parked-{scenario}.bin"));

        let (kind, item, conflicts, selected_source) = match scenario {
            "direct" => {
                let source = fixture.file("products/source.png", b"selected source");
                (
                    FileCommandKind::Trash,
                    FileCommandItem {
                        entity_id: source,
                        action: FileCommandAction::Trash,
                    },
                    Vec::new(),
                    fixture.project.root().join("products/source.png"),
                )
            }
            "copy_replace" => {
                let source = fixture.file("source/item.png", b"selected source");
                let destination = fixture.file("products/item.png", b"original destination");
                let _ = destination;
                (
                    FileCommandKind::Copy,
                    FileCommandItem {
                        entity_id: source,
                        action: FileCommandAction::Copy {
                            destination_folder: products,
                        },
                    },
                    vec![ConflictResolution {
                        entity_id: source,
                        policy: ConflictPolicy::Replace,
                        apply_to_remaining: false,
                    }],
                    fixture.project.root().join("source/item.png"),
                )
            }
            _ => {
                let source = fixture.file("products/source.png", b"selected source");
                fixture.file("products/destination.png", b"original destination");
                (
                    FileCommandKind::Rename,
                    FileCommandItem {
                        entity_id: source,
                        action: FileCommandAction::Rename {
                            proposed_name: "destination.png".into(),
                            edit_extension: true,
                        },
                    },
                    vec![ConflictResolution {
                        entity_id: source,
                        policy: ConflictPolicy::Replace,
                        apply_to_remaining: false,
                    }],
                    fixture.project.root().join("products/source.png"),
                )
            }
        };
        fixture
            .trash
            .swap_before_verified(parked.clone(), outside_victim.clone());
        let command = fixture.command(kind, vec![item]);
        let preflight = fixture.service.preflight(command).await.unwrap();
        let summary = fixture
            .service
            .execute(preflight, &conflicts, None)
            .await
            .unwrap();

        assert_eq!(summary.failed(), 1, "scenario {scenario}");
        assert_eq!(fs::read(&outside_victim).unwrap(), b"outside sentinel");
        assert_eq!(fixture.trash.count(), 0);
        assert!(parked.is_file());
        if scenario == "copy_replace" {
            assert_eq!(fs::read(&selected_source).unwrap(), b"selected source");
        } else if scenario == "rename_replace" {
            let persisted = fixture
                .journal
                .incomplete_items()
                .unwrap()
                .into_iter()
                .find(|item| item.kind == OperationKind::Rename)
                .unwrap();
            assert_eq!(persisted.state, OperationState::Staged);
            assert_eq!(
                fs::read(
                    fixture
                        .project
                        .root()
                        .join(persisted.temporary.unwrap().as_str()),
                )
                .unwrap(),
                b"selected source"
            );
        }
    }
}

#[tokio::test]
async fn post_isolation_trash_failure_remains_recovery_required_and_restores_on_reopen() {
    let fixture = Fixture::new(false);
    fixture.directory("products");
    let source = fixture.file("products/source.png", b"selected source");
    fixture.trash.fail_next_verified();
    let command = fixture.command(
        FileCommandKind::Trash,
        vec![FileCommandItem {
            entity_id: source,
            action: FileCommandAction::Trash,
        }],
    );
    let preflight = fixture.service.preflight(command).await.unwrap();

    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();

    assert_eq!(summary.failed(), 1);
    let persisted = fixture
        .journal
        .incomplete_items()
        .unwrap()
        .into_iter()
        .find(|item| item.kind == OperationKind::Trash)
        .expect("post-isolation failure must remain in the recovery query");
    assert_eq!(persisted.state, OperationState::Staged);
    assert!(!fixture.project.root().join("products/source.png").exists());
    assert!(
        fixture
            .project
            .root()
            .join(persisted.temporary.as_ref().unwrap().as_str())
            .is_file()
    );

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

    assert_eq!(report.actions[0].kind, RecoveryActionKind::RestoredSource);
    assert_eq!(
        fs::read(fixture.project.root().join("products/source.png")).unwrap(),
        b"selected source"
    );
    assert_eq!(
        fixture
            .journal
            .item(persisted.operation_id)
            .unwrap()
            .unwrap()
            .state,
        OperationState::Failed
    );
}

#[tokio::test]
async fn rename_replace_rejects_a_source_replacement_before_isolation() {
    let mutation = Arc::new(SwapSourceBeforeMutationSnapshot {
        delegate: LocalFileMutation,
        swap: Mutex::new(None),
    });
    let fixture = Fixture::with_mutation(false, mutation.clone());
    fixture.directory("products");
    let source = fixture.file("products/source.png", b"selected");
    fixture.file("products/destination.png", b"destination");
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
                proposed_name: "destination.png".into(),
                edit_extension: true,
            },
        }],
    );
    let preflight = fixture.service.preflight(command).await.unwrap();

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
    assert_eq!(fs::read(&source_path).unwrap(), b"replacement");
    assert_eq!(fs::read(&parked_original).unwrap(), b"selected");
    assert_eq!(
        fs::read(fixture.project.root().join("products/destination.png")).unwrap(),
        b"destination"
    );
    assert_eq!(fixture.trash.count(), 0);
}

struct ReplaceDestinationParentBeforeCreate {
    delegate: LocalFileMutation,
    swap: Mutex<Option<(PathBuf, PathBuf, Option<PathBuf>)>>,
}

struct ReplaceDestinationParentAfterCreate {
    delegate: LocalFileMutation,
    swap: Mutex<Option<(PathBuf, PathBuf, PathBuf)>>,
    outside_temporary: Mutex<Option<PathBuf>>,
}

#[async_trait]
impl FileMutationPort for ReplaceDestinationParentAfterCreate {
    async fn create_and_copy_cancellable_verified(
        &self,
        source: &Path,
        temporary: &Path,
        cancellation: &FileCommandCancellation,
        expected_source: &FileSnapshot,
        source_parent: FileIdentity,
        temporary_parent: FileIdentity,
    ) -> Result<FileContentEvidence, FileOperationError> {
        self.create_registered_temporary(temporary, temporary_parent)
            .await?;
        let copied = self
            .delegate
            .copy_and_hash_cancellable_verified(
                source,
                temporary,
                cancellation,
                expected_source,
                source_parent,
                temporary_parent,
            )
            .await;
        match copied {
            Ok((len, hash)) => {
                let snapshot = self.delegate.snapshot(temporary).await?;
                if snapshot.len != len {
                    return Err(FileOperationError::IdentityChanged);
                }
                Ok(FileContentEvidence { snapshot, hash })
            }
            Err(primary) => Err(FileOperationError::RegisteredTemporaryCleanupRequired {
                primary: Box::new(primary),
                cleanup: Box::new(FileOperationError::IdentityChanged),
            }),
        }
    }

    async fn snapshot(&self, path: &Path) -> Result<FileSnapshot, FileOperationError> {
        self.delegate.snapshot(path).await
    }

    async fn create_registered_temporary(
        &self,
        path: &Path,
        expected_parent: FileIdentity,
    ) -> Result<(), FileOperationError> {
        self.delegate
            .create_registered_temporary(path, expected_parent)
            .await?;
        if let Some((parent, parked, outside)) = self.swap.lock().unwrap().take() {
            fs::rename(&parent, &parked).map_err(|error| {
                FileOperationError::io("park destination directory", &parent, &error)
            })?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(&outside, &parent).map_err(|error| {
                FileOperationError::io("install destination symlink", &parent, &error)
            })?;
            let outside_temporary = outside.join(path.file_name().unwrap());
            fs::write(&outside_temporary, b"outside sentinel").map_err(|error| {
                FileOperationError::io("create outside sentinel", &outside_temporary, &error)
            })?;
            *self.outside_temporary.lock().unwrap() = Some(outside_temporary);
        }
        Ok(())
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
}

#[async_trait]
impl FileMutationPort for ReplaceDestinationParentBeforeCreate {
    async fn create_and_copy_cancellable_verified(
        &self,
        source: &Path,
        temporary: &Path,
        cancellation: &FileCommandCancellation,
        expected_source: &FileSnapshot,
        source_parent: FileIdentity,
        temporary_parent: FileIdentity,
    ) -> Result<FileContentEvidence, FileOperationError> {
        if let Some((parent, parked, symlink_target)) = self.swap.lock().unwrap().take() {
            fs::rename(&parent, &parked).map_err(|error| {
                FileOperationError::io("park destination directory", &parent, &error)
            })?;
            if let Some(target) = symlink_target {
                #[cfg(unix)]
                std::os::unix::fs::symlink(&target, &parent).map_err(|error| {
                    FileOperationError::io("install destination symlink", &parent, &error)
                })?;
            } else {
                fs::create_dir(&parent).map_err(|error| {
                    FileOperationError::io("install replacement directory", &parent, &error)
                })?;
            }
        }
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

    async fn create_registered_temporary(
        &self,
        path: &Path,
        expected_parent: FileIdentity,
    ) -> Result<(), FileOperationError> {
        if let Some((parent, parked, symlink_target)) = self.swap.lock().unwrap().take() {
            fs::rename(&parent, &parked).map_err(|error| {
                FileOperationError::io("park destination directory", &parent, &error)
            })?;
            if let Some(target) = symlink_target {
                #[cfg(unix)]
                std::os::unix::fs::symlink(&target, &parent).map_err(|error| {
                    FileOperationError::io("install destination symlink", &parent, &error)
                })?;
            } else {
                fs::create_dir(&parent).map_err(|error| {
                    FileOperationError::io("install replacement directory", &parent, &error)
                })?;
            }
        }
        self.delegate
            .create_registered_temporary(path, expected_parent)
            .await
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
}

#[tokio::test]
async fn copy_rejects_an_ordinary_destination_folder_replacement_before_create() {
    let mutation = Arc::new(ReplaceDestinationParentBeforeCreate {
        delegate: LocalFileMutation,
        swap: Mutex::new(None),
    });
    let fixture = Fixture::with_mutation(false, mutation.clone());
    fixture.directory("source");
    let destination = fixture.directory("exports");
    let source = fixture.file("source/item.png", b"source");
    let parent = fixture.project.root().join("exports");
    let parked = fixture.project.root().join("exports-original");
    *mutation.swap.lock().unwrap() = Some((parent.clone(), parked.clone(), None));
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

    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();

    assert_eq!(summary.failed(), 1);
    assert!(!parent.join("item.png").exists());
    assert!(!parked.join("item.png").exists());
    assert_eq!(
        fs::read(fixture.project.root().join("source/item.png")).unwrap(),
        b"source"
    );
    assert_eq!(fixture.trash.count(), 0);
}

#[tokio::test]
async fn copy_cleanup_cannot_follow_a_replaced_destination_parent_outside_the_project() {
    let outside = tempfile::tempdir().unwrap();
    let mutation = Arc::new(ReplaceDestinationParentAfterCreate {
        delegate: LocalFileMutation,
        swap: Mutex::new(None),
        outside_temporary: Mutex::new(None),
    });
    let fixture = Fixture::with_mutation(false, mutation.clone());
    fixture.directory("source");
    let destination = fixture.directory("exports");
    let source = fixture.file("source/item.png", b"source");
    let parent = fixture.project.root().join("exports");
    let parked = fixture.project.root().join("exports-original");
    *mutation.swap.lock().unwrap() = Some((parent, parked, outside.path().to_path_buf()));
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

    let summary = fixture.service.execute(preflight, &[], None).await.unwrap();

    assert_eq!(summary.failed(), 1);
    let outside_temporary = mutation
        .outside_temporary
        .lock()
        .unwrap()
        .clone()
        .expect("race hook created an outside sentinel");
    assert_eq!(
        fs::read(outside_temporary).unwrap(),
        b"outside sentinel",
        "cleanup must remain bound to the validated destination directory"
    );
    assert_eq!(
        fs::read(fixture.project.root().join("source/item.png")).unwrap(),
        b"source"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn cross_volume_move_rejects_a_destination_symlink_race_without_outside_write_or_source_delete()
 {
    let mutation = Arc::new(ReplaceDestinationParentBeforeCreate {
        delegate: LocalFileMutation,
        swap: Mutex::new(None),
    });
    let fixture = Fixture::with_mutation(true, mutation.clone());
    fixture.directory("source");
    let destination = fixture.directory("exports");
    let source = fixture.file("source/item.png", b"source");
    let outside = tempfile::tempdir().unwrap();
    let parent = fixture.project.root().join("exports");
    let parked = fixture.project.root().join("exports-original");
    *mutation.swap.lock().unwrap() = Some((
        parent.clone(),
        parked.clone(),
        Some(outside.path().to_path_buf()),
    ));
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
    assert!(!outside.path().join("item.png").exists());
    assert!(!parked.join("item.png").exists());
    assert_eq!(
        fs::read(fixture.project.root().join("source/item.png")).unwrap(),
        b"source"
    );
    assert_eq!(fixture.trash.count(), 0);
}

#[async_trait]
impl FileMutationPort for RewriteSourceAfterCopy {
    async fn create_and_copy_cancellable_verified(
        &self,
        source: &Path,
        temporary: &Path,
        cancellation: &FileCommandCancellation,
        expected_source: &FileSnapshot,
        source_parent: FileIdentity,
        temporary_parent: FileIdentity,
    ) -> Result<FileContentEvidence, FileOperationError> {
        let evidence = delegate_registered_copy(
            &self.delegate,
            source,
            temporary,
            cancellation,
            expected_source,
            source_parent,
            temporary_parent,
        )
        .await?;
        fs::write(source, b"mutated!")
            .map_err(|error| FileOperationError::io("rewrite copy source", source, &error))?;
        Ok(evidence)
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
    assert_eq!(
        *mutation.rename_count.lock().unwrap(),
        2,
        "only the first component's isolation and final-placement stages run"
    );
}

#[async_trait]
impl FileMutationPort for BlockingCopy {
    async fn create_and_copy_cancellable_verified(
        &self,
        source: &Path,
        temporary: &Path,
        cancellation: &FileCommandCancellation,
        expected_source: &FileSnapshot,
        source_parent: FileIdentity,
        temporary_parent: FileIdentity,
    ) -> Result<FileContentEvidence, FileOperationError> {
        self.started.notify_one();
        while !cancellation.is_cancelled() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
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
    async fn create_and_copy_cancellable_verified(
        &self,
        source: &Path,
        temporary: &Path,
        cancellation: &FileCommandCancellation,
        expected_source: &FileSnapshot,
        source_parent: FileIdentity,
        temporary_parent: FileIdentity,
    ) -> Result<FileContentEvidence, FileOperationError> {
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
}
