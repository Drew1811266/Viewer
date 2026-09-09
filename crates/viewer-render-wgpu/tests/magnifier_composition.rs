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
            pixels: viewer_render_core::SharedPixels::try_copy_from_slice(
                &viewer_render_core::ImageMemoryCoordinator::new(
                    viewer_render_core::ImageMemoryPolicy::baseline_8gb(),
                ),
                AssetGeneration(1),
                &[0, 0, 255, 255, 255, 0, 0, 255],
            )
            .unwrap(),
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
    let request = renderer.frame_state().take_request().unwrap();
    let (receipt, _) = renderer.render_headless_capture(request).unwrap();
    assert!(!receipt.magnifier_rendered);
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
    assert!(receipt.magnifier_rendered);
    let lens_center = &pixels[((12 * 64 + 48) * 4)..((12 * 64 + 48) * 4 + 4)];
    let untouched_background = &pixels[((5 * 64 + 5) * 4)..((5 * 64 + 5) * 4 + 4)];

    assert_eq!(receipt.gpu_resource_bytes, gpu_bytes_before_magnifier);
    assert!(
        lens_center[2] > 200 && lens_center[2] > lens_center[0].saturating_add(100),
        "unexpected lens center BGRA pixel: {lens_center:?}"
    );
    assert!(untouched_background[0] > 230 && untouched_background[1] > 230);
    renderer.set_magnifier(None).unwrap();
    let request = renderer.frame_state().take_request().unwrap();
    let (receipt, _) = renderer.render_headless_capture(request).unwrap();
    assert!(!receipt.magnifier_rendered);
}

#[test]
fn metal_lens_repositions_ordinals_and_caps_strokes_without_edit_handles() {
    if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() || !cfg!(target_os = "macos") {
        return;
    }
    for scale in [1.0, 2.0] {
        let logical = LogicalSize::new(256.0, 256.0).unwrap();
        let physical = PhysicalSize {
            width: (256.0 * scale) as u32,
            height: (256.0 * scale) as u32,
        };
        let mut renderer =
            WgpuImageRenderer::new(RendererDescriptor::headless(logical, physical, scale).unwrap())
                .unwrap();
        renderer
            .set_transform(
                TransformSnapshot::new(
                    SourceSize::new(256, 256).unwrap(),
                    ViewportLayout::new(logical, scale, 1.0).unwrap(),
                    CameraState::fit(Rotation::Deg0),
                )
                .unwrap(),
            )
            .unwrap();
        let point = AnnotationNode::new(
            AnnotationId::new("point").unwrap(),
            1,
            AnnotationGeometry::Point {
                position: NormalizedPoint::new(0.5, 0.5).unwrap(),
            },
        )
        .unwrap();
        renderer
            .apply_scene(&SceneSnapshot::new(SceneRevision(1), vec![point], None).unwrap())
            .unwrap();
        renderer
            .set_magnifier(Some(
                MagnifierConfig::new(
                    NormalizedPoint::new(0.5, 0.5).unwrap(),
                    LogicalPoint::new(128.0, 64.0).unwrap(),
                    96.0,
                    96.0,
                    4.0,
                    MagnifierShape::RoundedRectangle,
                )
                .unwrap(),
            ))
            .unwrap();
        let request = renderer.frame_state().take_request().unwrap();
        let (_, pixels) = renderer.render_headless_capture(request).unwrap();
        let red = |pixels: &[u8], x: f64, y: f64| {
            let index = (((y * scale) as u32 * physical.width + (x * scale) as u32) * 4) as usize;
            pixels[index + 2] > pixels[index + 1].saturating_add(30)
                && pixels[index + 2] > pixels[index].saturating_add(30)
        };
        // Lens width is clamped to 5, so clearance is 14 + 5 + 4 = 23.
        assert!(
            red(&pixels, 151.0, 41.0),
            "ordinal must be laid out in lens logical coordinates at DPR {scale}"
        );
        let edge_point = AnnotationNode::new(
            AnnotationId::new("edge-point").unwrap(),
            1,
            AnnotationGeometry::Point {
                position: NormalizedPoint::new(0.53125, 0.46875).unwrap(),
            },
        )
        .unwrap();
        renderer
            .apply_scene(&SceneSnapshot::new(SceneRevision(2), vec![edge_point], None).unwrap())
            .unwrap();
        let request = renderer.frame_state().take_request().unwrap();
        let (_, pixels) = renderer.render_headless_capture(request).unwrap();
        assert!(
            red(&pixels, 137.0, 55.0),
            "near the lens upper-right corner, ordinal must choose the in-bounds lower-left candidate"
        );
        let metrics = ('0'..='9')
            .map(|digit| {
                (
                    digit,
                    viewer_render_wgpu::GlyphMetrics {
                        uv_min: [0.0, 0.0],
                        uv_max: [1.0, 1.0],
                        size_px: [7.0, 10.0],
                        bearing_px: [0.0, 8.0],
                        advance_px: 8.0,
                    },
                )
            })
            .collect();
        renderer
            .set_ordinal_glyph_atlas(
                &viewer_render_wgpu::OrdinalGlyphAtlas::new(1, 1, vec![255], metrics).unwrap(),
            )
            .unwrap();
        let request = renderer.frame_state().take_request().unwrap();
        let (_, pixels) = renderer.render_headless_capture(request).unwrap();
        let center_index =
            (((55.0 * scale) as u32 * physical.width + (137.0 * scale) as u32) * 4) as usize;
        assert!(
            pixels[center_index..center_index + 3]
                .iter()
                .all(|value| *value > 245),
            "glyph follows lens-specific badge position"
        );
        assert!(
            !red(&pixels, 137.0, 59.0),
            "glyph height remains readable at each DPR"
        );
        assert!(
            red(&pixels, 137.0, 61.0),
            "glyph height stays fixed rather than scaling fourfold with lens"
        );
        let mut rectangle = AnnotationNode::new(
            AnnotationId::new("rect").unwrap(),
            2,
            AnnotationGeometry::Rectangle {
                rect: viewer_render_core::NormalizedRect::new(0.25, 0.25, 0.5, 0.5).unwrap(),
            },
        )
        .unwrap();
        rectangle.selected = true;
        renderer
            .apply_scene(&SceneSnapshot::new(SceneRevision(3), vec![rectangle], None).unwrap())
            .unwrap();
        for (magnification, inside, outside) in [(1.0, 0.5, 2.0), (2.0, 1.5, 3.0), (4.0, 1.5, 3.0)]
        {
            renderer
                .set_magnifier(Some(
                    MagnifierConfig::new(
                        NormalizedPoint::new(0.25, 0.25).unwrap(),
                        LogicalPoint::new(128.0, 64.0).unwrap(),
                        96.0,
                        96.0,
                        magnification,
                        MagnifierShape::RoundedRectangle,
                    )
                    .unwrap(),
                ))
                .unwrap();
            let retained = renderer.retained_scene_resources();
            let request = renderer.frame_state().take_request().unwrap();
            let (_, pixels) = renderer.render_headless_capture(request).unwrap();
            assert!(
                red(&pixels, 128.0, 64.0),
                "lens must not paint a white editing handle"
            );
            assert!(
                red(&pixels, 145.0, 64.0 + inside),
                "stroke width follows the old [2,5] cap, scale={scale}, mag={magnification}"
            );
            assert!(
                !red(&pixels, 145.0, 64.0 + outside),
                "stroke width must not exceed the old cap"
            );
            let stroke_pixels = ((54.0 * scale) as u32..(74.0 * scale) as u32)
                .filter(|y| red(&pixels, 145.0, f64::from(*y) / scale))
                .count();
            assert_eq!(
                stroke_pixels,
                ((2.0 * magnification).clamp(2.0, 5.0) * scale) as usize
            );
            assert_eq!(renderer.retained_scene_resources(), retained);
        }
    }
}
