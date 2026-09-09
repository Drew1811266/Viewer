use std::collections::BTreeSet;

use viewer_render_core::{
    AssetGeneration, DeviceLimits, MemoryBudget, NormalizedRect, PhysicalSize, ResourcePlan,
    ResourcePlanner, ResourceRequest, TextureStrategy, TileRequestQueue,
};

fn request(source_width: u32, source_height: u32, display_scale: f64) -> ResourceRequest {
    ResourceRequest {
        source_size: viewer_render_core::SourceSize::new(source_width, source_height).unwrap(),
        visible_normalized_rect: NormalizedRect::new(0.0, 0.0, 1.0, 1.0).unwrap(),
        display_scale,
        viewport_physical_size: PhysicalSize {
            width: 1_920,
            height: 1_080,
        },
    }
}

fn planner() -> ResourcePlanner {
    ResourcePlanner::new(
        DeviceLimits::new(16_384).unwrap(),
        MemoryBudget::baseline_8gb(),
    )
}

#[test]
fn one_k_and_four_k_fit_single_textures_but_eight_k_uses_tiles() {
    for size in [(1_024, 1_024), (3_840, 2_160)] {
        assert_eq!(
            planner()
                .plan(request(size.0, size.1, 1.0))
                .unwrap()
                .strategy,
            TextureStrategy::SingleTexture
        );
    }

    assert_eq!(
        planner().plan(request(7_680, 4_320, 1.0)).unwrap().strategy,
        TextureStrategy::Tiled { tile_size: 512 }
    );
}

#[test]
fn full_mip_chain_must_fit_the_current_budget_quarter_not_only_its_base_level() {
    // 3700² BGRA base fits 64MiB; its complete mip chain does not.
    for (width, height) in [(3_700, 3_700), (4_096, 4_096)] {
        assert_eq!(
            planner()
                .plan(request(width, height, 1.0))
                .unwrap()
                .strategy,
            TextureStrategy::Tiled { tile_size: 512 }
        );
    }
    let reduced = ResourcePlanner::new(
        DeviceLimits::new(16_384).unwrap(),
        MemoryBudget::new(256 << 20, 128 << 20, 2 << 30).unwrap(),
    );
    assert_eq!(
        reduced.plan(request(3_840, 2_160, 1.0)).unwrap().strategy,
        TextureStrategy::Tiled { tile_size: 512 }
    );
}

#[test]
fn non_power_of_two_and_skinny_mip_chains_respect_exact_admission_boundaries() {
    // Literal GPU allocation sizes: 7×5 + 3×2 + 1×1 = 42 BGRA texels;
    // 1×5 + 1×2 + 1×1 = 8; 16² + 8² + 4² + 2² + 1 = 341.
    for (width, height, gpu_at_boundary) in [(7, 5, 672), (1, 5, 128), (16, 16, 5_456), (1, 1, 16)]
    {
        for (gpu_bytes, expected) in [
            (gpu_at_boundary, TextureStrategy::SingleTexture),
            (
                gpu_at_boundary - 1,
                TextureStrategy::Tiled { tile_size: 512 },
            ),
        ] {
            let planner = ResourcePlanner::new(
                DeviceLimits::new(16_384).unwrap(),
                MemoryBudget::new(256 << 20, gpu_bytes, 2 << 30).unwrap(),
            );
            assert_eq!(
                planner.plan(request(width, height, 1.0)).unwrap().strategy,
                expected,
                "{width}×{height}, GPU budget {gpu_bytes}"
            );
        }
    }
}

#[test]
fn overflowing_full_image_estimate_uses_bounded_visible_tiles() {
    let mut request = request(u32::MAX, u32::MAX, 1.0);
    request.visible_normalized_rect =
        NormalizedRect::new(0.0, 0.0, 0.000_000_001, 0.000_000_001).unwrap();
    let planner = ResourcePlanner::new(
        DeviceLimits::new(u32::MAX).unwrap(),
        MemoryBudget::new(u64::MAX, u64::MAX, u64::MAX).unwrap(),
    );
    let plan = planner.plan(request).unwrap();
    assert_eq!(plan.strategy, TextureStrategy::Tiled { tile_size: 512 });
    assert_eq!(plan.required_tiles.len(), 1);
    assert_eq!(plan.prefetch_tiles.len(), 3);
}

#[test]
fn device_dimension_limit_forces_tiling_even_when_bytes_fit() {
    let planner = ResourcePlanner::new(
        DeviceLimits::new(4_096).unwrap(),
        MemoryBudget::baseline_8gb(),
    );

    assert_eq!(
        planner.plan(request(4_200, 1_000, 1.0)).unwrap().strategy,
        TextureStrategy::Tiled { tile_size: 512 }
    );
}

#[test]
fn mip_level_tracks_screen_sampling_density_during_fast_zoom() {
    let levels = [0.125, 0.25, 0.5, 1.0, 2.0]
        .map(|scale| planner().plan(request(8_192, 8_192, scale)).unwrap().level);

    assert_eq!(levels, [3, 2, 1, 0, 0]);
}

#[test]
fn rotating_the_viewport_does_not_change_source_space_resource_selection() {
    let mut landscape = request(7_680, 4_320, 0.5);
    landscape.visible_normalized_rect = NormalizedRect::new(0.25, 0.25, 0.5, 0.5).unwrap();
    landscape.viewport_physical_size = PhysicalSize {
        width: 1_600,
        height: 900,
    };
    let mut portrait = landscape;
    portrait.viewport_physical_size = PhysicalSize {
        width: 900,
        height: 1_600,
    };

    let landscape = planner().plan(landscape).unwrap();
    let portrait = planner().plan(portrait).unwrap();

    assert_eq!(landscape.level, portrait.level);
    assert_eq!(landscape.required_tiles, portrait.required_tiles);
    assert_eq!(landscape.prefetch_tiles, portrait.prefetch_tiles);
}

#[test]
fn tiled_plan_requests_visible_tiles_before_one_deduplicated_neighbor_ring() {
    let mut request = request(16_384, 16_384, 1.0);
    request.visible_normalized_rect = NormalizedRect::new(0.45, 0.45, 0.1, 0.1).unwrap();
    let ResourcePlan {
        required_tiles,
        prefetch_tiles,
        ..
    } = planner().plan(request).unwrap();

    assert!(!required_tiles.is_empty());
    assert!(!prefetch_tiles.is_empty());
    assert!(required_tiles.iter().all(|tile| tile.level == 0));
    assert!(prefetch_tiles.iter().all(|tile| tile.level == 0));
    assert!(
        required_tiles
            .iter()
            .all(|tile| !prefetch_tiles.contains(tile))
    );
    assert_eq!(
        prefetch_tiles.iter().collect::<BTreeSet<_>>().len(),
        prefetch_tiles.len()
    );
}

#[test]
fn tile_queue_deduplicates_requests_and_cancels_stale_generations() {
    let plan = planner().plan(request(8_192, 8_192, 1.0)).unwrap();
    let tile = plan.required_tiles[0];
    let mut queue = TileRequestQueue::default();

    assert!(queue.enqueue(AssetGeneration(1), tile));
    assert!(!queue.enqueue(AssetGeneration(1), tile));
    assert!(queue.enqueue(AssetGeneration(2), tile));
    assert_eq!(queue.len(), 2);

    let cancelled = queue.cancel_before(AssetGeneration(2));
    assert_eq!(cancelled, 1);
    assert_eq!(queue.len(), 1);
    assert_eq!(queue.pop_front(), Some((AssetGeneration(2), tile)));
}
