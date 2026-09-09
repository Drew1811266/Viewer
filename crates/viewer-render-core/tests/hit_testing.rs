use viewer_render_core::{
    AnnotationGeometry, AnnotationHandle, AnnotationHitPart, AnnotationId, AnnotationNode,
    CameraState, HitIndex, LogicalPoint, LogicalSize, NormalizedPoint, NormalizedRect, Rotation,
    SceneRevision, SceneSnapshot, SourceSize, TransformSnapshot, ViewportLayout,
};

fn transform(zoom: f64) -> TransformSnapshot {
    let source = SourceSize::new(1_000, 1_000).unwrap();
    let viewport =
        ViewportLayout::new(LogicalSize::new(1_000.0, 1_000.0).unwrap(), 2.0, 1.0).unwrap();
    let fit = TransformSnapshot::new(source, viewport, CameraState::fit(Rotation::Deg0)).unwrap();
    let camera = fit
        .zoom_at(zoom, LogicalPoint::new(500.0, 500.0).unwrap())
        .unwrap();
    TransformSnapshot::new(source, viewport, camera).unwrap()
}

fn point(id: &str, ordinal: u32, x: f64, y: f64) -> AnnotationNode {
    AnnotationNode::new(
        AnnotationId::new(id).unwrap(),
        ordinal,
        AnnotationGeometry::Point {
            position: NormalizedPoint::new(x, y).unwrap(),
        },
    )
    .unwrap()
}

#[test]
fn overlapping_annotations_hit_the_visually_topmost_node() {
    let scene = SceneSnapshot::new(
        SceneRevision(1),
        vec![point("lower", 1, 0.5, 0.5), point("upper", 2, 0.5, 0.5)],
        None,
    )
    .unwrap();
    let index = HitIndex::rebuild(&scene);

    let hit = index.hit_test(
        LogicalPoint::new(506.0, 500.0).unwrap(),
        8.0,
        &transform(1.0),
    );

    assert_eq!(hit.as_ref().map(AnnotationId::as_str), Some("upper"));
}

#[test]
fn hit_tolerance_stays_in_screen_pixels_at_different_zoom_levels() {
    let scene =
        SceneSnapshot::new(SceneRevision(1), vec![point("point", 1, 0.5, 0.5)], None).unwrap();
    let index = HitIndex::rebuild(&scene);
    let target = LogicalPoint::new(506.0, 500.0).unwrap();

    assert!(index.hit_test(target, 8.0, &transform(1.0)).is_some());
    assert!(index.hit_test(target, 8.0, &transform(4.0)).is_some());
}

#[test]
fn invisible_annotations_do_not_participate_in_hit_testing() {
    let mut hidden = point("hidden", 1, 0.5, 0.5);
    hidden.visible = false;
    let scene = SceneSnapshot::new(SceneRevision(1), vec![hidden], None).unwrap();
    let index = HitIndex::rebuild(&scene);

    assert!(
        index
            .hit_test(
                LogicalPoint::new(500.0, 500.0).unwrap(),
                8.0,
                &transform(1.0)
            )
            .is_none()
    );
}

#[test]
fn offset_ordinals_are_hittable_without_stealing_unselected_corner_handles() {
    let scene =
        SceneSnapshot::new(SceneRevision(1), vec![point("point", 1, 0.5, 0.5)], None).unwrap();
    let index = HitIndex::rebuild(&scene);
    let hit = index.hit_test_part(
        LogicalPoint::new(520.0, 480.0).unwrap(),
        8.0,
        &transform(1.0),
    );
    assert_eq!(hit.map(|hit| hit.part), Some(AnnotationHitPart::Ordinal));
    let rectangle = AnnotationNode::new(
        AnnotationId::new("box").unwrap(),
        2,
        AnnotationGeometry::Rectangle {
            rect: NormalizedRect::new(0.2, 0.2, 0.4, 0.4).unwrap(),
        },
    )
    .unwrap();
    let scene = SceneSnapshot::new(SceneRevision(2), vec![rectangle], None).unwrap();
    let hit = HitIndex::rebuild(&scene)
        .hit_test_part(
            LogicalPoint::new(600.0, 200.0).unwrap(),
            8.0,
            &transform(1.0),
        )
        .unwrap();
    assert_eq!(
        hit.part,
        AnnotationHitPart::Outline,
        "unselected annotations do not expose invisible handles"
    );
}

#[test]
fn spatial_query_does_not_scan_all_five_hundred_annotations() {
    let nodes = (0..500)
        .map(|index| {
            let column = index % 25;
            let row = index / 25;
            point(
                &format!("point-{index}"),
                index + 1,
                (column as f64 + 0.5) / 25.0,
                (row as f64 + 0.5) / 20.0,
            )
        })
        .collect();
    let scene = SceneSnapshot::new(SceneRevision(1), nodes, None).unwrap();
    let index = HitIndex::rebuild(&scene);

    assert!(
        index.candidate_count(
            LogicalPoint::new(500.0, 500.0).unwrap(),
            8.0,
            &transform(1.0)
        ) < 100
    );
}

#[test]
fn spatial_query_uses_source_coordinates_after_rotation() {
    let scene =
        SceneSnapshot::new(SceneRevision(1), vec![point("rotated", 1, 0.2, 0.7)], None).unwrap();
    let index = HitIndex::rebuild(&scene);
    let source = SourceSize::new(1_200, 800).unwrap();
    let viewport =
        ViewportLayout::new(LogicalSize::new(1_000.0, 800.0).unwrap(), 2.0, 1.0).unwrap();
    let transform =
        TransformSnapshot::new(source, viewport, CameraState::fit(Rotation::Deg90)).unwrap();
    let target = transform.image_to_view(NormalizedPoint::new(0.2, 0.7).unwrap());

    assert_eq!(
        index
            .hit_test(target, 8.0, &transform)
            .as_ref()
            .map(AnnotationId::as_str),
        Some("rotated")
    );
}

#[test]
fn selected_geometry_precedes_overlapping_interiors_but_handles_win_first() {
    let mut selected = AnnotationNode::new(
        AnnotationId::new("selected").unwrap(),
        1,
        AnnotationGeometry::Rectangle {
            rect: NormalizedRect::new(0.2, 0.2, 0.4, 0.4).unwrap(),
        },
    )
    .unwrap();
    selected.selected = true;
    let upper = AnnotationNode::new(
        AnnotationId::new("upper").unwrap(),
        2,
        AnnotationGeometry::Rectangle {
            rect: NormalizedRect::new(0.1, 0.1, 0.8, 0.8).unwrap(),
        },
    )
    .unwrap();
    let scene = SceneSnapshot::new(SceneRevision(1), vec![selected, upper], None).unwrap();
    let hit = HitIndex::rebuild(&scene)
        .hit_test_part(
            LogicalPoint::new(400.0, 400.0).unwrap(),
            8.0,
            &transform(1.0),
        )
        .unwrap();
    assert_eq!(hit.annotation_id.as_str(), "selected");
    assert_eq!(hit.part, AnnotationHitPart::Interior);
    let corner = HitIndex::rebuild(&scene)
        .hit_test_part(
            LogicalPoint::new(600.0, 200.0).unwrap(),
            8.0,
            &transform(1.0),
        )
        .unwrap();
    assert_eq!(corner.annotation_id.as_str(), "selected");
    assert_eq!(
        corner.part,
        AnnotationHitPart::Handle(AnnotationHandle::NorthEast)
    );
}

#[test]
fn the_visible_ordinal_badge_is_clickable_outside_outline_tolerance() {
    let scene =
        SceneSnapshot::new(SceneRevision(1), vec![point("point", 1, 0.5, 0.5)], None).unwrap();
    let hit = HitIndex::rebuild(&scene).hit_test_part(
        LogicalPoint::new(532.0, 480.0).unwrap(),
        8.0,
        &transform(1.0),
    );
    assert_eq!(hit.map(|hit| hit.part), Some(AnnotationHitPart::Ordinal));
}

#[test]
fn rotated_ordinal_layout_and_screen_constant_handles_agree_with_hit_testing() {
    use viewer_render_core::annotation_ordinal_position;
    for (rotation, expected) in [
        (Rotation::Deg0, (624.0, 276.0)),
        (Rotation::Deg90, (724.0, 176.0)),
        (Rotation::Deg180, (824.0, 476.0)),
        (Rotation::Deg270, (524.0, 376.0)),
    ] {
        let transform = TransformSnapshot::new(
            SourceSize::new(1000, 1000).unwrap(),
            ViewportLayout::new(LogicalSize::new(1000.0, 1000.0).unwrap(), 2.0, 1.0).unwrap(),
            CameraState::fit(rotation),
        )
        .unwrap();
        let mut node = AnnotationNode::new(
            AnnotationId::new("box").unwrap(),
            1,
            AnnotationGeometry::Rectangle {
                rect: NormalizedRect::new(0.2, 0.3, 0.4, 0.2).unwrap(),
            },
        )
        .unwrap();
        node.selected = true;
        let ordinal = annotation_ordinal_position(&node.geometry, &transform).unwrap();
        assert!(
            (ordinal.x - expected.0).abs() < 0.00001 && (ordinal.y - expected.1).abs() < 0.00001
        );
        let corner = transform.image_to_view(NormalizedPoint::new(0.2, 0.3).unwrap());
        let scene = SceneSnapshot::new(SceneRevision(1), vec![node], None).unwrap();
        let index = HitIndex::rebuild(&scene);
        assert_eq!(
            index.hit_test_part(ordinal, 8.0, &transform).unwrap().part,
            AnnotationHitPart::Ordinal
        );
        assert_eq!(
            index
                .hit_test_part(
                    LogicalPoint {
                        x: corner.x + 11.0,
                        y: corner.y + 11.0
                    },
                    8.0,
                    &transform
                )
                .unwrap()
                .part,
            AnnotationHitPart::Handle(AnnotationHandle::NorthWest)
        );
    }
    for zoom in [0.1, 1.0, 8.0] {
        let scene = SceneSnapshot::new(
            SceneRevision(1),
            vec![point("lower", 1, 0.5, 0.5), point("upper", 2, 0.5, 0.5)],
            None,
        )
        .unwrap();
        let hit = HitIndex::rebuild(&scene)
            .hit_test_part(LogicalPoint { x: 532.0, y: 480.0 }, 8.0, &transform(zoom))
            .unwrap();
        assert_eq!(hit.annotation_id.as_str(), "upper");
        assert_eq!(hit.part, AnnotationHitPart::Ordinal);
    }
}

#[test]
fn a_short_viewport_does_not_place_an_ordinal_over_an_arrow_endpoint_handle() {
    let transform = TransformSnapshot::new(
        SourceSize::new(200, 50).unwrap(),
        ViewportLayout::new(LogicalSize::new(200.0, 50.0).unwrap(), 1.0, 1.0).unwrap(),
        CameraState::fit(Rotation::Deg0),
    )
    .unwrap();
    let arrow = AnnotationGeometry::Arrow {
        tail: NormalizedPoint::new(0.5, 0.5).unwrap(),
        head: NormalizedPoint::new(0.1, 0.5).unwrap(),
    };
    assert_eq!(
        viewer_render_core::annotation_ordinal_position(&arrow, &transform),
        None,
        "only cardinal candidates fit; their 20px center distance overlaps the 14px badge and 12px handle"
    );
}
