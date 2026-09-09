use crate::gpu_memory::{BufferWrite, GpuBuffer, GpuMemory, GpuTexture};
use std::{cell::Cell, mem};
use viewer_render_core::{AllocationClass, AssetGeneration, MemoryAdmissionError};

use bytemuck::{Pod, Zeroable};
use viewer_render_core::{SceneSnapshot, TransformSnapshot};

use crate::{
    AnnotationBufferIdentity, AnnotationMeshCache, AnnotationMeshLayers, AnnotationVertex,
    BufferCapacityPlan, GlyphAtlasError, MagnifierConfig, MeshError, MeshUpdate, OrdinalGlyphAtlas,
    VertexKind,
};

const ANNOTATION_VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 7] = [
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
];

const GLYPH_VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 4] = [
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
];

pub(crate) fn annotation_vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: mem::size_of::<GpuAnnotationVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ANNOTATION_VERTEX_ATTRIBUTES,
    }
}

pub(crate) fn glyph_vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: mem::size_of::<GpuGlyphVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &GLYPH_VERTEX_ATTRIBUTES,
    }
}

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
    vertices: GpuBuffer,
    indices: GpuBuffer,
    vertex_capacity: usize,
    index_capacity: usize,
    index_count: u32,
}

struct GlyphTexture {
    _texture: GpuTexture,
    _view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    atlas: OrdinalGlyphAtlas,
    gpu_bytes: u64,
}

struct GlyphBuffer {
    vertices: GpuBuffer,
    lens_vertices: GpuBuffer,
    capacity: usize,
    vertex_count: u32,
    layout_vertices: Vec<GpuGlyphVertex>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingSceneUpload {
    Authoritative(viewer_render_core::SceneRevision),
    Transient(viewer_render_core::SceneRevision),
}

pub struct AnnotationPass {
    memory: GpuMemory,
    upload_failed: bool,
    /// Revision of the authoritative scene whose buffers are known to be
    /// resident. Mesh caches are updated before allocation, so this must be
    /// tracked independently to make a temporarily blocked upload retryable.
    uploaded_authoritative_revision: Option<viewer_render_core::SceneRevision>,
    pending_scene_upload: Option<PendingSceneUpload>,
    pending_draft_upload: bool,
    pipeline: wgpu::RenderPipeline,
    glyph_pipeline: wgpu::RenderPipeline,
    glyph_layout: wgpu::BindGroupLayout,
    camera_buffer: GpuBuffer,
    camera_bind_group: wgpu::BindGroup,
    mesh_layers: AnnotationMeshLayers,
    capacity: BufferCapacityPlan,
    draft_capacity: BufferCapacityPlan,
    retained: Option<RetainedBuffers>,
    draft: Option<RetainedBuffers>,
    glyph_texture: Option<GlyphTexture>,
    glyph_buffer: Option<GlyphBuffer>,
    buffer_identity: u64,
    control_transform: Cell<Option<TransformSnapshot>>,
    lens_control_key: Cell<Option<(TransformSnapshot, MagnifierConfig)>>,
    lens_badges: Option<RetainedBuffers>,
    lens_badge_capacity: BufferCapacityPlan,
}

impl AnnotationPass {
    pub(crate) fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        memory: &GpuMemory,
    ) -> Result<Self, MemoryAdmissionError> {
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
        let camera_buffer = memory.buffer(
            &wgpu::BufferDescriptor {
                label: Some("Viewer annotation camera uniform"),
                size: mem::size_of::<AnnotationCameraUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
            AllocationClass::RendererBuffers,
        )?;
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
                buffers: &[Some(annotation_vertex_layout())],
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
                buffers: &[Some(glyph_vertex_layout())],
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
        Ok(Self {
            memory: memory.clone(),
            upload_failed: false,
            uploaded_authoritative_revision: None,
            pending_scene_upload: None,
            pending_draft_upload: false,
            pipeline,
            glyph_pipeline,
            glyph_layout,
            camera_buffer,
            camera_bind_group,
            mesh_layers: AnnotationMeshLayers::default(),
            capacity: BufferCapacityPlan::default(),
            draft_capacity: BufferCapacityPlan::default(),
            retained: None,
            draft: None,
            glyph_texture: None,
            glyph_buffer: None,
            buffer_identity: 0,
            control_transform: Cell::new(None),
            lens_control_key: Cell::new(None),
            lens_badges: None,
            lens_badge_capacity: BufferCapacityPlan::default(),
        })
    }

    pub fn prepare_scene(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &SceneSnapshot,
    ) -> Result<MeshUpdate, MeshError> {
        let result = (|| {
            let update = self.mesh_layers.update_authoritative(scene)?;
            let needs_upload = update != MeshUpdate::Reused
                || self.uploaded_authoritative_revision != Some(scene.revision());
            if needs_upload {
                self.pending_scene_upload =
                    Some(PendingSceneUpload::Authoritative(scene.revision()));
            }
            let result = self.upload_scene_update(
                device,
                queue,
                if needs_upload {
                    MeshUpdate::Rebuilt
                } else {
                    MeshUpdate::Reused
                },
            );
            self.clear_draft_overlay();
            if result.is_ok() {
                self.uploaded_authoritative_revision = Some(scene.revision());
                self.pending_scene_upload = None;
            }
            result
        })();
        if matches!(
            &result,
            Err(MeshError::Memory(error))
                if !matches!(error, MemoryAdmissionError::TemporarilyBlocked)
        ) {
            self.upload_failed = true;
        }
        result
    }

    pub fn prepare_transient_scene(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &SceneSnapshot,
    ) -> Result<MeshUpdate, MeshError> {
        let result = (|| {
            let update = self.mesh_layers.update_transient_authoritative(scene)?;
            self.pending_scene_upload = Some(PendingSceneUpload::Transient(scene.revision()));
            let result = self.upload_scene_update(device, queue, update);
            self.clear_draft_overlay();
            if result.is_ok() {
                self.uploaded_authoritative_revision = None;
                self.pending_scene_upload = None;
            }
            result
        })();
        if matches!(
            &result,
            Err(MeshError::Memory(error))
                if !matches!(error, MemoryAdmissionError::TemporarilyBlocked)
        ) {
            self.upload_failed = true;
        }
        result
    }

    /// Uploads only renderer-owned draft geometry. The authoritative retained
    /// scene is neither tessellated nor rewritten on high-frequency pointer
    /// updates.
    pub fn prepare_draft_overlay(
        &mut self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        scene: &SceneSnapshot,
    ) -> Result<MeshUpdate, MeshError> {
        let result = (|| {
            let update = self.mesh_layers.update_draft(scene)?;
            self.pending_draft_upload = true;
            if update == MeshUpdate::Reused {
                return Ok(update);
            }
            let mesh = self
                .mesh_layers
                .draft()
                .mesh()
                .expect("rebuilt draft mesh must exist");
            let vertices = gpu_vertices(mesh.vertices());
            let indices = mesh.indices();
            self.draft_capacity.ensure(vertices.len(), indices.len());
            upload_mesh_buffers(
                &self.memory,
                &vertices,
                indices,
                self.draft_capacity,
                &mut self.draft,
                "Viewer draft annotation",
            )?;
            self.pending_draft_upload = false;
            Ok(update)
        })();
        if matches!(
            &result,
            Err(MeshError::Memory(error))
                if !matches!(error, MemoryAdmissionError::TemporarilyBlocked)
        ) {
            self.upload_failed = true;
        }
        result
    }

    pub fn clear_draft_overlay(&mut self) {
        self.pending_draft_upload = false;
        if let Some(draft) = self.draft.as_mut() {
            draft.index_count = 0;
        }
    }

    fn upload_scene_update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        update: MeshUpdate,
    ) -> Result<MeshUpdate, MeshError> {
        if update == MeshUpdate::Reused {
            return Ok(update);
        }
        let mesh = self
            .mesh_layers
            .authoritative()
            .mesh()
            .expect("rebuilt mesh must exist");
        let vertices = gpu_vertices(mesh.vertices());
        let indices = mesh.indices();
        self.capacity.ensure(vertices.len(), indices.len());
        let must_allocate = self.retained.as_ref().is_none_or(|buffers| {
            buffers.vertex_capacity < self.capacity.vertex_capacity
                || buffers.index_capacity < self.capacity.index_capacity
        });
        if must_allocate && !vertices.is_empty() && !indices.is_empty() {
            self.buffer_identity = self.buffer_identity.saturating_add(1);
        }
        upload_mesh_buffers(
            &self.memory,
            &vertices,
            indices,
            self.capacity,
            &mut self.retained,
            "Viewer retained annotation",
        )?;
        let mut badge_vertices = Vec::new();
        let mut badge_indices = Vec::new();
        for label in mesh.ordinal_labels() {
            let base = badge_vertices.len() as u32;
            badge_vertices.extend(gpu_vertices(&mesh.vertices()[label.badge_vertices.clone()]));
            badge_indices.extend(
                mesh.indices()[label.badge_indices.clone()]
                    .iter()
                    .map(|index| index - label.badge_vertices.start as u32 + base),
            );
        }
        self.lens_badge_capacity
            .ensure(badge_vertices.len(), badge_indices.len());
        upload_mesh_buffers(
            &self.memory,
            &badge_vertices,
            &badge_indices,
            self.lens_badge_capacity,
            &mut self.lens_badges,
            "Viewer lens ordinal projection",
        )?;
        self.rebuild_glyph_buffer(device, queue)?;
        Ok(update)
    }

    /// Retries scene and draft uploads that were admitted to the CPU mesh but
    /// temporarily refused by the shared GPU budget. The mesh cache is
    /// retained, so retrying never tessellates the scene again.
    pub fn retry_pending(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), MeshError> {
        if self.upload_failed {
            return Err(MemoryAdmissionError::ExceedsPolicy.into());
        }
        if self.pending_scene_upload.is_some() {
            let result = self.upload_scene_update(device, queue, MeshUpdate::Rebuilt);
            if result.is_ok()
                && let Some(pending) = self.pending_scene_upload.take()
            {
                self.uploaded_authoritative_revision = match pending {
                    PendingSceneUpload::Authoritative(revision) => Some(revision),
                    PendingSceneUpload::Transient(_) => None,
                };
            }
            if matches!(
                &result,
                Err(MeshError::Memory(error))
                    if !matches!(error, MemoryAdmissionError::TemporarilyBlocked)
            ) {
                self.upload_failed = true;
            }
            result.map(|_| ())?;
        }
        if self.pending_draft_upload {
            let mesh = self
                .mesh_layers
                .draft()
                .mesh()
                .expect("pending draft upload retains its mesh");
            let vertices = gpu_vertices(mesh.vertices());
            let indices = mesh.indices();
            self.draft_capacity.ensure(vertices.len(), indices.len());
            let result = upload_mesh_buffers(
                &self.memory,
                &vertices,
                indices,
                self.draft_capacity,
                &mut self.draft,
                "Viewer draft annotation",
            );
            if result.is_ok() {
                self.pending_draft_upload = false;
            }
            if matches!(
                &result,
                Err(MeshError::Memory(error))
                    if !matches!(error, MemoryAdmissionError::TemporarilyBlocked)
            ) {
                self.upload_failed = true;
            }
            result.map(|_| ())?;
        }
        Ok(())
    }

    pub fn prepare_camera(
        &self,
        queue: &wgpu::Queue,
        transform: &TransformSnapshot,
    ) -> Result<(), MemoryAdmissionError> {
        let logical = transform.viewport().logical_size;
        let physical = transform.physical_viewport();
        let origin = to_clip(transform.image_to_view(point(0.0, 0.0)), logical);
        let x = to_clip(transform.image_to_view(point(1.0, 0.0)), logical);
        let y = to_clip(transform.image_to_view(point(0.0, 1.0)), logical);
        let uniform = AnnotationCameraUniform {
            origin: [origin[0], origin[1], 0.0, 0.0],
            axis_x: [x[0] - origin[0], x[1] - origin[1], 0.0, 0.0],
            axis_y: [y[0] - origin[0], y[1] - origin[1], 0.0, 0.0],
            viewport_physical: [
                physical.width as f32,
                physical.height as f32,
                physical.width as f32 / logical.width as f32,
                0.0,
            ],
        };
        self.memory
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform))?;
        self.prepare_control_layout(queue, transform)?;
        Ok(())
    }

    // Camera changes only rewrite small control ranges; retained geometry and
    // index buffers keep their identity and are never rebuilt for pan/zoom.
    fn prepare_control_layout(
        &self,
        _queue: &wgpu::Queue,
        transform: &TransformSnapshot,
    ) -> Result<(), MemoryAdmissionError> {
        if self.control_transform.get() == Some(*transform) {
            return Ok(());
        }
        let (Some(mesh), Some(retained)) =
            (self.mesh_layers.authoritative().mesh(), &self.retained)
        else {
            return Ok(());
        };
        let mut glyphs = self
            .glyph_buffer
            .as_ref()
            .map(|buffer| buffer.layout_vertices.clone())
            .unwrap_or_default();
        let mut glyph_cursor = 0;
        for label in mesh.ordinal_labels() {
            let anchor = label.layout_anchor(transform);
            let source = anchor.unwrap_or(label.anchor);
            let mut badge = gpu_vertices(&mesh.vertices()[label.badge_vertices.clone()]);
            for vertex in &mut badge {
                vertex.source_position = [source.x as f32, source.y as f32];
                if anchor.is_none() {
                    vertex.color = [0.0; 4];
                }
            }
            self.memory.write_buffer(
                &retained.vertices,
                (label.badge_vertices.start * mem::size_of::<GpuAnnotationVertex>()) as u64,
                bytemuck::cast_slice(&badge),
            )?;
            if let Some(texture) = &self.glyph_texture {
                let count = label
                    .text
                    .chars()
                    .filter(|glyph| texture.atlas.metrics(*glyph).is_some())
                    .count()
                    * 6;
                for vertex in glyphs.iter_mut().skip(glyph_cursor).take(count) {
                    vertex.source_position = [source.x as f32, source.y as f32];
                    if anchor.is_none() {
                        vertex.color = [0.0; 4];
                    }
                }
                glyph_cursor += count;
            }
        }
        if let Some(buffer) = &self.glyph_buffer
            && !glyphs.is_empty()
        {
            self.memory
                .write_buffer(&buffer.vertices, 0, bytemuck::cast_slice(&glyphs))?;
        }
        self.control_transform.set(Some(*transform));
        Ok(())
    }

    /// Projects only small ordinal/glyph controls for the lens. Geometry and
    /// image resources remain the same retained scene used by the main pass.
    pub fn prepare_magnifier_controls(
        &self,
        _queue: &wgpu::Queue,
        transform: &TransformSnapshot,
        config: MagnifierConfig,
    ) -> Result<(), MemoryAdmissionError> {
        if self.lens_control_key.get() == Some((*transform, config)) {
            return Ok(());
        }
        let Some(mesh) = self.mesh_layers.authoritative().mesh() else {
            return Ok(());
        };
        let focus = transform.image_to_view(config.focus);
        let size = viewer_render_core::LogicalSize {
            width: config.width_px,
            height: config.height_px,
        };
        let mut badges = Vec::new();
        let mut glyphs = self
            .glyph_buffer
            .as_ref()
            .map(|buffer| buffer.layout_vertices.clone())
            .unwrap_or_default();
        let mut glyph_cursor = 0;
        for label in mesh.ordinal_labels() {
            let position = viewer_render_core::annotation_ordinal_position_projected(
                &label.geometry,
                |point| {
                    let projected = transform.image_to_view(point);
                    viewer_render_core::LogicalPoint {
                        x: (projected.x - focus.x) * config.magnification + size.width / 2.0,
                        y: (projected.y - focus.y) * config.magnification + size.height / 2.0,
                    }
                },
                size,
                (2.0 * config.magnification).clamp(2.0, 5.0),
            );
            let source = position
                .map(|position| {
                    transform.view_to_image_unclamped(viewer_render_core::LogicalPoint {
                        x: focus.x + (position.x - size.width / 2.0) / config.magnification,
                        y: focus.y + (position.y - size.height / 2.0) / config.magnification,
                    })
                })
                .unwrap_or(label.anchor);
            let mut vertices = gpu_vertices(&mesh.vertices()[label.badge_vertices.clone()]);
            for vertex in &mut vertices {
                vertex.source_position = [source.x as f32, source.y as f32];
                if position.is_none() {
                    vertex.color = [0.0; 4];
                }
            }
            badges.extend(vertices);
            if let Some(texture) = &self.glyph_texture {
                let count = label
                    .text
                    .chars()
                    .filter(|glyph| texture.atlas.metrics(*glyph).is_some())
                    .count()
                    * 6;
                for vertex in glyphs.iter_mut().skip(glyph_cursor).take(count) {
                    vertex.source_position = [source.x as f32, source.y as f32];
                    if position.is_none() {
                        vertex.color = [0.0; 4];
                    }
                }
                glyph_cursor += count;
            }
        }
        if let Some(buffer) = &self.lens_badges
            && !badges.is_empty()
        {
            self.memory
                .write_buffer(&buffer.vertices, 0, bytemuck::cast_slice(&badges))?;
        }
        if let Some(buffer) = &self.glyph_buffer
            && !glyphs.is_empty()
        {
            self.memory
                .write_buffer(&buffer.lens_vertices, 0, bytemuck::cast_slice(&glyphs))?;
        }
        self.lens_control_key.set(Some((*transform, config)));
        Ok(())
    }

    pub fn set_ordinal_glyph_atlas(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        atlas: &OrdinalGlyphAtlas,
    ) -> Result<(), GlyphAtlasError> {
        let result = (|| {
            let atlas = atlas.accounted(self.memory.coordinator())?;
            let bytes_per_row = aligned_row_bytes(atlas.width());
            let texture = self.memory.texture(
                &wgpu::TextureDescriptor {
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
                },
                u64::from(atlas.width()) * u64::from(atlas.height()),
                AssetGeneration(0),
            )?;
            let upload = self.memory.buffer(
                &wgpu::BufferDescriptor {
                    label: Some("Viewer one-time ordinal atlas initialization staging"),
                    size: u64::from(bytes_per_row) * u64::from(atlas.height()),
                    usage: wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: true,
                },
                AllocationClass::UploadStaging,
            )?;
            {
                let mut mapped = upload
                    .slice(..)
                    .get_mapped_range_mut()
                    .map_err(|_| MemoryAdmissionError::AllocationFailed)?;
                for row in 0..atlas.height() as usize {
                    let source = row * atlas.width() as usize;
                    let target = row * bytes_per_row as usize;
                    mapped
                        .slice(target..target + atlas.width() as usize)
                        .copy_from_slice(&atlas.pixels()[source..source + atlas.width() as usize]);
                }
            }
            upload.unmap();
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Viewer ordinal atlas initialization"),
            });
            encoder.copy_buffer_to_texture(
                wgpu::TexelCopyBufferInfo {
                    buffer: &upload,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_row),
                        rows_per_image: Some(atlas.height()),
                    },
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: atlas.width(),
                    height: atlas.height(),
                    depth_or_array_layers: 1,
                },
            );
            drop(upload);
            self.memory.submit([encoder.finish()]);
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
            self.rebuild_glyph_buffer(device, queue)?;
            Ok(())
        })();
        if matches!(
            &result,
            Err(GlyphAtlasError::Memory(error))
                if !matches!(error, MemoryAdmissionError::TemporarilyBlocked)
        ) {
            self.upload_failed = true;
        }
        result
    }

    pub fn encode<'pass>(&'pass self, render_pass: &mut wgpu::RenderPass<'pass>) {
        if let Some(retained) = &self.retained {
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, retained.vertices.slice(..));
            render_pass.set_index_buffer(retained.indices.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..retained.index_count, 0, 0..1);
        }
        encode_annotation_buffers(
            render_pass,
            &self.pipeline,
            &self.camera_bind_group,
            self.draft.as_ref(),
        );
        if let (Some(glyph_texture), Some(glyph_buffer)) = (&self.glyph_texture, &self.glyph_buffer)
        {
            render_pass.set_pipeline(&self.glyph_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &glyph_texture.bind_group, &[]);
            render_pass.set_vertex_buffer(0, glyph_buffer.vertices.slice(..));
            render_pass.draw(0..glyph_buffer.vertex_count, 0..1);
        }
    }

    pub(crate) fn upload_failed(&self) -> bool {
        self.upload_failed
    }

    pub(crate) fn scene_upload_pending(&self) -> bool {
        self.pending_scene_upload.is_some()
    }

    pub fn gpu_bytes(&self) -> u64 {
        let buffer_bytes = self.retained.as_ref().map_or(0, |retained| {
            (retained.vertex_capacity * mem::size_of::<GpuAnnotationVertex>()
                + retained.index_capacity * mem::size_of::<u32>()) as u64
        });
        let glyph_buffer_bytes = self.glyph_buffer.as_ref().map_or(0, |buffer| {
            (2 * buffer.capacity * mem::size_of::<GpuGlyphVertex>()) as u64
        });
        let draft_buffer_bytes = self.draft.as_ref().map_or(0, |draft| {
            (draft.vertex_capacity * mem::size_of::<GpuAnnotationVertex>()
                + draft.index_capacity * mem::size_of::<u32>()) as u64
        });
        buffer_bytes
            + self.lens_badges.as_ref().map_or(0, |buffer| {
                (buffer.vertex_capacity * mem::size_of::<GpuAnnotationVertex>()
                    + buffer.index_capacity * mem::size_of::<u32>()) as u64
            })
            + draft_buffer_bytes
            + glyph_buffer_bytes
            + self
                .glyph_texture
                .as_ref()
                .map_or(0, |atlas| atlas.gpu_bytes)
    }

    pub const fn mesh_cache(&self) -> &AnnotationMeshCache {
        self.mesh_layers.authoritative()
    }

    pub const fn glyph_layout(&self) -> &wgpu::BindGroupLayout {
        &self.glyph_layout
    }

    pub fn buffer_identity(&self) -> Option<AnnotationBufferIdentity> {
        self.retained
            .as_ref()
            .map(|_| AnnotationBufferIdentity(self.buffer_identity))
    }

    pub(crate) fn encode_with<'pass>(
        &'pass self,
        render_pass: &mut wgpu::RenderPass<'pass>,
        annotation_pipeline: &'pass wgpu::RenderPipeline,
        glyph_pipeline: &'pass wgpu::RenderPipeline,
        camera_bind_group: &'pass wgpu::BindGroup,
    ) {
        for (buffers, mesh) in [
            (
                self.retained.as_ref(),
                self.mesh_layers.authoritative().mesh(),
            ),
            (self.draft.as_ref(), self.mesh_layers.draft().mesh()),
        ] {
            let (Some(retained), Some(mesh)) = (buffers, mesh) else {
                continue;
            };
            if retained.index_count == 0 {
                continue;
            }
            render_pass.set_pipeline(annotation_pipeline);
            render_pass.set_bind_group(0, camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, retained.vertices.slice(..));
            render_pass.set_index_buffer(retained.indices.slice(..), wgpu::IndexFormat::Uint32);
            for range in mesh.geometry_index_ranges() {
                render_pass.draw_indexed(range.clone(), 0, 0..1);
            }
        }
        encode_annotation_buffers(
            render_pass,
            annotation_pipeline,
            camera_bind_group,
            self.lens_badges.as_ref(),
        );
        if let (Some(glyph_texture), Some(glyph_buffer)) = (&self.glyph_texture, &self.glyph_buffer)
        {
            render_pass.set_pipeline(glyph_pipeline);
            render_pass.set_bind_group(0, camera_bind_group, &[]);
            render_pass.set_bind_group(1, &glyph_texture.bind_group, &[]);
            render_pass.set_vertex_buffer(0, glyph_buffer.lens_vertices.slice(..));
            render_pass.draw(0..glyph_buffer.vertex_count, 0..1);
        }
    }

    fn rebuild_glyph_buffer(
        &mut self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) -> Result<(), MemoryAdmissionError> {
        let memory = &self.memory;
        self.control_transform.set(None);
        self.lens_control_key.set(None);
        let Some(glyph_texture) = &self.glyph_texture else {
            return Ok(());
        };
        let Some(mesh) = self.mesh_layers.authoritative().mesh() else {
            return Ok(());
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
            return Ok(());
        }
        let required = vertices.len().next_power_of_two();
        if self
            .glyph_buffer
            .as_ref()
            .is_none_or(|buffer| buffer.capacity < required)
        {
            self.glyph_buffer = Some(GlyphBuffer {
                lens_vertices: memory.buffer(
                    &wgpu::BufferDescriptor {
                        label: Some("Viewer lens ordinal glyph projection"),
                        size: (required * mem::size_of::<GpuGlyphVertex>()) as u64,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    },
                    AllocationClass::RendererBuffers,
                )?,
                vertices: memory.buffer(
                    &wgpu::BufferDescriptor {
                        label: Some("Viewer retained ordinal glyph vertices"),
                        size: (required * mem::size_of::<GpuGlyphVertex>()) as u64,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    },
                    AllocationClass::RendererBuffers,
                )?,
                capacity: required,
                vertex_count: 0,
                layout_vertices: Vec::new(),
            });
        }
        let buffer = self.glyph_buffer.as_mut().expect("glyph buffer allocated");
        memory.write_buffer(&buffer.vertices, 0, bytemuck::cast_slice(&vertices))?;
        buffer.vertex_count = vertices.len() as u32;
        buffer.layout_vertices = vertices;
        Ok(())
    }
}

fn gpu_vertices(vertices: &[AnnotationVertex]) -> Vec<GpuAnnotationVertex> {
    vertices
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
        .collect()
}

fn upload_mesh_buffers(
    memory: &GpuMemory,
    vertices: &[GpuAnnotationVertex],
    indices: &[u32],
    capacity: BufferCapacityPlan,
    buffers: &mut Option<RetainedBuffers>,
    label: &'static str,
) -> Result<(), MeshError> {
    if vertices.is_empty() || indices.is_empty() {
        if let Some(buffers) = buffers.as_mut() {
            buffers.index_count = 0;
        }
        return Ok(());
    }
    let must_allocate = buffers.as_ref().is_none_or(|buffers| {
        buffers.vertex_capacity < capacity.vertex_capacity
            || buffers.index_capacity < capacity.index_capacity
    });
    let index_count = u32::try_from(indices.len()).map_err(|_| MeshError::MeshTooLarge)?;
    let vertex_bytes = bytemuck::cast_slice(vertices);
    let index_bytes = bytemuck::cast_slice(indices);
    if must_allocate {
        // Keep the old pair installed until both replacement buffers have been
        // allocated and both queue writes have been admitted. A temporary
        // pressure refusal therefore cannot expose a new vertex buffer with
        // the old index buffer (or vice versa).
        let replacement_vertices = memory.buffer(
            &wgpu::BufferDescriptor {
                label: Some(label),
                size: (capacity.vertex_capacity * mem::size_of::<GpuAnnotationVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
            AllocationClass::RendererBuffers,
        )?;
        let replacement_indices = memory.buffer(
            &wgpu::BufferDescriptor {
                label: Some(label),
                size: (capacity.index_capacity * mem::size_of::<u32>()) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
            AllocationClass::RendererBuffers,
        )?;
        memory.write_buffers(&[
            BufferWrite {
                buffer: &replacement_vertices,
                offset: 0,
                data: vertex_bytes,
            },
            BufferWrite {
                buffer: &replacement_indices,
                offset: 0,
                data: index_bytes,
            },
        ])?;
        *buffers = Some(RetainedBuffers {
            vertices: replacement_vertices,
            indices: replacement_indices,
            vertex_capacity: capacity.vertex_capacity,
            index_capacity: capacity.index_capacity,
            index_count,
        });
    } else {
        let buffers = buffers.as_mut().expect("mesh buffers were allocated");
        memory.write_buffers(&[
            BufferWrite {
                buffer: &buffers.vertices,
                offset: 0,
                data: vertex_bytes,
            },
            BufferWrite {
                buffer: &buffers.indices,
                offset: 0,
                data: index_bytes,
            },
        ])?;
        buffers.index_count = index_count;
    }
    Ok(())
}

fn encode_annotation_buffers<'pass>(
    render_pass: &mut wgpu::RenderPass<'pass>,
    pipeline: &'pass wgpu::RenderPipeline,
    camera_bind_group: &'pass wgpu::BindGroup,
    buffers: Option<&'pass RetainedBuffers>,
) {
    let Some(buffers) = buffers else {
        return;
    };
    render_pass.set_pipeline(pipeline);
    render_pass.set_bind_group(0, camera_bind_group, &[]);
    render_pass.set_vertex_buffer(0, buffers.vertices.slice(..));
    render_pass.set_index_buffer(buffers.indices.slice(..), wgpu::IndexFormat::Uint32);
    render_pass.draw_indexed(0..buffers.index_count, 0, 0..1);
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
