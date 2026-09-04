use viewer_render_core::{
    AnnotationGeometry, AnnotationId, AnnotationNode, AssetGeneration, CameraState, LogicalPoint,
    LogicalSize, NormalizedPoint, PhysicalSize, Rotation, SceneRevision, SceneSnapshot, SourceSize,
    TransformSnapshot, ViewportLayout,
};
use viewer_render_wgpu::{
    AnnotationBufferIdentity, DecodedResource, MagnifierConfig, MagnifierPassPlan, MagnifierShape,
    RendererDescriptor, ResourceHandle, RetainedSceneResources, WgpuImageRenderer,
};

#[test]
fn magnifier_reuses_main_scene_handles_revision_and_annotation_buffers() {
    let retained = RetainedSceneResources::new(
        SceneRevision(42),
        vec![ResourceHandle(3), ResourceHandle(8)],
        Some(AnnotationBufferIdentity(5)),
    );
    let config = MagnifierConfig::new(
        NormalizedPoint::new(0.25, 0.75).unwrap(),
        LogicalPoint::new(640.0, 360.0).unwrap(),
        280.0,
        280.0,
        3.0,
        MagnifierShape::Circle,
    )
    .unwrap();

    let plan = MagnifierPassPlan::new(&retained, config);

    assert_eq!(plan.scene_revision(), retained.scene_revision());
    assert_eq!(plan.image_handles(), retained.image_handles());
    assert_eq!(
        plan.annotation_buffer_identity(),
        retained.annotation_buffer_identity()
    );
    assert_eq!(plan.additional_full_image_texture_bytes(), 0);
}

#[test]
fn magnifier_configuration_rejects_non_finite_or_non_positive_geometry() {
    let focus = NormalizedPoint::new(0.5, 0.5).unwrap();
    let center = LogicalPoint::new(10.0, 10.0).unwrap();

    assert!(MagnifierConfig::new(focus, center, 0.0, 100.0, 2.0, MagnifierShape::Circle).is_err());
    assert!(MagnifierConfig::new(focus, center, 100.0, 0.0, 2.0, MagnifierShape::Circle).is_err());
    assert!(
        MagnifierConfig::new(
            focus,
            center,
            100.0,
            100.0,
            f64::NAN,
            MagnifierShape::Circle
        )
        .is_err()
    );
}

#[test]
fn rounded_rectangle_remains_a_clip_shape_not_a_second_scene() {
    let retained = RetainedSceneResources::new(
        SceneRevision(7),
        vec![ResourceHandle(11)],
        Some(AnnotationBufferIdentity(9)),
    );
    let config = MagnifierConfig::new(
        NormalizedPoint::new(0.5, 0.5).unwrap(),
        LogicalPoint::new(100.0, 120.0).unwrap(),
        240.0,
        120.0,
        2.0,
        MagnifierShape::RoundedRectangle,
    )
    .unwrap();

    let plan = MagnifierPassPlan::new(&retained, config);

    assert_eq!(plan.config().shape, MagnifierShape::RoundedRectangle);
    assert!(plan.clip_contains(LogicalPoint::new(219.0, 120.0).unwrap()));
    assert!(!plan.clip_contains(LogicalPoint::new(100.0, 181.0).unwrap()));
    assert_eq!(plan.additional_full_image_texture_bytes(), 0);
}

#[test]
fn magnifier_projects_the_shared_scene_around_focus_without_reinterpreting_anchors() {
    let retained = RetainedSceneResources::new(SceneRevision(3), vec![], None);
    let focus = NormalizedPoint::new(0.25, 0.75).unwrap();
    let config = MagnifierConfig::new(
        focus,
        LogicalPoint::new(80.0, 20.0).unwrap(),
        40.0,
        40.0,
        3.0,
        MagnifierShape::Circle,
    )
    .unwrap();
    let plan = MagnifierPassPlan::new(&retained, config);
    let transform = TransformSnapshot::new(
        SourceSize::new(100, 100).unwrap(),
        ViewportLayout::new(LogicalSize::new(100.0, 100.0).unwrap(), 1.0, 1.0).unwrap(),
        CameraState::fit(Rotation::Deg0),
    )
    .unwrap();

    assert_eq!(plan.project_source(&transform, focus), config.center);
    assert_eq!(
        plan.project_source(&transform, NormalizedPoint::new(0.35, 0.75).unwrap()),
        LogicalPoint::new(110.0, 20.0).unwrap()
    );
    assert!(plan.clip_contains(LogicalPoint::new(80.0, 20.0).unwrap()));
    assert!(!plan.clip_contains(LogicalPoint::new(101.0, 20.0).unwrap()));
}

#[test]
fn magnifier_gpu_composition_is_explicitly_opt_in() {
    if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() || !cfg!(target_os = "macos") {
        return;
    }
    let logical = LogicalSize::new(64.0, 64.0).unwrap();
    let source = SourceSize::new(2, 1).unwrap();
    let mut renderer = WgpuImageRenderer::new(
        RendererDescriptor::headless(
            logical,
            PhysicalSize {
                width: 64,
                height: 64,
            },
            1.0,
        )
        .unwrap(),
    )
    .unwrap();
    renderer
        .set_transform(
            TransformSnapshot::new(
                source,
                ViewportLayout::new(logical, 1.0, 1.0).unwrap(),
                CameraState::fit(Rotation::Deg0),
            )
            .unwrap(),
        )
        .unwrap();
    renderer
        .upload_resource(DecodedResource::WholeImage {
            generation: AssetGeneration(1),
            level: 0,
            source_size: source,
            width: 2,
            height: 1,
            pixels: vec![0, 0, 255, 255, 255, 0, 0, 255],
        })
        .unwrap();
    renderer
        .apply_scene(
            &SceneSnapshot::new(
                SceneRevision(13),
                vec![
                    AnnotationNode::new(
                        AnnotationId::new("magnified-point").unwrap(),
                        1,
                        AnnotationGeometry::Point {
                            position: NormalizedPoint::new(0.25, 0.5).unwrap(),
                        },
                    )
                    .unwrap(),
                ],
                None,
            )
            .unwrap(),
        )
        .unwrap();
    let resources_before_magnifier = renderer.retained_scene_resources();
    let gpu_bytes_before_magnifier = renderer.gpu_resource_bytes();
    renderer
        .set_magnifier(Some(
            MagnifierConfig::new(
                NormalizedPoint::new(0.25, 0.5).unwrap(),
                LogicalPoint::new(48.0, 12.0).unwrap(),
                24.0,
                24.0,
                2.0,
                MagnifierShape::Circle,
            )
            .unwrap(),
        ))
        .unwrap();
    assert_eq!(
        renderer.retained_scene_resources(),
        resources_before_magnifier
    );
    assert_eq!(
        resources_before_magnifier.scene_revision(),
        SceneRevision(13)
    );
    assert!(
        resources_before_magnifier
            .annotation_buffer_identity()
            .is_some()
    );

    let request = renderer.frame_state().take_request().unwrap();
    let (receipt, pixels) = renderer.render_headless_capture(request).unwrap();
    let lens_center = &pixels[((12 * 64 + 48) * 4)..((12 * 64 + 48) * 4 + 4)];
    let untouched_background = &pixels[((5 * 64 + 5) * 4)..((5 * 64 + 5) * 4 + 4)];

    assert_eq!(receipt.gpu_resource_bytes, gpu_bytes_before_magnifier);
    assert!(
        lens_center[2] > 200 && lens_center[2] > lens_center[0].saturating_add(100),
        "unexpected lens center BGRA pixel: {lens_center:?}"
    );
    assert!(untouched_background[0] > 230 && untouched_background[1] > 230);
}
