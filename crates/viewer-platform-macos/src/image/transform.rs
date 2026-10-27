//! Batch image transformation (0.2.4, ADR 0008).
//!
//! Re-encodes a source image into one of the batch target formats through
//! the same CGImageSource/CGImageDestination pipeline that produces preview
//! artifacts, so color management and orientation handling stay identical.
//! Resize reuses the preview thumbnail path (`kCGImageSourceThumbnailMaxPixelSize`)
//! and only ever shrinks.

use std::path::Path;

use viewer_application::ImageError;
use viewer_domain::batch_image::{
    BatchImageResize, BatchImageSettings, BatchImageTargetFormat, MAX_BATCH_EXACT_EDGE,
};

use objc2_core_foundation::{
    CFDictionary, CFRetained, CFString, CFType, CFURL, CGPoint, CGRect, CGSize,
};
use objc2_core_graphics::{CGContext, CGImage};
use objc2_image_io::{
    CGImageDestination, CGImageSource, kCGImageDestinationLossyCompressionQuality,
};

use super::image_io::{open_image_source, probe_image_source, thumbnail_from_image_source};

/// Encode `image` to `destination` using the target format and quality.
pub(super) fn encode_image(
    image: &CFRetained<CGImage>,
    destination: &Path,
    format: BatchImageTargetFormat,
    quality_milli: u16,
) -> Result<(), ImageError> {
    if std::fs::metadata(destination).is_ok() {
        std::fs::remove_file(destination).map_err(|error| {
            ImageError::Io(format!(
                "remove existing batch output {}: {error}",
                destination.display()
            ))
        })?;
    }
    let url = CFURL::from_file_path(destination).ok_or_else(|| {
        ImageError::Io(format!(
            "batch output path cannot be represented as a file URL: {}",
            destination.display()
        ))
    })?;
    let type_identifier = CFString::from_str(target_type_identifier(format));
    let typed_options = destination_options(quality_milli);
    // The untyped Core Foundation dictionary view Image I/O expects.
    let destination_options = typed_options.as_opaque();
    // SAFETY: `url`, the destination UTI, one image, and typed options have
    // the Core Foundation types required by CGImageDestination.
    let image_destination = unsafe {
        CGImageDestination::with_url(&url, &type_identifier, 1, Some(destination_options))
    }
    .ok_or_else(|| {
        ImageError::Io(format!(
            "create batch image destination at {}",
            destination.display()
        ))
    })?;

    // SAFETY: `image` is retained for this call and the options dictionary
    // carries only Image I/O destination properties.
    unsafe { image_destination.add_image(image, Some(destination_options)) };
    // SAFETY: the destination contains exactly the single declared image and
    // is not used after finalization.
    if !unsafe { image_destination.finalize() } {
        let _ = std::fs::remove_file(destination);
        return Err(ImageError::Io(format!(
            "finalize batch image output at {}",
            destination.display()
        )));
    }
    Ok(())
}

/// Decode `source` (with the shared format gate), apply the resize rule, and
/// write the converted output to `destination`. Returns the written
/// dimensions and byte size for the progress result.
pub fn encode_batch_image(
    source: &Path,
    settings: &BatchImageSettings,
    destination: &Path,
) -> Result<(u32, u32, u64), ImageError> {
    let image_source = open_image_source(source)?;
    // The shared gate also revalidates the format before any pixel work.
    let probe = probe_image_source(&image_source)?;
    let thumbnail = match settings.resize {
        BatchImageResize::None => {
            thumbnail_from_image_source(&image_source, probe.width.max(probe.height))?
        }
        BatchImageResize::LongestEdge(edge) => {
            thumbnail_from_image_source(&image_source, probe.width.max(probe.height).min(edge))?
        }
        BatchImageResize::Percent(percent) => {
            let longest = u64::from(probe.width.max(probe.height));
            let target = ((longest * u64::from(percent)) / 100).max(1);
            thumbnail_from_image_source(&image_source, u32::try_from(target).unwrap_or(u32::MAX))?
        }
        BatchImageResize::Exact { width, height } => {
            exact_thumbnail(&image_source, width, height, settings.format)?
        }
    };
    let width =
        u32::try_from(CGImage::width(Some(&thumbnail))).map_err(|_| ImageError::BudgetExceeded)?;
    let height =
        u32::try_from(CGImage::height(Some(&thumbnail))).map_err(|_| ImageError::BudgetExceeded)?;
    encode_image(&thumbnail, destination, settings.format, settings.quality.0)?;
    let size_bytes = std::fs::metadata(destination)
        .map_err(|error| ImageError::Io(format!("stat batch output: {error}")))?
        .len();
    Ok((width, height, size_bytes))
}

/// Draw the oriented source into an exact `width x height` bitmap. The
/// requested geometry may differ from the source aspect ratio; opaque targets
/// are flattened onto white, alpha-capable targets keep a transparent
/// background. Both dimensions are validated by the domain and rechecked here
/// before allocating the context.
fn exact_thumbnail(
    image_source: &CGImageSource,
    width: u32,
    height: u32,
    format: BatchImageTargetFormat,
) -> Result<CFRetained<CGImage>, ImageError> {
    if width == 0 || height == 0 || width > MAX_BATCH_EXACT_EDGE || height > MAX_BATCH_EXACT_EDGE {
        return Err(ImageError::BudgetExceeded);
    }
    let oriented = thumbnail_from_image_source(image_source, width.max(height))?;
    let context = super::svg::rgba_bitmap_context(width, height)?;
    let bounds = CGRect::new(
        CGPoint::new(0.0, 0.0),
        CGSize::new(f64::from(width), f64::from(height)),
    );
    if matches!(
        format,
        BatchImageTargetFormat::Jpeg | BatchImageTargetFormat::Heic
    ) {
        CGContext::set_rgb_fill_color(Some(&context), 1.0, 1.0, 1.0, 1.0);
    } else {
        CGContext::set_rgb_fill_color(Some(&context), 0.0, 0.0, 0.0, 0.0);
    }
    CGContext::fill_rect(Some(&context), bounds);
    CGContext::draw_image(Some(&context), bounds, Some(&oriented));
    // SAFETY: the fill plus draw above initialized the whole bitmap.
    unsafe { super::svg::context_image(&context) }
}

/// Formats this host can actually encode right now (runtime probe;
/// AVIF-class capabilities vary by macOS version and are excluded from the
/// fixed batch target set).
pub fn available_batch_formats() -> Vec<BatchImageTargetFormat> {
    // SAFETY: the probe reads a process-global Image I/O capability table.
    let identifiers = unsafe { objc2_image_io::CGImageDestination::type_identifiers() };
    let count = identifiers.count();
    // SAFETY: Image I/O documents the returned array as CFString elements.
    let typed = unsafe { identifiers.cast_unchecked::<CFString>() };
    let mut encodable = Vec::with_capacity(count as usize);
    for index in 0..count {
        let value = objc2_core_foundation::CFArray::get(typed, index as usize);
        if let Some(value) = value {
            encodable.push(value.to_string());
        }
    }
    [
        BatchImageTargetFormat::Jpeg,
        BatchImageTargetFormat::Png,
        BatchImageTargetFormat::Heic,
        BatchImageTargetFormat::Tiff,
        BatchImageTargetFormat::Webp,
    ]
    .into_iter()
    .filter(|format| {
        encodable
            .iter()
            .any(|identifier| identifier == target_type_identifier(*format))
    })
    .collect()
}

fn target_type_identifier(format: BatchImageTargetFormat) -> &'static str {
    match format {
        BatchImageTargetFormat::Jpeg => "public.jpeg",
        BatchImageTargetFormat::Png => "public.png",
        BatchImageTargetFormat::Heic => "public.heic",
        BatchImageTargetFormat::Tiff => "public.tiff",
        BatchImageTargetFormat::Webp => "org.webmproject.webp",
    }
}

fn destination_options(quality_milli: u16) -> CFRetained<CFDictionary<CFString, CFType>> {
    let quality = objc2_core_foundation::CFNumber::new_f64(f64::from(quality_milli) / 1_000.0);
    CFDictionary::from_slices(
        &[unsafe { kCGImageDestinationLossyCompressionQuality }],
        &[quality.as_ref()],
    )
}

#[cfg(test)]
mod tests {
    use super::encode_batch_image;
    use viewer_domain::batch_image::{
        BatchImageConflictStrategy, BatchImageOutput, BatchImageQuality, BatchImageResize,
        BatchImageSettings, BatchImageTargetFormat,
    };
    use viewer_test_support::image_fixtures::image_fixture;

    fn settings(resize: BatchImageResize) -> BatchImageSettings {
        BatchImageSettings {
            format: BatchImageTargetFormat::Jpeg,
            quality: BatchImageQuality::DEFAULT,
            resize,
            output: BatchImageOutput::Sibling {
                suffix: "-out".into(),
            },
            conflict: BatchImageConflictStrategy::Skip,
        }
    }

    fn encode(resize: BatchImageResize) -> (u32, u32, u64) {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("out.jpg");
        encode_batch_image(&image_fixture("srgb.jpg"), &settings(resize), &destination).unwrap()
    }

    #[test]
    fn longest_edge_shrinks_without_changing_aspect() {
        assert_eq!(encode(BatchImageResize::LongestEdge(200)).0, 200);
        assert_eq!(encode(BatchImageResize::LongestEdge(200)).1, 150);
    }

    #[test]
    fn percent_scales_the_longest_edge() {
        let (width, height, _) = encode(BatchImageResize::Percent(50));
        assert_eq!((width, height), (512, 384));
    }

    #[test]
    fn exact_writes_the_requested_geometry() {
        let (width, height, _) = encode(BatchImageResize::Exact {
            width: 300,
            height: 200,
        });
        assert_eq!((width, height), (300, 200));
    }
}
