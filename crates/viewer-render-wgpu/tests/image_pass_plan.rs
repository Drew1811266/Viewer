use viewer_render_core::{
    CameraState, DeviceLimits, LogicalSize, MemoryBudget, NormalizedPoint, NormalizedRect,
    PhysicalSize, ResourcePlanner, ResourceRequest, Rotation, SourceSize, TextureStrategy,
    TileCoordinate, TransformSnapshot, ViewportLayout,
};
use viewer_render_wgpu::ImagePassPlan;

fn transform(source: SourceSize, rotation: Rotation) -> TransformSnapshot {
    TransformSnapshot::new(
        source,
        ViewportLayout::new(LogicalSize::new(1_000.0, 800.0).unwrap(), 2.0, 1.0).unwrap(),
        CameraState::fit(rotation),
    )
    .unwrap()
}

#[test]
fn tile_source_rect_and_uv_are_exact_for_regular_and_edge_tiles() {
    let source = SourceSize::new(1_000, 700).unwrap();
    let regular = TileCoordinate {
        level: 0,
        x: 0,
        y: 0,
    };
    let edge = TileCoordinate {
        level: 0,
        x: 1,
        y: 1,
    };
    let plan = ImagePassPlan::for_tiles(
        source,
        512,
        &[regular, edge],
        &transform(source, Rotation::Deg0),
    )
    .unwrap();

    assert_eq!(
        plan.draws[0].source_rect,
        NormalizedRect::new(0.0, 0.0, 0.512, 512.0 / 700.0).unwrap()
    );
    assert_eq!(
        plan.draws[1].source_rect,
        NormalizedRect::new(0.512, 512.0 / 700.0, 0.488, 188.0 / 700.0).unwrap()
    );
    assert_eq!(
        plan.draws[1].uv_corners,
        [
            NormalizedPoint::new(0.0, 0.0).unwrap(),
            NormalizedPoint::new(1.0, 0.0).unwrap(),
            NormalizedPoint::new(1.0, 1.0).unwrap(),
            NormalizedPoint::new(0.0, 1.0).unwrap(),
        ]
    );
}

#[test]
fn rotated_tile_vertices_use_the_canonical_transform() {
    let source = SourceSize::new(1_200, 800).unwrap();
    let transform = transform(source, Rotation::Deg90);
    let tile = TileCoordinate {
        level: 0,
        x: 1,
        y: 0,
    };

    let plan = ImagePassPlan::for_tiles(source, 512, &[tile], &transform).unwrap();
    let rect = plan.draws[0].source_rect;

    assert_eq!(
        plan.draws[0].view_corners,
        [
            transform.image_to_view(NormalizedPoint::new(rect.x, rect.y).unwrap()),
            transform.image_to_view(NormalizedPoint::new(rect.x + rect.width, rect.y).unwrap()),
            transform.image_to_view(
                NormalizedPoint::new(rect.x + rect.width, rect.y + rect.height).unwrap()
            ),
            transform.image_to_view(NormalizedPoint::new(rect.x, rect.y + rect.height).unwrap()),
        ]
    );
}

#[test]
fn visible_tile_plan_from_core_contains_no_duplicate_draws() {
    let source = SourceSize::new(16_384, 16_384).unwrap();
    let request = ResourceRequest {
        source_size: source,
        visible_normalized_rect: NormalizedRect::new(0.45, 0.45, 0.1, 0.1).unwrap(),
        display_scale: 1.0,
        viewport_physical_size: PhysicalSize {
            width: 2_000,
            height: 1_600,
        },
    };
    let resources = ResourcePlanner::new(
        DeviceLimits::new(16_384).unwrap(),
        MemoryBudget::baseline_8gb(),
    )
    .plan(request)
    .unwrap();
    assert_eq!(
        resources.strategy,
        TextureStrategy::Tiled { tile_size: 512 }
    );

    let plan =
        ImagePassPlan::from_resource_plan(source, &resources, &transform(source, Rotation::Deg270))
            .unwrap();

    assert_eq!(plan.draws.len(), resources.required_tiles.len());
    for (draw, tile) in plan.draws.iter().zip(&resources.required_tiles) {
        assert_eq!(&draw.tile, tile);
    }
}
