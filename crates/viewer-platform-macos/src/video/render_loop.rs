#![allow(deprecated)]

use super::{
    MacVideoSurface, SurfaceError, SurfaceRect,
    diagnostics::{ResourceKind, ResourceLease, VideoRenderDiagnostics, record_rendered_frame},
};
use objc2_app_kit::NSOpenGLContext;
use std::{path::Path, sync::Arc};
use thiserror::Error;
use viewer_video_mpv::PlaybackRate;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderedFrame {
    pub serial: u64,
    pub media_loaded: bool,
}

#[derive(Default)]
struct FrameReadiness {
    latest_rendered_frame: Option<RenderedFrame>,
    first_frame_ready: bool,
    decoded_picture_type: Option<String>,
}

impl FrameReadiness {
    fn record_rendered_frame(&mut self, frame: RenderedFrame) {
        self.latest_rendered_frame = Some(frame);
    }

    const fn latest_rendered_frame(&self) -> Option<RenderedFrame> {
        self.latest_rendered_frame
    }

    const fn first_frame_ready(&self) -> bool {
        self.first_frame_ready
    }

    fn decoded_picture_type(&self) -> Option<&str> {
        self.decoded_picture_type.as_deref()
    }

    fn confirm_first_decoded_frame(&mut self, serial: u64, picture_type: Option<String>) -> bool {
        let Some(rendered) = self.latest_rendered_frame else {
            return false;
        };
        let Some(picture_type) = picture_type else {
            return false;
        };
        if !rendered.media_loaded || rendered.serial != serial {
            return false;
        }
        self.first_frame_ready = true;
        self.decoded_picture_type = Some(picture_type);
        true
    }
}

pub struct MacVideoRenderSession {
    surface: Option<MacVideoSurface>,
    render_context: Option<MpvRenderContext>,
    client: Option<MpvClient>,
    open_gl_context: objc2::rc::Retained<NSOpenGLContext>,
    first_frame_revealed: bool,
    frame_readiness: FrameReadiness,
    auto_reveal_first_frame: bool,
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
        Self::new_with_reveal_policy(client, surface, schedule_draw, true)
    }

    pub fn new_first_frame_gated(
        client: MpvClient,
        surface: MacVideoSurface,
        schedule_draw: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, RenderLoopError> {
        Self::new_with_reveal_policy(client, surface, schedule_draw, false)
    }

    fn new_with_reveal_policy(
        client: MpvClient,
        surface: MacVideoSurface,
        schedule_draw: Arc<dyn Fn() + Send + Sync>,
        auto_reveal_first_frame: bool,
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
            frame_readiness: FrameReadiness::default(),
            auto_reveal_first_frame,
            media_loaded: false,
            _client_lease: ResourceLease::acquire(ResourceKind::Client),
            _render_context_lease: ResourceLease::acquire(ResourceKind::RenderContext),
        })
    }

    pub fn draw_if_needed(&mut self) -> Result<Option<RenderedFrame>, RenderLoopError> {
        let Some(render_context) = self.render_context.as_mut() else {
            return Ok(None);
        };
        let Some(surface) = self.surface.as_ref() else {
            return Ok(None);
        };

        self.open_gl_context.makeCurrentContext();
        let result = (|| {
            let should_draw = render_context.update();
            if should_draw {
                render_context.render(surface)?;
                self.open_gl_context.flushBuffer();
                render_context.report_swap();
                record_rendered_frame();
                let serial = self
                    .frame_readiness
                    .latest_rendered_frame()
                    .map_or(1, |frame| frame.serial.saturating_add(1));
                let rendered = RenderedFrame {
                    serial,
                    media_loaded: self.media_loaded,
                };
                self.frame_readiness.record_rendered_frame(rendered);
                return Ok(Some(rendered));
            }
            Ok(None)
        })();
        NSOpenGLContext::clearCurrentContext();
        result
    }

    pub fn update_geometry(&mut self, rect: SurfaceRect) -> Result<(), RenderLoopError> {
        if let Some(surface) = self.surface.as_ref() {
            surface.update_geometry(rect)?;
            self.open_gl_context
                .update(objc2::MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?);
        }
        Ok(())
    }

    pub fn redraw_retained_frame(&mut self) -> Result<(), RenderLoopError> {
        if !should_redraw_retained_frame(self.frame_readiness.first_frame_ready()) {
            return Ok(());
        }
        let Some(surface) = self.surface.as_ref() else {
            return Ok(());
        };
        let Some(render_context) = self.render_context.as_mut() else {
            return Ok(());
        };

        self.open_gl_context.makeCurrentContext();
        let result: Result<(), RenderLoopError> = (|| {
            render_context.render(surface)?;
            self.open_gl_context.flushBuffer();
            render_context.report_swap();
            Ok(())
        })();
        NSOpenGLContext::clearCurrentContext();
        result
    }

    pub fn diagnostics(&self) -> Result<VideoRenderDiagnostics, RenderLoopError> {
        let Some(client) = self.client.as_ref() else {
            return Ok(VideoRenderDiagnostics::snapshot("", ""));
        };
        let mut diagnostics = VideoRenderDiagnostics::snapshot(
            client.active_hardware_decoder()?.unwrap_or_default(),
            client.active_video_output()?.unwrap_or_default(),
        );
        diagnostics.mistimed_frames = client.mistimed_frame_count()?.unwrap_or(0);
        diagnostics.decoder_dropped_frames = client.decoder_frame_drop_count()?.unwrap_or(0);
        Ok(diagnostics)
    }

    pub fn playback_time_us(&self) -> Result<Option<u64>, RenderLoopError> {
        let Some(client) = self.client.as_ref() else {
            return Ok(None);
        };
        Ok(client.current_playback_time_us()?)
    }

    pub fn eof_reached(&self) -> Result<bool, RenderLoopError> {
        let Some(client) = self.client.as_ref() else {
            return Ok(false);
        };
        Ok(client.eof_reached()?.unwrap_or(false))
    }

    pub fn first_decoded_frame_revealed(&self) -> bool {
        self.first_frame_revealed
    }

    pub fn first_decoded_frame_ready(&self) -> bool {
        self.frame_readiness.first_frame_ready()
    }

    pub fn reveal_surface(&mut self) -> Result<(), RenderLoopError> {
        if self.frame_readiness.first_frame_ready()
            && !self.first_frame_revealed
            && let Some(surface) = self.surface.as_ref()
        {
            surface.reveal()?;
            self.first_frame_revealed = true;
        }
        Ok(())
    }

    pub fn decoded_picture_type(&self) -> Option<&str> {
        self.frame_readiness.decoded_picture_type()
    }

    pub fn latest_rendered_frame_serial(&self) -> Option<u64> {
        self.frame_readiness
            .latest_rendered_frame()
            .map(|frame| frame.serial)
    }

    pub fn confirm_first_decoded_frame(
        &mut self,
        serial: u64,
        picture_type: Option<String>,
    ) -> Result<bool, RenderLoopError> {
        let should_confirm = should_reveal_fixture_frame(
            self.media_loaded,
            self.first_frame_revealed,
            true,
            picture_type.is_some(),
        );
        let confirmed = should_confirm
            && self
                .frame_readiness
                .confirm_first_decoded_frame(serial, picture_type);
        if confirmed
            && self.auto_reveal_first_frame
            && !self.first_frame_revealed
            && let Some(surface) = self.surface.as_ref()
        {
            surface.reveal()?;
            self.first_frame_revealed = true;
        }
        Ok(confirmed)
    }

    pub fn sample_decoded_picture_type(&self) -> Result<Option<String>, RenderLoopError> {
        let Some(client) = self.client.as_ref() else {
            return Ok(None);
        };
        Ok(client.current_video_picture_type()?)
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

    pub fn play(&self) -> Result<(), RenderLoopError> {
        if let Some(client) = self.client.as_ref() {
            client.play()?;
        }
        Ok(())
    }

    pub fn set_volume(&self, percent: u8) -> Result<(), RenderLoopError> {
        if let Some(client) = self.client.as_ref() {
            client.set_volume_percent(percent)?;
        }
        Ok(())
    }

    pub fn set_muted(&self, muted: bool) -> Result<(), RenderLoopError> {
        if let Some(client) = self.client.as_ref() {
            client.set_muted(muted)?;
        }
        Ok(())
    }

    pub fn set_rate(&self, rate: PlaybackRate) -> Result<(), RenderLoopError> {
        if let Some(client) = self.client.as_ref() {
            client.set_rate(rate)?;
        }
        Ok(())
    }

    pub fn open_local_file_paused(&mut self, path: &Path) -> Result<(), RenderLoopError> {
        if let Some(client) = self.client.as_mut() {
            client.open_local_file_paused(path)?;
            self.media_loaded = true;
        }
        Ok(())
    }

    fn teardown(&mut self) {
        if let Some(surface) = self.surface.as_ref() {
            let _ = surface.hide();
        }
        if let Some(client) = self.client.as_ref() {
            let _ = client.pause();
            let _ = client.set_muted(true);
        }
        self.open_gl_context.makeCurrentContext();
        // Dropping the render context unregisters update callbacks before the
        // client or native surface can be destroyed.
        self.render_context.take();
        NSOpenGLContext::clearCurrentContext();
        self.client.take();
        if let Some(surface) = self.surface.as_ref() {
            let _ = surface.clear_gl_context();
            let _ = surface.unmount();
        }
        self.surface.take();
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

const fn should_redraw_retained_frame(first_frame_ready: bool) -> bool {
    first_frame_ready
}

impl Drop for MacVideoRenderSession {
    fn drop(&mut self) {
        self.teardown();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FrameReadiness, RenderedFrame, should_redraw_retained_frame, should_reveal_fixture_frame,
    };

    #[test]
    fn pre_load_render_context_wake_keeps_the_surface_hidden() {
        assert!(!should_reveal_fixture_frame(false, false, true, true));
        assert!(!should_reveal_fixture_frame(true, false, true, false));
        assert!(should_reveal_fixture_frame(true, false, true, true));
        assert!(!should_reveal_fixture_frame(true, true, true, true));
        assert!(!should_reveal_fixture_frame(true, false, false, true));
    }

    #[test]
    fn retained_frame_redraw_is_allowed_only_after_first_frame_readiness() {
        assert!(!should_redraw_retained_frame(false));
        assert!(should_redraw_retained_frame(true));
    }

    #[test]
    fn rendering_records_a_frame_without_reading_or_confirming_client_properties() {
        let mut readiness = FrameReadiness::default();
        let rendered = RenderedFrame {
            serial: 1,
            media_loaded: true,
        };

        readiness.record_rendered_frame(rendered);

        assert_eq!(readiness.latest_rendered_frame(), Some(rendered));
        assert!(!readiness.first_frame_ready());
    }

    #[test]
    fn first_frame_confirmation_requires_the_latest_rendered_serial_and_a_picture_type() {
        let mut readiness = FrameReadiness::default();
        readiness.record_rendered_frame(RenderedFrame {
            serial: 4,
            media_loaded: true,
        });

        assert!(!readiness.confirm_first_decoded_frame(3, Some("I".into())));
        assert!(!readiness.confirm_first_decoded_frame(4, None));
        assert!(readiness.confirm_first_decoded_frame(4, Some("I".into())));
        assert!(readiness.first_frame_ready());
        assert_eq!(readiness.decoded_picture_type(), Some("I"));
    }
}
