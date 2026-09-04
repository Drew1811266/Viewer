use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapInfo, CGColorSpace, CGContext, CGImage, CGImageAlphaInfo,
    CGImageByteOrderInfo, kCGColorSpaceSRGB,
};

use super::ImageResourceError;

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
    pub pixels: Vec<u8>,
}

pub(super) fn normalize_to_bgra_srgb(image: &CGImage) -> Result<DecodedPixels, ImageResourceError> {
    let width = u32::try_from(CGImage::width(Some(image)))
        .map_err(|_| ImageResourceError::LimitExceeded)?;
    let height = u32::try_from(CGImage::height(Some(image)))
        .map_err(|_| ImageResourceError::LimitExceeded)?;
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

    let mut pixels = vec![0_u8; byte_len];
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
            pixels.as_mut_ptr().cast(),
            width as usize,
            height as usize,
            8,
            bytes_per_row as usize,
            Some(&color_space),
            bitmap_info,
        )
    }
    .ok_or(ImageResourceError::ColorConversion)?;
    let bounds = CGRect::new(
        CGPoint::ZERO,
        CGSize::new(f64::from(width), f64::from(height)),
    );
    CGContext::draw_image(Some(&context), bounds, Some(image));
    drop(context);

    // Bitmap contexts expose their first row at Quartz y=0 (the bottom).
    // Renderer UVs use a top-left origin, so normalize the row order once at
    // decode time rather than adding a second coordinate convention.
    let row_len = bytes_per_row as usize;
    for top in 0..height as usize / 2 {
        let bottom = height as usize - 1 - top;
        let (head, tail) = pixels.split_at_mut(bottom * row_len);
        head[top * row_len..(top + 1) * row_len].swap_with_slice(&mut tail[..row_len]);
    }

    Ok(DecodedPixels {
        width,
        height,
        bytes_per_row,
        pixel_format: PixelFormat::Bgra8PremultipliedSrgb,
        pixels,
    })
}
