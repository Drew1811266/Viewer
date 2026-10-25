use super::encode::encode_png;
use objc2_core_foundation::{
    CFBoolean, CFData, CFDictionary, CFNumber, CFRetained, CFString, CFType, CFURL,
};
use objc2_core_graphics::CGImage;
use objc2_image_io::{
    CGImageSource, CGImageSourceStatus, kCGImagePropertyHasAlpha, kCGImagePropertyOrientation,
    kCGImagePropertyPixelHeight, kCGImagePropertyPixelWidth, kCGImagePropertyProfileName,
    kCGImageSourceCreateThumbnailFromImageAlways, kCGImageSourceCreateThumbnailWithTransform,
    kCGImageSourceShouldCacheImmediately, kCGImageSourceThumbnailMaxPixelSize,
};
use std::path::Path;
use viewer_application::ImageError;
use viewer_domain::image::{DecodeBudget, ImageFormat, ImageProbe, ImageRepresentationKind};

#[derive(Clone, Copy)]
pub struct ImageIoBackend {
    decode_budget: DecodeBudget,
}

impl Default for ImageIoBackend {
    fn default() -> Self {
        Self::new(DecodeBudget::new(700_000_000, 100_000_000))
    }
}

impl ImageIoBackend {
    pub const fn new(decode_budget: DecodeBudget) -> Self {
        Self { decode_budget }
    }

    pub fn probe_sync(&self, source: impl AsRef<Path>) -> Result<ImageProbe, ImageError> {
        let image_source = open_image_source(source.as_ref())?;
        probe_image_source(&image_source)
    }

    pub fn render_thumbnail_sync(
        &self,
        source: impl AsRef<Path>,
        max_pixels: u32,
        destination: impl AsRef<Path>,
    ) -> Result<(u32, u32), ImageError> {
        if max_pixels == 0 {
            return Err(ImageError::BudgetExceeded);
        }

        let image_source = open_image_source(source.as_ref())?;
        let thumbnail = thumbnail_from_image_source(&image_source, max_pixels)?;
        let width = u32::try_from(CGImage::width(Some(&thumbnail)))
            .map_err(|_| ImageError::BudgetExceeded)?;
        let height = u32::try_from(CGImage::height(Some(&thumbnail)))
            .map_err(|_| ImageError::BudgetExceeded)?;

        encode_png(&thumbnail, destination.as_ref())?;
        Ok((width, height))
    }

    pub fn render_sync(
        &self,
        source: impl AsRef<Path>,
        kind: ImageRepresentationKind,
        destination: impl AsRef<Path>,
    ) -> Result<(u32, u32), ImageError> {
        let source = source.as_ref();
        let max_dimension = match kind {
            ImageRepresentationKind::Thumbnail { max_pixels, .. } => max_pixels,
            ImageRepresentationKind::FitPreview {
                max_width,
                max_height,
                ..
            } => {
                let probe = self.probe_sync(source)?;
                fit_max_dimension(&probe, max_width, max_height)?
            }
            ImageRepresentationKind::Original100Percent => {
                let probe = self.probe_sync(source)?;
                if !self
                    .decode_budget
                    .allows_full_decode(probe.width, probe.height, 4)
                {
                    return Err(ImageError::BudgetExceeded);
                }
                let (width, height) = oriented_dimensions(&probe);
                width.max(height)
            }
        };

        self.render_thumbnail_sync(source, max_dimension, destination)
    }
}

pub(crate) fn oriented_dimensions(probe: &ImageProbe) -> (u32, u32) {
    if matches!(probe.orientation, 5..=8) {
        (probe.height, probe.width)
    } else {
        (probe.width, probe.height)
    }
}

pub(crate) fn fit_max_dimension(
    probe: &ImageProbe,
    max_width: u32,
    max_height: u32,
) -> Result<u32, ImageError> {
    if max_width == 0 || max_height == 0 {
        return Err(ImageError::BudgetExceeded);
    }

    let (width, height) = oriented_dimensions(probe);
    if width == 0 || height == 0 {
        return Err(ImageError::Corrupt);
    }
    if width <= max_width && height <= max_height {
        return Ok(width.max(height));
    }

    let (numerator, denominator) =
        if u64::from(max_width) * u64::from(height) <= u64::from(max_height) * u64::from(width) {
            (max_width, width)
        } else {
            (max_height, height)
        };
    let target_width = u64::from(width) * u64::from(numerator) / u64::from(denominator);
    let target_height = u64::from(height) * u64::from(numerator) / u64::from(denominator);
    u32::try_from(target_width.max(target_height).max(1)).map_err(|_| ImageError::BudgetExceeded)
}

pub(crate) fn open_image_source(
    source: &Path,
) -> Result<objc2_core_foundation::CFRetained<CGImageSource>, ImageError> {
    std::fs::metadata(source).map_err(|error| {
        ImageError::Io(format!("read image source {}: {error}", source.display()))
    })?;
    let url = CFURL::from_file_path(source).ok_or_else(|| {
        ImageError::Io(format!(
            "image source path cannot be represented as a file URL: {}",
            source.display()
        ))
    })?;
    // SAFETY: `url` is a valid local file URL and no untyped options are used.
    let image_source = unsafe { CGImageSource::with_url(&url, None) }.ok_or(ImageError::Corrupt)?;
    // SAFETY: The source is fully backed by a local file URL and remains alive
    // for the status query.
    let status = unsafe { image_source.status() };
    if status != CGImageSourceStatus::StatusComplete {
        return Err(ImageError::Corrupt);
    }
    // SAFETY: The source remains alive for the count query.
    if unsafe { image_source.count() } == 0 {
        return Err(ImageError::Corrupt);
    }
    Ok(image_source)
}

pub(super) fn open_image_bytes(bytes: &[u8]) -> Result<CFRetained<CGImageSource>, ImageError> {
    let data = CFData::from_bytes(bytes);
    // SAFETY: `data` contains immutable bytes for the lifetime retained by the
    // image source, and no untyped options are supplied.
    let image_source =
        unsafe { CGImageSource::with_data(&data, None) }.ok_or(ImageError::Corrupt)?;
    validate_image_source(&image_source)?;
    Ok(image_source)
}

fn validate_image_source(image_source: &CGImageSource) -> Result<(), ImageError> {
    // SAFETY: The source owns immutable local bytes and remains alive for both
    // status queries.
    if unsafe { image_source.status() } != CGImageSourceStatus::StatusComplete
        || unsafe { image_source.count() } == 0
    {
        return Err(ImageError::Corrupt);
    }
    Ok(())
}

pub(crate) fn probe_image_source(image_source: &CGImageSource) -> Result<ImageProbe, ImageError> {
    validate_image_source(image_source)?;
    let format = image_format(image_source)?;
    let properties = image_properties(image_source)?;
    let width = required_u32_property(&properties, unsafe { kCGImagePropertyPixelWidth })?;
    let height = required_u32_property(&properties, unsafe { kCGImagePropertyPixelHeight })?;
    let orientation = optional_i64_property(&properties, unsafe { kCGImagePropertyOrientation })
        .unwrap_or(1)
        .try_into()
        .map_err(|_| ImageError::Corrupt)?;
    let has_alpha =
        optional_bool_property(&properties, unsafe { kCGImagePropertyHasAlpha }).unwrap_or(false);
    let icc_profile_name =
        optional_string_property(&properties, unsafe { kCGImagePropertyProfileName });
    Ok(ImageProbe {
        format,
        width,
        height,
        orientation,
        has_alpha,
        icc_profile_name,
    })
}

pub(crate) fn thumbnail_from_image_source(
    image_source: &CGImageSource,
    max_pixels: u32,
) -> Result<CFRetained<CGImage>, ImageError> {
    if max_pixels == 0 {
        return Err(ImageError::BudgetExceeded);
    }
    let _ = image_format(image_source)?;
    let max_pixel_number = CFNumber::new_i64(i64::from(max_pixels));
    let options = CFDictionary::<CFString, CFType>::from_slices(
        &[
            unsafe { kCGImageSourceCreateThumbnailFromImageAlways },
            unsafe { kCGImageSourceCreateThumbnailWithTransform },
            unsafe { kCGImageSourceThumbnailMaxPixelSize },
            unsafe { kCGImageSourceShouldCacheImmediately },
        ],
        &[
            CFBoolean::new(true).as_ref(),
            CFBoolean::new(true).as_ref(),
            max_pixel_number.as_ref(),
            CFBoolean::new(true).as_ref(),
        ],
    );
    // SAFETY: The options dictionary contains only documented Image I/O
    // thumbnail keys with values of the required Core Foundation types.
    let thumbnail = unsafe { image_source.thumbnail_at_index(0, Some(options.as_opaque())) }
        .ok_or(ImageError::Corrupt)?;
    let width =
        u32::try_from(CGImage::width(Some(&thumbnail))).map_err(|_| ImageError::BudgetExceeded)?;
    let height =
        u32::try_from(CGImage::height(Some(&thumbnail))).map_err(|_| ImageError::BudgetExceeded)?;
    if width == 0 || height == 0 || width > max_pixels || height > max_pixels {
        return Err(ImageError::BudgetExceeded);
    }
    Ok(thumbnail)
}

fn image_format(image_source: &CGImageSource) -> Result<ImageFormat, ImageError> {
    // SAFETY: The image source is valid for this metadata-only query.
    let type_identifier = unsafe { image_source.r#type() }.ok_or(ImageError::Corrupt)?;
    image_format_for_type_identifier(&type_identifier.to_string()).ok_or(ImageError::Unsupported)
}

/// Formats admitted beyond JPEG/PNG through the same CGImageSource pipeline.
/// Unknown identifiers stay refused (deny by default). RAW formats decode at
/// Image I/O quick-preview quality (ADR 0008).
fn image_format_for_type_identifier(type_identifier: &str) -> Option<ImageFormat> {
    match type_identifier {
        "public.jpeg" => Some(ImageFormat::Jpeg),
        "public.png" => Some(ImageFormat::Png),
        "com.compuserve.gif" => Some(ImageFormat::Gif),
        "public.tiff" => Some(ImageFormat::Tiff),
        "org.webmproject.webp" => Some(ImageFormat::Webp),
        "public.avif" => Some(ImageFormat::Avif),
        "public.heic" | "public.heif" => Some(ImageFormat::Heic),
        "com.microsoft.bmp" => Some(ImageFormat::Bmp),
        "com.microsoft.ico" => Some(ImageFormat::Ico),
        "com.adobe.photoshop-image" => Some(ImageFormat::Psd),
        "public.camera-raw-image"
        | "com.adobe.dng"
        | "com.canon.cr2-raw-image"
        | "com.canon.cr3-raw-image"
        | "com.nikon.nef-raw-image"
        | "com.nikon.nrw-raw-image"
        | "com.sony.arw-raw-image"
        | "com.sony.sr2-raw-image"
        | "com.fuji.raw-image"
        | "com.olympus.orf-raw-image"
        | "com.panasonic.rw2-raw-image"
        | "com.pentax.pef-raw-image"
        | "com.samsung.srw-raw-image"
        | "com.sigma.x3f-raw-image" => Some(ImageFormat::Raw),
        _ => None,
    }
}

fn image_properties(
    image_source: &CGImageSource,
) -> Result<objc2_core_foundation::CFRetained<CFDictionary>, ImageError> {
    // SAFETY: Index zero exists (validated by `open_image_source`) and no
    // untyped options are used.
    unsafe { image_source.properties_at_index(0, None) }.ok_or(ImageError::Corrupt)
}

fn typed_properties(properties: &CFDictionary) -> &CFDictionary<CFString, CFType> {
    // SAFETY: Image I/O documents the returned property dictionary as CFString
    // keys and Core Foundation object values.
    unsafe { properties.cast_unchecked() }
}

fn required_u32_property(properties: &CFDictionary, key: &CFString) -> Result<u32, ImageError> {
    let value = optional_i64_property(properties, key).ok_or(ImageError::Corrupt)?;
    value.try_into().map_err(|_| ImageError::Corrupt)
}

fn optional_i64_property(properties: &CFDictionary, key: &CFString) -> Option<i64> {
    typed_properties(properties)
        .get(key)?
        .downcast::<CFNumber>()
        .ok()?
        .as_i64()
}

fn optional_bool_property(properties: &CFDictionary, key: &CFString) -> Option<bool> {
    typed_properties(properties)
        .get(key)?
        .downcast::<CFBoolean>()
        .ok()
        .map(|value| value.as_bool())
}

fn optional_string_property(properties: &CFDictionary, key: &CFString) -> Option<String> {
    typed_properties(properties)
        .get(key)?
        .downcast::<CFString>()
        .ok()
        .map(|value| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::ImageIoBackend;
    use viewer_application::ImageError;
    use viewer_domain::image::{DecodeBudget, ImageFormat, ImageRepresentationKind};
    use viewer_test_support::image_fixtures::{image_fixture, png_dimensions};

    #[test]
    fn probe_reads_dimensions_orientation_alpha_and_profile() {
        let backend = ImageIoBackend::default();
        let rotated = backend.probe_sync(image_fixture("rotated-6.jpg")).unwrap();
        assert_eq!((rotated.width, rotated.height), (800, 600));
        assert_eq!(rotated.orientation, 6);

        let alpha = backend.probe_sync(image_fixture("alpha.png")).unwrap();
        assert_eq!((alpha.width, alpha.height), (640, 480));
        assert!(alpha.has_alpha);

        let p3 = backend.probe_sync(image_fixture("p3.jpg")).unwrap();
        assert_eq!(p3.icc_profile_name.as_deref(), Some("Display P3"));
    }

    #[test]
    fn corrupt_jpeg_is_isolated() {
        let backend = ImageIoBackend::default();
        assert!(matches!(
            backend.probe_sync(image_fixture("corrupt.jpg")),
            Err(ImageError::Corrupt)
        ));
    }

    #[test]
    fn uti_allowlist_admits_only_documented_formats() {
        use super::image_format_for_type_identifier as map;
        let admitted = [
            ("public.jpeg", ImageFormat::Jpeg),
            ("public.png", ImageFormat::Png),
            ("com.compuserve.gif", ImageFormat::Gif),
            ("public.tiff", ImageFormat::Tiff),
            ("org.webmproject.webp", ImageFormat::Webp),
            ("public.avif", ImageFormat::Avif),
            ("public.heic", ImageFormat::Heic),
            ("public.heif", ImageFormat::Heic),
            ("com.microsoft.bmp", ImageFormat::Bmp),
            ("com.microsoft.ico", ImageFormat::Ico),
            ("com.adobe.photoshop-image", ImageFormat::Psd),
            ("public.camera-raw-image", ImageFormat::Raw),
            ("com.adobe.dng", ImageFormat::Raw),
            ("com.canon.cr3-raw-image", ImageFormat::Raw),
        ];
        for (identifier, format) in admitted {
            assert_eq!(map(identifier), Some(format), "{identifier}");
        }
        for refused in ["public.svg-image", "public.data", "com.microsoft.xlsx", ""] {
            assert_eq!(map(refused), None, "{refused}");
        }
    }

    #[test]
    fn imageio_format_matrix_decodes_admitted_formats() {
        let backend = ImageIoBackend::default();
        let cases = [
            ("animated.gif", ImageFormat::Gif, (640, 480)),
            ("pages.tiff", ImageFormat::Tiff, (800, 600)),
            ("solid.bmp", ImageFormat::Bmp, (512, 384)),
            ("p3.heic", ImageFormat::Heic, (640, 480)),
            ("grid.avif", ImageFormat::Avif, (640, 480)),
            ("dot.ico", ImageFormat::Ico, (16, 16)),
        ];
        for (name, format, dimensions) in cases {
            let probe = backend
                .probe_sync(image_fixture(name))
                .unwrap_or_else(|error| panic!("{name}: {error:?}"));
            assert_eq!(probe.format, format, "{name}");
            assert_eq!((probe.width, probe.height), dimensions, "{name}");
        }
    }

    #[test]
    fn heic_probe_reports_display_p3_profile() {
        let probe = ImageIoBackend::default()
            .probe_sync(image_fixture("p3.heic"))
            .unwrap();
        assert_eq!(probe.icc_profile_name.as_deref(), Some("Display P3"));
    }

    #[test]
    fn truncated_matrix_fixtures_are_isolated_as_corrupt() {
        let backend = ImageIoBackend::default();
        for name in [
            "corrupt.gif",
            "corrupt.tiff",
            "corrupt.heic",
            "corrupt.avif",
        ] {
            assert!(
                matches!(
                    backend.probe_sync(image_fixture(name)),
                    Err(ImageError::Corrupt)
                ),
                "{name} was not isolated as corrupt"
            );
        }
    }

    #[test]
    fn content_mismatch_extension_decodes_by_content_or_is_refused() {
        let result = ImageIoBackend::default().probe_sync(image_fixture("mismatch.webp"));
        match result {
            Ok(probe) => assert_eq!(probe.format, ImageFormat::Jpeg),
            Err(ImageError::Unsupported) => {}
            other => panic!("mismatch.webp: {other:?}"),
        }
    }

    #[test]
    fn forged_dimension_png_never_decodes_within_budget() {
        let backend = ImageIoBackend::default();
        let result = backend.probe_sync(image_fixture("forged.png"));
        match result {
            Err(_) => {}
            Ok(probe) => assert!(
                !DecodeBudget::new(700_000_000, 100_000_000).allows_full_decode(
                    probe.width,
                    probe.height,
                    4
                )
            ),
        }
    }

    #[test]
    fn matrix_formats_render_png_representations() {
        let backend = ImageIoBackend::default();
        for name in [
            "animated.gif",
            "pages.tiff",
            "solid.bmp",
            "p3.heic",
            "grid.avif",
            "dot.ico",
        ] {
            let output = tempfile::NamedTempFile::new().unwrap();
            let dimensions = backend
                .render_sync(
                    image_fixture(name),
                    ImageRepresentationKind::FitPreview {
                        max_width: 300,
                        max_height: 300,
                        scale_milli: 2_000,
                    },
                    output.path(),
                )
                .unwrap_or_else(|error| panic!("{name}: {error:?}"));
            assert_eq!(png_dimensions(output.path()).unwrap(), dimensions, "{name}");
        }
    }

    #[test]
    fn original_decode_of_matrix_formats_respects_the_hard_budget() {
        let backend = ImageIoBackend::new(DecodeBudget::new(1, 1));
        let output = tempfile::NamedTempFile::new().unwrap();
        assert!(matches!(
            backend.render_sync(
                image_fixture("p3.heic"),
                ImageRepresentationKind::Original100Percent,
                output.path()
            ),
            Err(ImageError::BudgetExceeded)
        ));
    }

    #[test]
    fn thumbnail_respects_max_pixel_size() {
        let output = tempfile::NamedTempFile::new().unwrap();
        ImageIoBackend::default()
            .render_thumbnail_sync(image_fixture("srgb.jpg"), 256, output.path())
            .unwrap();
        let (width, height) = png_dimensions(output.path()).unwrap();
        assert!(width <= 256 && height <= 256);
    }

    #[test]
    fn fit_preview_respects_oriented_bounding_box() {
        let output = tempfile::NamedTempFile::new().unwrap();
        let dimensions = ImageIoBackend::default()
            .render_sync(
                image_fixture("rotated-6.jpg"),
                ImageRepresentationKind::FitPreview {
                    max_width: 300,
                    max_height: 300,
                    scale_milli: 2_000,
                },
                output.path(),
            )
            .unwrap();
        assert_eq!(dimensions, (225, 300));
        assert_eq!(png_dimensions(output.path()).unwrap(), dimensions);
    }

    #[test]
    fn original_representation_preserves_oriented_source_dimensions() {
        let output = tempfile::NamedTempFile::new().unwrap();
        let dimensions = ImageIoBackend::default()
            .render_sync(
                image_fixture("rotated-6.jpg"),
                ImageRepresentationKind::Original100Percent,
                output.path(),
            )
            .unwrap();
        assert_eq!(dimensions, (600, 800));
        assert_eq!(png_dimensions(output.path()).unwrap(), dimensions);
    }

    #[test]
    fn original_decode_respects_the_hard_budget() {
        let output = tempfile::NamedTempFile::new().unwrap();
        let backend = ImageIoBackend::new(DecodeBudget::new(1, 1));
        assert!(matches!(
            backend.render_sync(
                image_fixture("srgb.jpg"),
                ImageRepresentationKind::Original100Percent,
                output.path(),
            ),
            Err(ImageError::BudgetExceeded)
        ));
    }
}
