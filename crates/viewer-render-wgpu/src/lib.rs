#![deny(unsafe_op_in_unsafe_fn)]

//! wgpu implementation of Viewer image rendering.

mod annotation_mesh;
mod annotation_pass;
mod device;
mod diagnostics;
mod frame;
mod image_pass;
mod magnifier_pass;
mod resources;
mod scheduler;

pub use annotation_mesh::{
    AnnotationMesh, AnnotationMeshBuilder, AnnotationMeshCache, AnnotationMeshFragment,
    AnnotationVertex, BufferCapacityPlan, GlyphAtlasError, GlyphMetrics, MeshError, MeshUpdate,
    OrdinalGlyphAtlas, OrdinalLabel, VertexKind,
};
pub use annotation_pass::AnnotationPass;
pub use device::{
    RenderError, RendererDescriptor, RendererInitError, SurfaceHandles, WgpuImageRenderer,
};
pub use diagnostics::{SurfaceAcquireFailure, SurfaceRecovery, surface_recovery};
pub use frame::{
    FRAMES_IN_FLIGHT, FrameReasons, FrameReceipt, FrameRequest, FrameRing, FrameSlot, FrameState,
};
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
pub use scheduler::FrameScheduler;
