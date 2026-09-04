use viewer_platform_macos::image_render::{
    AuthorizedImageSource, DecodedResourceKind, ImageResourceError, MacImageResourceProvider,
    PixelFormat, PreviewRequest,
};
use viewer_render_core::{
    AssetGeneration, MemoryBudget, ResourcePlan, TextureStrategy, TileCoordinate,
};
use viewer_test_support::image_fixtures::image_fixture;

fn provider() -> (tempfile::TempDir, MacImageResourceProvider) {
    let cache = tempfile::tempdir().unwrap();
    let provider =
        MacImageResourceProvider::new(cache.path(), MemoryBudget::baseline_8gb()).unwrap();
    (cache, provider)
}

fn source(name: &str) -> AuthorizedImageSource {
    AuthorizedImageSource::authorize_for_process(image_fixture(name)).unwrap()
}

#[test]
fn probe_reuses_image_io_metadata_for_jpeg_png_orientation_alpha_and_profile() {
    let (_cache, provider) = provider();
    let rotated = provider.probe(&source("rotated-6.jpg")).unwrap();
    assert_eq!(
        (rotated.width, rotated.height, rotated.orientation),
        (800, 600, 6)
    );

    let alpha = provider.probe(&source("alpha.png")).unwrap();
    assert!(alpha.has_alpha);

    let p3 = provider.probe(&source("p3.jpg")).unwrap();
    assert_eq!(p3.icc_profile_name.as_deref(), Some("Display P3"));
}

#[test]
fn previews_are_exif_upright_premultiplied_bgra_srgb() {
    let (_cache, provider) = provider();
    let preview = provider
        .request_preview(
            &source("rotated-6.jpg"),
            AssetGeneration(1),
            PreviewRequest::new(300, 300).unwrap(),
        )
        .unwrap();

    assert_eq!((preview.width, preview.height), (225, 300));
    assert_eq!(preview.bytes_per_row, preview.width * 4);
    assert_eq!(
        preview.pixels.len(),
        (preview.bytes_per_row * preview.height) as usize
    );
    assert_eq!(preview.pixel_format, PixelFormat::Bgra8PremultipliedSrgb);
    assert!(matches!(preview.kind, DecodedResourceKind::Preview { .. }));
}

#[test]
fn same_aspect_ratio_gets_the_same_requested_preview_geometry() {
    let (_cache, provider) = provider();
    let request = PreviewRequest::new(300, 300).unwrap();
    let small = provider
        .request_preview(&source("alpha.png"), AssetGeneration(1), request)
        .unwrap();
    let large = provider
        .request_preview(&source("srgb.jpg"), AssetGeneration(2), request)
        .unwrap();

    assert_eq!((small.width, small.height), (300, 225));
    assert_eq!((large.width, large.height), (300, 225));
}

#[test]
fn alpha_preview_bytes_are_premultiplied() {
    let (_cache, provider) = provider();
    let preview = provider
        .request_preview(
            &source("alpha.png"),
            AssetGeneration(1),
            PreviewRequest::new(320, 240).unwrap(),
        )
        .unwrap();

    for pixel in preview.pixels.chunks_exact(4) {
        let alpha = pixel[3];
        assert!(pixel[0] <= alpha && pixel[1] <= alpha && pixel[2] <= alpha);
    }
}

#[test]
fn requested_tiles_share_one_level_decode_and_preserve_edge_dimensions() {
    let (_cache, provider) = provider();
    let plan = ResourcePlan {
        strategy: TextureStrategy::Tiled { tile_size: 512 },
        level: 0,
        required_tiles: vec![
            TileCoordinate {
                level: 0,
                x: 0,
                y: 0,
            },
            TileCoordinate {
                level: 0,
                x: 1,
                y: 1,
            },
        ],
        prefetch_tiles: vec![],
    };
    let resources = provider
        .request_tiles(&source("srgb.jpg"), AssetGeneration(4), &plan)
        .unwrap();

    assert_eq!(resources.len(), 2);
    assert_eq!((resources[0].width, resources[0].height), (512, 512));
    assert_eq!((resources[1].width, resources[1].height), (512, 256));
    assert!(matches!(
        resources[1].kind,
        DecodedResourceKind::Tile(TileCoordinate { x: 1, y: 1, .. })
    ));
}

#[test]
fn corrupt_sources_and_cancelled_generations_never_produce_resources() {
    let (_cache, provider) = provider();
    assert!(matches!(
        provider.probe(&source("corrupt.jpg")),
        Err(ImageResourceError::Image(_))
    ));

    provider.cancel_generation(AssetGeneration(7));
    assert_eq!(
        provider
            .request_preview(
                &source("srgb.jpg"),
                AssetGeneration(7),
                PreviewRequest::new(300, 300).unwrap(),
            )
            .unwrap_err(),
        ImageResourceError::Cancelled
    );
}
