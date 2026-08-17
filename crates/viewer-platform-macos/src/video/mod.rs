mod adapter;
mod diagnostics;
mod media_worker;
mod render_loop;
mod surface;
mod theater;

pub use adapter::MacOsLibmpvAdapter;
pub use diagnostics::{
    VideoDiagnosticsCounters, VideoInteractionDiagnostics, VideoRenderDiagnostics,
};
pub use render_loop::{MacVideoRenderSession, RenderLoopError, RenderedFrame};
pub use surface::{
    AppKitFrame, MacVideoSurface, SurfaceError, SurfaceRect, appkit_frame, backing_pixels,
};
pub use theater::{
    TheaterBounds, TheaterFrame, VideoDisplayGeometry, contain_fit_frame, theater_viewport_frame,
};
