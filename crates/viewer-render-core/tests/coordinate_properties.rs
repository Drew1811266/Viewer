use viewer_render_core::{
    CameraState, LogicalPoint, LogicalSize, NormalizedPoint, Rotation, SourceSize,
    TransformSnapshot, ViewportLayout,
};

#[test]
fn normalized_points_round_trip_for_every_rotation() {
    let viewport =
        ViewportLayout::new(LogicalSize::new(1_137.0, 719.0).unwrap(), 2.0, 0.9).unwrap();
    let source = SourceSize::new(6_582, 4_388).unwrap();
    let rotations = [
        Rotation::Deg0,
        Rotation::Deg90,
        Rotation::Deg180,
        Rotation::Deg270,
    ];
    let mut state = 0x4d59_5df4_d0f3_3173_u64;

    for rotation in rotations {
        let transform =
            TransformSnapshot::new(source, viewport, CameraState::fit(rotation)).unwrap();
        for _ in 0..10_000 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let x = ((state >> 11) as f64) / ((1_u64 << 53) as f64);
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let y = ((state >> 11) as f64) / ((1_u64 << 53) as f64);
            let original = NormalizedPoint::new(x, y).unwrap();

            let view = transform.image_to_view(original);
            let round_trip = transform.view_to_image(view).unwrap();

            assert!((round_trip.x - original.x).abs() <= 1e-5);
            assert!((round_trip.y - original.y).abs() <= 1e-5);
        }
    }
}

#[test]
fn view_to_image_rejects_points_outside_the_displayed_image() {
    let transform = TransformSnapshot::new(
        SourceSize::new(300, 200).unwrap(),
        ViewportLayout::new(LogicalSize::new(1_200.0, 800.0).unwrap(), 2.0, 0.9).unwrap(),
        CameraState::fit(Rotation::Deg0),
    )
    .unwrap();

    assert_eq!(
        transform.view_to_image(LogicalPoint::new(10.0, 10.0).unwrap()),
        None
    );
}

#[test]
fn pan_is_clamped_to_the_visible_image_bounds() {
    let viewport =
        ViewportLayout::new(LogicalSize::new(1_200.0, 800.0).unwrap(), 2.0, 0.9).unwrap();
    let source = SourceSize::new(300, 200).unwrap();
    let initial =
        TransformSnapshot::new(source, viewport, CameraState::fit(Rotation::Deg0)).unwrap();
    let zoomed_camera = initial
        .zoom_at(2.0, LogicalPoint::new(600.0, 400.0).unwrap())
        .unwrap();
    let zoomed = TransformSnapshot::new(source, viewport, zoomed_camera).unwrap();

    let panned = zoomed
        .pan_by(LogicalPoint::new(10_000.0, -10_000.0).unwrap())
        .unwrap();

    assert_eq!(panned.offset.x, 480.0);
    assert_eq!(panned.offset.y, -320.0);
}
