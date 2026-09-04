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

static SYSTEM_ORDINAL_GLYPH_ATLAS: OnceLock<OrdinalGlyphAtlas> = OnceLock::new();

/// Returns the immutable system-font atlas shared by every native renderer.
/// The first call must happen on AppKit's main thread; subsequent calls clone
/// only the small CPU descriptor and never rasterize text in the frame path.
pub fn system_ordinal_glyph_atlas() -> Result<OrdinalGlyphAtlas, SurfaceError> {
    if let Some(atlas) = SYSTEM_ORDINAL_GLYPH_ATLAS.get() {
        return Ok(atlas.clone());
    }
    MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
    let atlas = render_system_ordinal_glyph_atlas()?;
    let _ = SYSTEM_ORDINAL_GLYPH_ATLAS.set(atlas);
    SYSTEM_ORDINAL_GLYPH_ATLAS
        .get()
        .cloned()
        .ok_or(SurfaceError::GlyphAtlasUnavailable)
}

fn render_system_ordinal_glyph_atlas() -> Result<OrdinalGlyphAtlas, SurfaceError> {
    let width = CELL_WIDTH * GLYPHS.len() as u32;
    let mut pixels = vec![0_u8; width as usize * ATLAS_HEIGHT as usize];
    // SAFETY: `pixels` is a checked, tightly packed one-byte alpha buffer and
    // stays alive until the bitmap context is dropped after synchronous text
    // drawing. Alpha-only contexts intentionally have no color space.
    let context = unsafe {
        CGBitmapContextCreate(
            pixels.as_mut_ptr().cast(),
            width as usize,
            ATLAS_HEIGHT as usize,
            8,
            width as usize,
            None,
            CGImageAlphaInfo::Only.0,
        )
    }
    .ok_or(SurfaceError::GlyphAtlasUnavailable)?;
    CGContext::set_should_antialias(Some(&context), true);

    let graphics_context = NSGraphicsContext::graphicsContextWithCGContext_flipped(&context, true);
    let _guard = GraphicsContextGuard::install(&context, &graphics_context);
    let font = NSFont::boldSystemFontOfSize(FONT_SIZE);
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
        let glyph_width =
            (measured.width.ceil() + HORIZONTAL_PADDING).clamp(1.0, f64::from(CELL_WIDTH));
        let cell_x = index as f64 * f64::from(CELL_WIDTH);
        let origin = CGPoint::new(
            cell_x + (glyph_width - measured.width) / 2.0,
            (f64::from(ATLAS_HEIGHT) - measured.height) / 2.0,
        );
        // SAFETY: The correctly typed attribute dictionary and bitmap context
        // remain alive for this synchronous draw operation.
        unsafe { label.drawAtPoint_withAttributes(origin, Some(&attributes)) };
        metrics.insert(
            glyph,
            GlyphMetrics {
                uv_min: [cell_x as f32 / width as f32, 0.0],
                uv_max: [(cell_x + glyph_width) as f32 / width as f32, 1.0],
                size_px: [glyph_width as f32, ATLAS_HEIGHT as f32],
                bearing_px: [0.0, 0.0],
                advance_px: glyph_width as f32,
            },
        );
    }
    drop(_guard);
    drop(context);

    OrdinalGlyphAtlas::new(width, ATLAS_HEIGHT, pixels, metrics)
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
