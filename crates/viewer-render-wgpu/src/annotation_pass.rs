use std::mem;

use bytemuck::{Pod, Zeroable};
use viewer_render_core::{SceneSnapshot, TransformSnapshot};

use crate::{
    AnnotationMeshCache, BufferCapacityPlan, GlyphAtlasError, MeshError, MeshUpdate,
    OrdinalGlyphAtlas, VertexKind,
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuAnnotationVertex {
    source_position: [f32; 2],
    neighbor_position: [f32; 2],
    screen_offset_px: [f32; 2],
    color: [f32; 4],
    kind: f32,
    dashed: f32,
    segment_factor: f32,
    _padding: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct AnnotationCameraUniform {
    origin: [f32; 4],
    axis_x: [f32; 4],
    axis_y: [f32; 4],
    viewport_physical: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuGlyphVertex {
    source_position: [f32; 2],
    screen_offset_px: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

struct RetainedBuffers {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    vertex_capacity: usize,
    index_capacity: usize,
    index_count: u32,
}

struct GlyphTexture {
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    atlas: OrdinalGlyphAtlas,
    gpu_bytes: u64,
}

struct GlyphBuffer {
    vertices: wgpu::Buffer,
    capacity: usize,
    vertex_count: u32,
}

pub struct AnnotationPass {
    pipeline: wgpu::RenderPipeline,
    glyph_pipeline: wgpu::RenderPipeline,
    glyph_layout: wgpu::BindGroupLayout,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    mesh_cache: AnnotationMeshCache,
    capacity: BufferCapacityPlan,
    retained: Option<RetainedBuffers>,
    glyph_texture: Option<GlyphTexture>,
    glyph_buffer: Option<GlyphBuffer>,
}

impl AnnotationPass {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Viewer annotation camera layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let glyph_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Viewer ordinal glyph texture layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
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
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Viewer annotation camera uniform"),
            size: mem::size_of::<AnnotationCameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Viewer annotation camera bind group"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/annotation.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Viewer annotation pipeline layout"),
            bind_group_layouts: &[Some(&camera_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Viewer annotation pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: mem::size_of::<GpuAnnotationVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
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
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 16,
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 24,
                            shader_location: 3,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32,
                            offset: 40,
                            shader_location: 4,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32,
                            offset: 44,
                            shader_location: 5,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32,
                            offset: 48,
                            shader_location: 6,
                        },
                    ],
                })],
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
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let glyph_shader =
            device.create_shader_module(wgpu::include_wgsl!("shaders/ordinal_glyph.wgsl"));
        let glyph_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Viewer ordinal glyph pipeline layout"),
                bind_group_layouts: &[Some(&camera_layout), Some(&glyph_layout)],
                immediate_size: 0,
            });
        let glyph_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Viewer ordinal glyph pipeline"),
            layout: Some(&glyph_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &glyph_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: mem::size_of::<GpuGlyphVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
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
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 16,
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 24,
                            shader_location: 3,
                        },
                    ],
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &glyph_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            glyph_pipeline,
            glyph_layout,
            camera_buffer,
            camera_bind_group,
            mesh_cache: AnnotationMeshCache::default(),
            capacity: BufferCapacityPlan::default(),
            retained: None,
            glyph_texture: None,
            glyph_buffer: None,
        }
    }

    pub fn prepare_scene(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &SceneSnapshot,
    ) -> Result<MeshUpdate, MeshError> {
        let update = self.mesh_cache.update(scene)?;
        if update == MeshUpdate::Reused {
            return Ok(update);
        }
        let mesh = self.mesh_cache.mesh().expect("rebuilt mesh must exist");
        let vertices = mesh
            .vertices()
            .iter()
            .map(|vertex| GpuAnnotationVertex {
                source_position: vertex.source_position,
                neighbor_position: vertex.neighbor_position,
                screen_offset_px: vertex.screen_offset_px,
                color: vertex.color,
                kind: match vertex.kind {
                    VertexKind::Segment => 0.0,
                    VertexKind::ScreenOffset => 1.0,
                    VertexKind::ArrowHead => 2.0,
                },
                dashed: if vertex.dashed { 1.0 } else { 0.0 },
                segment_factor: vertex.segment_factor,
                _padding: 0.0,
            })
            .collect::<Vec<_>>();
        let indices = mesh.indices();
        self.capacity.ensure(vertices.len(), indices.len());
        if vertices.is_empty() || indices.is_empty() {
            if let Some(retained) = self.retained.as_mut() {
                retained.index_count = 0;
            }
            self.rebuild_glyph_buffer(device, queue);
            return Ok(update);
        }
        let must_allocate = self.retained.as_ref().is_none_or(|buffers| {
            buffers.vertex_capacity < self.capacity.vertex_capacity
                || buffers.index_capacity < self.capacity.index_capacity
        });
        if must_allocate {
            self.retained = Some(RetainedBuffers {
                vertices: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Viewer retained annotation vertices"),
                    size: (self.capacity.vertex_capacity * mem::size_of::<GpuAnnotationVertex>())
                        as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                indices: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Viewer retained annotation indices"),
                    size: (self.capacity.index_capacity * mem::size_of::<u32>()) as u64,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                vertex_capacity: self.capacity.vertex_capacity,
                index_capacity: self.capacity.index_capacity,
                index_count: 0,
            });
        }
        let retained = self.retained.as_mut().expect("buffers were allocated");
        queue.write_buffer(&retained.vertices, 0, bytemuck::cast_slice(&vertices));
        queue.write_buffer(&retained.indices, 0, bytemuck::cast_slice(indices));
        retained.index_count = u32::try_from(indices.len()).map_err(|_| MeshError::MeshTooLarge)?;
        self.rebuild_glyph_buffer(device, queue);
        Ok(update)
    }

    pub fn prepare_camera(&self, queue: &wgpu::Queue, transform: &TransformSnapshot) {
        let logical = transform.viewport().logical_size;
        let physical = transform.physical_viewport();
        let origin = to_clip(transform.image_to_view(point(0.0, 0.0)), logical);
        let x = to_clip(transform.image_to_view(point(1.0, 0.0)), logical);
        let y = to_clip(transform.image_to_view(point(0.0, 1.0)), logical);
        let uniform = AnnotationCameraUniform {
            origin: [origin[0], origin[1], 0.0, 0.0],
            axis_x: [x[0] - origin[0], x[1] - origin[1], 0.0, 0.0],
            axis_y: [y[0] - origin[0], y[1] - origin[1], 0.0, 0.0],
            viewport_physical: [physical.width as f32, physical.height as f32, 0.0, 0.0],
        };
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    pub fn set_ordinal_glyph_atlas(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        atlas: &OrdinalGlyphAtlas,
    ) -> Result<(), GlyphAtlasError> {
        let bytes_per_row = aligned_row_bytes(atlas.width());
        let mut upload = vec![0; bytes_per_row as usize * atlas.height() as usize];
        for row in 0..atlas.height() as usize {
            let source = row * atlas.width() as usize;
            let target = row * bytes_per_row as usize;
            upload[target..target + atlas.width() as usize]
                .copy_from_slice(&atlas.pixels()[source..source + atlas.width() as usize]);
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Viewer ordinal glyph atlas"),
            size: wgpu::Extent3d {
                width: atlas.width(),
                height: atlas.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &upload,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(atlas.height()),
            },
            wgpu::Extent3d {
                width: atlas.width(),
                height: atlas.height(),
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Viewer ordinal glyph sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Viewer ordinal glyph bind group"),
            layout: &self.glyph_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        self.glyph_texture = Some(GlyphTexture {
            _texture: texture,
            _view: view,
            bind_group,
            atlas: atlas.clone(),
            gpu_bytes: u64::from(atlas.width()) * u64::from(atlas.height()),
        });
        self.rebuild_glyph_buffer(device, queue);
        Ok(())
    }

    pub fn encode<'pass>(&'pass self, render_pass: &mut wgpu::RenderPass<'pass>) {
        if let Some(retained) = &self.retained {
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, retained.vertices.slice(..));
            render_pass.set_index_buffer(retained.indices.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..retained.index_count, 0, 0..1);
        }
        if let (Some(glyph_texture), Some(glyph_buffer)) = (&self.glyph_texture, &self.glyph_buffer)
        {
            render_pass.set_pipeline(&self.glyph_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &glyph_texture.bind_group, &[]);
            render_pass.set_vertex_buffer(0, glyph_buffer.vertices.slice(..));
            render_pass.draw(0..glyph_buffer.vertex_count, 0..1);
        }
    }

    pub fn gpu_bytes(&self) -> u64 {
        let buffer_bytes = self.retained.as_ref().map_or(0, |retained| {
            (retained.vertex_capacity * mem::size_of::<GpuAnnotationVertex>()
                + retained.index_capacity * mem::size_of::<u32>()) as u64
        });
        let glyph_buffer_bytes = self.glyph_buffer.as_ref().map_or(0, |buffer| {
            (buffer.capacity * mem::size_of::<GpuGlyphVertex>()) as u64
        });
        buffer_bytes
            + glyph_buffer_bytes
            + self
                .glyph_texture
                .as_ref()
                .map_or(0, |atlas| atlas.gpu_bytes)
    }

    pub const fn mesh_cache(&self) -> &AnnotationMeshCache {
        &self.mesh_cache
    }

    fn rebuild_glyph_buffer(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let Some(glyph_texture) = &self.glyph_texture else {
            return;
        };
        let Some(mesh) = self.mesh_cache.mesh() else {
            return;
        };
        let mut vertices = Vec::new();
        for label in mesh.ordinal_labels() {
            let total_advance = label
                .text
                .chars()
                .filter_map(|glyph| glyph_texture.atlas.metrics(glyph))
                .map(|metrics| metrics.advance_px)
                .sum::<f32>();
            let mut cursor = -total_advance / 2.0;
            for glyph in label.text.chars() {
                let Some(metrics) = glyph_texture.atlas.metrics(glyph) else {
                    continue;
                };
                let left = cursor + metrics.bearing_px[0];
                let top = -metrics.size_px[1] / 2.0;
                let right = left + metrics.size_px[0];
                let bottom = top + metrics.size_px[1];
                let quad = [
                    glyph_vertex(label.anchor, [left, top], metrics.uv_min),
                    glyph_vertex(
                        label.anchor,
                        [right, top],
                        [metrics.uv_max[0], metrics.uv_min[1]],
                    ),
                    glyph_vertex(label.anchor, [right, bottom], metrics.uv_max),
                    glyph_vertex(label.anchor, [left, top], metrics.uv_min),
                    glyph_vertex(label.anchor, [right, bottom], metrics.uv_max),
                    glyph_vertex(
                        label.anchor,
                        [left, bottom],
                        [metrics.uv_min[0], metrics.uv_max[1]],
                    ),
                ];
                vertices.extend(quad);
                cursor += metrics.advance_px;
            }
        }
        if vertices.is_empty() {
            if let Some(buffer) = self.glyph_buffer.as_mut() {
                buffer.vertex_count = 0;
            }
            return;
        }
        let required = vertices.len().next_power_of_two();
        if self
            .glyph_buffer
            .as_ref()
            .is_none_or(|buffer| buffer.capacity < required)
        {
            self.glyph_buffer = Some(GlyphBuffer {
                vertices: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Viewer retained ordinal glyph vertices"),
                    size: (required * mem::size_of::<GpuGlyphVertex>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                capacity: required,
                vertex_count: 0,
            });
        }
        let buffer = self.glyph_buffer.as_mut().expect("glyph buffer allocated");
        queue.write_buffer(&buffer.vertices, 0, bytemuck::cast_slice(&vertices));
        buffer.vertex_count = vertices.len() as u32;
    }
}

fn glyph_vertex(
    source: viewer_render_core::NormalizedPoint,
    screen_offset_px: [f32; 2],
    uv: [f32; 2],
) -> GpuGlyphVertex {
    GpuGlyphVertex {
        source_position: [source.x as f32, source.y as f32],
        screen_offset_px,
        uv,
        color: [1.0, 1.0, 1.0, 1.0],
    }
}

fn point(x: f64, y: f64) -> viewer_render_core::NormalizedPoint {
    viewer_render_core::NormalizedPoint { x, y }
}

fn to_clip(
    point: viewer_render_core::LogicalPoint,
    viewport: viewer_render_core::LogicalSize,
) -> [f32; 2] {
    [
        (point.x / viewport.width * 2.0 - 1.0) as f32,
        (1.0 - point.y / viewport.height * 2.0) as f32,
    ]
}

fn aligned_row_bytes(width: u32) -> u32 {
    width.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
}
