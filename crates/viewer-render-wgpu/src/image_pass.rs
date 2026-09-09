use crate::gpu_memory::{GpuBuffer, GpuMemory};
use std::{error::Error, fmt, mem};
use viewer_render_core::MemoryAdmissionError;

use bytemuck::{Pod, Zeroable};
use viewer_render_core::{
    CameraState, LogicalPoint, LogicalSize, NormalizedPoint, NormalizedRect, ResourcePlan,
    Rotation, SourceSize, TextureStrategy, TileCoordinate, TransformSnapshot, ViewportLayout,
};

use crate::ResourceHandle;

const IMAGE_VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 2] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 8,
        shader_location: 1,
    },
];

pub(crate) fn image_vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: mem::size_of::<ImageVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &IMAGE_VERTEX_ATTRIBUTES,
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ImageVertex {
    pub(crate) source_position: [f32; 2],
    pub(crate) uv: [f32; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImageTileDraw {
    pub tile: TileCoordinate,
    pub source_rect: NormalizedRect,
    pub view_corners: [LogicalPoint; 4],
    pub uv_corners: [NormalizedPoint; 4],
}

impl ImageTileDraw {
    pub(crate) fn vertices(&self) -> [ImageVertex; 6] {
        let positions = [
            [self.source_rect.x as f32, self.source_rect.y as f32],
            [
                (self.source_rect.x + self.source_rect.width) as f32,
                self.source_rect.y as f32,
            ],
            [
                (self.source_rect.x + self.source_rect.width) as f32,
                (self.source_rect.y + self.source_rect.height) as f32,
            ],
            [
                self.source_rect.x as f32,
                (self.source_rect.y + self.source_rect.height) as f32,
            ],
        ];
        let vertices = positions
            .into_iter()
            .zip(self.uv_corners)
            .map(|(source_position, uv)| ImageVertex {
                source_position,
                uv: [uv.x as f32, uv.y as f32],
            })
            .collect::<Vec<_>>();
        [
            vertices[0],
            vertices[1],
            vertices[2],
            vertices[0],
            vertices[2],
            vertices[3],
        ]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImagePlanError {
    EmptyTileSize,
    TileOutsideSource(TileCoordinate),
    InvalidGeometry,
}

impl fmt::Display for ImagePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTileSize => formatter.write_str("image tile size must be positive"),
            Self::TileOutsideSource(tile) => write!(
                formatter,
                "tile ({}, {}) at level {} is outside the source image",
                tile.x, tile.y, tile.level
            ),
            Self::InvalidGeometry => formatter.write_str("image tile geometry is invalid"),
        }
    }
}

impl Error for ImagePlanError {}

#[derive(Clone, Debug, PartialEq)]
pub struct ImagePassPlan {
    pub draws: Vec<ImageTileDraw>,
}

impl ImagePassPlan {
    pub fn from_resource_plan(
        source_size: SourceSize,
        resources: &ResourcePlan,
        transform: &TransformSnapshot,
    ) -> Result<Self, ImagePlanError> {
        let tile_size = match resources.strategy {
            TextureStrategy::SingleTexture => {
                mip_dimension(source_size.width.max(source_size.height), resources.level)
            }
            TextureStrategy::Tiled { tile_size } => tile_size,
        };
        Self::for_tiles(source_size, tile_size, &resources.required_tiles, transform)
    }

    pub fn for_tiles(
        source_size: SourceSize,
        tile_size: u32,
        tiles: &[TileCoordinate],
        transform: &TransformSnapshot,
    ) -> Result<Self, ImagePlanError> {
        if tile_size == 0 {
            return Err(ImagePlanError::EmptyTileSize);
        }
        let mut draws = Vec::with_capacity(tiles.len());
        for tile in tiles {
            let level_scale = 1_u64 << tile.level.min(31);
            let source_span = u64::from(tile_size).saturating_mul(level_scale);
            let start_x = u64::from(tile.x).saturating_mul(source_span);
            let start_y = u64::from(tile.y).saturating_mul(source_span);
            if start_x >= u64::from(source_size.width) || start_y >= u64::from(source_size.height) {
                return Err(ImagePlanError::TileOutsideSource(*tile));
            }
            let end_x = start_x
                .saturating_add(source_span)
                .min(u64::from(source_size.width));
            let end_y = start_y
                .saturating_add(source_span)
                .min(u64::from(source_size.height));
            let source_rect = NormalizedRect::new(
                start_x as f64 / f64::from(source_size.width),
                start_y as f64 / f64::from(source_size.height),
                (end_x - start_x) as f64 / f64::from(source_size.width),
                (end_y - start_y) as f64 / f64::from(source_size.height),
            )
            .map_err(|_| ImagePlanError::InvalidGeometry)?;
            let image_corners = [
                NormalizedPoint {
                    x: source_rect.x,
                    y: source_rect.y,
                },
                NormalizedPoint {
                    x: source_rect.x + source_rect.width,
                    y: source_rect.y,
                },
                NormalizedPoint {
                    x: source_rect.x + source_rect.width,
                    y: source_rect.y + source_rect.height,
                },
                NormalizedPoint {
                    x: source_rect.x,
                    y: source_rect.y + source_rect.height,
                },
            ];
            draws.push(ImageTileDraw {
                tile: *tile,
                source_rect,
                view_corners: image_corners.map(|point| transform.image_to_view(point)),
                uv_corners: [
                    NormalizedPoint { x: 0.0, y: 0.0 },
                    NormalizedPoint { x: 1.0, y: 0.0 },
                    NormalizedPoint { x: 1.0, y: 1.0 },
                    NormalizedPoint { x: 0.0, y: 1.0 },
                ],
            });
        }
        Ok(Self { draws })
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CameraUniform {
    origin: [f32; 4],
    axis_x: [f32; 4],
    axis_y: [f32; 4],
}

pub struct VisibleImageResource<'a> {
    pub handle: ResourceHandle,
    pub(crate) texture_bind_group: &'a wgpu::BindGroup,
    pub(crate) vertex_buffer: &'a wgpu::Buffer,
}

pub struct VisibleResources<'a> {
    pub(crate) items: Vec<VisibleImageResource<'a>>,
}

impl VisibleResources<'_> {
    pub fn handles(&self) -> impl Iterator<Item = ResourceHandle> + '_ {
        self.items.iter().map(|item| item.handle)
    }
}

pub struct ImagePass {
    pipeline: wgpu::RenderPipeline,
    camera_buffer: GpuBuffer,
    memory: GpuMemory,
    camera_bind_group: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,
}

impl ImagePass {
    pub(crate) fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        memory: &GpuMemory,
    ) -> Result<Self, MemoryAdmissionError> {
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Viewer image camera layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Viewer image texture layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let camera_buffer = memory.buffer_init(
            "Viewer image camera uniform",
            bytemuck::bytes_of(&CameraUniform::zeroed()),
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        )?;
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Viewer image sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Viewer image camera bind group"),
            layout: &camera_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/image.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Viewer image pipeline layout"),
            bind_group_layouts: &[Some(&camera_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Viewer image pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(image_vertex_layout())],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Ok(Self {
            memory: memory.clone(),
            pipeline,
            camera_buffer,
            camera_bind_group,
            texture_layout,
        })
    }

    pub const fn texture_layout(&self) -> &wgpu::BindGroupLayout {
        &self.texture_layout
    }

    pub fn prepare_camera(
        &self,
        _queue: &wgpu::Queue,
        transform: &TransformSnapshot,
    ) -> Result<(), MemoryAdmissionError> {
        let viewport = transform.viewport().logical_size;
        let origin = to_clip(
            transform.image_to_view(NormalizedPoint { x: 0.0, y: 0.0 }),
            viewport,
        );
        let x = to_clip(
            transform.image_to_view(NormalizedPoint { x: 1.0, y: 0.0 }),
            viewport,
        );
        let y = to_clip(
            transform.image_to_view(NormalizedPoint { x: 0.0, y: 1.0 }),
            viewport,
        );
        let uniform = CameraUniform {
            origin: [origin[0], origin[1], 0.0, 0.0],
            axis_x: [x[0] - origin[0], x[1] - origin[1], 0.0, 0.0],
            axis_y: [y[0] - origin[0], y[1] - origin[1], 0.0, 0.0],
        };
        self.memory
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform))
    }

    pub fn encode<'pass>(
        &'pass self,
        render_pass: &mut wgpu::RenderPass<'pass>,
        _transform: &TransformSnapshot,
        visible: &'pass VisibleResources<'pass>,
    ) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
        for resource in &visible.items {
            render_pass.set_bind_group(1, resource.texture_bind_group, &[]);
            render_pass.set_vertex_buffer(0, resource.vertex_buffer.slice(..));
            render_pass.draw(0..6, 0..1);
        }
    }

    pub(crate) fn encode_with<'pass>(
        &'pass self,
        render_pass: &mut wgpu::RenderPass<'pass>,
        pipeline: &'pass wgpu::RenderPipeline,
        camera_bind_group: &'pass wgpu::BindGroup,
        visible: &'pass VisibleResources<'pass>,
    ) {
        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        for resource in &visible.items {
            render_pass.set_bind_group(1, resource.texture_bind_group, &[]);
            render_pass.set_vertex_buffer(0, resource.vertex_buffer.slice(..));
            render_pass.draw(0..6, 0..1);
        }
    }
}

pub(crate) fn identity_transform(
    source_size: SourceSize,
) -> Result<TransformSnapshot, ImagePlanError> {
    let logical = LogicalSize::new(source_size.width as f64, source_size.height as f64)
        .map_err(|_| ImagePlanError::InvalidGeometry)?;
    let viewport =
        ViewportLayout::new(logical, 1.0, 1.0).map_err(|_| ImagePlanError::InvalidGeometry)?;
    TransformSnapshot::new(source_size, viewport, CameraState::fit(Rotation::Deg0))
        .map_err(|_| ImagePlanError::InvalidGeometry)
}

fn mip_dimension(dimension: u32, level: u32) -> u32 {
    let divisor = 1_u64 << level.min(31);
    u32::try_from(u64::from(dimension).div_ceil(divisor)).unwrap_or(u32::MAX)
}

fn to_clip(point: LogicalPoint, viewport: LogicalSize) -> [f32; 2] {
    [
        (point.x / viewport.width * 2.0 - 1.0) as f32,
        (1.0 - point.y / viewport.height * 2.0) as f32,
    ]
}
