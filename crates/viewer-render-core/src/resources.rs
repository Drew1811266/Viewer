use std::{collections::BTreeSet, collections::VecDeque, error::Error, fmt};

use crate::{AssetGeneration, MemoryBudget, NormalizedRect, PhysicalSize, SourceSize};

pub const DEFAULT_TILE_SIZE: u32 = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceLimits {
    pub max_texture_dimension_2d: u32,
}

impl DeviceLimits {
    pub fn new(max_texture_dimension_2d: u32) -> Result<Self, ResourceError> {
        if max_texture_dimension_2d == 0 {
            return Err(ResourceError::InvalidDeviceLimit);
        }
        Ok(Self {
            max_texture_dimension_2d,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResourceRequest {
    pub source_size: SourceSize,
    pub visible_normalized_rect: NormalizedRect,
    /// Physical display pixels per source pixel at the current camera scale.
    pub display_scale: f64,
    pub viewport_physical_size: PhysicalSize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextureStrategy {
    SingleTexture,
    Tiled { tile_size: u32 },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TileCoordinate {
    pub level: u32,
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourcePlan {
    pub strategy: TextureStrategy,
    pub level: u32,
    pub required_tiles: Vec<TileCoordinate>,
    pub prefetch_tiles: Vec<TileCoordinate>,
}

#[derive(Clone, Copy, Debug)]
pub struct ResourcePlanner {
    limits: DeviceLimits,
    budget: MemoryBudget,
}

impl ResourcePlanner {
    pub const fn new(limits: DeviceLimits, budget: MemoryBudget) -> Self {
        Self { limits, budget }
    }

    pub fn plan(self, request: ResourceRequest) -> Result<ResourcePlan, ResourceError> {
        if !request.display_scale.is_finite() || request.display_scale <= 0.0 {
            return Err(ResourceError::InvalidDisplayScale);
        }
        if request.viewport_physical_size.width == 0 || request.viewport_physical_size.height == 0 {
            return Err(ResourceError::EmptyViewport);
        }

        let estimated_bytes = u64::from(request.source_size.width)
            .saturating_mul(u64::from(request.source_size.height))
            .saturating_mul(4);
        let fits_dimension = request.source_size.width <= self.limits.max_texture_dimension_2d
            && request.source_size.height <= self.limits.max_texture_dimension_2d;
        let strategy =
            if fits_dimension && estimated_bytes <= self.budget.single_texture_limit_bytes() {
                TextureStrategy::SingleTexture
            } else {
                TextureStrategy::Tiled {
                    tile_size: DEFAULT_TILE_SIZE,
                }
            };

        let level = choose_mip_level(request.source_size, request.display_scale);
        let tile_size = match strategy {
            TextureStrategy::SingleTexture => mip_dimension(
                request.source_size.width.max(request.source_size.height),
                level,
            ),
            TextureStrategy::Tiled { tile_size } => tile_size,
        };
        let (required_tiles, prefetch_tiles) = plan_tiles(
            request.source_size,
            request.visible_normalized_rect,
            level,
            tile_size,
        );

        Ok(ResourcePlan {
            strategy,
            level,
            required_tiles,
            prefetch_tiles,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceError {
    InvalidDeviceLimit,
    InvalidDisplayScale,
    EmptyViewport,
}

impl fmt::Display for ResourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDeviceLimit => {
                formatter.write_str("device texture limit must be positive")
            }
            Self::InvalidDisplayScale => {
                formatter.write_str("display scale must be finite and positive")
            }
            Self::EmptyViewport => formatter.write_str("physical viewport must be non-empty"),
        }
    }
}

impl Error for ResourceError {}

#[derive(Clone, Debug, Default)]
pub struct TileRequestQueue {
    queue: VecDeque<(AssetGeneration, TileCoordinate)>,
    pending: BTreeSet<(AssetGeneration, TileCoordinate)>,
}

impl TileRequestQueue {
    pub fn enqueue(&mut self, generation: AssetGeneration, tile: TileCoordinate) -> bool {
        if !self.pending.insert((generation, tile)) {
            return false;
        }
        self.queue.push_back((generation, tile));
        true
    }

    pub fn enqueue_plan(&mut self, generation: AssetGeneration, plan: &ResourcePlan) -> usize {
        plan.required_tiles
            .iter()
            .chain(&plan.prefetch_tiles)
            .filter(|tile| self.enqueue(generation, **tile))
            .count()
    }

    pub fn cancel_before(&mut self, generation: AssetGeneration) -> usize {
        let original_len = self.queue.len();
        self.queue.retain(|(queued, _)| *queued >= generation);
        self.pending.retain(|(queued, _)| *queued >= generation);
        original_len - self.queue.len()
    }

    pub fn pop_front(&mut self) -> Option<(AssetGeneration, TileCoordinate)> {
        let request = self.queue.pop_front()?;
        self.pending.remove(&request);
        Some(request)
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

fn choose_mip_level(source_size: SourceSize, display_scale: f64) -> u32 {
    if display_scale >= 1.0 {
        return 0;
    }
    let requested = (1.0 / display_scale).log2().floor().max(0.0) as u32;
    requested.min(max_mip_level(source_size))
}

fn max_mip_level(source_size: SourceSize) -> u32 {
    u32::BITS - source_size.width.max(source_size.height).leading_zeros() - 1
}

fn mip_dimension(dimension: u32, level: u32) -> u32 {
    let divisor = 1_u64 << level.min(31);
    u32::try_from(u64::from(dimension).div_ceil(divisor)).unwrap_or(u32::MAX)
}

fn plan_tiles(
    source_size: SourceSize,
    visible: NormalizedRect,
    level: u32,
    tile_size: u32,
) -> (Vec<TileCoordinate>, Vec<TileCoordinate>) {
    let width = mip_dimension(source_size.width, level);
    let height = mip_dimension(source_size.height, level);
    let columns = width.div_ceil(tile_size);
    let rows = height.div_ceil(tile_size);

    let start_x = ((visible.x * f64::from(width)).floor() as u32 / tile_size).min(columns - 1);
    let start_y = ((visible.y * f64::from(height)).floor() as u32 / tile_size).min(rows - 1);
    let end_pixel_x =
        (((visible.x + visible.width) * f64::from(width)).ceil() as u32).clamp(1, width);
    let end_pixel_y =
        (((visible.y + visible.height) * f64::from(height)).ceil() as u32).clamp(1, height);
    let end_x = ((end_pixel_x - 1) / tile_size).min(columns - 1);
    let end_y = ((end_pixel_y - 1) / tile_size).min(rows - 1);

    let mut required = BTreeSet::new();
    for y in start_y..=end_y {
        for x in start_x..=end_x {
            required.insert(TileCoordinate { level, x, y });
        }
    }

    let mut prefetch = BTreeSet::new();
    let ring_start_x = start_x.saturating_sub(1);
    let ring_start_y = start_y.saturating_sub(1);
    let ring_end_x = end_x.saturating_add(1).min(columns - 1);
    let ring_end_y = end_y.saturating_add(1).min(rows - 1);
    for y in ring_start_y..=ring_end_y {
        for x in ring_start_x..=ring_end_x {
            let tile = TileCoordinate { level, x, y };
            if !required.contains(&tile) {
                prefetch.insert(tile);
            }
        }
    }

    (
        required.into_iter().collect(),
        prefetch.into_iter().collect(),
    )
}
