//! Raster format matrix for the macOS Image I/O backend (ADR 0008).
//!
//! Each fixture is a real container (not an extension-only stub), so a decode
//! regression fails here before it reaches the preview path.

use viewer_domain::image::{ImageFormat, ImageRepresentationKind};
use viewer_platform_macos::image::image_io::ImageIoBackend;
use viewer_test_support::image_fixtures::image_fixture;

#[test]
fn probes_every_supported_raster_container() {
    let backend = ImageIoBackend::default();
    let cases = [
        ("srgb.jpg", ImageFormat::Jpeg),
        ("alpha.png", ImageFormat::Png),
        ("animated.gif", ImageFormat::Gif),
        ("pages.tiff", ImageFormat::Tiff),
        ("grid.avif", ImageFormat::Avif),
        ("p3.heic", ImageFormat::Heic),
        ("solid.bmp", ImageFormat::Bmp),
        ("tiny.webp", ImageFormat::Webp),
        ("tiny.psd", ImageFormat::Psd),
    ];

    for (name, expected) in cases {
        let probe = backend
            .probe_sync(image_fixture(name))
            .unwrap_or_else(|error| panic!("{name} failed to probe: {error:?}"));
        assert_eq!(probe.format, expected, "{name}");
        assert!(probe.width > 0 && probe.height > 0, "{name}");
    }
}

#[test]
fn decodes_webp_and_psd_through_the_shared_backend() {
    let backend = ImageIoBackend::default();
    for name in ["tiny.webp", "tiny.psd"] {
        let destination = tempfile::NamedTempFile::new().unwrap();
        let dimensions = backend
            .render_sync(
                image_fixture(name),
                ImageRepresentationKind::Thumbnail {
                    max_pixels: 1,
                    scale_milli: 1_000,
                },
                destination.path(),
            )
            .unwrap_or_else(|error| panic!("{name} failed to decode: {error:?}"));
        assert_eq!(dimensions, (1, 1), "{name}");
    }
}
