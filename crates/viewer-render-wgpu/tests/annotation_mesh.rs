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
            .any(|vertex| vertex.kind == VertexKind::ScreenSquare),
        "rectangle handles rasterize as AA-ramped screen squares"
    );
    assert!(
        mesh.vertices()
            .iter()
            .filter(|vertex| vertex.kind == VertexKind::ScreenOffset)
            .all(|vertex| vertex.screen_offset_px[0].abs() <= 15.0
                && vertex.screen_offset_px[1].abs() <= 15.0)
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
        mesh.vertices().iter().any(|vertex| vertex.edge_px == 4.0),
        "handle solid core is 8 logical pixels; the 24px target remains a \
         hit-test constant, not a visible square"
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

    // 2047 stroke quads (4 verts / 6 indices each) plus one ordinal badge:
    // two AA circles of 16 segments (96 verts / 96 indices).
    // The v2 halo doubles the segment quads for every stroked point.
    assert!(mesh.vertices().len() <= 2_047 * 4 * 2 + 128);
    assert!(mesh.indices().len() <= 2_047 * 6 * 2 + 144);
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
fn multiply_style_isolates_the_colored_body_for_the_multiply_pipeline() {
    let mut highlighter = node(
        "highlighter",
        1,
        AnnotationGeometry::Stroke {
            points: vec![
                NormalizedPoint::new(0.2, 0.2).unwrap(),
                NormalizedPoint::new(0.6, 0.25).unwrap(),
            ],
        },
    );
    highlighter.style = AnnotationStyle {
        halo: false,
        multiply: true,
        color: [1.0, 0.86, 0.1, 0.35],
        ..AnnotationStyle::default()
    };
    let scene = SceneSnapshot::new(SceneRevision(9), vec![highlighter], None).unwrap();
    let mesh = AnnotationMeshBuilder::default().build(&scene).unwrap();

    assert!(mesh.geometry_index_ranges().is_empty());
    assert_eq!(mesh.multiply_geometry_index_ranges().len(), 1);
    let range = mesh.multiply_geometry_index_ranges()[0].clone();
    assert!(range.end > range.start);
    assert!(range.end as usize <= mesh.indices().len());

    let mut haloed = node(
        "haloed",
        2,
        AnnotationGeometry::Rectangle {
            rect: NormalizedRect::new(0.1, 0.1, 0.2, 0.2).unwrap(),
        },
    );
    haloed.style = AnnotationStyle {
        halo: true,
        multiply: true,
        ..AnnotationStyle::default()
    };
    let scene = SceneSnapshot::new(SceneRevision(10), vec![haloed], None).unwrap();
    let mesh = AnnotationMeshBuilder::default().build(&scene).unwrap();
    assert_eq!(mesh.geometry_index_ranges().len(), 1);
    assert_eq!(mesh.multiply_geometry_index_ranges().len(), 1);
    assert_eq!(
        mesh.geometry_index_ranges()[0].end,
        mesh.multiply_geometry_index_ranges()[0].start
    );
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

#[test]
fn halo_is_a_white_underlay_and_can_be_disabled() {
    let mut rect = node(
        "rect",
        1,
        AnnotationGeometry::Rectangle {
            rect: NormalizedRect::new(0.2, 0.2, 0.3, 0.3).unwrap(),
        },
    );
    rect.style.line_width_px = 2.0;
    let scene = SceneSnapshot::new(SceneRevision(1), vec![rect.clone()], None).unwrap();
    let mesh = AnnotationMeshBuilder::default().build(&scene).unwrap();
    assert!(
        mesh.vertices()
            .iter()
            .any(|vertex| vertex.kind == VertexKind::Segment
                && vertex.color == [1.0, 1.0, 1.0, 0.9]
                && (vertex.edge_px - 2.5).abs() < 0.0001),
        "the white halo runs at stroke width + 1.5px per side"
    );

    rect.style.halo = false;
    let scene = SceneSnapshot::new(SceneRevision(2), vec![rect], None).unwrap();
    let mesh = AnnotationMeshBuilder::default().build(&scene).unwrap();
    assert!(
        !mesh
            .vertices()
            .iter()
            .any(|vertex| vertex.color == [1.0, 1.0, 1.0, 0.9]),
        "highlighter and explicitly halo-less strokes must stay clean"
    );
}

#[test]
fn selected_badges_use_a_color_disc_white_digits_and_a_lavender_ring() {
    let mut point = node(
        "point",
        3,
        AnnotationGeometry::Point {
            position: NormalizedPoint::new(0.5, 0.5).unwrap(),
        },
    );
    point.selected = true;
    let scene = SceneSnapshot::new(SceneRevision(1), vec![point], None).unwrap();
    let mesh = AnnotationMeshBuilder::default().build(&scene).unwrap();
    let label = &mesh.ordinal_labels()[0];
    assert_eq!(label.text, "3");
    assert_eq!(label.color, [1.0, 1.0, 1.0, 1.0]);
    assert!(
        mesh.vertices()
            .iter()
            .any(|vertex| vertex.color[0] > 0.5 && vertex.color[2] > 0.99 && vertex.color[1] < 0.6),
        "selected badges keep the lavender ring"
    );
}

#[test]
fn rounded_rectangle_arcs_replace_the_sharp_corner() {
    let mut rect = node(
        "rounded",
        1,
        AnnotationGeometry::Rectangle {
            rect: NormalizedRect::new(0.2, 0.2, 0.4, 0.2).unwrap(),
        },
    );
    rect.style.corner_radius = [0.02, 0.02];
    let scene = SceneSnapshot::new(SceneRevision(1), vec![rect], None).unwrap();
    let mesh = AnnotationMeshBuilder::default().build(&scene).unwrap();

    assert!(
        !mesh.vertices().iter().any(|vertex| {
            vertex.kind == VertexKind::Segment && vertex.source_position == [0.2, 0.2]
        }),
        "the sharp corner should be replaced by an arc"
    );
    assert!(
        mesh.vertices().iter().any(|vertex| {
            vertex.kind == VertexKind::Segment
                && vertex.source_position[0] > 0.2
                && vertex.source_position[0] < 0.22
                && vertex.source_position[1] > 0.2
                && vertex.source_position[1] < 0.22
        }),
        "arc samples must sit inside the rounded corner box"
    );
}
