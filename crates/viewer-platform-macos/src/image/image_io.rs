use super::encode::encode_png;
use objc2_core_foundation::{CFBoolean, CFDictionary, CFNumber, CFString, CFType, CFURL};
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
        let format = image_format(&image_source)?;
        let properties = image_properties(&image_source)?;

        let width = required_u32_property(&properties, unsafe { kCGImagePropertyPixelWidth })?;
        let height = required_u32_property(&properties, unsafe { kCGImagePropertyPixelHeight })?;
        let orientation =
            optional_i64_property(&properties, unsafe { kCGImagePropertyOrientation })
                .unwrap_or(1)
                .try_into()
                .map_err(|_| ImageError::Corrupt)?;
        let has_alpha = optional_bool_property(&properties, unsafe { kCGImagePropertyHasAlpha })
            .unwrap_or(false);
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

    pub fn render_thumbnail_sync(
        &self,
        source: impl AsRef<Path>,
        max_pixels: u32,
        destination: impl AsRef<Path>,
    ) -> Result<(u32, u32), ImageError> {
        if max_pixels == 0 {
            return Err(ImageError::BudgetExceeded);
        }

        let requested_max_pixels = max_pixels;
        let image_source = open_image_source(source.as_ref())?;
        let _ = image_format(&image_source)?;
        let max_pixel_number = CFNumber::new_i64(i64::from(requested_max_pixels));
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

        // SAFETY: The options dictionary contains only the documented Image I/O
        // thumbnail keys with CFBoolean/CFNumber values of the required types.
        let thumbnail = unsafe { image_source.thumbnail_at_index(0, Some(options.as_opaque())) }
            .ok_or(ImageError::Corrupt)?;
        let width = u32::try_from(CGImage::width(Some(&thumbnail)))
            .map_err(|_| ImageError::BudgetExceeded)?;
        let height = u32::try_from(CGImage::height(Some(&thumbnail)))
            .map_err(|_| ImageError::BudgetExceeded)?;
        if width > requested_max_pixels || height > requested_max_pixels {
            return Err(ImageError::BudgetExceeded);
        }

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

fn oriented_dimensions(probe: &ImageProbe) -> (u32, u32) {
    if matches!(probe.orientation, 5..=8) {
        (probe.height, probe.width)
    } else {
        (probe.width, probe.height)
    }
}

fn fit_max_dimension(
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

fn open_image_source(
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

fn image_format(image_source: &CGImageSource) -> Result<ImageFormat, ImageError> {
    // SAFETY: The image source is valid for this metadata-only query.
    let type_identifier = unsafe { image_source.r#type() }.ok_or(ImageError::Corrupt)?;
    match type_identifier.to_string().as_str() {
        "public.jpeg" => Ok(ImageFormat::Jpeg),
        "public.png" => Ok(ImageFormat::Png),
        _ => Err(ImageError::Unsupported),
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
    use viewer_domain::image::{DecodeBudget, ImageRepresentationKind};
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
