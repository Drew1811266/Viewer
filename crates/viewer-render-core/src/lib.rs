#![forbid(unsafe_code)]

//! Platform-neutral image renderer state and algorithms.

mod annotation_layout;
mod cache;
mod camera;
mod geometry;
mod geometry_edit;
mod hit_test;
mod ids;
mod interaction;
mod memory;
mod pixels;
mod protocol;
mod resources;
mod scene;

pub use annotation_layout::{
    BOX_ORDINAL_CLEARANCE_PX, HANDLE_RADIUS_PX, ORDINAL_CLEARANCE_PX, ORDINAL_RADIUS_PX,
    annotation_ordinal_position, annotation_ordinal_position_projected,
};
pub use cache::{
    BudgetedLru, CacheEntry, CacheError, CacheTier, MemoryBudget, PressureLevel, ResourcePriority,
};
pub use camera::{CameraMode, CameraState, MAX_PREVIEW_ZOOM, MIN_PREVIEW_ZOOM, TransformSnapshot};
pub use geometry::{
    GeometryError, LogicalPoint, LogicalRect, LogicalSize, NormalizedPoint, NormalizedRect,
    PhysicalSize, Rotation, SourceSize, ViewportLayout,
};
pub use geometry_edit::{AnnotationHandle, annotation_handles};
pub use hit_test::{AnnotationHit, AnnotationHitPart, HitIndex};
pub use ids::{
    AnnotationId, AssetGeneration, CommandId, IdentifierError, RenderSessionId, SceneRevision,
};
pub use interaction::{
    DraftGeometry, HoverSample, InteractionController, InteractionEvent, InteractionMode,
    MagnifySample, Modifiers, NativeInput, PointerButton, PointerPhase, PointerSample,
    ScrollSample,
};
pub use memory::{
    AllocationClass, AllocationPhase, ImageMemoryCoordinator, ImageMemoryPolicy, LiveMemoryLimits,
    MemoryAdmissionError, MemoryLease, MemorySnapshot,
};
pub use pixels::SharedPixels;
pub use protocol::{RenderCommand, RenderEnvelope, RevisionDecision, RevisionGate};
pub use resources::{
    DEFAULT_TILE_BORDER, DEFAULT_TILE_SIZE, DeviceLimits, ResourceError, ResourcePlan,
    ResourcePlanner, ResourceRequest, TextureStrategy, TileCoordinate, TileRequestQueue,
};
pub use scene::{
    AnnotationGeometry, AnnotationNode, AnnotationStyle, MAX_SCENE_ANNOTATIONS, MAX_STROKE_POINTS,
    SceneError, ScenePatch, ScenePatchDisposition, ScenePatchError, SceneSnapshot,
};
