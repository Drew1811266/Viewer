#![deny(unsafe_op_in_unsafe_fn)]

//! wgpu implementation of Viewer image rendering.

mod annotation_mesh;
mod annotation_pass;
mod device;
mod diagnostics;
mod frame;
mod gpu_memory;
mod gpu_timing;
mod image_pass;
mod magnifier_pass;
mod resources;
mod retirement;
mod scheduler;
mod upload_pool;

pub use annotation_mesh::{
    AnnotationMesh, AnnotationMeshBuilder, AnnotationMeshCache, AnnotationMeshFragment,
    AnnotationMeshLayers, AnnotationVertex, BufferCapacityPlan, GlyphAtlasError, GlyphMetrics,
    MeshError, MeshUpdate, OrdinalGlyphAtlas, OrdinalLabel, VertexKind,
};
pub use annotation_pass::AnnotationPass;
pub use device::{
    RenderError, RendererDescriptor, RendererInitError, SurfaceHandles, WgpuImageRenderer,
};
pub use diagnostics::{
    ImageRendererDiagnostics, ImageRendererPerformanceReceipt, ImageRendererPerformanceWorkload,
    MeasuredImageRendererWorkload, PerformancePercentiles, SurfaceAcquireFailure, SurfaceRecovery,
    surface_recovery,
};
pub use frame::{
    FRAMES_IN_FLIGHT, FrameReasons, FrameReceipt, FrameRequest, FrameRing, FrameSlot, FrameState,
};
pub use gpu_timing::{GpuFrameTiming, GpuTimingSupport};
pub use image_pass::{
    ImagePass, ImagePassPlan, ImagePlanError, ImageTileDraw, ImageVertex, VisibleImageResource,
    VisibleResources,
};
pub use magnifier_pass::{
    AnnotationBufferIdentity, MagnifierConfig, MagnifierConfigError, MagnifierPass,
    MagnifierPassPlan, MagnifierShape, RetainedSceneResources,
};
pub use resources::{
    DecodedResource, ResourceGenerationGate, ResourceHandle, ResourceKey, ResourceRegistry,
    UploadDisposition, UploadError, UploadLayout,
};
pub use retirement::GpuRetirementService;
pub use scheduler::FrameScheduler;
