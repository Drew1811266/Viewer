//! SVG decoding backend (0.2.2, ADR 0008).
//!
//! Raster sources go through CGImageSource in `image_io`; SVG and SVGZ
//! sources rasterize here via resvg (usvg parse + tiny-skia render) and then
//! reuse the shared PNG artifact encoder, so every representation kind
//! (thumbnail / fit preview / original 100%) keeps one pipeline.
//!
//! Untrusted-source containment:
//! - usvg parses with external resource resolution disabled; inline `data:`
//!   payloads stay embedded and file/http references never load.
//! - SVGZ streams decompress through a hard byte budget so gzip bombs fail
//!   closed instead of exhausting memory.
//! - Rasterization always happens at the requested representation size,
//!   never at the intrinsic viewBox size, so lying-dimension SVGs cannot
//!   force a giant allocation.

use std::{fs::File, io::Read, path::Path};

use viewer_application::ImageError;
use viewer_domain::image::{ImageFormat, ImageProbe};

use objc2_core_foundation::CFRetained;
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextCreateImage, CGBitmapContextGetData, CGColorSpace,
    CGContext, CGImage, CGImageAlphaInfo, CGImageByteOrderInfo,
};
use resvg::{tiny_skia, usvg};

use super::encode::encode_png;

/// Hard cap on decompressed SVGZ bytes before usvg parsing.
const SVGZ_DECOMPRESSED_LIMIT: u64 = 64 * 1024 * 1024;
/// Hard cap on the raw source bytes read for one vector file.
const VECTOR_SOURCE_LIMIT: u64 = 64 * 1024 * 1024;
/// Intrinsic size reported for SVGs without a usable width/height.
const DEFAULT_VIEWPORT: f32 = 512.0;

pub(super) fn probe_vector(source: &Path, extension: &str) -> Result<ImageProbe, ImageError> {
    match extension {
        "svg" | "svgz" => probe_svg(source),
        "ai" => super::pdf::probe_ai(source),
        _ => Err(ImageError::Unsupported),
    }
}

pub(super) fn render_vector(
    source: &Path,
    extension: &str,
    max_dimension: u32,
    destination: &Path,
) -> Result<(u32, u32), ImageError> {
    if max_dimension == 0 {
        return Err(ImageError::BudgetExceeded);
    }
    match extension {
        "svg" | "svgz" => render_svg(source, max_dimension, destination),
        "ai" => super::pdf::render_ai(source, max_dimension, destination),
        _ => Err(ImageError::Unsupported),
    }
}

fn probe_svg(source: &Path) -> Result<ImageProbe, ImageError> {
    let tree = load_svg_tree(source)?;
    let size = tree.size();
    let (width, height) = intrinsic_dimensions(size.width(), size.height());
    Ok(ImageProbe {
        format: ImageFormat::Svg,
        width: width as u32,
        height: height as u32,
        orientation: 1,
        has_alpha: true,
        icc_profile_name: None,
    })
}

fn render_svg(
    source: &Path,
    max_dimension: u32,
    destination: &Path,
) -> Result<(u32, u32), ImageError> {
    let tree = load_svg_tree(source)?;
    let size = tree.size();
    let (intrinsic_width, intrinsic_height) = intrinsic_dimensions(size.width(), size.height());
    let (width, height) = fit_dimensions(intrinsic_width, intrinsic_height, max_dimension);

    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or(ImageError::BudgetExceeded)?;
    let scale_x = width as f32 / intrinsic_width;
    let scale_y = height as f32 / intrinsic_height;
    let transform = tiny_skia::Transform::from_scale(scale_x, scale_y);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    write_png(pixmap.width(), pixmap.height(), pixmap.data(), destination)
}

/// Hard cap on XML element nesting depth. usvg's recursive descent parser
/// overflows the thread stack long before any built-in limit at hostile
/// depths, so the depth is budgeted with a local byte scan before parsing.
const MAX_ELEMENT_DEPTH: usize = 512;

fn load_svg_tree(source: &Path) -> Result<usvg::Tree, ImageError> {
    let bytes = read_source(source)?;
    if !element_depth_within_budget(&bytes) {
        return Err(ImageError::BudgetExceeded);
    }
    // usvg resolves no external resources by default: no font, file, or
    // network loading happens during parsing (ADR 0008 untrusted-source rule).
    let options = usvg::Options::default();
    usvg::Tree::from_data(&bytes, &options).map_err(|_| ImageError::Corrupt)
}

/// Conservative XML depth scan: counts unclosed element opens. Comments,
/// declarations, and processing instructions do not change depth. The scan
/// over-approximates (attributes containing `>` may split tags), which only
/// makes it stricter, never more permissive.
fn element_depth_within_budget(bytes: &[u8]) -> bool {
    let mut depth = 0_usize;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'<' {
            index += 1;
            continue;
        }
        if bytes[index..].starts_with(b"<!--") {
            let Some(end) = bytes[index..]
                .windows(3)
                .position(|window| window == b"-->")
            else {
                break;
            };
            index += end + 3;
            continue;
        }
        let Some(close) = bytes[index..].iter().position(|byte| *byte == b'>') else {
            break;
        };
        let tag = &bytes[index + 1..index + close];
        if tag.first() == Some(&b'/') {
            depth = depth.saturating_sub(1);
        } else if tag.last() != Some(&b'/')
            && tag
                .first()
                .is_some_and(|byte| *byte != b'!' && *byte != b'?')
        {
            depth += 1;
            if depth > MAX_ELEMENT_DEPTH {
                return false;
            }
        }
        index += close + 1;
    }
    true
}

fn read_source(source: &Path) -> Result<Vec<u8>, ImageError> {
    let mut file = File::open(source)
        .map_err(|error| ImageError::Io(format!("vector source open failed: {error}")))?;
    let mut limited = (&mut file).take(VECTOR_SOURCE_LIMIT + 1);
    let mut raw = Vec::new();
    limited
        .read_to_end(&mut raw)
        .map_err(|_| ImageError::Corrupt)?;
    if raw.len() as u64 > VECTOR_SOURCE_LIMIT {
        return Err(ImageError::BudgetExceeded);
    }
    if source
        .extension()
        .is_some_and(|value| value.eq_ignore_ascii_case("svgz"))
    {
        decompress_svgz(&raw)
    } else {
        Ok(raw)
    }
}

fn decompress_svgz(raw: &[u8]) -> Result<Vec<u8>, ImageError> {
    let decoder = flate2::read::GzDecoder::new(raw);
    let mut limited = decoder.take(SVGZ_DECOMPRESSED_LIMIT + 1);
    let mut out = Vec::new();
    limited
        .read_to_end(&mut out)
        .map_err(|_| ImageError::Corrupt)?;
    if out.len() as u64 > SVGZ_DECOMPRESSED_LIMIT {
        return Err(ImageError::BudgetExceeded);
    }
    Ok(out)
}

fn intrinsic_dimensions(width: f32, height: f32) -> (f32, f32) {
    let width = if width.is_finite() && width > 0.0 {
        width
    } else {
        DEFAULT_VIEWPORT
    };
    let height = if height.is_finite() && height > 0.0 {
        height
    } else {
        DEFAULT_VIEWPORT
    };
    (width, height)
}

pub(super) fn fit_dimensions(width: f32, height: f32, max_dimension: u32) -> (u32, u32) {
    let max = max_dimension as f32;
    let scale = (max / width).min(max / height).min(1.0);
    let scaled_width = (width * scale).round().clamp(1.0, max);
    let scaled_height = (height * scale).round().clamp(1.0, max);
    (scaled_width as u32, scaled_height as u32)
}

pub(super) fn srgb_color_space() -> Result<CFRetained<CGColorSpace>, ImageError> {
    // SAFETY: `kCGColorSpaceSRGB` is an immutable Core Graphics constant.
    unsafe { CGColorSpace::with_name(Some(objc2_core_graphics::kCGColorSpaceSRGB)) }
        .ok_or(ImageError::Corrupt)
}

/// Fresh RGBA bitmap context (`premultipliedLast`, 32Big) of the exact
/// declared geometry; the caller owns the buffer exclusively.
pub(super) fn rgba_bitmap_context(
    width: u32,
    height: u32,
) -> Result<CFRetained<CGContext>, ImageError> {
    let color_space = srgb_color_space()?;
    let bitmap_info = CGImageByteOrderInfo::Order32Big.0 | CGImageAlphaInfo::PremultipliedLast.0;
    // SAFETY: null data makes Core Graphics allocate a private buffer for the
    // declared geometry; every later access goes through its own accessors.
    unsafe {
        CGBitmapContextCreate(
            std::ptr::null_mut(),
            width as usize,
            height as usize,
            8,
            width as usize * 4,
            Some(&color_space),
            bitmap_info,
        )
    }
    .ok_or(ImageError::Corrupt)
}

/// Copy RGBA pixels into a bitmap context's own buffer.
///
/// # Safety
/// The context must come from [`rgba_bitmap_context`] with the same geometry
/// and must not be concurrently accessed.
pub(super) unsafe fn copy_rgba_into_context(
    context: &CGContext,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(), ImageError> {
    if rgba.len() != width as usize * height as usize * 4 {
        return Err(ImageError::Corrupt);
    }
    // The context owns a private buffer sized for `width * height * 4`.
    let data = CGBitmapContextGetData(Some(context));
    if data.is_null() {
        return Err(ImageError::Corrupt);
    }
    // SAFETY: destination length is validated against the slice above.
    unsafe { std::ptr::copy_nonoverlapping(rgba.as_ptr(), data.cast::<u8>(), rgba.len()) };
    Ok(())
}

/// Snapshot a bitmap context into a CGImage.
///
/// # Safety
/// The context buffer must be fully initialized for its declared geometry.
pub(super) unsafe fn context_image(context: &CGContext) -> Result<CFRetained<CGImage>, ImageError> {
    // The caller guarantees the buffer is fully initialized.
    CGBitmapContextCreateImage(Some(context)).ok_or(ImageError::Corrupt)
}

pub(super) fn write_png(
    width: u32,
    height: u32,
    rgba: &[u8],
    destination: &Path,
) -> Result<(u32, u32), ImageError> {
    let context = rgba_bitmap_context(width, height)?;
    // SAFETY: fresh context of the exact declared geometry, sole owner here.
    unsafe { copy_rgba_into_context(&context, width, height, rgba)? };
    // SAFETY: the copy above initialized the whole buffer.
    let image = unsafe { context_image(&context)? };
    encode_png(&image, destination)?;
    Ok((width, height))
}
