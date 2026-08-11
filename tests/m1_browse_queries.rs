use std::collections::HashMap;
use viewer_application::browse::{BrowseService, FolderWorkspace};
use viewer_domain::{
    EntityId, RelativePath,
    file::{FileKind, FileNode},
    video::{VideoMetadata, VideoProbeStatus},
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
    assert_eq!(
        (folders[0].image_count, folders[0].other_file_count),
        (1, 1)
    );
}

#[test]
fn content_query_splits_images_and_other_files() {
    let project = IndexedProject::new(&[
        ("id-001", FileKind::Directory),
        ("id-001/01.jpg", FileKind::Jpeg),
        ("id-001/02.png", FileKind::Png),
        ("id-001/source.webp", FileKind::UnsupportedImage),
        ("id-001/preview.mp4", FileKind::Video),
        ("id-001/notes.txt", FileKind::Text),
        ("id-001/license.pdf", FileKind::Other),
    ]);
    let service = BrowseService::new(&project.index);

    let workspace = service
        .folder_workspace(Some(project.id("id-001")))
        .unwrap();
    let FolderWorkspace::Content {
        images,
        videos,
        other_files,
    } = workspace
    else {
        panic!("id-001 should be a content workspace")
    };
    assert_eq!(names(&images), ["01.jpg", "02.png", "source.webp"]);
    assert_eq!(names(&videos), ["preview.mp4"]);
    assert_eq!(videos[0].video_metadata, None);
    assert_eq!(names(&other_files), ["license.pdf", "notes.txt"]);

    let root = service.folder_workspace(None).unwrap();
    let FolderWorkspace::Category { folders } = root else {
        panic!("root should summarize its content folder")
    };
    assert_eq!(folders[0].image_count, 3);
    assert_eq!(folders[0].video_count, 1);
    assert_eq!(
        names(&folders[0].representative_images),
        ["01.jpg", "02.png", "source.webp"]
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

    let FolderWorkspace::Content {
        images,
        videos,
        other_files,
    } = service.folder_workspace(None).unwrap()
    else {
        panic!("root should show its direct content")
    };
    assert_eq!(names(&images), ["front.jpg"]);
    assert!(videos.is_empty());
    assert_eq!(names(&other_files), ["README.txt"]);
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
        ("catalog/id-1/clip.webm", FileKind::Video),
        ("catalog/id-2", FileKind::Directory),
        ("catalog/id-2/prompt.txt", FileKind::Text),
        ("outside", FileKind::Directory),
        ("outside/back.jpg", FileKind::Jpeg),
    ]);
    let service = BrowseService::new(&project.index);

    let FolderWorkspace::Content {
        images,
        videos,
        other_files,
    } = service
        .aggregate_workspace(Some(project.id("catalog")))
        .unwrap()
    else {
        panic!("explicit aggregate should return mixed descendant files")
    };

    assert_eq!(names(&images), ["front.jpg"]);
    assert_eq!(names(&videos), ["clip.webm"]);
    assert_eq!(names(&other_files), ["prompt.txt"]);
}

#[test]
fn content_query_projects_video_metadata_from_the_companion_table() {
    let project = IndexedProject::new(&[("clip.mp4", FileKind::Video)]);
    let metadata = VideoMetadata {
        duration_us: None,
        display_width: Some(1_080),
        display_height: Some(1_920),
        rotation_degrees: 90,
        frame_rate_millihertz: None,
        video_codec: Some("hevc".to_owned()),
        audio_codec: None,
        probe_status: VideoProbeStatus::Ready,
    };
    project
        .index
        .replace_video_metadata(project.id("clip.mp4"), &metadata, 9)
        .unwrap();
    let service = BrowseService::new(&project.index);

    let FolderWorkspace::Content { videos, .. } = service.folder_workspace(None).unwrap() else {
        panic!("root should expose its video")
    };

    assert_eq!(videos.len(), 1);
    assert_eq!(videos[0].video_metadata, Some(metadata));
}

fn names(files: &[viewer_application::browse::BrowserFile]) -> Vec<&str> {
    files.iter().map(|file| file.name.as_str()).collect()
}
