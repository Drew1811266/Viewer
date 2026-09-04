#![forbid(unsafe_code)]

//! Platform-neutral image renderer state and algorithms.

mod cache;
mod camera;
mod geometry;
mod hit_test;
mod ids;
mod interaction;
mod protocol;
mod resources;
mod scene;

pub use cache::{
    BudgetedLru, CacheEntry, CacheError, CacheTier, MemoryBudget, PressureLevel, ResourcePriority,
};
pub use camera::{CameraMode, CameraState, MAX_PREVIEW_ZOOM, MIN_PREVIEW_ZOOM, TransformSnapshot};
pub use geometry::{
    GeometryError, LogicalPoint, LogicalRect, LogicalSize, NormalizedPoint, NormalizedRect,
    PhysicalSize, Rotation, SourceSize, ViewportLayout,
};
pub use hit_test::HitIndex;
pub use ids::{
    AnnotationId, AssetGeneration, CommandId, IdentifierError, RenderSessionId, SceneRevision,
};
pub use interaction::{
    DraftGeometry, InteractionController, InteractionEvent, InteractionMode, MagnifySample,
    Modifiers, NativeInput, PointerButton, PointerPhase, PointerSample, ScrollSample,
};
pub use protocol::{RenderCommand, RenderEnvelope, RevisionDecision, RevisionGate};
pub use resources::{
    DEFAULT_TILE_SIZE, DeviceLimits, ResourceError, ResourcePlan, ResourcePlanner, ResourceRequest,
    TextureStrategy, TileCoordinate, TileRequestQueue,
};
pub use scene::{
    AnnotationGeometry, AnnotationNode, AnnotationStyle, MAX_SCENE_ANNOTATIONS, MAX_STROKE_POINTS,
    SceneError, ScenePatch, ScenePatchDisposition, ScenePatchError, SceneSnapshot,
};
