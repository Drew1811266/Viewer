use viewer_platform_macos::image_render::{
    AuthorizedImageSource, MacImageResourceProvider, PreviewRequest,
};
use viewer_render_core::{
    AssetGeneration, MemoryBudget, ResourcePlan, TextureStrategy, TileCoordinate,
};
use viewer_test_support::image_fixtures::image_fixture;

#[test]
fn odd_dimension_mips_keep_the_planned_edge_tiles_on_cold_and_warm_reads() {
    for (width, height, expected_width, expected_height) in
        [(1026, 685, 513, 343), (685, 1026, 343, 513)]
    {
        let directory = tempfile::tempdir().unwrap();
        let fixture = directory.path().join("odd.jpg");
        let result = std::process::Command::new("/usr/bin/sips")
            .args([
                "--resampleHeightWidth",
                &height.to_string(),
                &width.to_string(),
            ])
            .arg(image_fixture("srgb.jpg"))
            .arg("--out")
            .arg(&fixture)
            .output()
            .unwrap();
        assert!(result.status.success());
        let source = AuthorizedImageSource::authorize_for_process(&fixture).unwrap();
        let cache = directory.path().join("cache");
        for generation in [1, 2] {
            let provider =
                MacImageResourceProvider::new(&cache, MemoryBudget::baseline_8gb()).unwrap();
            let result = provider.request_tiles(
                &source,
                AssetGeneration(generation),
                &ResourcePlan {
                    strategy: TextureStrategy::Tiled { tile_size: 512 },
                    level: 1,
                    required_tiles: vec![
                        TileCoordinate {
                            level: 1,
                            x: 0,
                            y: 0,
                        },
                        TileCoordinate {
                            level: 1,
                            x: u32::from(width > height),
                            y: u32::from(height > width),
                        },
                    ],
                    prefetch_tiles: vec![],
                },
            );
            assert!(
                result.is_ok(),
                "{width}x{height} generation {generation}: {result:?}"
            );
            let tiles = result.unwrap();
            assert_eq!(
                (tiles[0].width, tiles[0].height),
                (expected_width, expected_height)
            );
            let edge = &tiles[1];
            assert_eq!(
                (edge.width, edge.height),
                if width > height { (2, 343) } else { (343, 2) }
            );
            assert_eq!(edge.pixels.len(), (edge.width * edge.height * 4) as usize);
            assert!(
                edge.pixels.chunks_exact(4).all(|pixel| pixel[3] == 255),
                "no transparent missing edge"
            );
            // Both tiles independently sample the same two overlap columns or
            // rows. Resampling against per-tile bounds would create a seam.
            let first = &tiles[0];
            if width > height {
                for row in 0..343_usize {
                    let end = (row + 1) * first.bytes_per_row as usize;
                    assert_eq!(
                        &first.pixels[end - 8..end],
                        &edge.pixels[row * 8..row * 8 + 8]
                    );
                }
            } else {
                let length = edge.pixels.len();
                assert_eq!(
                    &first.pixels[first.pixels.len() - length..],
                    &edge.pixels[..]
                );
            }
        }
    }
}

#[test]
fn preview_normalizes_system_thumbnail_rounding_to_the_requested_bounds() {
    let directory = tempfile::tempdir().unwrap();
    let fixture = directory.path().join("odd.jpg");
    let result = std::process::Command::new("/usr/bin/sips")
        .args(["--resampleHeightWidth", "685", "1026"])
        .arg(image_fixture("srgb.jpg"))
        .arg("--out")
        .arg(&fixture)
        .output()
        .unwrap();
    assert!(result.status.success());
    let source = AuthorizedImageSource::authorize_for_process(&fixture).unwrap();
    let provider = MacImageResourceProvider::new(
        &directory.path().join("cache"),
        MemoryBudget::baseline_8gb(),
    )
    .unwrap();
    for (max_width, max_height, width, height) in [
        (400, 400, 400, 267),
        (513, 343, 513, 342),
        (800, 300, 449, 300),
    ] {
        let preview = provider.request_preview(
            &source,
            AssetGeneration(1),
            PreviewRequest::new(max_width, max_height).unwrap(),
        );
        assert!(
            preview.is_ok(),
            "request {max_width}x{max_height}: {preview:?}"
        );
        let preview = preview.unwrap();
        assert_eq!((preview.width, preview.height), (width, height));
    }
}
