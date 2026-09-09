use viewer_render_core::{
    AnnotationGeometry, AnnotationId, AnnotationNode, AnnotationStyle, NormalizedPoint,
    NormalizedRect, SceneRevision, SceneSnapshot,
};
use viewer_render_wgpu::{AnnotationMeshBuilder, MeshError, VertexKind};

fn node(id: &str, ordinal: u32, geometry: AnnotationGeometry) -> AnnotationNode {
    AnnotationNode::new(AnnotationId::new(id).unwrap(), ordinal, geometry).unwrap()
}

#[test]
fn every_review_primitive_produces_bounded_indexed_geometry() {
    let scene = SceneSnapshot::new(
        SceneRevision(1),
        vec![
            node(
                "point",
                1,
                AnnotationGeometry::Point {
                    position: NormalizedPoint::new(0.1, 0.1).unwrap(),
                },
            ),
            node(
                "arrow",
                2,
                AnnotationGeometry::Arrow {
                    tail: NormalizedPoint::new(0.2, 0.2).unwrap(),
                    head: NormalizedPoint::new(0.4, 0.3).unwrap(),
                },
            ),
            node(
                "rectangle",
                3,
                AnnotationGeometry::Rectangle {
                    rect: NormalizedRect::new(0.4, 0.1, 0.2, 0.2).unwrap(),
                },
            ),
            node(
                "ellipse",
                4,
                AnnotationGeometry::Ellipse {
                    rect: NormalizedRect::new(0.1, 0.5, 0.2, 0.3).unwrap(),
                },
            ),
            node(
                "stroke",
                5,
                AnnotationGeometry::Stroke {
                    points: vec![
                        NormalizedPoint::new(0.5, 0.5).unwrap(),
                        NormalizedPoint::new(0.6, 0.6).unwrap(),
                        NormalizedPoint::new(0.7, 0.55).unwrap(),
                    ],
                },
            ),
        ],
        None,
    )
    .unwrap();

    let mesh = AnnotationMeshBuilder::default().build(&scene).unwrap();

    assert!(!mesh.vertices().is_empty());
    assert!(!mesh.indices().is_empty());
    assert!(
        mesh.indices()
            .iter()
            .all(|index| (*index as usize) < mesh.vertices().len())
    );
    assert_eq!(mesh.ordinal_labels().len(), 5);
    assert!(
        mesh.vertices()
            .iter()
            .any(|vertex| vertex.kind == VertexKind::Segment)
    );
    assert!(
        mesh.vertices()
            .iter()
            .any(|vertex| vertex.kind == VertexKind::ScreenOffset)
    );
}

#[test]
fn selected_geometry_adds_four_fixed_screen_handles() {
    let mut selected = node(
        "selected",
        1,
        AnnotationGeometry::Rectangle {
            rect: NormalizedRect::new(0.2, 0.2, 0.4, 0.4).unwrap(),
        },
    );
    selected.selected = true;
    let scene = SceneSnapshot::new(SceneRevision(2), vec![selected], None).unwrap();

    let mesh = AnnotationMeshBuilder::default().build(&scene).unwrap();

    assert_eq!(mesh.selection_handle_count(), 4);
    assert!(
        mesh.vertices()
            .iter()
            .filter(|vertex| vertex.kind == VertexKind::ScreenOffset)
            .all(|vertex| vertex.screen_offset_px[0].abs() <= 14.0
                && vertex.screen_offset_px[1].abs() <= 14.0)
    );
}

#[test]
fn selected_arrow_handles_are_only_at_the_two_editable_endpoints() {
    let mut arrow = node(
        "arrow",
        1,
        AnnotationGeometry::Arrow {
            tail: NormalizedPoint::new(0.1, 0.2).unwrap(),
            head: NormalizedPoint::new(0.7, 0.8).unwrap(),
        },
    );
    arrow.selected = true;
    let scene = SceneSnapshot::new(SceneRevision(1), vec![arrow], None).unwrap();
    let mesh = AnnotationMeshBuilder::default().build(&scene).unwrap();
    assert_eq!(mesh.selection_handle_count(), 2);
    assert!(
        mesh.vertices()
            .iter()
            .filter(|vertex| { vertex.kind == VertexKind::ScreenOffset })
            .all(|vertex| matches!(vertex.source_position, [0.1, 0.2] | [0.7, 0.8]))
    );
}

#[test]
fn selected_handles_have_visible_white_interiors_and_full_size_hit_targets() {
    let mut arrow = node(
        "arrow",
        1,
        AnnotationGeometry::Arrow {
            tail: NormalizedPoint::new(0.2, 0.2).unwrap(),
            head: NormalizedPoint::new(0.6, 0.6).unwrap(),
        },
    );
    arrow.selected = true;
    let mesh = AnnotationMeshBuilder::default()
        .build(&SceneSnapshot::new(SceneRevision(1), vec![arrow], None).unwrap())
        .unwrap();
    assert!(
        mesh.vertices()
            .iter()
            .any(|vertex| vertex.color == [1.0, 1.0, 1.0, 1.0]),
        "handles need a contrasting white interior"
    );
    assert!(
        mesh.vertices()
            .iter()
            .any(|vertex| vertex.screen_offset_px[0] == 12.0),
        "handle diameter must remain 24 logical pixels"
    );
}

#[test]
fn degenerate_arrow_is_rejected_before_gpu_upload() {
    let point = NormalizedPoint::new(0.5, 0.5).unwrap();

    assert_eq!(
        AnnotationMeshBuilder::default().tessellate_geometry(
            &AnnotationGeometry::Arrow {
                tail: point,
                head: point,
            },
            AnnotationStyle::default(),
        ),
        Err(MeshError::DegenerateSegment)
    );
}

#[test]
fn arrow_head_uses_fixed_screen_offsets_instead_of_source_space_length() {
    let mesh = AnnotationMeshBuilder::default()
        .tessellate_geometry(
            &AnnotationGeometry::Arrow {
                tail: NormalizedPoint::new(0.1, 0.2).unwrap(),
                head: NormalizedPoint::new(0.9, 0.8).unwrap(),
            },
            AnnotationStyle::default(),
        )
        .unwrap();

    let arrow_head = mesh
        .vertices()
        .iter()
        .filter(|vertex| vertex.kind == VertexKind::ArrowHead)
        .collect::<Vec<_>>();
    assert_eq!(arrow_head.len(), 3);
    assert!(arrow_head.iter().all(|vertex| {
        vertex.screen_offset_px[0].abs() <= 8.0 && vertex.screen_offset_px[1].abs() <= 14.0
    }));
}

#[test]
fn maximum_length_stroke_stays_within_linear_mesh_bounds() {
    let points = (0..2_048)
        .map(|index| {
            NormalizedPoint::new(
                index as f64 / 2_047.0,
                if index % 2 == 0 { 0.25 } else { 0.75 },
            )
            .unwrap()
        })
        .collect();
    let scene = SceneSnapshot::new(
        SceneRevision(3),
        vec![node(
            "long-stroke",
            1,
            AnnotationGeometry::Stroke { points },
        )],
        None,
    )
    .unwrap();

    let mesh = AnnotationMeshBuilder::default().build(&scene).unwrap();

    assert!(mesh.vertices().len() <= 2_047 * 4 + 64);
    assert!(mesh.indices().len() <= 2_047 * 6 + 96);
}

#[test]
fn draft_geometry_is_present_without_an_ordinal_label() {
    let mut draft = node(
        "draft",
        99,
        AnnotationGeometry::Rectangle {
            rect: NormalizedRect::new(0.1, 0.1, 0.2, 0.2).unwrap(),
        },
    );
    draft.draft = true;
    let scene = SceneSnapshot::new(SceneRevision(4), vec![], Some(draft)).unwrap();

    let mesh = AnnotationMeshBuilder::default().build(&scene).unwrap();

    assert!(!mesh.vertices().is_empty());
    assert!(mesh.ordinal_labels().is_empty());
}

#[test]
fn dashed_style_is_retained_as_segment_shader_metadata() {
    let style = AnnotationStyle {
        dashed: true,
        ..AnnotationStyle::default()
    };
    let mesh = AnnotationMeshBuilder::default()
        .tessellate_geometry(
            &AnnotationGeometry::Rectangle {
                rect: NormalizedRect::new(0.1, 0.1, 0.8, 0.8).unwrap(),
            },
            style,
        )
        .unwrap();

    let segments = mesh
        .vertices()
        .iter()
        .filter(|vertex| vertex.kind == VertexKind::Segment)
        .collect::<Vec<_>>();
    assert!(!segments.is_empty());
    assert!(segments.iter().all(|vertex| vertex.dashed));
    assert!(segments.iter().any(|vertex| vertex.segment_factor == 0.0));
    assert!(segments.iter().any(|vertex| vertex.segment_factor == 1.0));
}
