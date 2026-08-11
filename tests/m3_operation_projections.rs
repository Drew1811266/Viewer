use rusqlite::{Connection, params};
use std::{collections::BTreeMap, fs};
use tempfile::TempDir;
use viewer_application::{
    ProjectAccess,
    metadata::{
        FavoritePatch, FileCopyProjection, FileMoveProjection, FilePathMove, Marker, MarkerPatch,
        MarkerProjectionPort, MarkerTarget, OperationProjectionError, OperationProjectionPort,
        PortableMetadataPort, ReviewPatch,
    },
};
use viewer_domain::{
    EntityId, RelativePath,
    file::{FileKind, FileNode, ImageIndexStatus, ImageMetadata, ReviewState, TextIndexStatus},
    search::Generation,
    video::{VideoMetadata, VideoProbeStatus},
};
use viewer_infrastructure::{
    portable::{PortableMarkerStore, PortableProjectMetadata},
    search::{index::SessionIndex, text::TextStatus},
};

fn path(value: &str) -> RelativePath {
    RelativePath::parse(value).unwrap()
}

fn node(entity_id: EntityId, relative_path: &str, kind: FileKind) -> FileNode {
    FileNode {
        entity_id,
        relative_path: path(relative_path),
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

fn portable_store(project: &TempDir) -> (PortableMarkerStore, std::path::PathBuf) {
    let metadata =
        PortableProjectMetadata::open(project.path(), ProjectAccess::ReadWrite, 1).unwrap();
    let database = metadata.database_path().unwrap().to_owned();
    (
        PortableMarkerStore::open(&database, true).unwrap(),
        database,
    )
}

fn marker_ids(database: &std::path::Path) -> BTreeMap<String, String> {
    let connection = Connection::open(database).unwrap();
    let mut statement = connection
        .prepare("SELECT relative_path, marker_id FROM markers ORDER BY relative_path")
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn portable_subtree_move_preserves_marker_identity_and_never_touches_source_files() {
    let project = TempDir::new().unwrap();
    let source_file = project.path().join("products/id-1/front.jpg");
    fs::create_dir_all(source_file.parent().unwrap()).unwrap();
    fs::write(&source_file, b"source bytes stay outside portable metadata").unwrap();
    let source_before = fs::read(&source_file).unwrap();
    let (store, database) = portable_store(&project);
    let folder = node(EntityId::new(), "products/id-1", FileKind::Directory);
    let image = node(EntityId::new(), "products/id-1/front.jpg", FileKind::Jpeg);
    store
        .apply_batch(
            &[target(&folder), target(&image)],
            MarkerPatch {
                review: ReviewPatch::Set(ReviewState::Keep),
                favorite: FavoritePatch::Set(true),
            },
            10,
        )
        .unwrap();
    let before = marker_ids(&database);

    let moved = store
        .move_paths(
            &[FilePathMove {
                source: path("products/id-1"),
                destination: path("archive/id-1"),
            }],
            false,
            11,
        )
        .unwrap();

    assert_eq!(moved, 2);
    assert_eq!(fs::read(source_file).unwrap(), source_before);
    assert!(
        store
            .markers_for_paths(&[path("products/id-1"), path("products/id-1/front.jpg")])
            .unwrap()
            .is_empty()
    );
    let recovered = store
        .markers_for_paths(&[path("archive/id-1"), path("archive/id-1/front.jpg")])
        .unwrap();
    assert_eq!(recovered.len(), 2);
    assert!(recovered.iter().all(|stored| {
        stored.marker
            == Marker {
                review_state: Some(ReviewState::Keep),
                favorite: true,
            }
    }));
    let after = marker_ids(&database);
    assert_eq!(after["archive/id-1"], before["products/id-1"]);
    assert_eq!(
        after["archive/id-1/front.jpg"],
        before["products/id-1/front.jpg"]
    );
}

#[test]
fn portable_path_collisions_reject_the_whole_batch_and_paths_are_type_safe() {
    let project = TempDir::new().unwrap();
    let (store, database) = portable_store(&project);
    let first = node(EntityId::new(), "a.jpg", FileKind::Jpeg);
    let second = node(EntityId::new(), "b.jpg", FileKind::Jpeg);
    store
        .apply_batch(
            &[target(&first), target(&second)],
            MarkerPatch {
                review: ReviewPatch::Set(ReviewState::Pending),
                favorite: FavoritePatch::Unchanged,
            },
            1,
        )
        .unwrap();
    let before = marker_ids(&database);

    assert!(
        store
            .move_paths(
                &[
                    FilePathMove {
                        source: path("a.jpg"),
                        destination: path("same.jpg"),
                    },
                    FilePathMove {
                        source: path("b.jpg"),
                        destination: path("SAME.jpg"),
                    },
                ],
                false,
                2,
            )
            .is_err()
    );
    assert_eq!(marker_ids(&database), before);
    assert!(
        store
            .move_paths(
                &[FilePathMove {
                    source: path("a.jpg"),
                    destination: path("b.jpg"),
                }],
                true,
                3,
            )
            .is_err()
    );
    assert_eq!(marker_ids(&database), before);
    let read_only = PortableMarkerStore::open(&database, false).unwrap();
    assert!(
        read_only
            .move_paths(
                &[FilePathMove {
                    source: path("a.jpg"),
                    destination: path("renamed.jpg"),
                }],
                true,
                4,
            )
            .is_err()
    );
    assert_eq!(marker_ids(&database), before);
    for invalid in ["/absolute.jpg", "../outside.jpg", ".viewer/metadata.sqlite"] {
        assert!(RelativePath::parse(invalid).is_err(), "accepted {invalid}");
    }
    Connection::open(&database)
        .unwrap()
        .execute(
            "UPDATE markers SET relative_path = '/absolute.jpg' WHERE relative_path = 'a.jpg'",
            [],
        )
        .unwrap();
    assert!(
        store
            .move_paths(
                &[FilePathMove {
                    source: path("b.jpg"),
                    destination: path("renamed.jpg"),
                }],
                true,
                5,
            )
            .is_err()
    );
    let persisted = marker_ids(&database);
    assert!(persisted.contains_key("/absolute.jpg"));
    assert!(persisted.contains_key("b.jpg"));
    assert!(!persisted.contains_key("renamed.jpg"));
}

#[test]
fn marker_and_session_path_swaps_stage_away_from_unique_constraints() {
    let project = TempDir::new().unwrap();
    let (store, portable_database) = portable_store(&project);
    let first = node(EntityId::new(), "a.png", FileKind::Png);
    let second = node(EntityId::new(), "b.png", FileKind::Png);
    store
        .apply_batch(
            &[target(&first), target(&second)],
            MarkerPatch {
                review: ReviewPatch::Set(ReviewState::Keep),
                favorite: FavoritePatch::Set(true),
            },
            1,
        )
        .unwrap();
    let portable_before = marker_ids(&portable_database);
    store
        .move_paths(
            &[
                FilePathMove {
                    source: path("a.png"),
                    destination: path("b.png"),
                },
                FilePathMove {
                    source: path("b.png"),
                    destination: path("a.png"),
                },
            ],
            true,
            2,
        )
        .unwrap();
    let portable_after = marker_ids(&portable_database);
    assert_eq!(portable_after["b.png"], portable_before["a.png"]);
    assert_eq!(portable_after["a.png"], portable_before["b.png"]);

    let session_database = project.path().join("swap-session.sqlite");
    let index = SessionIndex::open(session_database).unwrap();
    index
        .upsert_batch(&[first.clone(), second.clone()], Generation::new(1))
        .unwrap();
    index
        .replace_image_metadata(
            first.entity_id,
            &first.relative_path,
            Ok(ImageMetadata {
                width: 111,
                height: 222,
            }),
        )
        .unwrap();
    index
        .replace_image_metadata(
            second.entity_id,
            &second.relative_path,
            Ok(ImageMetadata {
                width: 333,
                height: 444,
            }),
        )
        .unwrap();
    index
        .apply_move(
            &[
                FileMoveProjection {
                    source: first.clone(),
                    destination: FileNode {
                        relative_path: path("b.png"),
                        ..first.clone()
                    },
                },
                FileMoveProjection {
                    source: second.clone(),
                    destination: FileNode {
                        relative_path: path("a.png"),
                        ..second.clone()
                    },
                },
            ],
            true,
        )
        .unwrap();
    let projected_first = index.indexed_node(first.entity_id).unwrap().unwrap();
    let projected_second = index.indexed_node(second.entity_id).unwrap().unwrap();
    assert_eq!(projected_first.node.relative_path, path("b.png"));
    assert_eq!(
        projected_first.image_metadata,
        Some(ImageMetadata {
            width: 111,
            height: 222,
        })
    );
    assert_eq!(projected_second.node.relative_path, path("a.png"));
    assert_eq!(
        projected_second.image_metadata,
        Some(ImageMetadata {
            width: 333,
            height: 444,
        })
    );

    index
        .apply_move(
            &[FileMoveProjection {
                source: projected_second.node.clone(),
                destination: FileNode {
                    relative_path: path("A.png"),
                    ..projected_second.node
                },
            }],
            false,
        )
        .unwrap();
    assert_eq!(
        index
            .indexed_node(second.entity_id)
            .unwrap()
            .unwrap()
            .node
            .relative_path,
        path("A.png")
    );
}

#[test]
fn session_subtree_move_preserves_entities_markers_derived_values_and_fts() {
    let directory = TempDir::new().unwrap();
    let database = directory.path().join("session.sqlite");
    let index = SessionIndex::open(&database).unwrap();
    let products = node(EntityId::new(), "products", FileKind::Directory);
    let source = node(EntityId::new(), "products/id-1", FileKind::Directory);
    let archive = node(EntityId::new(), "archive", FileKind::Directory);
    let image = node(EntityId::new(), "products/id-1/front.png", FileKind::Png);
    let text = node(
        EntityId::new(),
        "products/id-1/prompt.md",
        FileKind::Markdown,
    );
    index
        .upsert_batch(
            &[
                products,
                source.clone(),
                archive,
                image.clone(),
                text.clone(),
            ],
            Generation::new(1),
        )
        .unwrap();
    index
        .sync_markers(&[viewer_application::metadata::MarkerChange {
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
                width: 2048,
                height: 1024,
            }),
        )
        .unwrap();
    index
        .replace_text(
            text.entity_id,
            &text.relative_path,
            &TextStatus::Indexed("ceramic product prompt".into()),
        )
        .unwrap();

    assert_eq!(
        index.apply_move(
            &[
                FileMoveProjection {
                    source: image.clone(),
                    destination: FileNode {
                        relative_path: path("archive/collision"),
                        ..image.clone()
                    },
                },
                FileMoveProjection {
                    source: text.clone(),
                    destination: FileNode {
                        relative_path: path("archive/COLLISION"),
                        ..text.clone()
                    },
                },
            ],
            false,
        ),
        Err(OperationProjectionError::Conflict)
    );
    assert_eq!(
        index
            .indexed_node(image.entity_id)
            .unwrap()
            .unwrap()
            .node
            .relative_path,
        image.relative_path
    );
    assert_eq!(
        index
            .indexed_node(text.entity_id)
            .unwrap()
            .unwrap()
            .node
            .relative_path,
        text.relative_path
    );

    index
        .apply_move(
            &[FileMoveProjection {
                source: source.clone(),
                destination: FileNode {
                    relative_path: path("archive/id-1"),
                    ..source.clone()
                },
            }],
            false,
        )
        .unwrap();

    assert_eq!(
        index
            .indexed_node(source.entity_id)
            .unwrap()
            .unwrap()
            .node
            .relative_path,
        path("archive/id-1")
    );
    let moved_image = index.indexed_node(image.entity_id).unwrap().unwrap();
    assert_eq!(
        moved_image.node.relative_path,
        path("archive/id-1/front.png")
    );
    assert_eq!(
        moved_image.marker,
        Marker {
            review_state: Some(ReviewState::Keep),
            favorite: true,
        }
    );
    assert_eq!(
        moved_image.image_metadata,
        Some(ImageMetadata {
            width: 2048,
            height: 1024,
        })
    );
    assert_eq!(moved_image.image_status, ImageIndexStatus::Ready);
    let moved_text = index.indexed_node(text.entity_id).unwrap().unwrap();
    assert_eq!(
        moved_text.node.relative_path,
        path("archive/id-1/prompt.md")
    );
    assert_eq!(moved_text.text_status, TextIndexStatus::Ready);
    let fts_path: String = Connection::open(database)
        .unwrap()
        .query_row(
            "SELECT relative_path FROM text_fts WHERE entity_id = ?1 AND body = ?2",
            params![text.entity_id.to_string(), "ceramic product prompt"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(fts_path, "archive/id-1/prompt.md");
}

#[test]
fn identity_changing_move_rekeys_the_session_node_and_search_projection() {
    let directory = TempDir::new().unwrap();
    let database = directory.path().join("session.sqlite");
    let index = SessionIndex::open(&database).unwrap();
    let source = node(EntityId::new(), "source.txt", FileKind::Text);
    let destination = FileNode {
        entity_id: EntityId::new(),
        relative_path: path("archive.txt"),
        modified_ns: 11,
        ..source.clone()
    };
    index
        .upsert_batch(std::slice::from_ref(&source), Generation::new(1))
        .unwrap();
    index
        .replace_text(
            source.entity_id,
            &source.relative_path,
            &TextStatus::Indexed("identity-aware search body".into()),
        )
        .unwrap();

    index
        .apply_move(
            &[FileMoveProjection {
                source: source.clone(),
                destination: destination.clone(),
            }],
            true,
        )
        .unwrap();

    assert!(index.indexed_node(source.entity_id).unwrap().is_none());
    let projected = index
        .indexed_node(destination.entity_id)
        .unwrap()
        .expect("the destination filesystem identity must own the moved row");
    assert_eq!(projected.node, destination);
    let fts: (String, String) = Connection::open(database)
        .unwrap()
        .query_row(
            "SELECT relative_path, body FROM text_fts WHERE entity_id = ?1",
            [projected.node.entity_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        fts,
        ("archive.txt".into(), "identity-aware search body".into())
    );
}

#[test]
fn session_copy_is_fresh_unmarked_pending_and_conflict_batches_roll_back() {
    let directory = TempDir::new().unwrap();
    let index = SessionIndex::open(directory.path().join("session.sqlite")).unwrap();
    let folder = node(EntityId::new(), "id-1", FileKind::Directory);
    let source = node(EntityId::new(), "id-1/front.png", FileKind::Png);
    index
        .upsert_batch(&[folder, source.clone()], Generation::new(1))
        .unwrap();
    index
        .sync_markers(&[viewer_application::metadata::MarkerChange {
            target: target(&source),
            marker: Marker {
                review_state: Some(ReviewState::Reject),
                favorite: true,
            },
        }])
        .unwrap();
    index
        .replace_image_metadata(
            source.entity_id,
            &source.relative_path,
            Ok(ImageMetadata {
                width: 800,
                height: 600,
            }),
        )
        .unwrap();
    let copied = node(EntityId::new(), "id-1/front copy.png", FileKind::Png);

    index
        .apply_copy(
            &[FileCopyProjection {
                source: source.clone(),
                destination: copied.clone(),
            }],
            false,
            Generation::new(1),
        )
        .unwrap();

    let projected = index.indexed_node(copied.entity_id).unwrap().unwrap();
    assert_eq!(projected.marker, Marker::default());
    assert_eq!(projected.image_metadata, None);
    assert_eq!(projected.image_status, ImageIndexStatus::Pending);
    assert_eq!(projected.text_status, TextIndexStatus::Pending);

    let first = node(EntityId::new(), "id-1/collision.png", FileKind::Png);
    let second = node(EntityId::new(), "id-1/COLLISION.png", FileKind::Png);
    assert_eq!(
        index.apply_copy(
            &[
                FileCopyProjection {
                    source: source.clone(),
                    destination: first.clone(),
                },
                FileCopyProjection {
                    source,
                    destination: second.clone(),
                },
            ],
            false,
            Generation::new(1),
        ),
        Err(OperationProjectionError::Conflict)
    );
    assert!(index.indexed_node(first.entity_id).unwrap().is_none());
    assert!(index.indexed_node(second.entity_id).unwrap().is_none());
}

#[test]
fn copied_video_is_pending_at_the_current_projection_generation() {
    let directory = TempDir::new().unwrap();
    let database = directory.path().join("session.sqlite");
    let index = SessionIndex::open(&database).unwrap();
    let source = node(EntityId::new(), "source.mp4", FileKind::Video);
    index
        .upsert_batch(std::slice::from_ref(&source), Generation::new(4))
        .unwrap();
    index
        .replace_video_metadata(
            source.entity_id,
            &VideoMetadata {
                duration_us: Some(2_000_000),
                display_width: Some(1_920),
                display_height: Some(1_080),
                rotation_degrees: 0,
                frame_rate_millihertz: Some(24_000),
                video_codec: Some("h264".to_owned()),
                audio_codec: Some("aac".to_owned()),
                probe_status: VideoProbeStatus::Ready,
            },
            Generation::new(4),
        )
        .unwrap();
    let copied = FileNode {
        entity_id: EntityId::new(),
        relative_path: path("source copy.mp4"),
        ..source.clone()
    };

    index
        .apply_copy(
            &[FileCopyProjection {
                source: source.clone(),
                destination: copied.clone(),
            }],
            true,
            Generation::new(9),
        )
        .unwrap();

    let source_projection = index.indexed_node(source.entity_id).unwrap().unwrap();
    assert_eq!(
        source_projection.video_metadata.unwrap().probe_status,
        VideoProbeStatus::Ready
    );
    let copied_projection = index.indexed_node(copied.entity_id).unwrap().unwrap();
    assert_eq!(
        copied_projection.video_metadata,
        Some(VideoMetadata {
            duration_us: None,
            display_width: None,
            display_height: None,
            rotation_degrees: 0,
            frame_rate_millihertz: None,
            video_codec: None,
            audio_codec: None,
            probe_status: VideoProbeStatus::Pending,
        })
    );
    let generation = Connection::open(&database)
        .unwrap()
        .query_row(
            "SELECT updated_generation FROM video_metadata WHERE node_id = ?1",
            [copied.entity_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(generation, 9);
}

#[test]
fn copied_video_anchor_and_node_roll_back_together_when_a_later_copy_fails() {
    let directory = TempDir::new().unwrap();
    let database = directory.path().join("session.sqlite");
    let index = SessionIndex::open(&database).unwrap();
    let first_source = node(EntityId::new(), "first.mp4", FileKind::Video);
    let second_source = node(EntityId::new(), "second.mp4", FileKind::Video);
    index
        .upsert_batch(
            &[first_source.clone(), second_source.clone()],
            Generation::new(2),
        )
        .unwrap();
    let first_destination = FileNode {
        entity_id: EntityId::new(),
        relative_path: path("first copy.mp4"),
        ..first_source.clone()
    };
    let missing_parent_destination = FileNode {
        entity_id: EntityId::new(),
        relative_path: path("missing/second copy.mp4"),
        ..second_source.clone()
    };

    assert_eq!(
        index.apply_copy(
            &[
                FileCopyProjection {
                    source: first_source,
                    destination: first_destination.clone(),
                },
                FileCopyProjection {
                    source: second_source,
                    destination: missing_parent_destination,
                },
            ],
            true,
            Generation::new(7),
        ),
        Err(OperationProjectionError::Stale)
    );
    assert!(
        index
            .indexed_node(first_destination.entity_id)
            .unwrap()
            .is_none()
    );
    let metadata_count = Connection::open(&database)
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM video_metadata WHERE node_id = ?1",
            [first_destination.entity_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(metadata_count, 0);
}

#[test]
fn appended_kinds_do_not_stale_unrelated_copy_move_or_rename_projections() {
    let directory = TempDir::new().unwrap();
    let index = SessionIndex::open(directory.path().join("session.sqlite")).unwrap();
    let archive = node(EntityId::new(), "archive", FileKind::Directory);
    let source = node(EntityId::new(), "source.png", FileKind::Png);
    let unsupported = node(
        EntityId::new(),
        "registered.webp",
        FileKind::UnsupportedImage,
    );
    let other = node(EntityId::new(), "notes.data", FileKind::Other);
    index
        .upsert_batch(
            &[archive, source.clone(), unsupported.clone(), other.clone()],
            Generation::new(1),
        )
        .unwrap();

    let copied = FileNode {
        entity_id: EntityId::new(),
        relative_path: path("source copy.png"),
        ..source.clone()
    };
    index
        .apply_copy(
            &[FileCopyProjection {
                source: source.clone(),
                destination: copied.clone(),
            }],
            true,
            Generation::new(1),
        )
        .unwrap();

    let moved = FileNode {
        relative_path: path("archive/source.png"),
        ..source.clone()
    };
    index
        .apply_move(
            &[FileMoveProjection {
                source,
                destination: moved.clone(),
            }],
            true,
        )
        .unwrap();

    let renamed = FileNode {
        relative_path: path("renamed copy.png"),
        ..copied.clone()
    };
    index
        .apply_move(
            &[FileMoveProjection {
                source: copied,
                destination: renamed.clone(),
            }],
            true,
        )
        .unwrap();

    assert_eq!(
        index.indexed_node(moved.entity_id).unwrap().unwrap().node,
        moved
    );
    assert_eq!(
        index.indexed_node(renamed.entity_id).unwrap().unwrap().node,
        renamed
    );
    assert_eq!(
        index
            .indexed_node(unsupported.entity_id)
            .unwrap()
            .unwrap()
            .node,
        unsupported
    );
    assert_eq!(
        index.indexed_node(other.entity_id).unwrap().unwrap().node,
        other
    );
}

#[test]
fn appended_kinds_participate_in_copy_move_and_rename_projections() {
    let directory = TempDir::new().unwrap();
    let index = SessionIndex::open(directory.path().join("session.sqlite")).unwrap();
    let archive = node(EntityId::new(), "archive", FileKind::Directory);
    let unsupported = node(
        EntityId::new(),
        "registered.webp",
        FileKind::UnsupportedImage,
    );
    let other = node(EntityId::new(), "notes.data", FileKind::Other);
    index
        .upsert_batch(
            &[archive, unsupported.clone(), other.clone()],
            Generation::new(1),
        )
        .unwrap();

    let unsupported_copy = FileNode {
        entity_id: EntityId::new(),
        relative_path: path("registered copy.webp"),
        ..unsupported.clone()
    };
    index
        .apply_copy(
            &[FileCopyProjection {
                source: unsupported.clone(),
                destination: unsupported_copy.clone(),
            }],
            true,
            Generation::new(1),
        )
        .unwrap();

    let moved_other = FileNode {
        relative_path: path("archive/notes.data"),
        ..other.clone()
    };
    index
        .apply_move(
            &[FileMoveProjection {
                source: other,
                destination: moved_other.clone(),
            }],
            true,
        )
        .unwrap();

    let renamed_unsupported = FileNode {
        relative_path: path("renamed.webp"),
        ..unsupported.clone()
    };
    index
        .apply_move(
            &[FileMoveProjection {
                source: unsupported,
                destination: renamed_unsupported.clone(),
            }],
            true,
        )
        .unwrap();

    for expected in [unsupported_copy, moved_other, renamed_unsupported] {
        assert_eq!(
            index
                .indexed_node(expected.entity_id)
                .unwrap()
                .unwrap()
                .node,
            expected
        );
    }
}

#[test]
fn identity_changing_video_move_rekeys_companion_metadata_atomically() {
    let directory = TempDir::new().unwrap();
    let index = SessionIndex::open(directory.path().join("session.sqlite")).unwrap();
    let source = node(EntityId::new(), "clip.mp4", FileKind::Video);
    let destination = FileNode {
        entity_id: EntityId::new(),
        relative_path: path("moved.mp4"),
        ..source.clone()
    };
    let metadata = VideoMetadata {
        duration_us: Some(1_000_000),
        display_width: Some(640),
        display_height: Some(480),
        rotation_degrees: 0,
        frame_rate_millihertz: None,
        video_codec: Some("h264".to_owned()),
        audio_codec: None,
        probe_status: VideoProbeStatus::Ready,
    };
    let generation = Generation::new(3);
    index
        .upsert_batch(std::slice::from_ref(&source), generation)
        .unwrap();
    index
        .replace_video_metadata(source.entity_id, &metadata, generation)
        .unwrap();

    index
        .apply_move(
            &[FileMoveProjection {
                source: source.clone(),
                destination: destination.clone(),
            }],
            true,
        )
        .unwrap();

    assert_eq!(index.indexed_node(source.entity_id).unwrap(), None);
    let moved = index.indexed_node(destination.entity_id).unwrap().unwrap();
    assert_eq!(moved.node, destination);
    assert_eq!(moved.video_metadata, Some(metadata));
}

#[test]
fn unknown_persisted_kinds_still_reject_operation_projections() {
    for invalid_kind in [-1_i64, 8] {
        let directory = TempDir::new().unwrap();
        let database = directory.path().join("session.sqlite");
        let index = SessionIndex::open(&database).unwrap();
        let source = node(EntityId::new(), "source.png", FileKind::Png);
        let corrupted = node(EntityId::new(), "corrupted.data", FileKind::Other);
        index
            .upsert_batch(&[source.clone(), corrupted.clone()], Generation::new(1))
            .unwrap();
        Connection::open(&database)
            .unwrap()
            .execute(
                "UPDATE nodes SET kind = ?2 WHERE entity_id = ?1",
                params![corrupted.entity_id.to_string(), invalid_kind],
            )
            .unwrap();
        let destination = FileNode {
            entity_id: EntityId::new(),
            relative_path: path("source copy.png"),
            ..source.clone()
        };

        assert_eq!(
            index.apply_copy(
                &[FileCopyProjection {
                    source,
                    destination,
                }],
                true,
                Generation::new(1),
            ),
            Err(OperationProjectionError::Stale),
            "persisted kind {invalid_kind} must remain invalid"
        );
    }
}

#[test]
fn trash_removes_session_subtree_and_fts_but_keeps_dormant_portable_marker() {
    let project = TempDir::new().unwrap();
    let (store, _portable_database) = portable_store(&project);
    let portable_text = node(EntityId::new(), "id-1/prompt.md", FileKind::Markdown);
    store
        .apply_batch(
            &[target(&portable_text)],
            MarkerPatch {
                review: ReviewPatch::Set(ReviewState::Keep),
                favorite: FavoritePatch::Set(true),
            },
            1,
        )
        .unwrap();

    let session_database = project.path().join("session.sqlite");
    let index = SessionIndex::open(&session_database).unwrap();
    let folder = node(EntityId::new(), "id-1", FileKind::Directory);
    let text = FileNode {
        entity_id: portable_text.entity_id,
        ..portable_text.clone()
    };
    index
        .upsert_batch(&[folder, text.clone()], Generation::new(1))
        .unwrap();
    index
        .replace_text(
            text.entity_id,
            &text.relative_path,
            &TextStatus::Indexed("restorable prompt".into()),
        )
        .unwrap();
    let missing = node(EntityId::new(), "id-1/missing.md", FileKind::Markdown);

    assert_eq!(
        index.apply_trash(&[text.clone(), missing]),
        Err(OperationProjectionError::Stale)
    );
    assert!(index.indexed_node(text.entity_id).unwrap().is_some());

    index.apply_trash(std::slice::from_ref(&text)).unwrap();

    assert!(index.indexed_node(text.entity_id).unwrap().is_none());
    let fts_rows: i64 = Connection::open(session_database)
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM text_fts WHERE entity_id = ?1",
            [text.entity_id.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(fts_rows, 0);
    let dormant = store
        .markers_for_paths(std::slice::from_ref(&text.relative_path))
        .unwrap();
    assert_eq!(dormant.len(), 1);
    assert_eq!(
        dormant[0].marker,
        Marker {
            review_state: Some(ReviewState::Keep),
            favorite: true,
        }
    );
}
