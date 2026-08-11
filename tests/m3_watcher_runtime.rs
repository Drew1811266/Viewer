use rusqlite::OptionalExtension;
use std::{fs, path::Path, sync::Arc};
use viewer_application::{
    ProjectAccess,
    metadata::{FavoritePatch, MarkerPatch, MarkerTarget, PortableMetadataPort, ReviewPatch},
    scheduler::TaskCoordinator,
    watcher::{ReconcileReason, ReconcileRequest},
};
use viewer_domain::{
    EntityId, RelativePath, SessionId,
    file::{FileKind, FileNode, ImageIndexStatus, ImageMetadata, ReviewState, TextIndexStatus},
};
use viewer_infrastructure::{
    portable::{PortableMarkerStore, PortableProjectMetadata},
    scan::reconcile_service::{ProjectReconcileError, ProjectReconciler},
    search::{index::SessionIndex, text::TextStatus},
};
use viewer_test_support::{FixedClock, project_fixture::ProjectFixture};

struct Fixture {
    project: ProjectFixture,
    index: Arc<SessionIndex>,
    markers: Arc<PortableMarkerStore>,
    coordinator: Arc<TaskCoordinator>,
    reconciler: ProjectReconciler,
    session_id: SessionId,
    generation: viewer_domain::search::Generation,
}

impl Fixture {
    fn new() -> Self {
        let project = ProjectFixture::new();
        let portable = PortableProjectMetadata::open(project.root(), ProjectAccess::ReadWrite, 1)
            .expect("create portable metadata");
        let database = portable.database_path().unwrap().to_path_buf();
        drop(portable);
        let markers = Arc::new(PortableMarkerStore::open(&database, true).unwrap());
        let index =
            Arc::new(SessionIndex::open(project.root().join(".viewer/session.sqlite")).unwrap());
        let coordinator = Arc::new(TaskCoordinator::default());
        let session_id = SessionId::new();
        let generation = coordinator.begin_session(session_id);
        let reconciler = ProjectReconciler::new(
            project.root(),
            Arc::clone(&coordinator),
            Arc::clone(&index),
            Some(markers.clone() as Arc<dyn PortableMetadataPort>),
            Arc::new(FixedClock::new(10_000)),
            Arc::new(tokio::sync::Mutex::new(())),
            true,
        )
        .unwrap();
        Self {
            project,
            index,
            markers,
            coordinator,
            reconciler,
            session_id,
            generation,
        }
    }

    fn directory(&self, relative: &str) -> EntityId {
        self.project.create_directory(relative);
        self.index_existing(relative, FileKind::Directory)
    }

    fn file(&self, relative: &str, contents: &[u8], kind: FileKind) -> EntityId {
        self.project.create_file(relative, contents);
        self.index_existing(relative, kind)
    }

    fn index_existing(&self, relative: &str, kind: FileKind) -> EntityId {
        let node = filesystem_node(self.project.root(), relative, kind);
        let entity_id = node.entity_id;
        self.index.upsert_batch(&[node]).unwrap();
        entity_id
    }

    async fn reconcile(&self, roots: &[&str]) -> viewer_application::watcher::ReconcileSummary {
        self.reconciler
            .reconcile(ReconcileRequest {
                session_id: self.session_id,
                generation: self.generation,
                roots: roots
                    .iter()
                    .map(|relative| self.project.root().canonicalize().unwrap().join(relative))
                    .collect(),
                reason: ReconcileReason::ExternalChange,
            })
            .await
            .unwrap()
    }
}

fn filesystem_node(root: &Path, relative: &str, kind: FileKind) -> FileNode {
    use std::os::unix::fs::MetadataExt;
    let metadata = fs::symlink_metadata(root.join(relative)).unwrap();
    FileNode {
        entity_id: EntityId::from_u128(
            (u128::from(metadata.dev()) << 64) | u128::from(metadata.ino()),
        ),
        relative_path: RelativePath::parse(relative).unwrap(),
        kind,
        size: if kind == FileKind::Directory {
            0
        } else {
            metadata.len()
        },
        modified_ns: i128::from(metadata.mtime()) * 1_000_000_000
            + i128::from(metadata.mtime_nsec()),
    }
}

fn fts_entry(project: &ProjectFixture, entity_id: EntityId) -> Option<(String, String)> {
    rusqlite::Connection::open(project.root().join(".viewer/session.sqlite"))
        .unwrap()
        .query_row(
            "SELECT relative_path, body FROM text_fts WHERE entity_id = ?1",
            [entity_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .unwrap()
}

#[tokio::test]
async fn external_create_partial_write_modify_and_delete_commit_one_truth_delta() {
    let fixture = Fixture::new();
    fixture.directory("work");
    let image = fixture.file("work/old.png", b"old", FileKind::Png);
    let note = fixture.file("work/note.txt", b"searchable", FileKind::Text);
    let edited_text = fixture.file("work/edit.txt", b"before edit", FileKind::Text);
    fixture
        .index
        .replace_image_metadata(
            image,
            &RelativePath::parse("work/old.png").unwrap(),
            Ok(ImageMetadata {
                width: 10,
                height: 20,
            }),
        )
        .unwrap();
    fixture
        .index
        .replace_text(
            note,
            &RelativePath::parse("work/note.txt").unwrap(),
            &TextStatus::Indexed("searchable".into()),
        )
        .unwrap();
    fixture
        .index
        .replace_text(
            edited_text,
            &RelativePath::parse("work/edit.txt").unwrap(),
            &TextStatus::Indexed("before edit".into()),
        )
        .unwrap();

    fs::write(
        fixture.project.root().join("work/old.png"),
        b"old-but-modified",
    )
    .unwrap();
    fs::write(
        fixture.project.root().join("work/edit.txt"),
        b"after edit with different bytes",
    )
    .unwrap();
    fs::write(fixture.project.root().join("work/new.png"), []).unwrap();
    fs::remove_file(fixture.project.root().join("work/note.txt")).unwrap();
    let first = fixture.reconcile(&["work"]).await;
    assert_eq!((first.added, first.modified, first.removed), (1, 2, 1));
    assert_eq!(first.moved, 0);
    let changed = fixture.index.indexed_node(image).unwrap().unwrap();
    assert_eq!(changed.image_status, ImageIndexStatus::Pending);
    assert_eq!(changed.image_metadata, None);
    assert_eq!(fixture.index.indexed_node(note).unwrap(), None);
    assert_eq!(fts_entry(&fixture.project, note), None);
    assert_eq!(
        fixture
            .index
            .indexed_node(edited_text)
            .unwrap()
            .unwrap()
            .text_status,
        TextIndexStatus::Pending
    );
    assert_eq!(fts_entry(&fixture.project, edited_text), None);
    let created = filesystem_node(fixture.project.root(), "work/new.png", FileKind::Png);
    assert_eq!(
        fixture
            .index
            .indexed_node(created.entity_id)
            .unwrap()
            .unwrap()
            .node
            .size,
        0
    );

    fs::write(
        fixture.project.root().join("work/new.png"),
        b"finalized-image-bytes",
    )
    .unwrap();
    let finalized = fixture.reconcile(&["work"]).await;
    assert_eq!(
        (finalized.added, finalized.modified, finalized.removed),
        (0, 1, 0)
    );
    let created = fixture
        .index
        .indexed_node(created.entity_id)
        .unwrap()
        .unwrap();
    assert_eq!(created.node.size, 21);
    assert_eq!(created.image_status, ImageIndexStatus::Pending);
}

#[tokio::test]
async fn directory_move_preserves_entities_derived_data_and_relocates_markers_by_identity() {
    let fixture = Fixture::new();
    fixture.directory("source");
    let folder = fixture.directory("source/id-1");
    fixture.directory("archive");
    let text = fixture.file("source/id-1/prompt.txt", b"prompt", FileKind::Text);
    fixture
        .index
        .replace_text(
            text,
            &RelativePath::parse("source/id-1/prompt.txt").unwrap(),
            &TextStatus::Indexed("red product prompt".into()),
        )
        .unwrap();
    let target = MarkerTarget {
        entity_id: text,
        relative_path: RelativePath::parse("source/id-1/prompt.txt").unwrap(),
        kind: FileKind::Text,
        size: 6,
        modified_ns: filesystem_node(
            fixture.project.root(),
            "source/id-1/prompt.txt",
            FileKind::Text,
        )
        .modified_ns,
    };
    let changes = fixture
        .markers
        .apply_batch(
            &[target],
            MarkerPatch {
                review: ReviewPatch::Set(ReviewState::Keep),
                favorite: FavoritePatch::Set(true),
            },
            2,
        )
        .unwrap();
    viewer_application::metadata::MarkerProjectionPort::sync_markers(
        fixture.index.as_ref(),
        &changes,
    )
    .unwrap();

    fs::rename(
        fixture.project.root().join("source/id-1"),
        fixture.project.root().join("archive/id-1"),
    )
    .unwrap();
    let summary = fixture.reconcile(&["source", "archive"]).await;
    assert_eq!(
        (summary.added, summary.removed, summary.modified),
        (0, 0, 0)
    );
    assert_eq!(summary.moved, 2);
    assert_eq!(summary.marker_paths_moved, 1);
    assert_eq!(
        fixture
            .index
            .indexed_node(folder)
            .unwrap()
            .unwrap()
            .node
            .relative_path,
        RelativePath::parse("archive/id-1").unwrap()
    );
    let moved = fixture.index.indexed_node(text).unwrap().unwrap();
    assert_eq!(
        moved.node.relative_path,
        RelativePath::parse("archive/id-1/prompt.txt").unwrap()
    );
    assert_eq!(moved.text_status, TextIndexStatus::Ready);
    assert_eq!(moved.marker.review_state, Some(ReviewState::Keep));
    assert!(moved.marker.favorite);
    assert_eq!(
        fts_entry(&fixture.project, text),
        Some((
            "archive/id-1/prompt.txt".into(),
            "red product prompt".into()
        ))
    );
    assert!(
        fixture
            .markers
            .markers_for_paths(&[RelativePath::parse("source/id-1/prompt.txt").unwrap()])
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        fixture
            .markers
            .markers_for_paths(&[RelativePath::parse("archive/id-1/prompt.txt").unwrap()])
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn same_path_identity_replacement_keeps_the_moved_other_file_and_does_not_inherit_its_marker()
{
    let fixture = Fixture::new();
    fixture.directory("work");
    let old = fixture.file("work/item.png", b"old", FileKind::Png);
    let target = MarkerTarget {
        entity_id: old,
        relative_path: RelativePath::parse("work/item.png").unwrap(),
        kind: FileKind::Png,
        size: 3,
        modified_ns: filesystem_node(fixture.project.root(), "work/item.png", FileKind::Png)
            .modified_ns,
    };
    let changes = fixture
        .markers
        .apply_batch(
            &[target],
            MarkerPatch {
                review: ReviewPatch::Unchanged,
                favorite: FavoritePatch::Set(true),
            },
            3,
        )
        .unwrap();
    viewer_application::metadata::MarkerProjectionPort::sync_markers(
        fixture.index.as_ref(),
        &changes,
    )
    .unwrap();
    fs::rename(
        fixture.project.root().join("work/item.png"),
        fixture.project.root().join("work/replaced.bin"),
    )
    .unwrap();
    fs::write(
        fixture.project.root().join("work/item.png"),
        b"new identity",
    )
    .unwrap();
    let replacement = filesystem_node(fixture.project.root(), "work/item.png", FileKind::Png);
    assert_ne!(old, replacement.entity_id);

    let summary = fixture.reconcile(&["work"]).await;
    assert_eq!((summary.added, summary.removed, summary.moved), (1, 0, 1));
    let moved = fixture.index.indexed_node(old).unwrap().unwrap();
    assert_eq!(
        moved.node.relative_path,
        RelativePath::parse("work/replaced.bin").unwrap()
    );
    assert_eq!(moved.node.kind, FileKind::Other);
    assert!(moved.marker.favorite);
    let replacement = fixture
        .index
        .indexed_node(replacement.entity_id)
        .unwrap()
        .unwrap();
    assert_eq!(replacement.marker, Default::default());
}

#[tokio::test]
async fn subtree_snapshot_excludes_hidden_reserved_and_symlink_entries_and_classifies_candidates() {
    let fixture = Fixture::new();
    fixture.directory("work");
    fs::write(fixture.project.root().join("work/visible.png"), b"visible").unwrap();
    fs::write(
        fixture.project.root().join("outside-scope.png"),
        b"outside scope",
    )
    .unwrap();
    fs::write(fixture.project.root().join("work/ignored.pdf"), b"pdf").unwrap();
    fs::write(
        fixture.project.root().join("work/poster.webp"),
        b"unsupported image",
    )
    .unwrap();
    fs::write(fixture.project.root().join("work/clip.mov"), b"video").unwrap();
    fs::create_dir(fixture.project.root().join("work/.hidden")).unwrap();
    fs::write(
        fixture.project.root().join("work/.hidden/private.png"),
        b"private",
    )
    .unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("outside.png"), b"outside").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("outside.png"),
        fixture.project.root().join("work/link.png"),
    )
    .unwrap();

    let summary = fixture.reconcile(&["work"]).await;
    assert_eq!((summary.added, summary.failed), (4, 0));
    let children = fixture
        .index
        .directory_children(Some(
            filesystem_node(fixture.project.root(), "work", FileKind::Directory).entity_id,
        ))
        .unwrap();
    assert_eq!(
        children
            .iter()
            .map(|node| node.relative_path.as_str())
            .collect::<Vec<_>>(),
        [
            "work/clip.mov",
            "work/ignored.pdf",
            "work/poster.webp",
            "work/visible.png",
        ]
    );
    let ignored =
        filesystem_node(fixture.project.root(), "work/ignored.pdf", FileKind::Other).entity_id;
    assert_eq!(
        fixture
            .index
            .indexed_node(ignored)
            .unwrap()
            .unwrap()
            .node
            .kind,
        FileKind::Other
    );
    let poster = filesystem_node(
        fixture.project.root(),
        "work/poster.webp",
        FileKind::UnsupportedImage,
    )
    .entity_id;
    assert_eq!(
        fixture
            .index
            .indexed_node(poster)
            .unwrap()
            .unwrap()
            .node
            .kind,
        FileKind::UnsupportedImage
    );
    let video = filesystem_node(fixture.project.root(), "work/clip.mov", FileKind::Video).entity_id;
    assert_eq!(
        fixture
            .index
            .indexed_node(video)
            .unwrap()
            .unwrap()
            .node
            .kind,
        FileKind::Video
    );

    fs::rename(
        fixture.project.root().join("work/ignored.pdf"),
        fixture.project.root().join("work/ignored.webp"),
    )
    .unwrap();
    fixture.reconcile(&["work"]).await;
    assert_eq!(
        filesystem_node(
            fixture.project.root(),
            "work/ignored.webp",
            FileKind::UnsupportedImage,
        )
        .entity_id,
        ignored
    );
    assert_eq!(
        fixture
            .index
            .indexed_node(ignored)
            .unwrap()
            .unwrap()
            .node
            .kind,
        FileKind::UnsupportedImage
    );

    fs::rename(
        fixture.project.root().join("work/ignored.webp"),
        fixture.project.root().join("work/ignored.mov"),
    )
    .unwrap();
    fixture.reconcile(&["work"]).await;
    assert_eq!(
        fixture
            .index
            .indexed_node(ignored)
            .unwrap()
            .unwrap()
            .node
            .kind,
        FileKind::Video
    );
    let outside_scope = filesystem_node(fixture.project.root(), "outside-scope.png", FileKind::Png);
    assert_eq!(
        fixture.index.indexed_node(outside_scope.entity_id).unwrap(),
        None
    );
}

#[tokio::test]
async fn stale_or_closed_session_and_symlinked_requested_root_never_publish() {
    let fixture = Fixture::new();
    fixture.directory("work");
    fs::write(fixture.project.root().join("work/new.png"), b"new").unwrap();
    let request = ReconcileRequest {
        session_id: fixture.session_id,
        generation: fixture.generation,
        roots: vec![fixture.project.root().canonicalize().unwrap().join("work")],
        reason: ReconcileReason::ExternalChange,
    };
    fixture
        .coordinator
        .bump_generation(fixture.session_id)
        .unwrap();
    assert_eq!(
        fixture.reconciler.reconcile(request.clone()).await,
        Err(ProjectReconcileError::StaleSession)
    );
    assert_eq!(
        fixture
            .index
            .directory_children(Some(
                filesystem_node(fixture.project.root(), "work", FileKind::Directory).entity_id
            ))
            .unwrap()
            .len(),
        0
    );
    fixture.coordinator.cancel_session(fixture.session_id);
    assert_eq!(
        fixture.reconciler.reconcile(request).await,
        Err(ProjectReconcileError::StaleSession)
    );

    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), fixture.project.root().join("escape")).unwrap();
    let current = fixture.coordinator.begin_session(fixture.session_id);
    assert!(matches!(
        fixture
            .reconciler
            .reconcile(ReconcileRequest {
                session_id: fixture.session_id,
                generation: current,
                roots: vec![
                    fixture
                        .project
                        .root()
                        .canonicalize()
                        .unwrap()
                        .join("escape")
                ],
                reason: ReconcileReason::ExternalChange,
            })
            .await,
        Err(ProjectReconcileError::InvalidRoot)
    ));
}
