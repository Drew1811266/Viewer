use objc2_core_foundation::{CFString, CFURL};
use objc2_core_graphics::CGImage;
use objc2_image_io::CGImageDestination;
use std::{fs, path::Path};
use viewer_application::ImageError;

pub(super) fn encode_png(image: &CGImage, destination: &Path) -> Result<(), ImageError> {
    if destination.exists() {
        fs::remove_file(destination).map_err(|error| {
            ImageError::Io(format!(
                "remove existing image artifact {}: {error}",
                destination.display()
            ))
        })?;
    }

    let url = CFURL::from_file_path(destination).ok_or_else(|| {
        ImageError::Io(format!(
            "image artifact path cannot be represented as a file URL: {}",
            destination.display()
        ))
    })?;
    let png_type = CFString::from_str("public.png");
    // SAFETY: `url`, `png_type`, the image count, and the absent options have
    // the Core Foundation types required by CGImageDestination.
    let image_destination = unsafe { CGImageDestination::with_url(&url, &png_type, 1, None) }
        .ok_or_else(|| {
            ImageError::Io(format!(
                "create PNG image destination at {}",
                destination.display()
            ))
        })?;

    // SAFETY: `image` is retained for this call and no untyped properties are
    // passed to Image I/O.
    unsafe { image_destination.add_image(image, None) };
    // SAFETY: The destination contains exactly the single image declared at
    // construction and is not used after finalization.
    if !unsafe { image_destination.finalize() } {
        let _ = fs::remove_file(destination);
        return Err(ImageError::Io(format!(
            "finalize PNG image artifact at {}",
            destination.display()
        )));
    }

    Ok(())
}
