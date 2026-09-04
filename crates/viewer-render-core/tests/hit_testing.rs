use viewer_render_core::{
    AnnotationGeometry, AnnotationId, AnnotationNode, CameraState, HitIndex, LogicalPoint,
    LogicalSize, NormalizedPoint, Rotation, SceneRevision, SceneSnapshot, SourceSize,
    TransformSnapshot, ViewportLayout,
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
