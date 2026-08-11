use std::collections::HashMap;

use viewer_application::browse::{
    BrowseService, FolderWorkspace, SelectionAgreement, SelectionTypeCounts,
};
use viewer_domain::{
    EntityId, RelativePath,
    file::{FileKind, FileNode, Marker, ReviewState},
    search::{SearchSort, SearchSortKey, SortDirection},
};
use viewer_infrastructure::search::index::SessionIndex;

struct IndexedProject {
    _directory: tempfile::TempDir,
    index: SessionIndex,
    ids: HashMap<String, EntityId>,
}

impl IndexedProject {
    fn new(entries: &[(&str, FileKind, u64)]) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let index = SessionIndex::open(directory.path().join("session.sqlite")).unwrap();
        let mut ids = HashMap::new();
        let nodes = entries
            .iter()
            .enumerate()
            .map(|(offset, (path, kind, size))| {
                let entity_id = EntityId::from_u128(offset as u128 + 1);
                ids.insert((*path).to_owned(), entity_id);
                FileNode {
                    entity_id,
                    relative_path: RelativePath::parse(path).unwrap(),
                    kind: *kind,
                    size: *size,
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

    fn mark(&self, path: &str, review_state: Option<ReviewState>, favorite: bool) {
        self.index
            .set_review_metadata(self.id(path), review_state, favorite)
            .unwrap();
    }
}

fn fixture() -> IndexedProject {
    let project = IndexedProject::new(&[
        ("catalog", FileKind::Directory, 0),
        ("catalog/id10", FileKind::Directory, 0),
        ("catalog/id10/image1.jpg", FileKind::Jpeg, 11),
        ("catalog/id2", FileKind::Directory, 0),
        ("catalog/id2/image10.jpg", FileKind::Jpeg, 100),
        ("catalog/id2/image2.jpg", FileKind::Jpeg, 20),
        ("catalog/id2/nested", FileKind::Directory, 0),
        ("catalog/id2/nested/note.txt", FileKind::Text, 7),
        ("catalog/id2/nested/license.pdf", FileKind::Other, 29),
        ("empty", FileKind::Directory, 0),
    ]);
    project.mark("catalog", Some(ReviewState::Reject), true);
    project.mark("catalog/id2", Some(ReviewState::Keep), false);
    project.mark("catalog/id2/image10.jpg", Some(ReviewState::Keep), true);
    project.mark("catalog/id2/image2.jpg", Some(ReviewState::Pending), false);
    project.mark(
        "catalog/id2/nested/note.txt",
        Some(ReviewState::Reject),
        true,
    );
    project
}

#[test]
fn tree_files_and_cards_use_one_natural_order_and_expose_independent_markers() {
    let project = fixture();
    let service = BrowseService::new(&project.index);

    let tree = service.folder_tree().unwrap();
    assert_eq!(
        tree.iter()
            .map(|folder| folder.relative_path.as_str())
            .collect::<Vec<_>>(),
        [
            "catalog",
            "catalog/id2",
            "catalog/id2/nested",
            "catalog/id10",
            "empty",
        ]
    );
    assert_eq!(
        tree.iter()
            .find(|folder| folder.relative_path.as_str() == "catalog")
            .unwrap()
            .marker,
        Marker {
            review_state: Some(ReviewState::Reject),
            favorite: true,
        }
    );
    assert_eq!(
        tree.iter()
            .find(|folder| folder.relative_path.as_str() == "catalog/id2")
            .unwrap()
            .marker
            .review_state,
        Some(ReviewState::Keep)
    );

    let FolderWorkspace::Content { images, .. } = service
        .folder_workspace(Some(project.id("catalog/id2")))
        .unwrap()
    else {
        panic!("id2 should be a content workspace")
    };
    assert_eq!(
        images
            .iter()
            .map(|file| file.name.as_str())
            .collect::<Vec<_>>(),
        ["image2.jpg", "image10.jpg"]
    );
    assert_eq!(images[0].marker.review_state, Some(ReviewState::Pending));

    let FolderWorkspace::Category { folders } = service.folder_workspace(None).unwrap() else {
        panic!("root should be a category workspace")
    };
    assert_eq!(
        folders
            .iter()
            .map(|folder| folder.relative_path.as_str())
            .collect::<Vec<_>>(),
        ["catalog/id2", "catalog/id2/nested", "catalog/id10"]
    );
    let id2 = folders
        .iter()
        .find(|folder| folder.relative_path.as_str() == "catalog/id2")
        .unwrap();
    assert_eq!(id2.marker.review_state, Some(ReviewState::Keep));
    assert_eq!(id2.review_progress.total, 4);
    assert_eq!(id2.review_progress.keep, 1);
    assert_eq!(id2.review_progress.pending, 1);
    assert_eq!(id2.review_progress.reject, 1);
    assert_eq!(id2.review_progress.unmarked, 1);
    assert_eq!(id2.review_progress.favorite, 2);
}

#[test]
fn folder_statistics_cover_unmarked_and_update_from_the_current_projection() {
    let project = fixture();
    let service = BrowseService::new(&project.index);

    let FolderWorkspace::Category { folders } = service.folder_workspace(None).unwrap() else {
        panic!("root should be a category workspace")
    };
    let id10 = folders
        .iter()
        .find(|folder| folder.relative_path.as_str() == "catalog/id10")
        .unwrap();
    assert_eq!(id10.review_progress.total, 1);
    assert_eq!(id10.review_progress.unmarked, 1);
    assert_eq!(id10.review_progress.favorite, 0);
    assert!(
        service
            .folder_tree()
            .unwrap()
            .iter()
            .any(|folder| folder.relative_path.as_str() == "empty")
    );

    project.mark("catalog/id10/image1.jpg", Some(ReviewState::Keep), true);
    let FolderWorkspace::Category { folders } = service.folder_workspace(None).unwrap() else {
        panic!("root should remain a category workspace")
    };
    let id10 = folders
        .iter()
        .find(|folder| folder.relative_path.as_str() == "catalog/id10")
        .unwrap();
    assert_eq!(id10.review_progress.keep, 1);
    assert_eq!(id10.review_progress.unmarked, 0);
    assert_eq!(id10.review_progress.favorite, 1);
}

#[test]
fn content_files_honor_an_explicit_sort_while_defaulting_to_natural_ascending() {
    let project = fixture();
    let service = BrowseService::new(&project.index);

    let FolderWorkspace::Content { images, .. } = service
        .folder_workspace_sorted(
            Some(project.id("catalog/id2")),
            SearchSort {
                key: SearchSortKey::Size,
                direction: SortDirection::Descending,
            },
        )
        .unwrap()
    else {
        panic!("id2 should be a content workspace")
    };
    assert_eq!(
        images
            .iter()
            .map(|file| file.name.as_str())
            .collect::<Vec<_>>(),
        ["image10.jpg", "image2.jpg"]
    );

    let FolderWorkspace::Content { images, .. } = service
        .folder_workspace(Some(project.id("catalog/id2")))
        .unwrap()
    else {
        panic!("id2 should be a content workspace")
    };
    assert_eq!(
        images
            .iter()
            .map(|file| file.name.as_str())
            .collect::<Vec<_>>(),
        ["image2.jpg", "image10.jpg"]
    );
}

#[test]
fn selection_info_aggregates_relative_paths_size_types_and_marker_agreement() {
    let project = fixture();
    let service = BrowseService::new(&project.index);

    let selection = service
        .selection_info(&[
            project.id("catalog/id2/image10.jpg"),
            project.id("catalog/id2/image2.jpg"),
            project.id("catalog/id2/nested/note.txt"),
            project.id("catalog/id2/nested/license.pdf"),
        ])
        .unwrap();
    assert_eq!(selection.total_size, 156);
    assert_eq!(
        selection.types,
        SelectionTypeCounts {
            folders: 0,
            images: 2,
            videos: 0,
            other_files: 2,
        }
    );
    assert_eq!(
        selection
            .relative_paths
            .iter()
            .map(RelativePath::as_str)
            .collect::<Vec<_>>(),
        [
            "catalog/id2/image2.jpg",
            "catalog/id2/image10.jpg",
            "catalog/id2/nested/license.pdf",
            "catalog/id2/nested/note.txt",
        ]
    );
    assert_eq!(selection.common_review, SelectionAgreement::Mixed);
    assert_eq!(selection.common_favorite, SelectionAgreement::Mixed);

    let common = service
        .selection_info(&[
            project.id("catalog/id2"),
            project.id("catalog/id2/image10.jpg"),
        ])
        .unwrap();
    assert_eq!(
        common.common_review,
        SelectionAgreement::Common(Some(ReviewState::Keep))
    );
    assert_eq!(common.common_favorite, SelectionAgreement::Mixed);

    let empty = service.selection_info(&[]).unwrap();
    assert_eq!(empty.common_review, SelectionAgreement::NoneSelected);
    assert_eq!(empty.common_favorite, SelectionAgreement::NoneSelected);
}

#[test]
fn selection_info_counts_videos_separately_from_other_files() {
    let project = IndexedProject::new(&[
        ("clip.mp4", FileKind::Video, 123),
        ("notes.txt", FileKind::Text, 7),
    ]);
    let service = BrowseService::new(&project.index);

    let selection = service
        .selection_info(&[project.id("clip.mp4"), project.id("notes.txt")])
        .unwrap();

    assert_eq!(
        selection.types,
        SelectionTypeCounts {
            folders: 0,
            images: 0,
            videos: 1,
            other_files: 1,
        }
    );
}
