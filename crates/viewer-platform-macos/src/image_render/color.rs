use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapInfo, CGColorSpace, CGContext, CGImage, CGImageAlphaInfo,
    CGImageByteOrderInfo, kCGColorSpaceSRGB,
};

use super::ImageResourceError;
use viewer_render_core::{AssetGeneration, ImageMemoryCoordinator, SharedPixels};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PixelFormat {
    Bgra8PremultipliedSrgb,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedPixels {
    pub width: u32,
    pub height: u32,
    pub bytes_per_row: u32,
    pub pixel_format: PixelFormat,
    pub pixels: SharedPixels,
}

pub(super) fn normalize_to_bgra_srgb(
    image: &CGImage,
    memory: &ImageMemoryCoordinator,
    generation: AssetGeneration,
) -> Result<DecodedPixels, ImageResourceError> {
    let width = u32::try_from(CGImage::width(Some(image)))
        .map_err(|_| ImageResourceError::LimitExceeded)?;
    let height = u32::try_from(CGImage::height(Some(image)))
        .map_err(|_| ImageResourceError::LimitExceeded)?;
    normalize_to_bgra_srgb_in_rect(
        image,
        (width, height),
        CGRect::new(
            CGPoint::ZERO,
            CGSize::new(f64::from(width), f64::from(height)),
        ),
        memory,
        generation,
    )
}

// Draw directly into the final preview/tile allocation. Image I/O rounds the
// short edge of a thumbnail independently of our integer mip dimensions.
// A shared full-image transform keeps neighboring tile samples aligned without
// allocating a second full mip to reconcile that rounding.
pub(super) fn normalize_to_bgra_srgb_in_rect(
    image: &CGImage,
    output_size: (u32, u32),
    image_bounds: CGRect,
    memory: &ImageMemoryCoordinator,
    generation: AssetGeneration,
) -> Result<DecodedPixels, ImageResourceError> {
    let (width, height) = output_size;
    let bytes_per_row = width
        .checked_mul(4)
        .ok_or(ImageResourceError::LimitExceeded)?;
    let byte_len = usize::try_from(
        u64::from(bytes_per_row)
            .checked_mul(u64::from(height))
            .ok_or(ImageResourceError::LimitExceeded)?,
    )
    .map_err(|_| ImageResourceError::LimitExceeded)?;
    if width == 0 || height == 0 || byte_len == 0 {
        return Err(ImageResourceError::InvalidRequest);
    }

    let mut pixels = SharedPixels::try_zeroed(memory, generation, byte_len as u64)?;
    // SAFETY: `kCGColorSpaceSRGB` is an immutable Core Graphics constant.
    let color_space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB }))
        .ok_or(ImageResourceError::ColorConversion)?;
    let bitmap_info = CGImageAlphaInfo::PremultipliedFirst.0
        | CGBitmapInfo::from_bits_retain(CGImageByteOrderInfo::Order32Little.0).bits();
    // SAFETY: The vector owns exactly `bytes_per_row * height` writable bytes
    // for the context lifetime. The sRGB color space and 32-bit little-endian
    // premultiplied-first layout produce BGRA bytes on Apple silicon and Intel.
    let context = unsafe {
        CGBitmapContextCreate(
            pixels
                .get_mut()
                .expect("conversion storage is uniquely owned")
                .as_mut_ptr()
                .cast(),
            width as usize,
            height as usize,
            8,
            bytes_per_row as usize,
            Some(&color_space),
            bitmap_info,
        )
    }
    .ok_or(ImageResourceError::ColorConversion)?;
    CGContext::draw_image(Some(&context), image_bounds, Some(image));
    drop(context);

    Ok(DecodedPixels {
        width,
        height,
        bytes_per_row,
        pixel_format: PixelFormat::Bgra8PremultipliedSrgb,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use objc2_core_foundation::CFData;
    use objc2_core_graphics::{
        CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGDataProvider, CGImage,
        CGImageAlphaInfo,
    };

    use super::normalize_to_bgra_srgb;

    #[test]
    fn normalization_preserves_top_to_bottom_cgimage_scanlines() {
        let rgba = [
            255, 0, 0, 255, // top row: red
            0, 0, 255, 255, // bottom row: blue
        ];
        let data = CFData::from_bytes(&rgba);
        let provider = CGDataProvider::with_cf_data(Some(&data)).unwrap();
        let color_space = CGColorSpace::new_device_rgb().unwrap();
        // SAFETY: The provider owns two complete RGBA scanlines and `decode`
        // is null as permitted by Core Graphics for the default channel map.
        let image = unsafe {
            CGImage::new(
                1,
                2,
                8,
                32,
                4,
                Some(&color_space),
                CGBitmapInfo::from_bits_retain(CGImageAlphaInfo::PremultipliedLast.0),
                Some(&provider),
                ptr::null(),
                false,
                CGColorRenderingIntent::RenderingIntentDefault,
            )
        }
        .unwrap();

        let memory = viewer_render_core::ImageMemoryCoordinator::new(
            viewer_render_core::ImageMemoryPolicy::baseline_8gb(),
        );
        let decoded =
            normalize_to_bgra_srgb(&image, &memory, viewer_render_core::AssetGeneration(1))
                .unwrap();

        assert_eq!(&decoded.pixels[..4], &[0, 0, 255, 255]);
        assert_eq!(&decoded.pixels[4..8], &[255, 0, 0, 255]);
        // Region offsets use top-to-bottom image coordinates, just like native
        // CGImage crops. A wrong Y transform would exchange these two rows.
        for (top, expected) in [(0.0, [0, 0, 255, 255]), (1.0, [255, 0, 0, 255])] {
            let region = super::normalize_to_bgra_srgb_in_rect(
                &image,
                (1, 1),
                objc2_core_foundation::CGRect::new(
                    objc2_core_foundation::CGPoint::new(0.0, -(2.0 - top - 1.0)),
                    objc2_core_foundation::CGSize::new(1.0, 2.0),
                ),
                &memory,
                viewer_render_core::AssetGeneration(1),
            )
            .unwrap();
            assert_eq!(&region.pixels[..], &expected);
        }
    }
}
