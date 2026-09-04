#![forbid(unsafe_code)]

//! Platform-neutral image renderer state and algorithms.

mod camera;
mod geometry;

pub use camera::{CameraMode, CameraState, MAX_PREVIEW_ZOOM, MIN_PREVIEW_ZOOM, TransformSnapshot};
pub use geometry::{
    GeometryError, LogicalPoint, LogicalRect, LogicalSize, NormalizedPoint, PhysicalSize, Rotation,
    SourceSize, ViewportLayout,
};
