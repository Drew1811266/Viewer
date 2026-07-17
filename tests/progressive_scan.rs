use std::fs;
use viewer_application::{
    ScanPort,
    scan::{ScanError, ScanEvent, ScanRequest},
};
use viewer_domain::{SessionId, file::FileKind, search::Generation};
use viewer_infrastructure::scan::walker::ProjectWalker;
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
