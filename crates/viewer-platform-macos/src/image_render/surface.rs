use std::ptr::NonNull;

use objc2::{MainThreadMarker, MainThreadOnly, msg_send, rc::Retained};
use objc2_app_kit::{NSColor, NSView, NSWindow, NSWindowOrderingMode};
use objc2_core_foundation::CGRect;
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};
use objc2_metal_kit::MTKView;
use tauri::{Runtime, WebviewWindow};
use thiserror::Error;
use viewer_render_wgpu::SurfaceHandles;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceLayout {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
    pub scale_factor: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AppKitFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SurfaceError {
    #[error("native image surface operations must run on the AppKit main thread")]
    NotMainThread,
    #[error("Tauri could not provide the native macOS window")]
    WindowUnavailable,
    #[error("the native window has no content view")]
    MissingContentView,
    #[error("the Tauri content view has no WKWebView")]
    MissingWebview,
    #[error("the WKWebView could not be prepared for native image-stage transparency")]
    WebviewTransparencyUnavailable,
    #[error("the image surface layout is invalid")]
    InvalidGeometry,
    #[error("the image surface is already unmounted")]
    Unmounted,
    #[error("Core Video display-link operation failed with status {0}")]
    DisplayLink(i32),
    #[error("the system font glyph atlas could not be created")]
    GlyphAtlasUnavailable,
}

pub struct MacImageSurface {
    view: Retained<MTKView>,
    parent: Retained<NSView>,
    webview: Retained<NSView>,
    mounted: bool,
}

impl MacImageSurface {
    pub fn mount<R: Runtime>(
        window: &WebviewWindow<R>,
        layout: SurfaceLayout,
    ) -> Result<Self, SurfaceError> {
        let mtm = MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
        let native_window = window
            .ns_window()
            .map_err(|_| SurfaceError::WindowUnavailable)?;
        // SAFETY: Tauri documents `ns_window` as a valid NSWindow pointer.
        // The main-thread marker above keeps every AppKit dereference in this
        // method on AppKit's owner thread.
        let native_window: &NSWindow = unsafe { &*native_window.cast() };
        let parent = native_window
            .contentView()
            .ok_or(SurfaceError::MissingContentView)?;
        let subviews = parent.subviews();
        if subviews.is_empty() {
            return Err(SurfaceError::MissingWebview);
        }
        let webview = subviews.objectAtIndex(0);
        let frame = ns_frame(layout, webview.bounds().size.height)?;
        let (backing_width, backing_height) =
            backing_pixels(layout).ok_or(SurfaceError::InvalidGeometry)?;
        configure_transparent_webview(&webview)?;
        let view = MTKView::initWithFrame_device(MTKView::alloc(mtm), frame, None);
        view.setPaused(true);
        view.setEnableSetNeedsDisplay(false);
        view.setAutoResizeDrawable(true);
        view.setFramebufferOnly(true);
        view.setDrawableSize(objc2_core_foundation::CGSize::new(
            f64::from(backing_width),
            f64::from(backing_height),
        ));
        view.setHidden(true);
        parent.addSubview_positioned_relativeTo(&view, NSWindowOrderingMode::Below, Some(&webview));
        Ok(Self {
            view,
            parent,
            webview,
            mounted: true,
        })
    }

    pub fn set_layout(&self, layout: SurfaceLayout) -> Result<(), SurfaceError> {
        MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
        if !self.mounted {
            return Err(SurfaceError::Unmounted);
        }
        let frame = ns_frame(layout, self.webview.bounds().size.height)?;
        self.view.setFrame(frame);
        self.view
            .setDrawableSize(objc2_core_foundation::CGSize::new(
                layout.width * layout.scale_factor,
                layout.height * layout.scale_factor,
            ));
        Ok(())
    }

    pub fn set_visible(&self, visible: bool) -> Result<(), SurfaceError> {
        MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
        if !self.mounted {
            return Err(SurfaceError::Unmounted);
        }
        self.view.setHidden(!visible);
        Ok(())
    }

    pub fn unmount(&mut self) -> Result<(), SurfaceError> {
        MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
        if !self.mounted {
            return Ok(());
        }
        self.view.setHidden(true);
        self.view.removeFromSuperview();
        self.mounted = false;
        Ok(())
    }

    pub const fn is_mounted(&self) -> bool {
        self.mounted
    }

    pub fn view(&self) -> &MTKView {
        &self.view
    }

    pub fn renderer_surface_handles(&self) -> Result<SurfaceHandles, SurfaceError> {
        MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
        if !self.mounted {
            return Err(SurfaceError::Unmounted);
        }
        let ns_view = NonNull::from(&*self.view).cast();
        // SAFETY: `view` is a retained MTKView (and therefore an NSView), this
        // method is gated to AppKit's main thread, and SurfaceHandles converts
        // it immediately to an independently retained CAMetalLayer owner.
        Ok(unsafe { SurfaceHandles::from_appkit_view(ns_view) })
    }

    pub fn display_id(&self) -> Result<Option<u32>, SurfaceError> {
        MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
        if !self.mounted {
            return Err(SurfaceError::Unmounted);
        }
        Ok(self
            .view
            .window()
            .and_then(|window| window.screen())
            .map(|screen| screen.CGDirectDisplayID())
            .filter(|display_id| *display_id != 0))
    }

    pub fn parent(&self) -> &NSView {
        &self.parent
    }
}

fn configure_transparent_webview(webview: &NSView) -> Result<(), SurfaceError> {
    if !webview.respondsToSelector(objc2::sel!(setUnderPageBackgroundColor:)) {
        return Err(SurfaceError::WebviewTransparencyUnavailable);
    }
    let clear = NSColor::clearColor();
    // SAFETY: `mount` reads this object from Wry's WKWebView slot, checks the
    // public selector, and calls it on AppKit's main thread. React keeps the
    // toolbar and side panels opaque; only the stage uses a transparent fill.
    unsafe {
        let _: () = msg_send![webview, setUnderPageBackgroundColor: &*clear];
    }
    Ok(())
}

impl Drop for MacImageSurface {
    fn drop(&mut self) {
        if MainThreadMarker::new().is_some() {
            let _ = self.unmount();
        } else {
            debug_assert!(
                !self.mounted,
                "mounted native image surface dropped away from AppKit main thread"
            );
        }
    }
}

fn ns_frame(layout: SurfaceLayout, container_height: f64) -> Result<CGRect, SurfaceError> {
    let frame = appkit_frame(layout, container_height).ok_or(SurfaceError::InvalidGeometry)?;
    Ok(NSRect::new(
        NSPoint::new(frame.x, frame.y),
        NSSize::new(frame.width, frame.height),
    ))
}

pub fn appkit_frame(layout: SurfaceLayout, container_height: f64) -> Option<AppKitFrame> {
    let values = [
        layout.left,
        layout.top,
        layout.width,
        layout.height,
        layout.scale_factor,
        container_height,
    ];
    if values.iter().any(|value| !value.is_finite())
        || layout.left < 0.0
        || layout.top < 0.0
        || layout.width <= 0.0
        || layout.height <= 0.0
        || layout.scale_factor <= 0.0
        || container_height <= 0.0
    {
        return None;
    }
    Some(AppKitFrame {
        x: layout.left,
        y: container_height - layout.top - layout.height,
        width: layout.width,
        height: layout.height,
    })
}

pub fn backing_pixels(layout: SurfaceLayout) -> Option<(u32, u32)> {
    let values = [layout.width, layout.height, layout.scale_factor];
    if values.iter().any(|value| !value.is_finite())
        || layout.width <= 0.0
        || layout.height <= 0.0
        || layout.scale_factor <= 0.0
    {
        return None;
    }
    let width = (layout.width * layout.scale_factor).round();
    let height = (layout.height * layout.scale_factor).round();
    if width <= 0.0 || height <= 0.0 || width > u32::MAX as f64 || height > u32::MAX as f64 {
        return None;
    }
    Some((width as u32, height as u32))
}
