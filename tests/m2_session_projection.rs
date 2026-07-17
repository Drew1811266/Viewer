use viewer_application::metadata::{
    Marker, MarkerChange, MarkerProjectionPort, MarkerTarget, PortableMarker,
};
use viewer_domain::{
    EntityId, RelativePath,
    file::{FileKind, FileNode, ImageIndexStatus, ImageMetadata, ReviewState, TextIndexStatus},
};
use viewer_infrastructure::search::{index::SessionIndex, text::TextStatus};

fn node(entity_id: EntityId, path: &str, kind: FileKind) -> FileNode {
    FileNode {
        entity_id,
        relative_path: RelativePath::parse(path).unwrap(),
        kind,
        size: if kind == FileKind::Directory { 0 } else { 128 },
        modified_ns: 7,
    }
}

fn target(node: &FileNode) -> MarkerTarget {
    MarkerTarget {
        entity_id: node.entity_id,
        relative_path: node.relative_path.clone(),
        kind: node.kind,
        size: node.size,
        modified_ns: node.modified_ns,
    }
}

#[test]
fn identical_scan_upsert_preserves_marker_and_derived_projection() {
    let directory = tempfile::tempdir().unwrap();
    let index = SessionIndex::open(directory.path().join("session.sqlite")).unwrap();
    let folder = node(EntityId::new(), "id-1", FileKind::Directory);
    let image = node(EntityId::new(), "id-1/front.png", FileKind::Png);
    let text = node(EntityId::new(), "id-1/prompt.md", FileKind::Markdown);
    index
        .upsert_batch(&[folder, image.clone(), text.clone()])
        .unwrap();
    index
        .sync_markers(&[MarkerChange {
            target: target(&image),
            marker: Marker {
                review_state: Some(ReviewState::Keep),
                favorite: true,
            },
        }])
        .unwrap();
    index
        .replace_image_metadata(
            image.entity_id,
            &image.relative_path,
            Ok(ImageMetadata {
                width: 2_000,
                height: 1_000,
            }),
        )
        .unwrap();
    index
        .replace_text(
            text.entity_id,
            &text.relative_path,
            &TextStatus::Indexed("正面产品图".into()),
        )
        .unwrap();

    index.upsert_batch(&[image.clone(), text.clone()]).unwrap();

    let projected = index.indexed_node(image.entity_id).unwrap().unwrap();
    assert_eq!(projected.marker.review_state, Some(ReviewState::Keep));
    assert!(projected.marker.favorite);
    assert_eq!(
        projected.image_metadata,
        Some(ImageMetadata {
            width: 2_000,
            height: 1_000,
        })
    );
    assert_eq!(projected.image_status, ImageIndexStatus::Ready);
    assert_eq!(
        index
            .indexed_node(text.entity_id)
            .unwrap()
            .unwrap()
            .text_status,
        TextIndexStatus::Ready
    );
}

#[test]
fn portable_hydration_matches_exact_relative_paths_and_ignores_stale_rows() {
    let directory = tempfile::tempdir().unwrap();
    let index = SessionIndex::open(directory.path().join("session.sqlite")).unwrap();
    let file = node(EntityId::new(), "catalog/id-1/front.jpg", FileKind::Jpeg);
    index
        .upsert_batch(&[
            node(EntityId::new(), "catalog", FileKind::Directory),
            node(EntityId::new(), "catalog/id-1", FileKind::Directory),
            file.clone(),
        ])
        .unwrap();
    let marker = Marker {
        review_state: Some(ReviewState::Pending),
        favorite: true,
    };

    let hydrated = index
        .hydrate_markers(&[
            PortableMarker {
                relative_path: file.relative_path.clone(),
                kind: file.kind,
                marker,
                evidence_size: Some(file.size),
                evidence_modified_ns: Some(file.modified_ns),
                content_hash: None,
            },
            PortableMarker {
                relative_path: RelativePath::parse("catalog/deleted.jpg").unwrap(),
                kind: FileKind::Jpeg,
                marker: Marker {
                    review_state: Some(ReviewState::Reject),
                    favorite: false,
                },
                evidence_size: Some(1),
                evidence_modified_ns: Some(1),
                content_hash: None,
            },
        ])
        .unwrap();

    assert_eq!(hydrated, 1);
    assert_eq!(
        index.indexed_node(file.entity_id).unwrap().unwrap().marker,
        marker
    );
}

#[test]
fn batch_marker_projection_rolls_back_when_one_entity_is_missing() {
    let directory = tempfile::tempdir().unwrap();
    let index = SessionIndex::open(directory.path().join("session.sqlite")).unwrap();
    let existing = node(EntityId::new(), "id-1/front.jpg", FileKind::Jpeg);
    let missing = node(EntityId::new(), "id-1/missing.jpg", FileKind::Jpeg);
    index
        .upsert_batch(&[
            node(EntityId::new(), "id-1", FileKind::Directory),
            existing.clone(),
        ])
        .unwrap();

    assert!(
        index
            .sync_markers(&[
                MarkerChange {
                    target: target(&existing),
                    marker: Marker {
                        review_state: Some(ReviewState::Keep),
                        favorite: true,
                    },
                },
                MarkerChange {
                    target: target(&missing),
                    marker: Marker {
                        review_state: Some(ReviewState::Reject),
                        favorite: false,
                    },
                },
            ])
            .is_err()
    );
    assert_eq!(
        index
            .indexed_node(existing.entity_id)
            .unwrap()
            .unwrap()
            .marker,
        Marker::default()
    );
}

#[test]
fn image_metadata_requires_a_current_image_path_and_nonzero_dimensions() {
    let directory = tempfile::tempdir().unwrap();
    let index = SessionIndex::open(directory.path().join("session.sqlite")).unwrap();
    let image = node(EntityId::new(), "id-1/front.jpg", FileKind::Jpeg);
    let text = node(EntityId::new(), "id-1/notes.txt", FileKind::Text);
    index
        .upsert_batch(&[
            node(EntityId::new(), "id-1", FileKind::Directory),
            image.clone(),
            text.clone(),
        ])
        .unwrap();

    for invalid in [
        ImageMetadata {
            width: 0,
            height: 10,
        },
        ImageMetadata {
            width: 10,
            height: 0,
        },
    ] {
        assert!(
            index
                .replace_image_metadata(image.entity_id, &image.relative_path, Ok(invalid))
                .is_err()
        );
    }
    assert!(
        index
            .replace_image_metadata(
                image.entity_id,
                &RelativePath::parse("id-1/other.jpg").unwrap(),
                Ok(ImageMetadata {
                    width: 10,
                    height: 10,
                }),
            )
            .is_err()
    );
    assert!(
        index
            .replace_image_metadata(
                text.entity_id,
                &text.relative_path,
                Ok(ImageMetadata {
                    width: 10,
                    height: 10,
                }),
            )
            .is_err()
    );
}

#[test]
fn derived_index_progress_is_monotonic_and_counts_isolated_failures() {
    let directory = tempfile::tempdir().unwrap();
    let index = SessionIndex::open(directory.path().join("session.sqlite")).unwrap();
    let image_ready = node(EntityId::new(), "ready.png", FileKind::Png);
    let image_failed = node(EntityId::new(), "failed.jpg", FileKind::Jpeg);
    let text_ready = node(EntityId::new(), "ready.md", FileKind::Markdown);
    let text_skipped = node(EntityId::new(), "skipped.txt", FileKind::Text);
    index
        .upsert_batch(&[
            image_ready.clone(),
            image_failed.clone(),
            text_ready.clone(),
            text_skipped.clone(),
        ])
        .unwrap();
    let initial = index.index_progress().unwrap();
    assert_eq!((initial.images_total, initial.text_total), (2, 2));
    assert_eq!(initial.completed_items(), 0);

    index
        .replace_image_metadata(
            image_ready.entity_id,
            &image_ready.relative_path,
            Ok(ImageMetadata {
                width: 640,
                height: 480,
            }),
        )
        .unwrap();
    index
        .replace_image_metadata(
            image_failed.entity_id,
            &image_failed.relative_path,
            Err(ImageIndexStatus::Failed),
        )
        .unwrap();
    index
        .replace_text(
            text_ready.entity_id,
            &text_ready.relative_path,
            &TextStatus::Indexed("说明".into()),
        )
        .unwrap();
    index
        .replace_text(
            text_skipped.entity_id,
            &text_skipped.relative_path,
            &TextStatus::TooLarge,
        )
        .unwrap();
    let complete = index.index_progress().unwrap();

    assert!(complete.completed_items() >= initial.completed_items());
    assert_eq!((complete.images_ready, complete.images_failed), (1, 1));
    assert_eq!((complete.text_ready, complete.text_skipped), (1, 1));
    assert!(complete.is_complete());
}

#[test]
fn empty_session_index_reports_zero_progress() {
    let directory = tempfile::tempdir().unwrap();
    let index = SessionIndex::open(directory.path().join("session.sqlite")).unwrap();

    let progress = index.index_progress().unwrap();

    assert_eq!(progress.total_items(), 0);
    assert_eq!(progress.completed_items(), 0);
    assert!(progress.is_complete());
}
