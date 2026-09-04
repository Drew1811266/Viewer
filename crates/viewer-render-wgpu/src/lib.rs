#![deny(unsafe_op_in_unsafe_fn)]

//! wgpu implementation of Viewer image rendering.

mod device;
mod diagnostics;
mod frame;
mod image_pass;
mod resources;

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
pub use resources::{
    DecodedResource, ResourceGenerationGate, ResourceHandle, ResourceKey, ResourceRegistry,
    UploadDisposition, UploadError, UploadLayout,
};
