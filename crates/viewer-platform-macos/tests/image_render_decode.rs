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
fn tile_decode_rejects_full_mip_staging_that_exceeds_budget() {
    let cache = tempfile::tempdir().unwrap();
    let budget = MemoryBudget::new(1024 * 1024, 1024 * 1024, 16 * 1024 * 1024).unwrap();
    let provider = MacImageResourceProvider::new(cache.path(), budget).unwrap();
    let result = provider.request_tiles(
        &source("srgb.jpg"),
        AssetGeneration(1),
        &ResourcePlan {
            strategy: TextureStrategy::Tiled { tile_size: 512 },
            level: 0,
            required_tiles: vec![TileCoordinate {
                level: 0,
                x: 0,
                y: 0,
            }],
            prefetch_tiles: vec![],
        },
    );
    assert!(matches!(result, Err(ImageResourceError::LimitExceeded)));
}

#[test]
fn tile_decode_crops_before_normalizing_to_fit_native_plus_tiles_budget() {
    let cache = tempfile::tempdir().unwrap();
    let budget = MemoryBudget::new(5 * 1024 * 1024, 1024 * 1024, 16 * 1024 * 1024).unwrap();
    let provider = MacImageResourceProvider::new(cache.path(), budget).unwrap();
    let result = provider.request_tiles(
        &source("srgb.jpg"),
        AssetGeneration(1),
        &ResourcePlan {
            strategy: TextureStrategy::Tiled { tile_size: 512 },
            level: 0,
            required_tiles: vec![TileCoordinate {
                level: 0,
                x: 0,
                y: 0,
            }],
            prefetch_tiles: vec![],
        },
    );
    assert!(
        result.is_ok(),
        "a 3MiB native mip plus 1MiB tile fits without a duplicate normalized mip"
    );
}

#[test]
fn native_tile_crops_match_upright_srgb_preview_rows() {
    for name in ["srgb.jpg", "rotated-6.jpg", "p3.jpg", "alpha.png"] {
        let (_cache, provider) = provider();
        let source = source(name);
        let preview = provider
            .request_preview(
                &source,
                AssetGeneration(1),
                PreviewRequest::new(2048, 2048).unwrap(),
            )
            .unwrap();
        let tile = provider
            .request_tiles(
                &source,
                AssetGeneration(1),
                &ResourcePlan {
                    strategy: TextureStrategy::Tiled { tile_size: 256 },
                    level: 0,
                    required_tiles: vec![TileCoordinate {
                        level: 0,
                        x: 1,
                        y: 1,
                    }],
                    prefetch_tiles: vec![],
                },
            )
            .unwrap()
            .remove(0);
        assert_eq!(tile.sample_border, 1);
        // The requested 256×256 interior starts one pixel into the padded
        // texture. The duplicated ring is intentionally not compared as
        // part of the logical tile payload.
        let logical_width = (preview.width - 256).min(256) as usize;
        let logical_height = (preview.height - 256).min(256) as usize;
        for row in 0..logical_height {
            let source_start = (row + 256) * preview.bytes_per_row as usize + 256 * 4;
            let tile_start = (row + 1) * tile.bytes_per_row as usize + 4;
            assert!(
                preview.pixels[source_start..source_start + logical_width * 4]
                    == tile.pixels[tile_start..tile_start + logical_width * 4],
                "tile row {row} in {name} changed orientation or color"
            );
        }
    }
}

#[test]
fn eight_k_level_zero_tiles_fit_baseline_staging_budget() {
    if std::env::var_os("VIEWER_RUN_LARGE_IMAGE_TESTS").is_none() {
        return;
    }
    let fixture_dir = tempfile::tempdir().unwrap();
    let fixture = fixture_dir.path().join("8k.jpg");
    let output = std::process::Command::new("/usr/bin/sips")
        .args(["--resampleHeightWidth", "4320", "7680"])
        .arg(image_fixture("srgb.jpg"))
        .arg("--out")
        .arg(&fixture)
        .output()
        .unwrap();
    assert!(output.status.success());
    let source = AuthorizedImageSource::authorize_for_process(&fixture).unwrap();
    let (_cache, provider) = provider();
    let probe = provider.probe(&source).unwrap();
    assert_eq!((probe.width, probe.height), (7680, 4320));
    let tiles = (0..9)
        .flat_map(|y| (0..15).map(move |x| TileCoordinate { level: 0, x, y }))
        .collect();
    let result = provider
        .request_tiles(
            &source,
            AssetGeneration(1),
            &ResourcePlan {
                strategy: TextureStrategy::Tiled { tile_size: 512 },
                level: 0,
                required_tiles: tiles,
                prefetch_tiles: vec![],
            },
        )
        .unwrap();
    assert_eq!(result.len(), 135);
    let expected_bytes = (0..9)
        .flat_map(|y| (0..15).map(move |x| (x, y)))
        .map(|(x, y)| {
            let width = 512_u32.min(7680 - x * 512);
            let height = 512_u32.min(4320 - y * 512);
            let left = u32::from(x > 0);
            let top = u32::from(y > 0);
            let right = u32::from(x < 14);
            let bottom = u32::from(y < 8);
            usize::try_from((width + left + right) * (height + top + bottom) * 4).unwrap()
        })
        .sum::<usize>();
    assert_eq!(
        result
            .iter()
            .map(|resource| resource.pixels.len())
            .sum::<usize>(),
        expected_bytes
    );
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
    assert_eq!((resources[0].width, resources[0].height), (513, 513));
    assert_eq!((resources[1].width, resources[1].height), (513, 257));
    assert!(resources.iter().all(|resource| resource.sample_border == 1));
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
