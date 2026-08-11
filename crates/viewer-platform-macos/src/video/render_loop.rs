#![allow(deprecated)]

use super::{
    MacVideoSurface, SurfaceError, SurfaceRect,
    diagnostics::{ResourceKind, ResourceLease, VideoRenderDiagnostics, record_rendered_frame},
};
use objc2_app_kit::NSOpenGLContext;
use std::{path::Path, sync::Arc};
use thiserror::Error;
use viewer_video_mpv::{FrameDirection, MpvClient, MpvError, MpvRenderContext, MpvRenderError};

#[derive(Debug, Error)]
pub enum RenderLoopError {
    #[error(transparent)]
    Surface(#[from] SurfaceError),
    #[error(transparent)]
    Mpv(#[from] MpvRenderError),
    #[error(transparent)]
    Client(#[from] MpvError),
}

pub struct MacVideoRenderSession {
    surface: Option<MacVideoSurface>,
    render_context: Option<MpvRenderContext>,
    client: Option<MpvClient>,
    open_gl_context: objc2::rc::Retained<NSOpenGLContext>,
    first_frame_revealed: bool,
    decoded_picture_type: Option<String>,
    media_loaded: bool,
    _client_lease: ResourceLease,
    _render_context_lease: ResourceLease,
}

impl MacVideoRenderSession {
    /// Creates a main-thread OpenGL render session for an initialized client.
    ///
    /// `schedule_draw` is called from libmpv's update thread and must only
    /// enqueue work onto AppKit's main thread. It must not draw directly.
    pub fn new(
        client: MpvClient,
        surface: MacVideoSurface,
        schedule_draw: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, RenderLoopError> {
        let open_gl_context = surface.open_gl_context()?;
        open_gl_context.makeCurrentContext();
        let render_context =
            unsafe { MpvRenderContext::new_opengl(&client, surface.open_gl_init(), schedule_draw) };
        NSOpenGLContext::clearCurrentContext();
        let render_context = render_context?;

        Ok(Self {
            surface: Some(surface),
            render_context: Some(render_context),
            client: Some(client),
            open_gl_context,
            first_frame_revealed: false,
            decoded_picture_type: None,
            media_loaded: false,
            _client_lease: ResourceLease::acquire(ResourceKind::Client),
            _render_context_lease: ResourceLease::acquire(ResourceKind::RenderContext),
        })
    }

    pub fn draw_if_needed(&mut self) -> Result<bool, RenderLoopError> {
        let Some(render_context) = self.render_context.as_mut() else {
            return Ok(false);
        };
        let Some(surface) = self.surface.as_ref() else {
            return Ok(false);
        };

        self.open_gl_context.makeCurrentContext();
        let result = (|| {
            let should_draw = render_context.update();
            if should_draw {
                render_context.render(surface)?;
                self.open_gl_context.flushBuffer();
                render_context.report_swap();
                record_rendered_frame();
                let decoded_picture_type = self
                    .client
                    .as_ref()
                    .map(MpvClient::current_video_picture_type)
                    .transpose()?
                    .flatten();
                if should_reveal_fixture_frame(
                    self.media_loaded,
                    self.first_frame_revealed,
                    should_draw,
                    decoded_picture_type.is_some(),
                ) {
                    surface.reveal()?;
                    self.first_frame_revealed = true;
                    self.decoded_picture_type = decoded_picture_type;
                }
            }
            Ok(should_draw)
        })();
        NSOpenGLContext::clearCurrentContext();
        result
    }

    pub fn update_geometry(&self, rect: SurfaceRect) -> Result<(), RenderLoopError> {
        if let Some(surface) = self.surface.as_ref() {
            surface.update_geometry(rect)?;
            self.open_gl_context
                .update(objc2::MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?);
        }
        Ok(())
    }

    pub fn diagnostics(&self) -> Result<VideoRenderDiagnostics, RenderLoopError> {
        let Some(client) = self.client.as_ref() else {
            return Ok(VideoRenderDiagnostics::snapshot("", ""));
        };
        Ok(VideoRenderDiagnostics::snapshot(
            client.active_hardware_decoder()?.unwrap_or_default(),
            client.active_video_output()?.unwrap_or_default(),
        ))
    }

    pub fn playback_time_us(&self) -> Result<Option<u64>, RenderLoopError> {
        let Some(client) = self.client.as_ref() else {
            return Ok(None);
        };
        Ok(client.current_playback_time_us()?)
    }

    pub fn first_decoded_frame_revealed(&self) -> bool {
        self.first_frame_revealed
    }

    pub fn decoded_picture_type(&self) -> Option<&str> {
        self.decoded_picture_type.as_deref()
    }

    pub fn frame_step(&self, direction: FrameDirection) -> Result<(), RenderLoopError> {
        if let Some(client) = self.client.as_ref() {
            client.pause()?;
            client.frame_step(direction)?;
        }
        Ok(())
    }

    pub fn pause(&self) -> Result<(), RenderLoopError> {
        if let Some(client) = self.client.as_ref() {
            client.pause()?;
        }
        Ok(())
    }

    pub fn open_local_file(&mut self, path: &Path) -> Result<(), RenderLoopError> {
        if let Some(client) = self.client.as_mut() {
            client.open_local_file(path)?;
            self.media_loaded = true;
        }
        Ok(())
    }

    fn teardown(&mut self) {
        if let Some(surface) = self.surface.as_ref() {
            let _ = surface.hide();
            let _ = surface.unmount();
        }
        self.open_gl_context.makeCurrentContext();
        self.render_context.take();
        NSOpenGLContext::clearCurrentContext();
        if let Some(surface) = self.surface.as_ref() {
            let _ = surface.clear_gl_context();
        }
        self.surface.take();
        self.client.take();
    }
}

fn should_reveal_fixture_frame(
    media_loaded: bool,
    first_frame_revealed: bool,
    drew_frame: bool,
    decoded_video_frame: bool,
) -> bool {
    media_loaded && !first_frame_revealed && drew_frame && decoded_video_frame
}

impl Drop for MacVideoRenderSession {
    fn drop(&mut self) {
        self.teardown();
    }
}

#[cfg(test)]
mod tests {
    use super::should_reveal_fixture_frame;

    #[test]
    fn pre_load_render_context_wake_keeps_the_surface_hidden() {
        assert!(!should_reveal_fixture_frame(false, false, true, true));
        assert!(!should_reveal_fixture_frame(true, false, true, false));
        assert!(should_reveal_fixture_frame(true, false, true, true));
        assert!(!should_reveal_fixture_frame(true, true, true, true));
        assert!(!should_reveal_fixture_frame(true, false, false, true));
    }
}
