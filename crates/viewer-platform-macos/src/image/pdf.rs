//! Adobe Illustrator decoding backend (0.2.2, ADR 0008).
//!
//! Modern `.ai` files are PDF-compatible streams; this module renders page 1
//! through `CGPDFDocument` into the shared PNG artifact pipeline. Legacy
//! pure-PostScript `.ai` sources (pre-Illustrator 9) are detected by the
//! `%!PS` signature and refused with a clear unsupported error instead of
//! being mis-rendered.

use std::{fs::File, io::Read, path::Path};

use viewer_application::ImageError;
use viewer_domain::image::{ImageFormat, ImageProbe};

use objc2_core_foundation::{CFRetained, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGContext, CGPDFBox, CGPDFDocument, CGPDFPage};

use super::svg::{context_image, fit_dimensions, rgba_bitmap_context};

pub(super) fn probe_ai(source: &Path) -> Result<ImageProbe, ImageError> {
    let page = open_first_page(source)?;
    let rect = media_box(&page);
    let width = (rect.size.width.max(1.0))
        .round()
        .clamp(1.0, f64::from(u32::MAX));
    let height = (rect.size.height.max(1.0))
        .round()
        .clamp(1.0, f64::from(u32::MAX));
    Ok(ImageProbe {
        format: ImageFormat::Ai,
        width: width as u32,
        height: height as u32,
        orientation: 1,
        has_alpha: false,
        icc_profile_name: None,
    })
}

pub(super) fn render_ai(
    source: &Path,
    max_dimension: u32,
    destination: &Path,
) -> Result<(u32, u32), ImageError> {
    let page = open_first_page(source)?;
    let rect = media_box(&page);
    let page_width = rect.size.width.max(1.0) as f32;
    let page_height = rect.size.height.max(1.0) as f32;
    let (width, height) = fit_dimensions(page_width, page_height, max_dimension);

    let context = rgba_bitmap_context(width, height)?;
    CGContext::set_rgb_fill_color(Some(&context), 1.0, 1.0, 1.0, 1.0);
    CGContext::fill_rect(
        Some(&context),
        CGRect::new(
            CGPoint::ZERO,
            CGSize::new(f64::from(width), f64::from(height)),
        ),
    );
    let scale_x = f64::from(width) / f64::from(page_width);
    let scale_y = f64::from(height) / f64::from(page_height);
    CGContext::translate_ctm(
        Some(&context),
        -rect.origin.x * scale_x,
        -rect.origin.y * scale_y,
    );
    CGContext::scale_ctm(Some(&context), scale_x, scale_y);
    CGContext::draw_pdf_page(Some(&context), Some(&page));

    // SAFETY: the white fill initialized the whole buffer before drawing.
    let image = unsafe { context_image(&context)? };
    super::encode::encode_png(&image, destination)?;
    Ok((width, height))
}

fn open_first_page(source: &Path) -> Result<CFRetained<CGPDFPage>, ImageError> {
    ensure_pdf_signature(source)?;
    let url = objc2_core_foundation::CFURL::from_file_path(source).ok_or(ImageError::Corrupt)?;
    // SAFETY: metadata-only document open with a valid file URL.
    let document = CGPDFDocument::with_url(Some(&url)).ok_or(ImageError::Corrupt)?;
    // SAFETY: page lookup on a valid document; CGPDFDocument pages are 1-based.
    let page = CGPDFDocument::page(Some(&document), 1).ok_or(ImageError::Corrupt)?;
    Ok(page)
}

fn ensure_pdf_signature(source: &Path) -> Result<(), ImageError> {
    let mut file = File::open(source).map_err(|_| ImageError::Corrupt)?;
    let mut prefix = [0_u8; 1024];
    let read = file.read(&mut prefix).map_err(|_| ImageError::Corrupt)?;
    let prefix = &prefix[..read];
    if prefix.starts_with(b"%PDF") {
        return Ok(());
    }
    if prefix.starts_with(b"%!PS") {
        return Err(ImageError::Unsupported);
    }
    Err(ImageError::Corrupt)
}

fn media_box(page: &CGPDFPage) -> CGRect {
    CGPDFPage::box_rect(Some(page), CGPDFBox::MediaBox)
}
