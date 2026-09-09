use std::{collections::BTreeMap, error::Error, fmt};

use crate::gpu_memory::{GpuBuffer, GpuMemory, GpuTexture};
use crate::upload_pool::UploadPool;
use viewer_render_core::{AssetGeneration, SourceSize, TileCoordinate};

use crate::{ImagePassPlan, ImagePlanError, ImageVertex, VisibleImageResource, VisibleResources};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UploadDisposition {
    Accepted,
    Advanced { previous: AssetGeneration },
    DiscardedStale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceGenerationGate {
    current: AssetGeneration,
}

impl ResourceGenerationGate {
    pub const fn new(current: AssetGeneration) -> Self {
        Self { current }
    }

    pub const fn current(self) -> AssetGeneration {
        self.current
    }

    pub fn admit(&mut self, generation: AssetGeneration) -> UploadDisposition {
        if generation < self.current {
            UploadDisposition::DiscardedStale
        } else if generation == self.current {
            UploadDisposition::Accepted
        } else {
            let previous = self.current;
            self.current = generation;
            UploadDisposition::Advanced { previous }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UploadLayout {
    bytes: Vec<u8>,
    unpadded_bytes_per_row: u32,
    padded_bytes_per_row: u32,
    rows: u32,
}

impl UploadLayout {
    pub fn bgra8(width: u32, height: u32, pixels: &[u8]) -> Result<Self, UploadError> {
        if width == 0 || height == 0 {
            return Err(UploadError::ZeroDimensions);
        }
        let unpadded_bytes_per_row = width.checked_mul(4).ok_or(UploadError::SizeOverflow)?;
        let expected = usize::try_from(unpadded_bytes_per_row)
            .ok()
            .and_then(|row| row.checked_mul(height as usize))
            .ok_or(UploadError::SizeOverflow)?;
        if pixels.len() != expected {
            return Err(UploadError::PixelLength {
                expected,
                actual: pixels.len(),
            });
        }
        let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row
            .checked_add(alignment - 1)
            .ok_or(UploadError::SizeOverflow)?
            / alignment
            * alignment;
        let padded_len = usize::try_from(padded_bytes_per_row)
            .ok()
            .and_then(|row| row.checked_mul(height as usize))
            .ok_or(UploadError::SizeOverflow)?;
        let mut bytes = vec![0; padded_len];
        let source_row = unpadded_bytes_per_row as usize;
        let target_row = padded_bytes_per_row as usize;
        for row in 0..height as usize {
            let source_start = row * source_row;
            let target_start = row * target_row;
            bytes[target_start..target_start + source_row]
                .copy_from_slice(&pixels[source_start..source_start + source_row]);
        }
        Ok(Self {
            bytes,
            unpadded_bytes_per_row,
            padded_bytes_per_row,
            rows: height,
        })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn unpadded_bytes_per_row(&self) -> u32 {
        self.unpadded_bytes_per_row
    }

    pub const fn padded_bytes_per_row(&self) -> u32 {
        self.padded_bytes_per_row
    }

    pub const fn rows(&self) -> u32 {
        self.rows
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedResource {
    WholeImage {
        generation: AssetGeneration,
        level: u32,
        source_size: SourceSize,
        width: u32,
        height: u32,
        pixels: viewer_render_core::SharedPixels,
    },
    Tile {
        generation: AssetGeneration,
        tile: TileCoordinate,
        tile_size: u32,
        /// Duplicated edge pixels surrounding a tiled texture. The value is
        /// zero for legacy/test resources without padding.
        sample_border: u32,
        source_size: SourceSize,
        width: u32,
        height: u32,
        pixels: viewer_render_core::SharedPixels,
    },
}

impl DecodedResource {
    pub const fn generation(&self) -> AssetGeneration {
        match self {
            Self::WholeImage { generation, .. } | Self::Tile { generation, .. } => *generation,
        }
    }

    pub fn key(&self) -> ResourceKey {
        match self {
            Self::WholeImage { level, .. } => ResourceKey::WholeImage { level: *level },
            Self::Tile { tile, .. } => ResourceKey::Tile(*tile),
        }
    }

    const fn dimensions(&self) -> (u32, u32) {
        match self {
            Self::WholeImage { width, height, .. } | Self::Tile { width, height, .. } => {
                (*width, *height)
            }
        }
    }

    fn pixels(&self) -> &[u8] {
        match self {
            Self::WholeImage { pixels, .. } | Self::Tile { pixels, .. } => pixels,
        }
    }

    fn image_vertices(&self) -> Result<[ImageVertex; 6], UploadError> {
        let (source_size, tile_size, tiles, sample_border) = match self {
            Self::WholeImage {
                level, source_size, ..
            } => (
                *source_size,
                source_size.width.max(source_size.height),
                [TileCoordinate {
                    level: *level,
                    x: 0,
                    y: 0,
                }],
                0,
            ),
            Self::Tile {
                tile,
                tile_size,
                source_size,
                sample_border,
                ..
            } => (*source_size, *tile_size, [*tile], *sample_border),
        };
        let identity = crate::image_pass::identity_transform(source_size)?;
        let mut plan = ImagePassPlan::for_tiles(source_size, tile_size, &tiles, &identity)?;
        if sample_border > 0 {
            apply_tile_sample_border(
                &mut plan.draws[0].uv_corners,
                tiles[0],
                source_size,
                tile_size,
                sample_border,
                self.dimensions(),
            );
        }
        Ok(plan.draws[0].vertices())
    }
}

fn apply_tile_sample_border(
    uv: &mut [viewer_render_core::NormalizedPoint; 4],
    tile: TileCoordinate,
    source_size: SourceSize,
    tile_size: u32,
    border: u32,
    texture_dimensions: (u32, u32),
) {
    let level_width = mip_dimension(source_size.width, tile.level);
    let level_height = mip_dimension(source_size.height, tile.level);
    let logical_x = tile.x.saturating_mul(tile_size);
    let logical_y = tile.y.saturating_mul(tile_size);
    let logical_width = tile_size.min(level_width.saturating_sub(logical_x));
    let logical_height = tile_size.min(level_height.saturating_sub(logical_y));
    if logical_width == 0 || logical_height == 0 {
        return;
    }
    let left = border.min(logical_x);
    let top = border.min(logical_y);
    let right = border.min(level_width.saturating_sub(logical_x + logical_width));
    let bottom = border.min(level_height.saturating_sub(logical_y + logical_height));
    let (texture_width, texture_height) = texture_dimensions;
    if texture_width == 0 || texture_height == 0 {
        return;
    }
    let min_x = f64::from(left) / f64::from(texture_width);
    let min_y = f64::from(top) / f64::from(texture_height);
    let max_x = f64::from(texture_width.saturating_sub(right)) / f64::from(texture_width);
    let max_y = f64::from(texture_height.saturating_sub(bottom)) / f64::from(texture_height);
    *uv = [
        viewer_render_core::NormalizedPoint { x: min_x, y: min_y },
        viewer_render_core::NormalizedPoint { x: max_x, y: min_y },
        viewer_render_core::NormalizedPoint { x: max_x, y: max_y },
        viewer_render_core::NormalizedPoint { x: min_x, y: max_y },
    ];
}

fn mip_dimension(dimension: u32, level: u32) -> u32 {
    if level >= u32::BITS {
        1
    } else {
        dimension.saturating_add((1_u32 << level).saturating_sub(1)) >> level
    }
    .max(1)
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ResourceKey {
    WholeImage { level: u32 },
    Tile(TileCoordinate),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ResourceHandle(pub u64);

struct GpuResource {
    handle: ResourceHandle,
    _texture: GpuTexture,
    _view: wgpu::TextureView,
    texture_bind_group: wgpu::BindGroup,
    vertex_buffer: GpuBuffer,
    gpu_bytes: u64,
    last_used: u64,
}

pub struct ResourceRegistry {
    device: wgpu::Device,
    memory: GpuMemory,
    pool: UploadPool,
    pending: Option<PendingUpload>,
    texture_layout: wgpu::BindGroupLayout,
    generation: ResourceGenerationGate,
    resources: BTreeMap<ResourceKey, GpuResource>,
    next_handle: u64,
    gpu_bytes: u64,
    clock: u64,
}

struct PendingUpload {
    source: DecodedResource,
    gpu: GpuResource,
    next_row: u32,
}

impl ResourceRegistry {
    pub(crate) fn generation(&self) -> AssetGeneration {
        self.generation.current()
    }

    pub(crate) fn new(
        device: &wgpu::Device,
        memory: &GpuMemory,
        texture_layout: &wgpu::BindGroupLayout,
        generation: AssetGeneration,
    ) -> Result<Self, UploadError> {
        Ok(Self {
            device: device.clone(),
            memory: memory.clone(),
            pool: UploadPool::new(memory)?,
            pending: None,
            texture_layout: texture_layout.clone(),
            generation: ResourceGenerationGate::new(generation),
            resources: BTreeMap::new(),
            next_handle: 0,
            gpu_bytes: 0,
            clock: 0,
        })
    }

    pub fn cancel_pending_upload(&mut self) {
        self.pending = None;
    }
    pub fn has_pending_upload(&self) -> bool {
        self.pending.is_some()
    }
    pub fn is_ready(&self, handle: ResourceHandle) -> bool {
        self.resources
            .values()
            .any(|resource| resource.handle == handle)
    }
    pub fn contains_key(&self, key: ResourceKey) -> bool {
        self.resources.contains_key(&key)
    }

    pub fn upsert(&mut self, mut resource: DecodedResource) -> Result<ResourceHandle, UploadError> {
        if resource.generation() < self.generation.current() {
            return Err(UploadError::StaleGeneration);
        }
        if self.pending.is_some() && resource.generation() == self.generation.current() {
            return Err(UploadError::PendingCapacity);
        }
        match self.generation.admit(resource.generation()) {
            UploadDisposition::DiscardedStale => return Err(UploadError::StaleGeneration),
            UploadDisposition::Advanced { .. } => {
                self.pending = None;
                self.resources.clear();
                self.gpu_bytes = 0;
            }
            UploadDisposition::Accepted => {}
        }
        let (width, height) = resource.dimensions();
        if width == 0 || height == 0 {
            return Err(UploadError::ZeroDimensions);
        }
        if width > self.device.limits().max_texture_dimension_2d
            || height > self.device.limits().max_texture_dimension_2d
        {
            return Err(UploadError::DeviceDimensions);
        }
        UploadPool::padded_row(width, 4)?;
        let gpu_bytes = u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|n| n.checked_mul(4))
            .ok_or(UploadError::SizeOverflow)?;
        let expected = usize::try_from(gpu_bytes).map_err(|_| UploadError::SizeOverflow)?;
        if resource.pixels().len() != expected {
            return Err(UploadError::PixelLength {
                expected,
                actual: resource.pixels().len(),
            });
        }
        let generation = resource.generation();
        let pixels = match &mut resource {
            DecodedResource::WholeImage { pixels, .. } | DecodedResource::Tile { pixels, .. } => {
                pixels
            }
        };
        if !pixels.is_accounted_by(self.memory.coordinator()) {
            *pixels = viewer_render_core::SharedPixels::try_copy_from_slice(
                self.memory.coordinator(),
                generation,
                pixels,
            )?;
        }
        let vertices = resource.image_vertices()?;
        let texture = self.memory.texture(
            &wgpu::TextureDescriptor {
                label: Some("Viewer image resource"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            gpu_bytes,
            resource.generation(),
        )?;
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let texture_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Viewer image texture bind group"),
            layout: &self.texture_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            }],
        });
        let vertex_buffer = self.memory.buffer_init(
            "Viewer image tile vertices",
            bytemuck::cast_slice(&vertices),
            wgpu::BufferUsages::VERTEX,
        )?;
        let handle = ResourceHandle(self.next_handle);
        self.next_handle = self
            .next_handle
            .checked_add(1)
            .ok_or(UploadError::SizeOverflow)?;
        self.pending = Some(PendingUpload {
            source: resource,
            gpu: GpuResource {
                handle,
                _texture: texture,
                _view: view,
                texture_bind_group,
                vertex_buffer,
                gpu_bytes,
                last_used: self.clock,
            },
            next_row: 0,
        });
        self.progress_upload()?;
        Ok(handle)
    }

    pub fn progress_upload(&mut self) -> Result<bool, UploadError> {
        let Some(pending) = self.pending.as_mut() else {
            return Ok(false);
        };
        let (width, height) = pending.source.dimensions();
        while pending.next_row < height {
            let rows = self.pool.copy_rows(
                &self.device,
                &pending.gpu._texture,
                pending.source.pixels(),
                (width, height),
                4,
                pending.next_row,
            )?;
            if rows == 0 {
                return Ok(false);
            }
            pending.next_row += rows;
        }
        // Every band precedes the next draw on this queue. Never expose a
        // partially copied texture, and never replace the old same-key texture early.
        let pending = self.pending.take().unwrap();
        if let Some(previous) = self.resources.insert(pending.source.key(), pending.gpu) {
            self.gpu_bytes -= previous.gpu_bytes;
        }
        self.gpu_bytes += u64::from(width) * u64::from(height) * 4;
        Ok(true)
    }

    pub fn begin_generation(&mut self, generation: AssetGeneration) -> Result<(), UploadError> {
        if self.generation.admit(generation) == UploadDisposition::DiscardedStale {
            return Err(UploadError::StaleGeneration);
        }
        self.pending = None;
        self.resources.clear();
        self.gpu_bytes = 0;
        Ok(())
    }

    pub const fn gpu_bytes(&self) -> u64 {
        self.gpu_bytes
    }

    pub fn retain(&mut self, keys: &[ResourceKey]) {
        self.resources.retain(|key, _| keys.contains(key));
        self.gpu_bytes = self
            .resources
            .values()
            .map(|resource| resource.gpu_bytes)
            .sum();
    }

    pub fn touch(&mut self, keys: &[ResourceKey]) {
        self.clock = self.clock.saturating_add(1);
        for key in keys {
            if let Some(resource) = self.resources.get_mut(key) {
                resource.last_used = self.clock;
            }
        }
    }

    /// Live GPU policy only: neither this metadata nor retirement touches disk.
    pub fn reclaim(
        &mut self,
        visible: &[ResourceKey],
        preview: Option<ResourceKey>,
        prefetch: &[ResourceKey],
        budget: u64,
    ) {
        let mut victims = self
            .resources
            .iter()
            .map(|(key, resource)| {
                let rank = if Some(*key) == preview {
                    3
                } else if visible.contains(key) {
                    2
                } else if prefetch.contains(key) {
                    0
                } else {
                    1
                };
                (rank, resource.last_used, *key)
            })
            .collect::<Vec<_>>();
        // Stable key order breaks ties; all current main/lens requests have
        // equal priority. Coverage is last to retire, never relabelled sharp.
        victims.sort_unstable();
        for (rank, _, key) in victims {
            if rank != 0 && self.gpu_bytes <= budget {
                break;
            }
            if let Some(resource) = self.resources.remove(&key) {
                self.gpu_bytes -= resource.gpu_bytes;
            }
        }
    }

    pub fn visible_all(&self) -> VisibleResources<'_> {
        let mut ordered = self.resources.iter().collect::<Vec<_>>();
        // Paint whole-image coverage first, then coarse-to-fine tiles. BTree
        // key order puts mip zero first, which would hide detail under previews.
        ordered.sort_by_key(|(key, _)| match key {
            ResourceKey::WholeImage { level } => (0, std::cmp::Reverse(*level)),
            ResourceKey::Tile(tile) => (1, std::cmp::Reverse(tile.level)),
        });
        VisibleResources {
            items: ordered
                .into_iter()
                .map(|(_, resource)| VisibleImageResource {
                    handle: resource.handle,
                    texture_bind_group: &resource.texture_bind_group,
                    vertex_buffer: &resource.vertex_buffer,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UploadError {
    ZeroDimensions,
    Memory(viewer_render_core::MemoryAdmissionError),
    PendingCapacity,
    RowExceedsSlot,
    DeviceDimensions,
    StagingMapFailed,
    SizeOverflow,
    PixelLength { expected: usize, actual: usize },
    StaleGeneration,
    InvalidImagePlan(ImagePlanError),
}

impl From<viewer_render_core::MemoryAdmissionError> for UploadError {
    fn from(error: viewer_render_core::MemoryAdmissionError) -> Self {
        Self::Memory(error)
    }
}

impl From<ImagePlanError> for UploadError {
    fn from(error: ImagePlanError) -> Self {
        Self::InvalidImagePlan(error)
    }
}

impl fmt::Display for UploadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Memory(error) => error.fmt(formatter),
            Self::PendingCapacity => formatter.write_str("one resource upload is already pending"),
            Self::DeviceDimensions => {
                formatter.write_str("resource dimensions exceed GPU texture limits")
            }
            Self::RowExceedsSlot => formatter.write_str("padded upload row exceeds 1 MiB slot"),
            Self::StagingMapFailed => formatter.write_str("image upload staging remap failed"),
            Self::ZeroDimensions => formatter.write_str("decoded resource dimensions are empty"),
            Self::SizeOverflow => formatter.write_str("decoded resource byte size overflowed"),
            Self::PixelLength { expected, actual } => write!(
                formatter,
                "decoded BGRA8 pixels have length {actual}, expected {expected}"
            ),
            Self::StaleGeneration => formatter.write_str("decoded resource generation is stale"),
            Self::InvalidImagePlan(error) => error.fmt(formatter),
        }
    }
}

impl Error for UploadError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixels(width: u32, height: u32) -> viewer_render_core::SharedPixels {
        let memory = viewer_render_core::ImageMemoryCoordinator::new(
            viewer_render_core::ImageMemoryPolicy::baseline_8gb(),
        );
        viewer_render_core::SharedPixels::try_zeroed(
            &memory,
            AssetGeneration(1),
            u64::from(width) * u64::from(height) * 4,
        )
        .unwrap()
    }

    #[test]
    fn padded_tiles_shrink_uvs_to_the_logical_interior() {
        let source_size = SourceSize::new(1_536, 1_536).unwrap();
        let resource = DecodedResource::Tile {
            generation: AssetGeneration(1),
            tile: TileCoordinate {
                level: 0,
                x: 1,
                y: 1,
            },
            tile_size: 512,
            sample_border: 1,
            source_size,
            width: 514,
            height: 514,
            pixels: pixels(514, 514),
        };

        let vertices = resource.image_vertices().unwrap();
        assert_eq!(vertices[0].uv, [1.0 / 514.0, 1.0 / 514.0]);
        assert_eq!(vertices[1].uv, [513.0 / 514.0, 1.0 / 514.0]);
        assert_eq!(vertices[2].uv, [513.0 / 514.0, 513.0 / 514.0]);
    }

    #[test]
    fn image_edge_tiles_only_inset_sides_with_a_neighbour() {
        let source_size = SourceSize::new(1_024, 1_024).unwrap();
        let resource = DecodedResource::Tile {
            generation: AssetGeneration(1),
            tile: TileCoordinate {
                level: 0,
                x: 0,
                y: 0,
            },
            tile_size: 512,
            sample_border: 1,
            source_size,
            width: 513,
            height: 513,
            pixels: pixels(513, 513),
        };

        let vertices = resource.image_vertices().unwrap();
        assert_eq!(vertices[0].uv, [0.0, 0.0]);
        assert_eq!(vertices[1].uv, [512.0 / 513.0, 0.0]);
        assert_eq!(vertices[2].uv, [512.0 / 513.0, 512.0 / 513.0]);
    }
}
