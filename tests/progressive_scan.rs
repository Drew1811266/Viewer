use async_trait::async_trait;
use std::{fs, sync::Arc};
use viewer_application::{
    ScanPort,
    scan::{CoordinatedScan, ScanError, ScanEvent, ScanRequest, ScanSink, ScanTotals},
    scheduler::TaskCoordinator,
};
use viewer_domain::{
    EntityId, RelativePath, SessionId,
    file::{FileKind, FileNode},
    search::Generation,
};
use viewer_infrastructure::{scan::walker::ProjectWalker, search::index::SessionIndex};
use viewer_test_support::project_fixture::ProjectFixture;

#[tokio::test]
async fn progressive_scan_publishes_folders_first_and_excludes_unsafe_entries() {
    let project = ProjectFixture::new();
    project.create_directory("products/id-1/details");
    project.create_file("products/id-1/front.jpg", b"jpeg");
    project.create_file("products/id-1/back.PNG", b"png");
    project.create_file("products/id-1/prompt.md", b"prompt");
    project.create_file("notes.txt", b"notes");
    project.create_file("ignored.pdf", b"pdf");
    fs::create_dir_all(project.root().join(".hidden/nested")).unwrap();
    fs::write(project.root().join(".hidden/nested/front.jpg"), b"hidden").unwrap();
    fs::write(
        project.root().join(".viewer/metadata-note.txt"),
        b"reserved",
    )
    .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("outside.jpg"), b"outside").unwrap();
        symlink(outside.path(), project.root().join("linked-folder")).unwrap();
        symlink(
            outside.path().join("outside.jpg"),
            project.root().join("linked-file.jpg"),
        )
        .unwrap();

        run_and_assert_scan(project.root()).await;
    }

    #[cfg(not(unix))]
    run_and_assert_scan(project.root()).await;
}

async fn run_and_assert_scan(root: &std::path::Path) {
    let (sink, mut events) = tokio::sync::mpsc::channel(8);
    let request = ScanRequest {
        session_id: SessionId::new(),
        generation: Generation::new(7),
        root: root.to_path_buf(),
    };
    ProjectWalker.scan(request, sink).await.unwrap();

    let mut folder_paths = Vec::new();
    let mut file_paths = Vec::new();
    let mut saw_files = false;
    let mut finished = None;
    let mut failed_items = Vec::new();
    while let Some(event) = events.recv().await {
        match event {
            ScanEvent::Folders { generation, nodes } => {
                assert_eq!(generation, Generation::new(7));
                assert!(!saw_files, "folder batches must precede file batches");
                assert!(nodes.len() <= 128);
                assert!(nodes.iter().all(|node| node.kind == FileKind::Directory));
                folder_paths.extend(
                    nodes
                        .into_iter()
                        .map(|node| node.relative_path.as_str().to_owned()),
                );
            }
            ScanEvent::Files { generation, nodes } => {
                assert_eq!(generation, Generation::new(7));
                saw_files = true;
                assert!(nodes.len() <= 128);
                assert!(nodes.iter().all(|node| node.kind != FileKind::Directory));
                file_paths.extend(
                    nodes
                        .into_iter()
                        .map(|node| node.relative_path.as_str().to_owned()),
                );
            }
            ScanEvent::Finished { generation, totals } => {
                assert_eq!(generation, Generation::new(7));
                finished = Some(totals);
            }
            ScanEvent::FailedItem {
                generation,
                relative_display,
                code,
            } => {
                assert_eq!(generation, Generation::new(7));
                failed_items.push((relative_display, code));
            }
        }
    }

    folder_paths.sort();
    file_paths.sort();
    assert_eq!(
        folder_paths,
        ["products", "products/id-1", "products/id-1/details"]
    );
    assert_eq!(
        file_paths,
        [
            "notes.txt",
            "products/id-1/back.PNG",
            "products/id-1/front.jpg",
            "products/id-1/prompt.md",
        ]
    );
    let totals = finished.expect("finished event");
    assert_eq!(totals.folders, 3);
    assert_eq!(totals.files, 4);
    assert_eq!(totals.failed, 0);
    assert!(failed_items.is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn progressive_scan_isolates_an_unreadable_subtree() {
    use std::os::unix::fs::PermissionsExt;

    let project = ProjectFixture::new();
    project.create_file("good/visible.txt", b"visible");
    project.create_file("unreadable/hidden.txt", b"hidden");
    let unreadable = project.root().join("unreadable");
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();

    let (sink, mut events) = tokio::sync::mpsc::channel(8);
    let result = ProjectWalker
        .scan(
            ScanRequest {
                session_id: SessionId::new(),
                generation: Generation::new(9),
                root: project.root().to_path_buf(),
            },
            sink,
        )
        .await;
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o700)).unwrap();
    result.unwrap();

    let mut visible_file_was_scanned = false;
    let mut failed_items = 0;
    let mut totals = None;
    while let Some(event) = events.recv().await {
        match event {
            ScanEvent::Files { nodes, .. } => {
                visible_file_was_scanned |= nodes
                    .iter()
                    .any(|node| node.relative_path.as_str() == "good/visible.txt");
            }
            ScanEvent::FailedItem { code, .. } => {
                assert_eq!(code, "walk_error");
                failed_items += 1;
            }
            ScanEvent::Finished {
                totals: scan_totals,
                ..
            } => totals = Some(scan_totals),
            ScanEvent::Folders { .. } => {}
        }
    }

    assert!(visible_file_was_scanned);
    assert_eq!(failed_items, 1);
    assert_eq!(totals.unwrap().failed, 1);
}

#[tokio::test]
async fn progressive_scan_rejects_a_non_directory_root() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("not-a-directory");
    fs::write(&root, b"file").unwrap();
    let (sink, _events) = tokio::sync::mpsc::channel(8);

    let result = ProjectWalker
        .scan(
            ScanRequest {
                session_id: SessionId::new(),
                generation: Generation::new(1),
                root,
            },
            sink,
        )
        .await;

    assert!(matches!(result, Err(ScanError::RootUnreadable(_))));
}

#[test]
fn session_index_commits_a_batch_and_reopens_with_the_same_hierarchy() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("session.sqlite");
    let products_id = EntityId::new();
    let item_id = EntityId::new();
    let nodes = vec![
        indexed_node(products_id, "products", FileKind::Directory),
        indexed_node(item_id, "products/id-1", FileKind::Directory),
        indexed_node(EntityId::new(), "products/id-1/front.jpg", FileKind::Jpeg),
        indexed_node(EntityId::new(), "notes.txt", FileKind::Text),
    ];

    let index = SessionIndex::open(&database).unwrap();
    index.upsert_batch(&nodes).unwrap();
    index.close().unwrap();

    let index = SessionIndex::open(&database).unwrap();
    let root_children = index.directory_children(None).unwrap();
    assert_eq!(paths(&root_children), ["notes.txt", "products"]);
    let product_children = index.directory_children(Some(products_id)).unwrap();
    assert_eq!(paths(&product_children), ["products/id-1"]);
    let item_children = index.directory_children(Some(item_id)).unwrap();
    assert_eq!(paths(&item_children), ["products/id-1/front.jpg"]);
    assert_eq!(index.remove_subtree(products_id).unwrap(), 3);
    assert_eq!(
        paths(&index.directory_children(None).unwrap()),
        ["notes.txt"]
    );
    assert!(index.directory_children(Some(item_id)).unwrap().is_empty());
}

#[test]
fn session_index_rolls_back_the_entire_batch_when_one_item_conflicts() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("session.sqlite");
    let duplicate_path = "products";
    let nodes = vec![
        indexed_node(EntityId::new(), duplicate_path, FileKind::Directory),
        indexed_node(EntityId::new(), "products/id-1", FileKind::Directory),
        indexed_node(EntityId::new(), duplicate_path, FileKind::Directory),
        indexed_node(EntityId::new(), "notes.txt", FileKind::Text),
    ];

    let index = SessionIndex::open(&database).unwrap();
    assert!(index.upsert_batch(&nodes).is_err());
    assert!(index.directory_children(None).unwrap().is_empty());
}

fn indexed_node(entity_id: EntityId, path: &str, kind: FileKind) -> FileNode {
    FileNode {
        entity_id,
        relative_path: RelativePath::parse(path).unwrap(),
        kind,
        size: if kind == FileKind::Directory { 0 } else { 42 },
        modified_ns: 1_725_000_000_123_456_789,
    }
}

fn paths(nodes: &[FileNode]) -> Vec<&str> {
    nodes
        .iter()
        .map(|node| node.relative_path.as_str())
        .collect()
}

struct BarrierScanner {
    started: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
    blocked_generation: Generation,
}

#[async_trait]
impl ScanPort for BarrierScanner {
    async fn scan(&self, request: ScanRequest, sink: ScanSink) -> Result<(), ScanError> {
        if request.generation == self.blocked_generation {
            self.started.notify_one();
            self.release.notified().await;
        }
        sink.send(ScanEvent::Finished {
            generation: request.generation,
            totals: ScanTotals::default(),
        })
        .await
        .map_err(|_| ScanError::Cancelled)
    }
}

#[tokio::test]
async fn stale_scan_generation_never_reaches_the_publication_sink() {
    let coordinator = Arc::new(TaskCoordinator::default());
    let session_id = SessionId::new();
    let old_generation = coordinator.begin_session(session_id);
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let scanner = Arc::new(CoordinatedScan::new(
        Arc::new(BarrierScanner {
            started: Arc::clone(&started),
            release: Arc::clone(&release),
            blocked_generation: old_generation,
        }),
        Arc::clone(&coordinator),
    ));
    let (sink, mut events) = tokio::sync::mpsc::channel(8);
    let root = tempfile::tempdir().unwrap();
    let old_scanner = Arc::clone(&scanner);
    let old_sink = sink.clone();
    let old_root = root.path().to_path_buf();
    let old_task = tokio::spawn(async move {
        old_scanner
            .scan(
                ScanRequest {
                    session_id,
                    generation: old_generation,
                    root: old_root,
                },
                old_sink,
            )
            .await
    });
    started.notified().await;

    let current_generation = coordinator.bump_generation(session_id).unwrap();
    scanner
        .scan(
            ScanRequest {
                session_id,
                generation: current_generation,
                root: root.path().to_path_buf(),
            },
            sink.clone(),
        )
        .await
        .unwrap();
    release.notify_one();
    assert!(matches!(old_task.await.unwrap(), Err(ScanError::Cancelled)));
    drop(scanner);
    drop(sink);

    let mut published = Vec::new();
    while let Some(event) = events.recv().await {
        published.push(event.generation());
    }
    assert_eq!(published, [current_generation]);
}
