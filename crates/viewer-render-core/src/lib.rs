#![forbid(unsafe_code)]

//! Platform-neutral image renderer state and algorithms.

mod camera;
mod geometry;
mod hit_test;
mod ids;
mod interaction;
mod protocol;
mod scene;

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
pub use scene::{
    AnnotationGeometry, AnnotationNode, AnnotationStyle, MAX_SCENE_ANNOTATIONS, MAX_STROKE_POINTS,
    SceneError, ScenePatch, ScenePatchDisposition, ScenePatchError, SceneSnapshot,
};
