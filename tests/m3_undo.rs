use async_trait::async_trait;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use viewer_application::{
    BrowseIndexPort, ClockPort, CommitStage, FileContentEvidence, FileMutationPort,
    FileOperationError, FileSnapshot, OperationCommit, OperationCommitError, OperationCommitPort,
    ProjectAccess, TrashPort, VolumePort,
    file_commands::{
        FileCommand, FileCommandAction, FileCommandCancellation, FileCommandItem, FileCommandKind,
        FileCommandService,
    },
    metadata::{
        FavoritePatch, FileMoveProjection, MarkerChange, MarkerPatch, MarkerProjectionError,
        MarkerProjectionPort, MarkerRestore, MarkerService, MarkerTarget, OperationProjectionPort,
        PortableMarker, PortableMetadataPort, ReviewPatch,
    },
    scheduler::TaskCoordinator,
    undo::{UndoAction, UndoError, UndoFilePort, UndoService, UndoServiceError, UndoStack},
    watcher::FileIdentity,
};
use viewer_domain::{
    EntityId, OperationId, RelativePath, SessionId,
    file::{FileKind, FileNode, Marker, ReviewState},
    operation::OperationKind,
};
use viewer_infrastructure::{
    operation::{
        copy::LocalFileMutation, journal::OperationJournal, service::LocalFileCommandAdapter,
    },
    search::index::SessionIndex,
};
use viewer_test_support::{
    operation_commits::InMemoryOperationCommitPort, project_fixture::ProjectFixture,
};

struct SameVolumeProjectionCommit {
    index: Arc<SessionIndex>,
    root: PathBuf,
}

#[async_trait]
impl OperationCommitPort for SameVolumeProjectionCommit {
    async fn commit_metadata(&self, _commit: &OperationCommit) -> Result<(), OperationCommitError> {
        Ok(())
    }

    async fn sync_index(&self, commit: &OperationCommit) -> Result<(), OperationCommitError> {
        let source = self
            .index
            .node(commit.entity_id)
            .map_err(|_| OperationCommitError::new(CommitStage::Index, "index_unavailable"))?
            .ok_or_else(|| OperationCommitError::new(CommitStage::Index, "projection_stale"))?;
        let relative_path = commit
            .destination
            .clone()
            .ok_or_else(|| OperationCommitError::new(CommitStage::Index, "destination_missing"))?;
        let metadata = fs::metadata(self.root.join(relative_path.as_str()))
            .map_err(|_| OperationCommitError::new(CommitStage::Index, "destination_missing"))?;
        self.index
            .apply_move(
                &[FileMoveProjection {
                    destination: FileNode {
                        entity_id: source.entity_id,
                        relative_path,
                        kind: source.kind,
                        size: metadata.len(),
                        modified_ns: source.modified_ns,
                    },
                    source,
                }],
                true,
            )
            .map_err(|_| OperationCommitError::new(CommitStage::Index, "projection_stale"))
    }
}

#[derive(Default)]
struct MemoryMarkers {
    markers: Mutex<HashMap<RelativePath, (MarkerTarget, Marker)>>,
}

impl MemoryMarkers {
    fn marker(&self, path: &str) -> Marker {
        self.markers
            .lock()
            .unwrap()
            .get(&RelativePath::parse(path).unwrap())
            .map(|(_, marker)| *marker)
            .unwrap_or_default()
    }
}

impl PortableMetadataPort for MemoryMarkers {
    fn markers_for_paths(
        &self,
        paths: &[RelativePath],
    ) -> Result<Vec<PortableMarker>, viewer_application::metadata::MarkerStoreError> {
        let markers = self.markers.lock().unwrap();
        Ok(paths
            .iter()
            .filter_map(|path| {
                markers.get(path).map(|(target, marker)| PortableMarker {
                    relative_path: path.clone(),
                    kind: target.kind,
                    marker: *marker,
                    evidence_size: Some(target.size),
                    evidence_modified_ns: Some(target.modified_ns),
                    content_hash: None,
                })
            })
            .collect())
    }

    fn apply_batch(
        &self,
        targets: &[MarkerTarget],
        patch: MarkerPatch,
        _updated_at_ms: i64,
    ) -> Result<Vec<MarkerChange>, viewer_application::metadata::MarkerStoreError> {
        let mut markers = self.markers.lock().unwrap();
        Ok(targets
            .iter()
            .map(|target| {
                let current = markers
                    .get(&target.relative_path)
                    .map(|(_, marker)| *marker)
                    .unwrap_or_default();
                let review_state = match patch.review {
                    ReviewPatch::Unchanged => current.review_state,
                    ReviewPatch::Set(value) => Some(value),
                    ReviewPatch::Clear => None,
                };
                let favorite = match patch.favorite {
                    FavoritePatch::Unchanged => current.favorite,
                    FavoritePatch::Set(value) => value,
                    FavoritePatch::Toggle => !current.favorite,
                };
                let marker = Marker {
                    review_state,
                    favorite,
                };
                markers.insert(target.relative_path.clone(), (target.clone(), marker));
                MarkerChange {
                    target: target.clone(),
                    marker,
                }
            })
            .collect())
    }

    fn restore_batch(
        &self,
        restores: &[MarkerRestore],
        _updated_at_ms: i64,
    ) -> Result<Vec<MarkerChange>, viewer_application::metadata::MarkerStoreError> {
        let mut markers = self.markers.lock().unwrap();
        if restores.iter().any(|restore| {
            markers
                .get(&restore.target.relative_path)
                .map(|(_, marker)| *marker)
                .unwrap_or_default()
                != restore.expected
        }) {
            return Err(viewer_application::metadata::MarkerStoreError::InvalidTarget);
        }
        Ok(restores
            .iter()
            .map(|restore| {
                markers.insert(
                    restore.target.relative_path.clone(),
                    (restore.target.clone(), restore.previous),
                );
                MarkerChange {
                    target: restore.target.clone(),
                    marker: restore.previous,
                }
            })
            .collect())
    }

    fn move_paths(
        &self,
        _moves: &[viewer_application::metadata::FilePathMove],
        _case_sensitive: bool,
        _updated_at_ms: i64,
    ) -> Result<usize, viewer_application::metadata::MarkerStoreError> {
        Ok(0)
    }
}

#[derive(Default)]
struct MemoryProjection(Mutex<Vec<Vec<MarkerChange>>>);

impl MarkerProjectionPort for MemoryProjection {
    fn sync_markers(&self, changes: &[MarkerChange]) -> Result<(), MarkerProjectionError> {
        self.0.lock().unwrap().push(changes.to_vec());
        Ok(())
    }
}

struct FixedClock;

impl ClockPort for FixedClock {
    fn unix_millis(&self) -> i64 {
        42
    }
}

#[derive(Default)]
struct FsMutation;

#[async_trait]
impl FileMutationPort for FsMutation {
    async fn create_and_copy_cancellable_verified(
        &self,
        _source: &Path,
        _temporary: &Path,
        _cancellation: &FileCommandCancellation,
        _expected_source: &FileSnapshot,
        _source_parent: FileIdentity,
        _temporary_parent: FileIdentity,
    ) -> Result<FileContentEvidence, FileOperationError> {
        unreachable!("undo test mutation never copies")
    }

    async fn snapshot(&self, path: &Path) -> Result<FileSnapshot, FileOperationError> {
        use std::os::unix::fs::MetadataExt;
        let metadata = fs::metadata(path)
            .map_err(|error| FileOperationError::io("test snapshot", path, &error))?;
        Ok(FileSnapshot {
            len: metadata.len(),
            volume_id: metadata.dev(),
            file_id: Some(u128::from(metadata.ino())),
            modified_ns: Some(unix_timestamp_ns(metadata.mtime(), metadata.mtime_nsec())),
            changed_ns: Some(unix_timestamp_ns(metadata.ctime(), metadata.ctime_nsec())),
        })
    }

    async fn copy_and_hash(
        &self,
        _source: &Path,
        _temporary: &Path,
    ) -> Result<(u64, [u8; 32]), FileOperationError> {
        unreachable!()
    }

    async fn rename(&self, source: &Path, destination: &Path) -> Result<(), FileOperationError> {
        fs::rename(source, destination)
            .map_err(|error| FileOperationError::io("test reverse", destination, &error))
    }
}

#[derive(Default)]
struct FakeFileUndo {
    fail: Mutex<bool>,
    calls: Mutex<Vec<viewer_application::undo::UndoBatch>>,
}

#[async_trait]
impl UndoFilePort for FakeFileUndo {
    async fn reverse_batch(
        &self,
        project_root: &Path,
        batch: &viewer_application::undo::UndoBatch,
    ) -> Result<(), UndoError> {
        self.calls.lock().unwrap().push(batch.clone());
        if *self.fail.lock().unwrap() {
            return Err(UndoError::File(FileOperationError::Io {
                action: "injected reverse",
                path: PathBuf::new(),
                message: "injected".into(),
            }));
        }
        for action in &batch.actions {
            if let UndoAction::File {
                current, restore, ..
            } = action
            {
                fs::rename(
                    project_root.join(current.as_str()),
                    project_root.join(restore.as_str()),
                )
                .map_err(|error| {
                    UndoError::File(FileOperationError::io(
                        "fake reverse",
                        restore.as_str(),
                        &error,
                    ))
                })?;
            }
        }
        Ok(())
    }
}

fn target(path: &str) -> MarkerTarget {
    MarkerTarget {
        entity_id: EntityId::new(),
        relative_path: RelativePath::parse(path).unwrap(),
        kind: FileKind::Png,
        size: 1,
        modified_ns: 2,
    }
}

#[derive(Default)]
struct SameVolume;

impl VolumePort for SameVolume {
    fn volume_id(&self, _path: &Path) -> Result<u64, FileOperationError> {
        Ok(1)
    }

    fn is_case_sensitive(&self, _path: &Path) -> Result<bool, FileOperationError> {
        Ok(true)
    }

    fn name_max(&self, _path: &Path) -> Result<usize, FileOperationError> {
        Ok(255)
    }
}

#[derive(Default)]
struct UnusedTrash;

#[async_trait]
impl TrashPort for UnusedTrash {
    async fn trash(&self, _path: &Path) -> Result<(), FileOperationError> {
        unreachable!("rename and undo must not use Trash")
    }
}

fn index_node(
    index: &SessionIndex,
    project: &ProjectFixture,
    relative: &str,
    kind: FileKind,
) -> EntityId {
    let metadata = fs::metadata(project.root().join(relative)).unwrap();
    let entity_id = EntityId::new();
    index
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

#[tokio::test]
async fn marker_review_and_favorite_undo_are_lifo_and_restore_mixed_prior_values() {
    let session = SessionId::new();
    let store = Arc::new(MemoryMarkers::default());
    let projection = Arc::new(MemoryProjection::default());
    let stack = Arc::new(Mutex::new(UndoStack::new(session)));
    let first = target("a.png");
    let second = target("b.png");
    store.markers.lock().unwrap().insert(
        second.relative_path.clone(),
        (
            second.clone(),
            Marker {
                review_state: Some(ReviewState::Reject),
                favorite: true,
            },
        ),
    );

    MarkerService::new(store.as_ref(), projection.as_ref(), true)
        .apply_with_undo(
            &[first.clone(), second.clone()],
            MarkerPatch {
                review: ReviewPatch::Set(ReviewState::Keep),
                favorite: FavoritePatch::Unchanged,
            },
            1,
            &mut stack.lock().unwrap(),
        )
        .unwrap();
    MarkerService::new(store.as_ref(), projection.as_ref(), true)
        .apply_with_undo(
            &[first.clone(), second.clone()],
            MarkerPatch {
                review: ReviewPatch::Unchanged,
                favorite: FavoritePatch::Toggle,
            },
            2,
            &mut stack.lock().unwrap(),
        )
        .unwrap();
    assert_eq!(stack.lock().unwrap().len(), 2);

    let files = Arc::new(FakeFileUndo::default());
    let service = UndoService::new(
        session,
        ProjectAccess::ReadWrite,
        PathBuf::new(),
        Arc::new(FsMutation),
        files,
        store.clone(),
        projection.clone(),
        Arc::new(FixedClock),
        Arc::clone(&stack),
        Arc::new(tokio::sync::Mutex::new(())),
    );
    service.undo_last(session).await.unwrap().unwrap();
    assert_eq!(
        store.marker("a.png"),
        Marker {
            review_state: Some(ReviewState::Keep),
            favorite: false
        }
    );
    assert_eq!(
        store.marker("b.png"),
        Marker {
            review_state: Some(ReviewState::Keep),
            favorite: true
        }
    );
    service.undo_last(session).await.unwrap().unwrap();
    assert_eq!(store.marker("a.png"), Marker::default());
    assert_eq!(
        store.marker("b.png"),
        Marker {
            review_state: Some(ReviewState::Reject),
            favorite: true
        }
    );
    assert!(stack.lock().unwrap().is_empty());
}

#[tokio::test]
async fn file_command_service_records_and_reverses_a_real_rename_cycle() {
    let project = ProjectFixture::new();
    project.create_directory("products");
    project.create_file("products/a.png", b"A");
    project.create_file("products/b.png", b"B");
    let index =
        Arc::new(SessionIndex::open(project.root().join(".viewer/session.sqlite")).unwrap());
    index_node(&index, &project, "products", FileKind::Directory);
    let a = index_node(&index, &project, "products/a.png", FileKind::Png);
    let b = index_node(&index, &project, "products/b.png", FileKind::Png);
    let journal = Arc::new(OperationJournal::open(project.metadata_path()).unwrap());
    let adapter = Arc::new(
        LocalFileCommandAdapter::new(
            project.root(),
            index,
            journal,
            Arc::new(LocalFileMutation),
            Arc::new(UnusedTrash),
            Arc::new(SameVolume),
            Arc::new(FixedClock),
            Arc::new(InMemoryOperationCommitPort::default()),
        )
        .unwrap(),
    );
    let session = SessionId::new();
    let coordinator = Arc::new(TaskCoordinator::default());
    let generation = coordinator.begin_session(session);
    let stack = Arc::new(Mutex::new(UndoStack::new(session)));
    let write_lane = Arc::new(tokio::sync::Mutex::new(()));
    let commands = FileCommandService::new_with_undo(
        session,
        ProjectAccess::ReadWrite,
        coordinator,
        adapter.clone(),
        Arc::clone(&write_lane),
        Arc::clone(&stack),
    );
    let preflight = commands
        .preflight(FileCommand {
            session_id: session,
            generation,
            kind: FileCommandKind::Rename,
            items: vec![
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
        })
        .await
        .unwrap();
    assert!(preflight.is_executable());
    let summary = commands.execute(preflight, &[], None).await.unwrap();
    assert_eq!(summary.completed(), 2);
    assert_eq!(stack.lock().unwrap().len(), 1);
    assert_eq!(
        fs::read(project.root().join("products/a.png")).unwrap(),
        b"B"
    );
    assert_eq!(
        fs::read(project.root().join("products/b.png")).unwrap(),
        b"A"
    );

    let undo = UndoService::new(
        session,
        ProjectAccess::ReadWrite,
        project.root().to_path_buf(),
        Arc::new(LocalFileMutation),
        adapter,
        Arc::new(MemoryMarkers::default()),
        Arc::new(MemoryProjection::default()),
        Arc::new(FixedClock),
        Arc::clone(&stack),
        write_lane,
    );
    let receipt = undo.undo_last(session).await.unwrap().unwrap();
    assert_eq!(receipt.kind, OperationKind::Rename);
    assert_eq!(receipt.action_count, 2);
    assert!(stack.lock().unwrap().is_empty());
    assert_eq!(
        fs::read(project.root().join("products/a.png")).unwrap(),
        b"A"
    );
    assert_eq!(
        fs::read(project.root().join("products/b.png")).unwrap(),
        b"B"
    );
}

#[tokio::test]
async fn a_partial_move_records_and_undoes_only_the_completed_item() {
    let project = ProjectFixture::new();
    project.create_directory("products");
    project.create_directory("archive");
    project.create_file("products/a.png", b"A");
    project.create_file("products/b.png", b"B");
    let index =
        Arc::new(SessionIndex::open(project.root().join(".viewer/session.sqlite")).unwrap());
    index_node(&index, &project, "products", FileKind::Directory);
    let archive = index_node(&index, &project, "archive", FileKind::Directory);
    let a = index_node(&index, &project, "products/a.png", FileKind::Png);
    let b = index_node(&index, &project, "products/b.png", FileKind::Png);
    let commits = Arc::new(SameVolumeProjectionCommit {
        index: Arc::clone(&index),
        root: project.root().to_path_buf(),
    });
    let adapter = Arc::new(
        LocalFileCommandAdapter::new(
            project.root(),
            index,
            Arc::new(OperationJournal::open(project.metadata_path()).unwrap()),
            Arc::new(LocalFileMutation),
            Arc::new(UnusedTrash),
            Arc::new(SameVolume),
            Arc::new(FixedClock),
            commits,
        )
        .unwrap(),
    );
    let session = SessionId::new();
    let coordinator = Arc::new(TaskCoordinator::default());
    let generation = coordinator.begin_session(session);
    let stack = Arc::new(Mutex::new(UndoStack::new(session)));
    let write_lane = Arc::new(tokio::sync::Mutex::new(()));
    let commands = FileCommandService::new_with_undo(
        session,
        ProjectAccess::ReadWrite,
        coordinator,
        adapter.clone(),
        Arc::clone(&write_lane),
        Arc::clone(&stack),
    );
    let preflight = commands
        .preflight(FileCommand {
            session_id: session,
            generation,
            kind: FileCommandKind::Move,
            items: vec![
                FileCommandItem {
                    entity_id: a,
                    action: FileCommandAction::Move {
                        destination_folder: archive,
                    },
                },
                FileCommandItem {
                    entity_id: b,
                    action: FileCommandAction::Move {
                        destination_folder: archive,
                    },
                },
            ],
        })
        .await
        .unwrap();
    fs::write(
        project.root().join("products/b.png"),
        b"changed-after-preflight",
    )
    .unwrap();

    let summary = commands.execute(preflight, &[], None).await.unwrap();
    assert_eq!((summary.completed(), summary.failed()), (1, 1));
    let batch = stack.lock().unwrap().last().unwrap();
    assert_eq!(batch.kind, OperationKind::Move);
    assert_eq!(batch.actions.len(), 1);
    assert!(!project.root().join("products/a.png").exists());
    assert!(project.root().join("archive/a.png").exists());
    assert!(project.root().join("products/b.png").exists());

    let undo = UndoService::new(
        session,
        ProjectAccess::ReadWrite,
        project.root().to_path_buf(),
        Arc::new(LocalFileMutation),
        adapter,
        Arc::new(MemoryMarkers::default()),
        Arc::new(MemoryProjection::default()),
        Arc::new(FixedClock),
        Arc::clone(&stack),
        write_lane,
    );
    let receipt = undo.undo_last(session).await.unwrap().unwrap();
    assert_eq!(receipt.kind, OperationKind::Move);
    assert_eq!(receipt.action_count, 1);
    assert!(stack.lock().unwrap().is_empty());
    assert_eq!(
        fs::read(project.root().join("products/a.png")).unwrap(),
        b"A"
    );
    assert!(!project.root().join("archive/a.png").exists());
    assert_eq!(
        fs::read(project.root().join("products/b.png")).unwrap(),
        b"changed-after-preflight"
    );
}

#[tokio::test]
async fn file_undo_prevalidates_the_whole_batch_and_retains_failures() {
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("one-new.png"), b"one").unwrap();
    fs::write(project.path().join("two-new.png"), b"two").unwrap();
    let mutation = Arc::new(FsMutation);
    let session = SessionId::new();
    let stack = Arc::new(Mutex::new(UndoStack::new(session)));
    let actions = ["one", "two"]
        .into_iter()
        .map(|name| {
            let current = project.path().join(format!("{name}-new.png"));
            UndoAction::File {
                entity_id: EntityId::new(),
                current: RelativePath::parse(&format!("{name}-new.png")).unwrap(),
                restore: RelativePath::parse(&format!("{name}.png")).unwrap(),
                expected: futures_snapshot(&current),
            }
        })
        .collect::<Vec<_>>();
    stack
        .lock()
        .unwrap()
        .record_batch(OperationId::new(), OperationKind::Rename, actions);
    fs::write(project.path().join("two.png"), b"occupied").unwrap();
    let files = Arc::new(FakeFileUndo::default());
    let service = UndoService::new(
        session,
        ProjectAccess::ReadWrite,
        project.path().to_path_buf(),
        mutation,
        files.clone(),
        Arc::new(MemoryMarkers::default()),
        Arc::new(MemoryProjection::default()),
        Arc::new(FixedClock),
        Arc::clone(&stack),
        Arc::new(tokio::sync::Mutex::new(())),
    );

    assert!(matches!(
        service.undo_last(session).await,
        Err(UndoServiceError::Undo(UndoError::DestinationOccupied))
    ));
    assert_eq!(stack.lock().unwrap().len(), 1);
    assert!(files.calls.lock().unwrap().is_empty());
    fs::remove_file(project.path().join("two.png")).unwrap();
    *files.fail.lock().unwrap() = true;
    assert!(service.undo_last(session).await.is_err());
    assert_eq!(stack.lock().unwrap().len(), 1);
    *files.fail.lock().unwrap() = false;
    service.undo_last(session).await.unwrap().unwrap();
    assert!(stack.lock().unwrap().is_empty());
}

#[tokio::test]
async fn case_only_rename_undo_treats_the_current_file_as_the_restore_target() {
    let project = tempfile::tempdir().unwrap();
    let current = project.path().join("A.jpg");
    fs::write(&current, b"case-only").unwrap();
    let session = SessionId::new();
    let stack = Arc::new(Mutex::new(UndoStack::new(session)));
    stack.lock().unwrap().record_batch(
        OperationId::new(),
        OperationKind::Rename,
        vec![UndoAction::File {
            entity_id: EntityId::new(),
            current: RelativePath::parse("A.jpg").unwrap(),
            restore: RelativePath::parse("a.jpg").unwrap(),
            expected: futures_snapshot(&current),
        }],
    );
    let service = UndoService::new(
        session,
        ProjectAccess::ReadWrite,
        project.path().to_path_buf(),
        Arc::new(FsMutation),
        Arc::new(FakeFileUndo::default()),
        Arc::new(MemoryMarkers::default()),
        Arc::new(MemoryProjection::default()),
        Arc::new(FixedClock),
        Arc::clone(&stack),
        Arc::new(tokio::sync::Mutex::new(())),
    );

    service.undo_last(session).await.unwrap().unwrap();

    assert!(project.path().join("a.jpg").exists());
    assert_eq!(
        fs::read(project.path().join("a.jpg")).unwrap(),
        b"case-only"
    );
    assert!(stack.lock().unwrap().is_empty());
}

#[tokio::test]
async fn identity_drift_and_symlink_escape_are_rejected_without_consuming_undo() {
    let project = tempfile::tempdir().unwrap();
    let current = project.path().join("current.png");
    fs::write(&current, b"original").unwrap();
    let session = SessionId::new();
    let stack = Arc::new(Mutex::new(UndoStack::new(session)));
    stack.lock().unwrap().record_batch(
        OperationId::new(),
        OperationKind::Rename,
        vec![UndoAction::File {
            entity_id: EntityId::new(),
            current: RelativePath::parse("current.png").unwrap(),
            restore: RelativePath::parse("restored.png").unwrap(),
            expected: futures_snapshot(&current),
        }],
    );
    fs::write(&current, b"changed-after-command").unwrap();
    let files = Arc::new(FakeFileUndo::default());
    let service = UndoService::new(
        session,
        ProjectAccess::ReadWrite,
        project.path().to_path_buf(),
        Arc::new(FsMutation),
        files.clone(),
        Arc::new(MemoryMarkers::default()),
        Arc::new(MemoryProjection::default()),
        Arc::new(FixedClock),
        Arc::clone(&stack),
        Arc::new(tokio::sync::Mutex::new(())),
    );
    assert_eq!(
        service.undo_last(session).await,
        Err(UndoServiceError::Undo(UndoError::IdentityChanged))
    );
    assert_eq!(stack.lock().unwrap().len(), 1);
    assert!(files.calls.lock().unwrap().is_empty());

    stack.lock().unwrap().close_session(session);
    let outside = tempfile::tempdir().unwrap();
    let external = outside.path().join("external.png");
    fs::write(&external, b"external").unwrap();
    fs::remove_file(&current).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&external, &current).unwrap();
    stack.lock().unwrap().record_batch(
        OperationId::new(),
        OperationKind::Move,
        vec![UndoAction::File {
            entity_id: EntityId::new(),
            current: RelativePath::parse("current.png").unwrap(),
            restore: RelativePath::parse("restored.png").unwrap(),
            expected: futures_snapshot(&external),
        }],
    );
    assert_eq!(
        service.undo_last(session).await,
        Err(UndoServiceError::Undo(UndoError::OutsideProject))
    );
    assert_eq!(stack.lock().unwrap().len(), 1);
    assert!(files.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn read_only_wrong_session_exclusions_and_close_reset_are_enforced() {
    let session = SessionId::new();
    let stack = Arc::new(Mutex::new(UndoStack::new(session)));
    let file_action = UndoAction::File {
        entity_id: EntityId::new(),
        current: RelativePath::parse("current.png").unwrap(),
        restore: RelativePath::parse("restored.png").unwrap(),
        expected: FileSnapshot {
            len: 1,
            volume_id: 1,
            file_id: Some(1),
            modified_ns: Some(1),
            changed_ns: Some(1),
        },
    };
    assert!(!stack.lock().unwrap().record_batch(
        OperationId::new(),
        OperationKind::Copy,
        vec![file_action.clone()]
    ));
    assert!(!stack.lock().unwrap().record_batch(
        OperationId::new(),
        OperationKind::Trash,
        vec![file_action]
    ));
    let service = UndoService::new(
        session,
        ProjectAccess::ReadOnly,
        PathBuf::new(),
        Arc::new(FsMutation),
        Arc::new(FakeFileUndo::default()),
        Arc::new(MemoryMarkers::default()),
        Arc::new(MemoryProjection::default()),
        Arc::new(FixedClock),
        Arc::clone(&stack),
        Arc::new(tokio::sync::Mutex::new(())),
    );
    assert_eq!(
        service.undo_last(session).await,
        Err(UndoServiceError::ReadOnly)
    );
    assert_eq!(
        service.undo_last(SessionId::new()).await,
        Err(UndoServiceError::StaleSession)
    );
    stack.lock().unwrap().close_session(session);
    assert!(stack.lock().unwrap().is_empty());
}

fn futures_snapshot(path: &Path) -> FileSnapshot {
    use std::os::unix::fs::MetadataExt;
    let metadata = fs::metadata(path).unwrap();
    FileSnapshot {
        len: metadata.len(),
        volume_id: metadata.dev(),
        file_id: Some(u128::from(metadata.ino())),
        modified_ns: Some(unix_timestamp_ns(metadata.mtime(), metadata.mtime_nsec())),
        changed_ns: Some(unix_timestamp_ns(metadata.ctime(), metadata.ctime_nsec())),
    }
}

fn unix_timestamp_ns(seconds: i64, nanoseconds: i64) -> i128 {
    i128::from(seconds) * 1_000_000_000 + i128::from(nanoseconds)
}
