//! AppKit font rasterization requires the process main thread, hence this
//! explicit test harness. No windows are created.
use viewer_platform_macos::image_render::system_ordinal_glyph_atlas;
use viewer_render_core::{
    AnnotationGeometry, AnnotationId, AnnotationNode, CameraState, LogicalPoint, LogicalSize,
    NormalizedPoint, PhysicalSize, Rotation, SceneRevision, SceneSnapshot, SourceSize,
    TransformSnapshot, ViewportLayout,
};
use viewer_render_wgpu::{
    MagnifierConfig, MagnifierShape, OrdinalGlyphAtlas, RendererDescriptor, WgpuImageRenderer,
};

fn main() {
    if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
        return;
    }
    let atlas = system_ordinal_glyph_atlas().expect("system glyph atlas on main thread");
    for glyph in ['2', '5'] {
        let metrics = atlas.metrics(glyph).unwrap();
        let left = (metrics.uv_min[0] * atlas.width() as f32).round() as usize;
        // The atlas rasterizes at a supersampled ratio; derive the glyph's
        // physical extent from its uv span instead of the logical size_px.
        let width =
            ((metrics.uv_max[0] - metrics.uv_min[0]) * atlas.width() as f32).round() as usize;
        let rows = atlas
            .pixels()
            .chunks_exact(atlas.width() as usize)
            .map(|row| {
                row[left..left + width]
                    .iter()
                    .map(|value| u64::from(*value))
                    .sum()
            })
            .collect::<Vec<_>>();
        assert_orientation(glyph, &rows, "real system atlas");
    }
    for scale in [1.0, 2.0] {
        capture_orientation(&atlas, scale);
    }
    println!(
        "PASS real system digits 2/5: atlas + main/lens Metal orientation, DPR 1/2, retained buffers"
    );
}

fn assert_orientation(glyph: char, rows: &[u64], context: &str) {
    let ink = rows
        .iter()
        .copied()
        .filter(|sum| *sum > 32)
        .collect::<Vec<_>>();
    assert!(
        ink.len() >= 6,
        "missing system-font ink: {context}: {rows:?}"
    );
    let top: u64 = ink.iter().take(2).sum();
    let bottom: u64 = ink.iter().rev().take(2).sum();
    if glyph == '2' {
        let depth = (ink.len() / 3).max(2);
        assert!(
            ink.iter().rev().take(depth).max() > ink.iter().take(depth).max(),
            "digit 2 must have its strongest horizontal baseline at bottom: {context}: {rows:?}"
        );
    } else {
        assert!(
            top > bottom,
            "digit 5 must have its broad horizontal cap at top: {context}: {rows:?}"
        );
    }
}

fn capture_orientation(atlas: &OrdinalGlyphAtlas, scale: f64) {
    let logical = LogicalSize::new(128.0, 160.0).unwrap();
    let physical = PhysicalSize {
        width: (128.0 * scale) as u32,
        height: (160.0 * scale) as u32,
    };
    let mut renderer =
        WgpuImageRenderer::new(RendererDescriptor::headless(logical, physical, scale).unwrap())
            .unwrap();
    let transform = TransformSnapshot::new(
        SourceSize::new(128, 160).unwrap(),
        ViewportLayout::new(logical, scale, 1.0).unwrap(),
        CameraState::fit(Rotation::Deg0),
    )
    .unwrap();
    renderer.set_transform(transform).unwrap();
    renderer.set_ordinal_glyph_atlas(atlas).unwrap();
    for (index, glyph) in ['2', '5'].into_iter().enumerate() {
        let node = AnnotationNode::new(
            AnnotationId::new("digit").unwrap(),
            glyph.to_digit(10).unwrap(),
            AnnotationGeometry::Point {
                position: NormalizedPoint::new(0.5, 0.375).unwrap(),
            },
        )
        .unwrap();
        renderer
            .apply_scene(
                &SceneSnapshot::new(SceneRevision(index as u64 + 1), vec![node], None).unwrap(),
            )
            .unwrap();
        let retained = renderer.retained_scene_resources();
        renderer
            .set_magnifier(Some(
                MagnifierConfig::new(
                    NormalizedPoint::new(0.5, 0.375).unwrap(),
                    LogicalPoint::new(64.0, 112.0).unwrap(),
                    100.0,
                    100.0,
                    4.0,
                    MagnifierShape::RoundedRectangle,
                )
                .unwrap(),
            ))
            .unwrap();
        let request = renderer.frame_state().take_request().unwrap();
        let (_, pixels) = renderer.render_headless_capture(request).unwrap();
        // Main point=(64,60), clearance20. Lens center=(64,112), clearance23.
        for (center, context) in [((84.0, 40.0), "main Metal"), ((87.0, 89.0), "lens Metal")] {
            let metrics = atlas.metrics(glyph).unwrap();
            let left = ((center.0 - metrics.size_px[0] as f64 / 2.0) * scale).floor() as u32;
            let top = ((center.1 - metrics.size_px[1] as f64 / 2.0) * scale).floor() as u32;
            let width = (metrics.size_px[0] as f64 * scale) as u32;
            let height = (metrics.size_px[1] as f64 * scale) as u32;
            let rows = (top..top + height)
                .map(|y| {
                    (left..left + width)
                        .map(|x| {
                            let offset = ((y * physical.width + x) * 4) as usize;
                            // The badge is a white disc with colored digits, so
                            // digit ink is the *absence* of green: invert the
                            // green channel to keep strokes as the strong signal.
                            u64::from(255u8.saturating_sub(pixels[offset + 1]))
                        })
                        .sum()
                })
                .collect::<Vec<_>>();
            assert_orientation(glyph, &rows, context);
        }
        assert_eq!(renderer.retained_scene_resources(), retained);
    }
}
