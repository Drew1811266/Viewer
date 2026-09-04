use std::collections::BTreeMap;

use viewer_render_core::{
    AnnotationGeometry, AnnotationId, AnnotationNode, CameraState, LogicalPoint, LogicalSize,
    NormalizedPoint, Rotation, SceneRevision, SceneSnapshot, SourceSize, TransformSnapshot,
    ViewportLayout,
};
use viewer_render_wgpu::{
    AnnotationMeshCache, BufferCapacityPlan, GlyphMetrics, MeshUpdate, OrdinalGlyphAtlas,
    RendererDescriptor, WgpuImageRenderer,
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
    let logical = LogicalSize::new(64.0, 64.0).unwrap();
    let mut renderer = WgpuImageRenderer::new(
        RendererDescriptor::headless(
            logical,
            viewer_render_core::PhysicalSize {
                width: 64,
                height: 64,
            },
            1.0,
        )
        .unwrap(),
    )
    .unwrap();
    let source = SourceSize::new(64, 64).unwrap();
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
    let center = &pixels[((32 * 64 + 32) * 4)..((32 * 64 + 32) * 4 + 4)];
    assert!(center[0] > 240 && center[1] > 240 && center[2] > 240);
}
