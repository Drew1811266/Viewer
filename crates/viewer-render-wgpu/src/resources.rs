use std::{collections::BTreeMap, error::Error, fmt};

use viewer_render_core::{AssetGeneration, SourceSize, TileCoordinate};
use wgpu::util::DeviceExt;

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
        pixels: Vec<u8>,
    },
    Tile {
        generation: AssetGeneration,
        tile: TileCoordinate,
        tile_size: u32,
        source_size: SourceSize,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    },
}

impl DecodedResource {
    const fn generation(&self) -> AssetGeneration {
        match self {
            Self::WholeImage { generation, .. } | Self::Tile { generation, .. } => *generation,
        }
    }

    fn key(&self) -> ResourceKey {
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
        let (source_size, tile_size, tiles) = match self {
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
            ),
            Self::Tile {
                tile,
                tile_size,
                source_size,
                ..
            } => (*source_size, *tile_size, [*tile]),
        };
        let identity = crate::image_pass::identity_transform(source_size)?;
        let plan = ImagePassPlan::for_tiles(source_size, tile_size, &tiles, &identity)?;
        Ok(plan.draws[0].vertices())
    }
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
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    texture_bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    gpu_bytes: u64,
}

pub struct ResourceRegistry {
    device: wgpu::Device,
    queue: wgpu::Queue,
    texture_layout: wgpu::BindGroupLayout,
    generation: ResourceGenerationGate,
    resources: BTreeMap<ResourceKey, GpuResource>,
    next_handle: u64,
    gpu_bytes: u64,
}

impl ResourceRegistry {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_layout: &wgpu::BindGroupLayout,
        generation: AssetGeneration,
    ) -> Self {
        Self {
            device: device.clone(),
            queue: queue.clone(),
            texture_layout: texture_layout.clone(),
            generation: ResourceGenerationGate::new(generation),
            resources: BTreeMap::new(),
            next_handle: 0,
            gpu_bytes: 0,
        }
    }

    pub fn upsert(&mut self, resource: DecodedResource) -> Result<ResourceHandle, UploadError> {
        match self.generation.admit(resource.generation()) {
            UploadDisposition::DiscardedStale => return Err(UploadError::StaleGeneration),
            UploadDisposition::Advanced { .. } => {
                self.resources.clear();
                self.gpu_bytes = 0;
            }
            UploadDisposition::Accepted => {}
        }

        let key = resource.key();
        let (width, height) = resource.dimensions();
        let upload = UploadLayout::bgra8(width, height, resource.pixels())?;
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
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
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            upload.bytes(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(upload.padded_bytes_per_row()),
                rows_per_image: Some(upload.rows()),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let texture_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Viewer image texture bind group"),
            layout: &self.texture_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            }],
        });
        let vertices = resource.image_vertices()?;
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Viewer image tile vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let gpu_bytes = u64::from(width) * u64::from(height) * 4;
        let handle = self.resources.get(&key).map_or_else(
            || {
                let handle = ResourceHandle(self.next_handle);
                self.next_handle = self.next_handle.saturating_add(1);
                handle
            },
            |existing| existing.handle,
        );
        if let Some(previous) = self.resources.remove(&key) {
            self.gpu_bytes = self.gpu_bytes.saturating_sub(previous.gpu_bytes);
        }
        self.resources.insert(
            key,
            GpuResource {
                handle,
                _texture: texture,
                _view: view,
                texture_bind_group,
                vertex_buffer,
                gpu_bytes,
            },
        );
        self.gpu_bytes = self.gpu_bytes.saturating_add(gpu_bytes);
        Ok(handle)
    }

    pub const fn gpu_bytes(&self) -> u64 {
        self.gpu_bytes
    }

    pub fn visible_all(&self) -> VisibleResources<'_> {
        VisibleResources {
            items: self
                .resources
                .values()
                .map(|resource| VisibleImageResource {
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
    SizeOverflow,
    PixelLength { expected: usize, actual: usize },
    StaleGeneration,
    InvalidImagePlan(ImagePlanError),
}

impl From<ImagePlanError> for UploadError {
    fn from(error: ImagePlanError) -> Self {
        Self::InvalidImagePlan(error)
    }
}

impl fmt::Display for UploadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
