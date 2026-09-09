use std::collections::BTreeMap;

use viewer_render_core::{
    AnnotationGeometry, AnnotationId, AnnotationNode, CameraState, LogicalPoint, LogicalSize,
    NormalizedPoint, Rotation, SceneRevision, SceneSnapshot, SourceSize, TransformSnapshot,
    ViewportLayout,
};
use viewer_render_wgpu::{
    AnnotationMeshCache, AnnotationMeshLayers, BufferCapacityPlan, GlyphMetrics, MeshUpdate,
    OrdinalGlyphAtlas, RendererDescriptor, WgpuImageRenderer,
};

fn scene(revision: u64) -> SceneSnapshot {
    SceneSnapshot::new(
        SceneRevision(revision),
        vec![
            AnnotationNode::new(
                AnnotationId::new("point").unwrap(),
                1,
                AnnotationGeometry::Point {
                    position: NormalizedPoint::new(0.5, 0.5).unwrap(),
                },
            )
            .unwrap(),
        ],
        None,
    )
    .unwrap()
}

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

#[test]
fn unchanged_scene_revision_reuses_retained_mesh_across_camera_changes() {
    let mut cache = AnnotationMeshCache::default();
    let scene = scene(8);

    assert_eq!(cache.update(&scene).unwrap(), MeshUpdate::Rebuilt);
    let vertices = cache.mesh().unwrap().vertices().to_vec();
    let _first_camera = transform(1.0);
    let _second_camera = transform(4.0);
    assert_eq!(cache.update(&scene).unwrap(), MeshUpdate::Reused);

    assert_eq!(cache.rebuild_count(), 1);
    assert_eq!(cache.mesh().unwrap().vertices(), vertices);
}

#[test]
fn transient_projection_never_claims_the_authoritative_scene_revision() {
    let mut cache = AnnotationMeshCache::default();
    let authoritative = scene(8);
    let mut selected = authoritative.annotations()[0].clone();
    selected.selected = true;
    let transient = SceneSnapshot::new(SceneRevision(8), vec![selected], None).unwrap();

    assert_eq!(cache.update(&authoritative).unwrap(), MeshUpdate::Rebuilt);
    assert_eq!(
        cache.update_transient(&transient).unwrap(),
        MeshUpdate::Rebuilt
    );
    assert_eq!(cache.update(&authoritative).unwrap(), MeshUpdate::Rebuilt);
    assert_eq!(cache.update(&authoritative).unwrap(), MeshUpdate::Reused);
    assert_eq!(cache.rebuild_count(), 3);
}

#[test]
fn continuous_draft_updates_do_not_rebuild_a_500_marker_retained_scene() {
    let annotations = (0..500)
        .map(|ordinal| {
            AnnotationNode::new(
                AnnotationId::new(format!("marker-{ordinal}")).unwrap(),
                ordinal + 1,
                AnnotationGeometry::Point {
                    position: NormalizedPoint::new(0.5, 0.5).unwrap(),
                },
            )
            .unwrap()
        })
        .collect();
    let retained = SceneSnapshot::new(SceneRevision(12), annotations, None).unwrap();
    let mut layers = AnnotationMeshLayers::default();
    assert_eq!(
        layers.update_authoritative(&retained).unwrap(),
        MeshUpdate::Rebuilt
    );

    for step in 1..=120 {
        let mut draft = AnnotationNode::new(
            AnnotationId::new("draft").unwrap(),
            0,
            AnnotationGeometry::Rectangle {
                rect: viewer_render_core::NormalizedRect::new(
                    0.1,
                    0.1,
                    f64::from(step) / 240.0,
                    0.25,
                )
                .unwrap(),
            },
        )
        .unwrap();
        draft.draft = true;
        let overlay = SceneSnapshot::new(SceneRevision(12), Vec::new(), Some(draft)).unwrap();
        layers.update_draft(&overlay).unwrap();
    }

    assert_eq!(layers.authoritative().rebuild_count(), 1);
    assert!(layers.authoritative().mesh().unwrap().vertices().len() > 5_000);
    assert_eq!(layers.draft().rebuild_count(), 120);
    assert!(layers.draft().mesh().unwrap().vertices().len() < 100);
}

#[test]
fn gpu_capacity_grows_by_power_of_two_and_does_not_shrink_per_frame() {
    let mut capacity = BufferCapacityPlan::default();

    assert!(capacity.ensure(65, 129));
    assert_eq!(capacity.vertex_capacity, 128);
    assert_eq!(capacity.index_capacity, 256);
    assert!(!capacity.ensure(12, 24));
    assert_eq!(capacity.vertex_capacity, 128);
    assert_eq!(capacity.index_capacity, 256);
}

#[test]
fn glyph_atlas_requires_digits_and_keeps_platform_metrics() {
    let mut metrics = BTreeMap::new();
    for digit in '0'..='9' {
        metrics.insert(
            digit,
            GlyphMetrics {
                uv_min: [digit.to_digit(10).unwrap() as f32 / 10.0, 0.0],
                uv_max: [(digit.to_digit(10).unwrap() + 1) as f32 / 10.0, 1.0],
                size_px: [7.0, 10.0],
                bearing_px: [0.0, 8.0],
                advance_px: 8.0,
            },
        );
    }

    let atlas = OrdinalGlyphAtlas::new(10, 1, vec![255; 10], metrics.clone()).unwrap();

    assert_eq!(atlas.metrics('7'), metrics.get(&'7'));
    assert_eq!(atlas.pixels().len(), 10);
}

#[test]
fn camera_zoom_does_not_change_fixed_screen_line_width_metadata() {
    let mut cache = AnnotationMeshCache::default();
    cache.update(&scene(9)).unwrap();
    let before = cache
        .mesh()
        .unwrap()
        .vertices()
        .iter()
        .map(|vertex| vertex.screen_offset_px)
        .collect::<Vec<_>>();

    let _zoomed = transform(8.0);
    cache.update(&scene(9)).unwrap();
    let after = cache
        .mesh()
        .unwrap()
        .vertices()
        .iter()
        .map(|vertex| vertex.screen_offset_px)
        .collect::<Vec<_>>();

    assert_eq!(before, after);
}

#[test]
fn retained_annotation_gpu_pass_smoke_test_is_explicitly_opt_in() {
    if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() || !cfg!(target_os = "macos") {
        return;
    }
    let logical = LogicalSize::new(128.0, 128.0).unwrap();
    let mut renderer = WgpuImageRenderer::new(
        RendererDescriptor::headless(
            logical,
            viewer_render_core::PhysicalSize {
                width: 128,
                height: 128,
            },
            1.0,
        )
        .unwrap(),
    )
    .unwrap();
    let source = SourceSize::new(128, 128).unwrap();
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
    let mut metrics = BTreeMap::new();
    for digit in '0'..='9' {
        metrics.insert(
            digit,
            GlyphMetrics {
                uv_min: [digit.to_digit(10).unwrap() as f32 / 10.0, 0.0],
                uv_max: [(digit.to_digit(10).unwrap() + 1) as f32 / 10.0, 1.0],
                size_px: [7.0, 10.0],
                bearing_px: [0.0, 8.0],
                advance_px: 8.0,
            },
        );
    }
    renderer
        .set_ordinal_glyph_atlas(&OrdinalGlyphAtlas::new(10, 1, vec![255; 10], metrics).unwrap())
        .unwrap();
    renderer.apply_scene(&scene(10)).unwrap();

    let request = renderer.frame_state().take_request().unwrap();
    let (receipt, pixels) = renderer.render_headless_capture(request).unwrap();
    let red_pixels = pixels
        .chunks_exact(4)
        .filter(|pixel| {
            pixel[2] > pixel[1].saturating_add(20) && pixel[2] > pixel[0].saturating_add(20)
        })
        .count();

    assert!(receipt.gpu_resource_bytes > 0);
    assert!((200..800).contains(&red_pixels));
    let center = &pixels[((44 * 128 + 84) * 4)..((44 * 128 + 84) * 4 + 4)];
    assert!(center[0] > 240 && center[1] > 240 && center[2] > 240);
}

#[test]
fn temporary_annotation_memory_block_does_not_poison_a_later_scene_upload() {
    if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() || !cfg!(target_os = "macos") {
        return;
    }
    use viewer_render_core::{
        AllocationClass, AssetGeneration, ImageMemoryCoordinator, ImageMemoryPolicy,
    };
    use viewer_render_wgpu::RenderError;

    let memory = ImageMemoryCoordinator::new(ImageMemoryPolicy::baseline_8gb());
    let mut renderer = WgpuImageRenderer::new(
        RendererDescriptor::headless(
            LogicalSize::new(256.0, 256.0).unwrap(),
            viewer_render_core::PhysicalSize {
                width: 256,
                height: 256,
            },
            1.0,
        )
        .unwrap()
        .with_memory(memory.clone()),
    )
    .unwrap();

    let scene = SceneSnapshot::new(
        SceneRevision(1),
        (0..500)
            .map(|ordinal| {
                AnnotationNode::new(
                    AnnotationId::new(format!("memory-marker-{ordinal}")).unwrap(),
                    ordinal + 1,
                    AnnotationGeometry::Point {
                        position: NormalizedPoint::new(
                            f64::from(ordinal % 25) / 24.0,
                            f64::from(ordinal / 25) / 19.0,
                        )
                        .unwrap(),
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        None,
    )
    .unwrap();

    let available = memory
        .snapshot()
        .limits
        .gpu_bytes
        .saturating_sub(memory.snapshot().gpu_bytes);
    assert!(
        available > 1,
        "renderer must leave room for the injected blocker"
    );
    let blocker = memory
        .try_reserve(
            AllocationClass::GpuTexture,
            available - 1,
            AssetGeneration(99),
        )
        .unwrap();
    assert!(matches!(
        renderer.apply_scene(&scene),
        Err(RenderError::AnnotationMesh(
            viewer_render_wgpu::MeshError::Memory(
                viewer_render_core::MemoryAdmissionError::TemporarilyBlocked
            )
        ))
    ));

    renderer
        .set_transform(
            TransformSnapshot::new(
                SourceSize::new(256, 256).unwrap(),
                ViewportLayout::new(LogicalSize::new(256.0, 256.0).unwrap(), 1.0, 1.0).unwrap(),
                CameraState::fit(Rotation::Deg0),
            )
            .unwrap(),
        )
        .unwrap();
    drop(blocker);
    let request = renderer
        .frame_state()
        .take_request()
        .expect("the refused scene must keep a frame dirty");
    assert!(renderer.render(request).is_ok());
}

#[test]
fn metal_capture_keeps_ordinals_clear_of_visible_handles_at_retina_scale_and_rotation() {
    if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() || !cfg!(target_os = "macos") {
        return;
    }
    for scale in [1.0, 2.0] {
        let logical = LogicalSize::new(256.0, 256.0).unwrap();
        let physical = viewer_render_core::PhysicalSize {
            width: (256.0 * scale) as u32,
            height: (256.0 * scale) as u32,
        };
        let mut renderer =
            WgpuImageRenderer::new(RendererDescriptor::headless(logical, physical, scale).unwrap())
                .unwrap();
        let mut selected = AnnotationNode::new(
            AnnotationId::new("selected").unwrap(),
            1,
            AnnotationGeometry::Rectangle {
                rect: viewer_render_core::NormalizedRect::new(0.25, 0.25, 0.5, 0.5).unwrap(),
            },
        )
        .unwrap();
        selected.selected = true;
        let overlap = AnnotationNode::new(
            AnnotationId::new("overlap").unwrap(),
            2,
            AnnotationGeometry::Point {
                position: NormalizedPoint::new(0.75, 0.25).unwrap(),
            },
        )
        .unwrap();
        let scene = SceneSnapshot::new(SceneRevision(1), vec![selected, overlap], None).unwrap();
        renderer.apply_scene(&scene).unwrap();
        let retained = renderer.retained_scene_resources();
        for rotation in [
            Rotation::Deg0,
            Rotation::Deg90,
            Rotation::Deg180,
            Rotation::Deg270,
        ] {
            renderer
                .set_transform(
                    TransformSnapshot::new(
                        SourceSize::new(256, 256).unwrap(),
                        ViewportLayout::new(logical, scale, 1.0).unwrap(),
                        CameraState::fit(rotation),
                    )
                    .unwrap(),
                )
                .unwrap();
            let request = renderer.frame_state().take_request().unwrap();
            let (_, pixels) = renderer.render_headless_capture(request).unwrap();
            let pixel = |x: f64, y: f64| {
                let index =
                    (((y * scale) as u32 * physical.width + (x * scale) as u32) * 4) as usize;
                &pixels[index..index + 4]
            };
            let red = |pixel: &[u8]| {
                pixel[2] > pixel[1].saturating_add(30) && pixel[2] > pixel[0].saturating_add(30)
            };
            let white = pixel(192.0, 64.0);
            assert!(
                white[0] > 245 && white[1] > 245 && white[2] > 245,
                "selected corner stays visible over overlapping geometry: scale={scale} rotation={rotation:?}"
            );
            assert!(
                red(pixel(203.0, 64.0)),
                "24 logical px handle rim: scale={scale}"
            );
            assert!(
                red(pixel(216.0, 40.0)),
                "offset ordinal uses same core position: scale={scale}"
            );
            assert!(
                !red(pixel(212.0, 64.0)),
                "badge does not cover the gap beside the handle"
            );
            assert_eq!(
                renderer.retained_scene_resources(),
                retained,
                "camera layout must retain base buffers"
            );
        }
        // Isolate the box's own badge; a different annotation's ordinal is
        // intentionally allowed to overlap in the dense scene above.
        renderer
            .apply_scene(
                &SceneSnapshot::new(SceneRevision(2), vec![scene.annotations()[0].clone()], None)
                    .unwrap(),
            )
            .unwrap();
        let request = renderer.frame_state().take_request().unwrap();
        let (_, pixels) = renderer.render_headless_capture(request).unwrap();
        let index =
            (((51.0 * scale) as u32 * physical.width + (205.0 * scale) as u32) * 4) as usize;
        assert!(
            pixels[index + 2] <= pixels[index + 1].saturating_add(30),
            "box corner and its own badge need a visible diagonal gap"
        );
    }
}
