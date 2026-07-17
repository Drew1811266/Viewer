use std::collections::HashMap;
use viewer_application::browse::{BrowseService, FolderWorkspace};
use viewer_domain::{
    EntityId, RelativePath,
    file::{FileKind, FileNode},
};
use viewer_infrastructure::search::index::SessionIndex;

struct IndexedProject {
    _directory: tempfile::TempDir,
    index: SessionIndex,
    ids: HashMap<String, EntityId>,
}

impl IndexedProject {
    fn new(entries: &[(&str, FileKind)]) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let index = SessionIndex::open(directory.path().join("session.sqlite")).unwrap();
        let mut ids = HashMap::new();
        let nodes = entries
            .iter()
            .enumerate()
            .map(|(offset, (path, kind))| {
                let entity_id = EntityId::from_u128(offset as u128 + 1);
                ids.insert((*path).to_owned(), entity_id);
                FileNode {
                    entity_id,
                    relative_path: RelativePath::parse(path).unwrap(),
                    kind: *kind,
                    size: if *kind == FileKind::Directory {
                        0
                    } else {
                        (offset as u64 + 1) * 10
                    },
                    modified_ns: offset as i128 + 1,
                }
            })
            .collect::<Vec<_>>();
        index.upsert_batch(&nodes).unwrap();
        Self {
            _directory: directory,
            index,
            ids,
        }
    }

    fn id(&self, path: &str) -> EntityId {
        self.ids[path]
    }
}

#[test]
fn category_query_returns_descendant_content_folders_at_arbitrary_depth() {
    let project = IndexedProject::new(&[
        ("catalog", FileKind::Directory),
        ("catalog/shoes", FileKind::Directory),
        ("catalog/shoes/id-001", FileKind::Directory),
        ("catalog/shoes/id-001/front.jpg", FileKind::Jpeg),
        ("catalog/shoes/id-001/prompt.md", FileKind::Markdown),
        ("empty", FileKind::Directory),
    ]);
    let service = BrowseService::new(&project.index);

    let tree = service.folder_tree().unwrap();

    assert_eq!(
        tree.iter()
            .map(|folder| folder.relative_path.as_str())
            .collect::<Vec<_>>(),
        ["catalog", "catalog/shoes", "catalog/shoes/id-001", "empty"]
    );
    assert_eq!(tree[0].parent_entity_id, None);
    assert_eq!(tree[1].parent_entity_id, Some(project.id("catalog")));
    assert_eq!(tree[2].parent_entity_id, Some(project.id("catalog/shoes")));

    let workspace = service
        .folder_workspace(Some(project.id("catalog")))
        .unwrap();
    let FolderWorkspace::Category { folders } = workspace else {
        panic!("catalog should be a category workspace")
    };
    assert_eq!(folders.len(), 1);
    assert_eq!(folders[0].relative_path.as_str(), "catalog/shoes/id-001");
    assert_eq!((folders[0].image_count, folders[0].text_count), (1, 1));
}

#[test]
fn content_query_splits_images_and_text_and_selects_four_stable_representatives() {
    let project = IndexedProject::new(&[
        ("id-001", FileKind::Directory),
        ("id-001/01.jpg", FileKind::Jpeg),
        ("id-001/02.png", FileKind::Png),
        ("id-001/03.jpg", FileKind::Jpeg),
        ("id-001/04.jpg", FileKind::Jpeg),
        ("id-001/05.jpg", FileKind::Jpeg),
        ("id-001/notes.txt", FileKind::Text),
        ("id-001/prompt.markdown", FileKind::Markdown),
    ]);
    let service = BrowseService::new(&project.index);

    let workspace = service
        .folder_workspace(Some(project.id("id-001")))
        .unwrap();
    let FolderWorkspace::Content { images, text_files } = workspace else {
        panic!("id-001 should be a content workspace")
    };
    assert_eq!(
        names(&images),
        ["01.jpg", "02.png", "03.jpg", "04.jpg", "05.jpg"]
    );
    assert_eq!(names(&text_files), ["notes.txt", "prompt.markdown"]);

    let root = service.folder_workspace(None).unwrap();
    let FolderWorkspace::Category { folders } = root else {
        panic!("root should summarize its content folder")
    };
    assert_eq!(folders[0].image_count, 5);
    assert_eq!(
        names(&folders[0].representative_images),
        ["01.jpg", "02.png", "03.jpg", "04.jpg"]
    );
}

#[test]
fn project_root_can_be_a_content_workspace_and_empty_folders_remain_visible() {
    let project = IndexedProject::new(&[
        ("empty", FileKind::Directory),
        ("front.jpg", FileKind::Jpeg),
        ("README.txt", FileKind::Text),
    ]);
    let service = BrowseService::new(&project.index);

    let FolderWorkspace::Content { images, text_files } = service.folder_workspace(None).unwrap()
    else {
        panic!("root should show its direct content")
    };
    assert_eq!(names(&images), ["front.jpg"]);
    assert_eq!(names(&text_files), ["README.txt"]);
    assert!(
        service
            .folder_tree()
            .unwrap()
            .iter()
            .any(|folder| folder.name == "empty")
    );

    assert_eq!(
        service
            .node_by_relative_path(&RelativePath::parse("front.jpg").unwrap())
            .unwrap()
            .unwrap()
            .entity_id,
        project.id("front.jpg")
    );
}

#[test]
fn duplicate_folder_names_keep_distinct_full_relative_paths() {
    let project = IndexedProject::new(&[
        ("a", FileKind::Directory),
        ("a/id-1", FileKind::Directory),
        ("b", FileKind::Directory),
        ("b/id-1", FileKind::Directory),
    ]);
    let service = BrowseService::new(&project.index);

    let duplicates = service
        .folder_tree()
        .unwrap()
        .into_iter()
        .filter(|folder| folder.name == "id-1")
        .map(|folder| folder.relative_path.as_str().to_owned())
        .collect::<Vec<_>>();

    assert_eq!(duplicates, ["a/id-1", "b/id-1"]);
}

#[test]
fn aggregate_workspace_is_explicit_and_collects_descendant_files_only() {
    let project = IndexedProject::new(&[
        ("catalog", FileKind::Directory),
        ("catalog/id-1", FileKind::Directory),
        ("catalog/id-1/front.jpg", FileKind::Jpeg),
        ("catalog/id-2", FileKind::Directory),
        ("catalog/id-2/prompt.txt", FileKind::Text),
        ("outside", FileKind::Directory),
        ("outside/back.jpg", FileKind::Jpeg),
    ]);
    let service = BrowseService::new(&project.index);

    let FolderWorkspace::Content { images, text_files } = service
        .aggregate_workspace(Some(project.id("catalog")))
        .unwrap()
    else {
        panic!("explicit aggregate should return mixed descendant files")
    };

    assert_eq!(names(&images), ["front.jpg"]);
    assert_eq!(names(&text_files), ["prompt.txt"]);
}

fn names(files: &[viewer_application::browse::BrowserFile]) -> Vec<&str> {
    files.iter().map(|file| file.name.as_str()).collect()
}
