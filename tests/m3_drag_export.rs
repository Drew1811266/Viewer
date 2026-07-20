use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use viewer_application::{
    FinderDragPort, ProjectAccess, ProjectProbeError, ProjectProbePort,
    file_commands::MAX_FILE_COMMAND_ITEMS,
    finder_drag::{FinderDragError, PreparedFinderDrag, begin_finder_drag, prepare_finder_drag},
};
use viewer_desktop::{dto::BeginFinderDragRequestDto, error::CommandError, state::DesktopRuntime};
use viewer_domain::{
    EntityId, RelativePath, SessionId,
    file::{FileKind, FileNode},
    search::Generation,
};
use viewer_infrastructure::search::index::SessionIndex;

struct IndexedProject {
    root: tempfile::TempDir,
    _index_directory: tempfile::TempDir,
    index: SessionIndex,
    front_id: EntityId,
    prompt_id: EntityId,
}

struct FixedProbe;

impl ProjectProbePort for FixedProbe {
    fn probe(&self, _root: &std::path::Path) -> Result<ProjectAccess, ProjectProbeError> {
        Ok(ProjectAccess::ReadOnly)
    }
}

impl IndexedProject {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("catalog/id-1")).unwrap();
        fs::write(root.path().join("catalog/id-1/front.jpg"), b"jpg").unwrap();
        fs::write(root.path().join("catalog/id-1/prompt.txt"), b"prompt").unwrap();
        let front_id = entity_id_for_path(&root.path().join("catalog/id-1/front.jpg"));
        let prompt_id = entity_id_for_path(&root.path().join("catalog/id-1/prompt.txt"));
        let index_directory = tempfile::tempdir().unwrap();
        let index = SessionIndex::open(index_directory.path().join("session.sqlite")).unwrap();
        index
            .upsert_batch(&[
                node(1, "catalog", FileKind::Directory),
                node(2, "catalog/id-1", FileKind::Directory),
                node_with_id(front_id, "catalog/id-1/front.jpg", FileKind::Jpeg),
                node_with_id(prompt_id, "catalog/id-1/prompt.txt", FileKind::Text),
            ])
            .unwrap();
        Self {
            root,
            _index_directory: index_directory,
            index,
            front_id,
            prompt_id,
        }
    }
}

#[derive(Default)]
struct RecordingPort {
    paths: Mutex<Vec<PathBuf>>,
}

impl FinderDragPort for RecordingPort {
    fn begin_drag(&self, selection: &PreparedFinderDrag) -> Result<(), FinderDragError> {
        *self.paths.lock().unwrap() = selection.files().to_vec();
        Ok(())
    }
}

#[test]
fn multi_file_export_resolves_ordered_current_entities_and_never_accepts_paths() {
    let project = IndexedProject::new();
    let prepared = prepare_finder_drag(
        project.root.path(),
        &project.index,
        &[project.prompt_id, project.front_id],
    )
    .unwrap();
    let port = RecordingPort::default();

    let receipt = begin_finder_drag(&port, &prepared).unwrap();

    assert_eq!(receipt.file_count, 2);
    assert_eq!(
        port.paths
            .lock()
            .unwrap()
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        ["prompt.txt", "front.jpg"]
    );
    assert!(prepared.files().iter().all(|path| path.is_absolute()));
}

#[test]
fn preparation_rejects_empty_duplicate_stale_directory_and_missing_files() {
    let project = IndexedProject::new();
    assert_eq!(
        prepare_finder_drag(project.root.path(), &project.index, &[]).unwrap_err(),
        FinderDragError::EmptySelection
    );
    assert_eq!(
        prepare_finder_drag(
            project.root.path(),
            &project.index,
            &[project.front_id, project.front_id],
        )
        .unwrap_err(),
        FinderDragError::DuplicateSelection
    );
    assert_eq!(
        prepare_finder_drag(
            project.root.path(),
            &project.index,
            &[EntityId::from_u128(99)],
        )
        .unwrap_err(),
        FinderDragError::EntityNotFound
    );
    assert_eq!(
        prepare_finder_drag(
            project.root.path(),
            &project.index,
            &[EntityId::from_u128(2)],
        )
        .unwrap_err(),
        FinderDragError::DirectoryNotAllowed
    );
    fs::remove_file(project.root.path().join("catalog/id-1/front.jpg")).unwrap();
    assert_eq!(
        prepare_finder_drag(project.root.path(), &project.index, &[project.front_id],).unwrap_err(),
        FinderDragError::NotRegularFile
    );
}

#[test]
fn preparation_rejects_an_excessive_selection_before_resolving_entities() {
    let project = IndexedProject::new();
    let ids = (0..=MAX_FILE_COMMAND_ITEMS)
        .map(|index| EntityId::from_u128(index as u128 + 10_000))
        .collect::<Vec<_>>();

    assert_eq!(
        prepare_finder_drag(project.root.path(), &project.index, &ids).unwrap_err(),
        FinderDragError::TooManySelection
    );
    let error = CommandError::from(FinderDragError::TooManySelection);
    assert_eq!(error.code, "finder_drag_selection_too_large");
    assert!(error.user_message.contains("数量过多"));
}

#[test]
fn changed_selection_maps_to_a_safe_refreshable_error() {
    let error = CommandError::from(FinderDragError::EntityNotFound);
    assert_eq!(error.code, "finder_drag_selection_stale");
    assert_eq!(
        error.user_message,
        "部分所选文件已不可用，请刷新项目后重试。"
    );
    assert!(error.retryable);
}

#[cfg(unix)]
#[test]
fn preparation_rejects_an_indexed_id_after_the_file_at_its_path_is_replaced() {
    let project = IndexedProject::new();
    let path = project.root.path().join("catalog/id-1/front.jpg");
    fs::rename(&path, project.root.path().join("catalog/id-1/original.jpg")).unwrap();
    fs::write(&path, b"replacement").unwrap();

    assert_eq!(
        prepare_finder_drag(project.root.path(), &project.index, &[project.front_id]).unwrap_err(),
        FinderDragError::EntityNotFound
    );
}

#[cfg(unix)]
#[test]
fn prepared_drag_rejects_a_file_replaced_before_native_start() {
    let project = IndexedProject::new();
    let prepared =
        prepare_finder_drag(project.root.path(), &project.index, &[project.front_id]).unwrap();
    let path = project.root.path().join("catalog/id-1/front.jpg");
    fs::rename(&path, project.root.path().join("catalog/id-1/original.jpg")).unwrap();
    fs::write(&path, b"replacement").unwrap();

    assert_eq!(
        prepared.revalidate_file(0).unwrap_err(),
        FinderDragError::EntityNotFound
    );
}

#[cfg(unix)]
#[test]
fn preparation_rejects_symlink_entries_even_when_the_target_is_inside_or_outside_root() {
    use std::os::unix::fs::symlink;

    let project = IndexedProject::new();
    let outside = tempfile::NamedTempFile::new().unwrap();
    symlink(outside.path(), project.root.path().join("outside-link.jpg")).unwrap();
    symlink(
        project.root.path().join("catalog/id-1/front.jpg"),
        project.root.path().join("inside-link.jpg"),
    )
    .unwrap();
    project
        .index
        .upsert_batch(&[
            node(5, "outside-link.jpg", FileKind::Jpeg),
            node(6, "inside-link.jpg", FileKind::Jpeg),
        ])
        .unwrap();

    for entity_id in [EntityId::from_u128(5), EntityId::from_u128(6)] {
        assert_eq!(
            prepare_finder_drag(project.root.path(), &project.index, &[entity_id]).unwrap_err(),
            FinderDragError::SymlinkNotAllowed
        );
    }
}

#[test]
fn desktop_drag_request_accepts_entity_ids_and_rejects_any_path_field() {
    let valid = serde_json::from_value::<BeginFinderDragRequestDto>(serde_json::json!({
        "sessionId": "c5e0f046-5518-4bce-9887-d86bc44ef171",
        "generation": 7,
        "entityIds": ["77745c1c-54a8-4293-889c-9609ea73c2ad"]
    }));
    assert!(valid.is_ok());

    let with_path = serde_json::from_value::<BeginFinderDragRequestDto>(serde_json::json!({
        "sessionId": "c5e0f046-5518-4bce-9887-d86bc44ef171",
        "generation": 7,
        "entityIds": ["77745c1c-54a8-4293-889c-9609ea73c2ad"],
        "path": "/Users/private/project/image.jpg"
    }));
    assert!(with_path.is_err());
}

#[tokio::test]
async fn close_start_invalidates_the_native_drag_guard_before_cleanup_can_yield() {
    let project = IndexedProject::new();
    let cache = tempfile::tempdir().unwrap();
    let runtime = DesktopRuntime::new(cache.path().to_path_buf(), Arc::new(FixedProbe));
    let snapshot = runtime.open_project(project.root.path()).await.unwrap();

    let close = runtime.close_project();
    tokio::pin!(close);
    let close_finished = tokio::select! {
        biased;
        result = &mut close => {
            result.unwrap();
            true
        }
        () = tokio::task::yield_now() => false,
    };

    let guarded = runtime.run_if_project_current(
        snapshot.session_id.parse::<SessionId>().unwrap(),
        Generation::new(snapshot.generation),
        || "must not run",
    );
    assert_eq!(guarded.unwrap_err().code, "stale_project_session");

    if !close_finished {
        close.await.unwrap();
    }
}

fn node(id: u128, path: &str, kind: FileKind) -> FileNode {
    node_with_id(EntityId::from_u128(id), path, kind)
}

fn node_with_id(entity_id: EntityId, path: &str, kind: FileKind) -> FileNode {
    FileNode {
        entity_id,
        relative_path: RelativePath::parse(path).unwrap(),
        kind,
        size: 1,
        modified_ns: 1,
    }
}

#[cfg(unix)]
fn entity_id_for_path(path: &std::path::Path) -> EntityId {
    use std::os::unix::fs::MetadataExt;
    let metadata = fs::metadata(path).unwrap();
    EntityId::from_u128((u128::from(metadata.dev()) << 64) | u128::from(metadata.ino()))
}

#[cfg(not(unix))]
fn entity_id_for_path(path: &std::path::Path) -> EntityId {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    EntityId::from_u128(u128::from(hasher.finish()))
}
