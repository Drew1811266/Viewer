use viewer_render_core::{
    CameraMode, CameraState, LogicalPoint, LogicalRect, LogicalSize, Rotation, SourceSize,
    TransformSnapshot, ViewportLayout,
};

fn viewport() -> ViewportLayout {
    ViewportLayout::new(LogicalSize::new(1_200.0, 800.0).unwrap(), 2.0, 0.9).unwrap()
}

#[test]
fn fit_depends_on_oriented_aspect_ratio_not_pixel_count() {
    let camera = CameraState::fit(Rotation::Deg0);
    let small =
        TransformSnapshot::new(SourceSize::new(300, 200).unwrap(), viewport(), camera).unwrap();
    let large =
        TransformSnapshot::new(SourceSize::new(6_570, 4_380).unwrap(), viewport(), camera).unwrap();

    assert_eq!(small.fitted_rect(), large.fitted_rect());
    assert_eq!(
        small.fitted_rect(),
        LogicalRect::new(60.0, 40.0, 1_080.0, 720.0).unwrap()
    );
}

#[test]
fn quarter_turn_uses_the_oriented_aspect_ratio() {
    let transform = TransformSnapshot::new(
        SourceSize::new(300, 200).unwrap(),
        viewport(),
        CameraState::fit(Rotation::Deg90),
    )
    .unwrap();

    assert_eq!(
        transform.fitted_rect(),
        LogicalRect::new(360.0, 40.0, 480.0, 720.0).unwrap()
    );
}

#[test]
fn physical_viewport_uses_scale_factor_only_after_logical_fit() {
    let transform = TransformSnapshot::new(
        SourceSize::new(300, 200).unwrap(),
        viewport(),
        CameraState::fit(Rotation::Deg0),
    )
    .unwrap();

    assert_eq!(transform.physical_viewport().width, 2_400);
    assert_eq!(transform.physical_viewport().height, 1_600);
}

#[test]
fn zoom_keeps_the_image_point_under_the_anchor_stable() {
    let source = SourceSize::new(300, 200).unwrap();
    let initial =
        TransformSnapshot::new(source, viewport(), CameraState::fit(Rotation::Deg0)).unwrap();
    let anchor = LogicalPoint::new(850.0, 400.0).unwrap();
    let image_point = initial.view_to_image(anchor).unwrap();

    let camera = initial.zoom_at(2.0, anchor).unwrap();
    let zoomed = TransformSnapshot::new(source, viewport(), camera).unwrap();

    assert_eq!(camera.mode, CameraMode::Free);
    assert!((camera.zoom - 2.0).abs() < 1e-12);
    let projected = zoomed.image_to_view(image_point);
    assert!((projected.x - anchor.x).abs() < 1e-9);
    assert!((projected.y - anchor.y).abs() < 1e-9);
}

#[test]
fn invalid_geometry_is_rejected_before_projection() {
    assert!(SourceSize::new(0, 200).is_err());
    assert!(LogicalSize::new(f64::NAN, 800.0).is_err());
    assert!(ViewportLayout::new(LogicalSize::new(1.0, 1.0).unwrap(), 0.0, 0.9).is_err());
    assert!(ViewportLayout::new(LogicalSize::new(1.0, 1.0).unwrap(), 1.0, 1.1).is_err());
}
