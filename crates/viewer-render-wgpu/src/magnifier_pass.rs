use std::{error::Error, fmt, mem};

use bytemuck::{Pod, Zeroable};
use viewer_render_core::{LogicalPoint, NormalizedPoint, SceneRevision, TransformSnapshot};

use crate::{
    AnnotationPass, ImagePass, ResourceHandle, VisibleResources,
    annotation_pass::{annotation_vertex_layout, glyph_vertex_layout},
    image_pass::image_vertex_layout,
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MagnifierUniform {
    origin: [f32; 4],
    axis_x: [f32; 4],
    axis_y: [f32; 4],
    viewport_physical: [f32; 4],
    clip: [f32; 4],
    style: [f32; 4],
}

pub struct MagnifierPass {
    image_pipeline: wgpu::RenderPipeline,
    annotation_pipeline: wgpu::RenderPipeline,
    glyph_pipeline: wgpu::RenderPipeline,
    border_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
}

impl MagnifierPass {
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        image_texture_layout: &wgpu::BindGroupLayout,
        glyph_texture_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Viewer magnifier uniform layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Viewer magnifier uniform"),
            size: mem::size_of::<MagnifierUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Viewer magnifier image sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Viewer magnifier uniform bind group"),
            layout: &uniform_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let image_shader =
            device.create_shader_module(wgpu::include_wgsl!("shaders/magnifier_image.wgsl"));
        let image_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Viewer magnifier image pipeline layout"),
                bind_group_layouts: &[Some(&uniform_layout), Some(image_texture_layout)],
                immediate_size: 0,
            });
        let image_pipeline = create_pipeline(
            device,
            "Viewer magnifier image pipeline",
            &image_pipeline_layout,
            &image_shader,
            image_vertex_layout(),
            target_format,
        );

        let annotation_shader =
            device.create_shader_module(wgpu::include_wgsl!("shaders/magnifier_annotation.wgsl"));
        let annotation_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Viewer magnifier annotation pipeline layout"),
                bind_group_layouts: &[Some(&uniform_layout)],
                immediate_size: 0,
            });
        let annotation_pipeline = create_pipeline(
            device,
            "Viewer magnifier annotation pipeline",
            &annotation_pipeline_layout,
            &annotation_shader,
            annotation_vertex_layout(),
            target_format,
        );

        let glyph_shader =
            device.create_shader_module(wgpu::include_wgsl!("shaders/magnifier_glyph.wgsl"));
        let glyph_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Viewer magnifier glyph pipeline layout"),
                bind_group_layouts: &[Some(&uniform_layout), Some(glyph_texture_layout)],
                immediate_size: 0,
            });
        let glyph_pipeline = create_pipeline(
            device,
            "Viewer magnifier glyph pipeline",
            &glyph_pipeline_layout,
            &glyph_shader,
            glyph_vertex_layout(),
            target_format,
        );

        let border_shader =
            device.create_shader_module(wgpu::include_wgsl!("shaders/magnifier_border.wgsl"));
        let border_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Viewer magnifier border pipeline layout"),
                bind_group_layouts: &[Some(&uniform_layout)],
                immediate_size: 0,
            });
        let border_pipeline = create_pipeline_without_vertices(
            device,
            "Viewer magnifier border pipeline",
            &border_pipeline_layout,
            &border_shader,
            target_format,
        );

        Self {
            image_pipeline,
            annotation_pipeline,
            glyph_pipeline,
            border_pipeline,
            uniform_buffer,
            uniform_bind_group,
        }
    }

    pub fn prepare(
        &self,
        queue: &wgpu::Queue,
        transform: &TransformSnapshot,
        config: MagnifierConfig,
    ) {
        let viewport = transform.viewport();
        let origin = to_clip(
            project_source(config, transform, NormalizedPoint { x: 0.0, y: 0.0 }),
            viewport.logical_size.width,
            viewport.logical_size.height,
        );
        let axis_x_end = to_clip(
            project_source(config, transform, NormalizedPoint { x: 1.0, y: 0.0 }),
            viewport.logical_size.width,
            viewport.logical_size.height,
        );
        let axis_y_end = to_clip(
            project_source(config, transform, NormalizedPoint { x: 0.0, y: 1.0 }),
            viewport.logical_size.width,
            viewport.logical_size.height,
        );
        let physical = transform.physical_viewport();
        let scale = viewport.scale_factor as f32;
        let half_width = config.width_px as f32 * scale / 2.0;
        let half_height = config.height_px as f32 * scale / 2.0;
        let uniform = MagnifierUniform {
            origin: [origin[0], origin[1], 0.0, 0.0],
            axis_x: [
                axis_x_end[0] - origin[0],
                axis_x_end[1] - origin[1],
                0.0,
                0.0,
            ],
            axis_y: [
                axis_y_end[0] - origin[0],
                axis_y_end[1] - origin[1],
                0.0,
                0.0,
            ],
            viewport_physical: [physical.width as f32, physical.height as f32, 0.0, 0.0],
            clip: [
                config.center.x as f32 * scale,
                config.center.y as f32 * scale,
                half_width,
                half_height,
            ],
            style: [
                config.width_px.min(config.height_px) as f32 * scale * 0.12,
                2.0 * scale,
                match config.shape {
                    MagnifierShape::Circle => 0.0,
                    MagnifierShape::RoundedRectangle => 1.0,
                },
                0.0,
            ],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    pub fn encode<'pass>(
        &'pass self,
        render_pass: &mut wgpu::RenderPass<'pass>,
        image_pass: &'pass ImagePass,
        visible: &'pass VisibleResources<'pass>,
        annotation_pass: &'pass AnnotationPass,
    ) {
        image_pass.encode_with(
            render_pass,
            &self.image_pipeline,
            &self.uniform_bind_group,
            visible,
        );
        annotation_pass.encode_with(
            render_pass,
            &self.annotation_pipeline,
            &self.glyph_pipeline,
            &self.uniform_bind_group,
        );
        render_pass.set_pipeline(&self.border_pipeline);
        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }
}

fn create_pipeline(
    device: &wgpu::Device,
    label: &'static str,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    vertex_buffer: wgpu::VertexBufferLayout<'static>,
    target_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let targets = [Some(color_target(target_format))];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(vertex_buffer)],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &targets,
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn create_pipeline_without_vertices(
    device: &wgpu::Device,
    label: &'static str,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    target_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let targets = [Some(color_target(target_format))];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &targets,
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn color_target(target_format: wgpu::TextureFormat) -> wgpu::ColorTargetState {
    wgpu::ColorTargetState {
        format: target_format,
        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
    }
}

fn to_clip(point: LogicalPoint, width: f64, height: f64) -> [f32; 2] {
    [
        (point.x / width * 2.0 - 1.0) as f32,
        (1.0 - point.y / height * 2.0) as f32,
    ]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MagnifierShape {
    Circle,
    RoundedRectangle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MagnifierConfig {
    pub focus: NormalizedPoint,
    pub center: LogicalPoint,
    pub width_px: f64,
    pub height_px: f64,
    pub magnification: f64,
    pub shape: MagnifierShape,
}

impl MagnifierConfig {
    pub fn new(
        focus: NormalizedPoint,
        center: LogicalPoint,
        width_px: f64,
        height_px: f64,
        magnification: f64,
        shape: MagnifierShape,
    ) -> Result<Self, MagnifierConfigError> {
        if !width_px.is_finite() || width_px <= 0.0 || !height_px.is_finite() || height_px <= 0.0 {
            return Err(MagnifierConfigError::InvalidSize);
        }
        if !magnification.is_finite() || magnification <= 0.0 {
            return Err(MagnifierConfigError::InvalidMagnification);
        }
        Ok(Self {
            focus,
            center,
            width_px,
            height_px,
            magnification,
            shape,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MagnifierConfigError {
    InvalidSize,
    InvalidMagnification,
}

impl fmt::Display for MagnifierConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSize => formatter.write_str("magnifier size must be positive"),
            Self::InvalidMagnification => {
                formatter.write_str("magnifier magnification must be positive")
            }
        }
    }
}

impl Error for MagnifierConfigError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnnotationBufferIdentity(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedSceneResources {
    scene_revision: SceneRevision,
    image_handles: Vec<ResourceHandle>,
    annotation_buffer_identity: Option<AnnotationBufferIdentity>,
}

impl RetainedSceneResources {
    pub fn new(
        scene_revision: SceneRevision,
        image_handles: Vec<ResourceHandle>,
        annotation_buffer_identity: Option<AnnotationBufferIdentity>,
    ) -> Self {
        Self {
            scene_revision,
            image_handles,
            annotation_buffer_identity,
        }
    }

    pub const fn scene_revision(&self) -> SceneRevision {
        self.scene_revision
    }

    pub fn image_handles(&self) -> &[ResourceHandle] {
        &self.image_handles
    }

    pub const fn annotation_buffer_identity(&self) -> Option<AnnotationBufferIdentity> {
        self.annotation_buffer_identity
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MagnifierPassPlan {
    scene: RetainedSceneResources,
    config: MagnifierConfig,
}

impl MagnifierPassPlan {
    pub fn new(scene: &RetainedSceneResources, config: MagnifierConfig) -> Self {
        Self {
            scene: scene.clone(),
            config,
        }
    }

    pub const fn scene_revision(&self) -> SceneRevision {
        self.scene.scene_revision()
    }

    pub fn image_handles(&self) -> &[ResourceHandle] {
        self.scene.image_handles()
    }

    pub const fn annotation_buffer_identity(&self) -> Option<AnnotationBufferIdentity> {
        self.scene.annotation_buffer_identity()
    }

    pub const fn config(&self) -> MagnifierConfig {
        self.config
    }

    pub const fn additional_full_image_texture_bytes(&self) -> u64 {
        0
    }

    pub fn project_source(
        &self,
        transform: &TransformSnapshot,
        point: NormalizedPoint,
    ) -> LogicalPoint {
        project_source(self.config, transform, point)
    }

    pub fn clip_contains(&self, point: LogicalPoint) -> bool {
        let half_width = self.config.width_px / 2.0;
        let half_height = self.config.height_px / 2.0;
        let x = (point.x - self.config.center.x).abs();
        let y = (point.y - self.config.center.y).abs();
        match self.config.shape {
            MagnifierShape::Circle => x.hypot(y) <= half_width.min(half_height),
            MagnifierShape::RoundedRectangle => {
                let corner_radius = self.config.width_px.min(self.config.height_px) * 0.12;
                let corner_x = (x - (half_width - corner_radius)).max(0.0);
                let corner_y = (y - (half_height - corner_radius)).max(0.0);
                x <= half_width && y <= half_height && corner_x.hypot(corner_y) <= corner_radius
            }
        }
    }
}

fn project_source(
    config: MagnifierConfig,
    transform: &TransformSnapshot,
    point: NormalizedPoint,
) -> LogicalPoint {
    let source_focus = transform.image_to_view(config.focus);
    let source_point = transform.image_to_view(point);
    LogicalPoint {
        x: config.center.x + (source_point.x - source_focus.x) * config.magnification,
        y: config.center.y + (source_point.y - source_focus.y) * config.magnification,
    }
}
