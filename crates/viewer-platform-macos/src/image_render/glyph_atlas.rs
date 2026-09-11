use std::{collections::BTreeMap, sync::OnceLock};

use objc2::{ClassType, MainThreadMarker, runtime::AnyObject};
use objc2_app_kit::{
    NSColor, NSFont, NSFontAttributeName, NSForegroundColorAttributeName, NSGraphicsContext,
    NSStringDrawing,
};
use objc2_core_foundation::CGPoint;
use objc2_core_graphics::{CGBitmapContextCreate, CGContext, CGImageAlphaInfo};
use objc2_foundation::{NSDictionary, NSString};
use viewer_render_wgpu::{GlyphMetrics, OrdinalGlyphAtlas};

use super::SurfaceError;

const GLYPHS: [char; 10] = ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];
const CELL_WIDTH: u32 = 16;
const ATLAS_HEIGHT: u32 = 20;
const FONT_SIZE: f64 = 14.0;
const HORIZONTAL_PADDING: f64 = 2.0;
/// Fallback rasterization ratio for callers without a concrete surface scale.
/// 2x matches Retina; 1x displays minify harmlessly through the linear
/// sampler, so an oversupplied atlas never looks worse than an undersupplied
/// one.
const FALLBACK_SCALE: f64 = 2.0;

static SYSTEM_ORDINAL_GLYPH_ATLAS: OnceLock<OrdinalGlyphAtlas> = OnceLock::new();

/// Returns the immutable system-font atlas shared by every native renderer.
/// The first call must happen on AppKit's main thread; subsequent calls clone
/// only the small CPU descriptor and never rasterize text in the frame path.
pub fn system_ordinal_glyph_atlas() -> Result<OrdinalGlyphAtlas, SurfaceError> {
    if let Some(atlas) = SYSTEM_ORDINAL_GLYPH_ATLAS.get() {
        return Ok(atlas.clone());
    }
    let memory = viewer_render_core::ImageMemoryCoordinator::new(
        viewer_render_core::ImageMemoryPolicy::baseline_8gb(),
    );
    let atlas = system_ordinal_glyph_atlas_with_memory(&memory, FALLBACK_SCALE)?;
    let _ = SYSTEM_ORDINAL_GLYPH_ATLAS.set(atlas);
    SYSTEM_ORDINAL_GLYPH_ATLAS
        .get()
        .cloned()
        .ok_or(SurfaceError::GlyphAtlasUnavailable)
}

/// Rasterizes the ordinal glyph atlas at an integer multiple of the logical
/// design size. Metrics are reported in logical pixels (the drawing path is
/// scale-agnostic) while the texture holds `scale`-denser texels, so glyph
/// quads sample near 1:1 on a Retina display instead of magnifying a 1x
/// raster by the backing scale factor.
///
/// `scale` is rounded up to an integer ratio (fractional backing scales such
/// as 1.5x rasterize at 2x) and clamped to [1, 3]. The atlas is rebuilt per
/// surface mount, so a surface moving between displays with different scales
/// picks up the new ratio on its next mount.
pub fn system_ordinal_glyph_atlas_with_memory(
    memory: &viewer_render_core::ImageMemoryCoordinator,
    scale: f64,
) -> Result<OrdinalGlyphAtlas, SurfaceError> {
    let scale = scale.ceil().clamp(1.0, 3.0);
    MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
    let atlas_width = CELL_WIDTH * GLYPHS.len() as u32 * scale as u32;
    let atlas_height = ATLAS_HEIGHT * scale as u32;
    let mut pixels = viewer_render_core::SharedPixels::try_zeroed(
        memory,
        viewer_render_core::AssetGeneration(0),
        u64::from(atlas_width) * u64::from(atlas_height),
    )
    .map_err(|_| SurfaceError::GlyphAtlasUnavailable)?;
    // SAFETY: `pixels` is a checked, tightly packed one-byte alpha buffer and
    // stays alive until the bitmap context is dropped after synchronous text
    // drawing. Alpha-only contexts intentionally have no color space.
    let context = unsafe {
        CGBitmapContextCreate(
            pixels
                .get_mut()
                .expect("unique atlas storage")
                .as_mut_ptr()
                .cast(),
            atlas_width as usize,
            atlas_height as usize,
            8,
            atlas_width as usize,
            None,
            CGImageAlphaInfo::Only.0,
        )
    }
    .ok_or(SurfaceError::GlyphAtlasUnavailable)?;
    CGContext::set_should_antialias(Some(&context), true);

    let graphics_context = NSGraphicsContext::graphicsContextWithCGContext_flipped(&context, true);
    let _guard = GraphicsContextGuard::install(&context, &graphics_context);
    let font = NSFont::boldSystemFontOfSize(FONT_SIZE * scale);
    let color = NSColor::whiteColor();
    let values: [&AnyObject; 2] = [font.as_super().as_super(), color.as_super().as_super()];
    // SAFETY: AppKit exports both attribute-name objects for the process
    // lifetime, and the paired values use their documented object types.
    let keys = unsafe { [NSFontAttributeName, NSForegroundColorAttributeName] };
    let attributes = NSDictionary::from_slices(&keys, &values);
    let mut metrics = BTreeMap::new();
    for (index, glyph) in GLYPHS.into_iter().enumerate() {
        let label = NSString::from_str(&glyph.to_string());
        // SAFETY: AppKit receives its documented NSFont and NSColor values.
        let measured = unsafe { label.sizeWithAttributes(Some(&attributes)) };
        let glyph_width = (measured.width.ceil() + HORIZONTAL_PADDING * scale)
            .clamp(1.0, f64::from(CELL_WIDTH) * scale);
        let cell_x = index as f64 * f64::from(CELL_WIDTH) * scale;
        let origin = CGPoint::new(
            cell_x + (glyph_width - measured.width) / 2.0,
            (f64::from(atlas_height) - measured.height) / 2.0,
        );
        // SAFETY: The correctly typed attribute dictionary and bitmap context
        // remain alive for this synchronous draw operation.
        unsafe { label.drawAtPoint_withAttributes(origin, Some(&attributes)) };
        metrics.insert(
            glyph,
            GlyphMetrics {
                uv_min: [cell_x as f32 / atlas_width as f32, 0.0],
                uv_max: [(cell_x + glyph_width) as f32 / atlas_width as f32, 1.0],
                size_px: [(glyph_width / scale) as f32, ATLAS_HEIGHT as f32],
                bearing_px: [0.0, 0.0],
                advance_px: (glyph_width / scale) as f32,
            },
        );
    }
    drop(_guard);
    drop(context);

    // AppKit's flipped flag describes text layout, but does not normalize the
    // bitmap CGContext's bottom-up storage. Texture v=0 is the top row in both
    // native glyph passes, so normalize once at this raster/atlas boundary.
    let row_bytes = atlas_width as usize;
    for top in 0..atlas_height as usize / 2 {
        let bottom = atlas_height as usize - 1 - top;
        for x in 0..row_bytes {
            pixels
                .get_mut()
                .expect("unique atlas storage")
                .swap(top * row_bytes + x, bottom * row_bytes + x);
        }
    }

    OrdinalGlyphAtlas::from_shared(atlas_width, atlas_height, pixels, metrics)
        .map_err(|_| SurfaceError::GlyphAtlasUnavailable)
}

struct GraphicsContextGuard<'a> {
    context: &'a CGContext,
    previous: Option<objc2::rc::Retained<NSGraphicsContext>>,
}

impl<'a> GraphicsContextGuard<'a> {
    fn install(context: &'a CGContext, graphics_context: &NSGraphicsContext) -> Self {
        let previous = NSGraphicsContext::currentContext();
        CGContext::save_g_state(Some(context));
        NSGraphicsContext::setCurrentContext(Some(graphics_context));
        Self { context, previous }
    }
}

impl Drop for GraphicsContextGuard<'_> {
    fn drop(&mut self) {
        CGContext::restore_g_state(Some(self.context));
        NSGraphicsContext::setCurrentContext(self.previous.as_deref());
    }
}
