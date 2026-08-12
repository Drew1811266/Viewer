mod adapter;
mod diagnostics;
mod render_loop;
mod surface;

pub use adapter::MacOsLibmpvAdapter;
pub use diagnostics::{VideoDiagnosticsCounters, VideoRenderDiagnostics};
pub use render_loop::{MacVideoRenderSession, RenderLoopError};
pub use surface::{
    AppKitFrame, MacVideoSurface, SurfaceError, SurfaceRect, appkit_frame, backing_pixels,
};
