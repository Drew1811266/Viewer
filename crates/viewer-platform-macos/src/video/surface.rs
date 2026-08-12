#![allow(deprecated)]

use super::diagnostics::{ResourceKind, ResourceLease};
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, Message, msg_send, rc::Retained};
use objc2_app_kit::{
    NSColor, NSOpenGLContext, NSOpenGLPFAAccelerated, NSOpenGLPFADoubleBuffer,
    NSOpenGLPFAOpenGLProfile, NSOpenGLPixelFormat, NSOpenGLProfileVersion3_2Core, NSOpenGLView,
    NSView, NSWindow, NSWindowOrderingMode,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};
use std::{
    ffi::{c_char, c_void},
    ptr::NonNull,
    sync::atomic::{AtomicBool, Ordering},
};
use tauri::{Runtime, WebviewWindow};
use thiserror::Error;
use viewer_video_mpv::{OpenGlInit, RenderTarget};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
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
    #[error("native video surface operations must run on the AppKit main thread")]
    NotMainThread,
    #[error("the WKWebView has no native parent view")]
    MissingParent,
    #[error("the video surface rectangle is invalid")]
    InvalidGeometry,
    #[error("AppKit could not create an OpenGL view")]
    OpenGlViewUnavailable,
    #[error("AppKit could not create an OpenGL context")]
    OpenGlContextUnavailable,
    #[error("Tauri could not provide the WKWebView on the main thread")]
    WebviewUnavailable,
    #[error("the WKWebView could not be prepared for native video transparency")]
    WebviewTransparencyUnavailable,
    #[error("the Tauri window content view has no WKWebView")]
    MissingWebview,
}

pub struct MacVideoSurface {
    view: Retained<NSOpenGLView>,
    parent: Retained<NSView>,
    webview: Retained<NSView>,
    mounted: AtomicBool,
    _lease: ResourceLease,
}

impl MacVideoSurface {
    pub fn mount<R: Runtime>(
        window: &WebviewWindow<R>,
        rect: SurfaceRect,
    ) -> Result<Self, SurfaceError> {
        MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
        let native_window = window
            .ns_window()
            .map_err(|_| SurfaceError::WebviewUnavailable)?;
        // SAFETY: Tauri documents ns_window as a valid NSWindow pointer. This
        // method is restricted to AppKit's main thread for the pointer's use.
        let native_window: &NSWindow = unsafe { &*native_window.cast() };
        let parent = native_window
            .contentView()
            .ok_or(SurfaceError::MissingParent)?;
        let subviews = parent.subviews();
        if subviews.is_empty() {
            return Err(SurfaceError::MissingWebview);
        }
        // Wry installs its WKWebView as the first child of the window content
        // view. We retain it in the surface before adding the OpenGL sibling.
        let webview = subviews.objectAtIndex(0);
        Self::mount_in_webview(&webview, rect)
    }

    fn mount_in_webview(webview: &NSView, rect: SurfaceRect) -> Result<Self, SurfaceError> {
        let mtm = MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
        // SAFETY: the WKWebView is retained by its window and this method runs on
        // AppKit's main thread. objc2 retains the returned parent for this owner.
        let parent = unsafe { webview.superview() }.ok_or(SurfaceError::MissingParent)?;
        let frame = frame_in_parent(webview, rect).ok_or(SurfaceError::InvalidGeometry)?;
        mount_after_webview_transparency(
            || configure_transparent_webview(webview),
            || {
                let mut attributes = [
                    NSOpenGLPFAOpenGLProfile,
                    NSOpenGLProfileVersion3_2Core,
                    NSOpenGLPFAAccelerated,
                    NSOpenGLPFADoubleBuffer,
                    0,
                ];
                let pixel_format = unsafe {
                    NSOpenGLPixelFormat::initWithAttributes(
                        NSOpenGLPixelFormat::alloc(),
                        NonNull::new(attributes.as_mut_ptr())
                            .expect("pixel format attributes are present"),
                    )
                }
                .ok_or(SurfaceError::OpenGlViewUnavailable)?;
                let view = NSOpenGLView::initWithFrame_pixelFormat(
                    NSOpenGLView::alloc(mtm),
                    frame,
                    Some(&pixel_format),
                )
                .ok_or(SurfaceError::OpenGlViewUnavailable)?;
                view.setWantsBestResolutionOpenGLSurface(true);
                view.setHidden(true);
                parent.addSubview_positioned_relativeTo(
                    &view,
                    NSWindowOrderingMode::Below,
                    Some(webview),
                );
                view.prepareOpenGL();
                if view.openGLContext().is_none() {
                    view.removeFromSuperview();
                    return Err(SurfaceError::OpenGlContextUnavailable);
                }

                Ok(Self {
                    view,
                    parent,
                    webview: webview.retain(),
                    mounted: AtomicBool::new(true),
                    _lease: ResourceLease::acquire(ResourceKind::Surface),
                })
            },
        )
    }

    pub fn update_geometry(&self, rect: SurfaceRect) -> Result<(), SurfaceError> {
        MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
        if !self.is_mounted() {
            return Ok(());
        }
        let frame = frame_in_parent(&self.webview, rect).ok_or(SurfaceError::InvalidGeometry)?;
        self.view.setFrame(frame);
        self.view.update();
        Ok(())
    }

    pub fn hide(&self) -> Result<(), SurfaceError> {
        MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
        self.view.setHidden(true);
        Ok(())
    }

    pub fn reveal(&self) -> Result<(), SurfaceError> {
        MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
        if self.is_mounted() {
            self.view.setHidden(false);
        }
        Ok(())
    }

    pub fn unmount(&self) -> Result<(), SurfaceError> {
        MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
        if self.mounted.swap(false, Ordering::AcqRel) {
            self.view.setHidden(true);
            self.view.removeFromSuperview();
        }
        Ok(())
    }

    pub fn is_mounted(&self) -> bool {
        self.mounted.load(Ordering::Acquire)
    }

    pub fn open_gl_context(&self) -> Result<Retained<NSOpenGLContext>, SurfaceError> {
        MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
        self.view
            .openGLContext()
            .ok_or(SurfaceError::OpenGlContextUnavailable)
    }

    pub fn open_gl_init(&self) -> OpenGlInit {
        OpenGlInit {
            get_proc_address,
            context: std::ptr::null_mut(),
        }
    }

    pub(crate) fn clear_gl_context(&self) -> Result<(), SurfaceError> {
        MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
        self.view.clearGLContext();
        Ok(())
    }

    pub fn parent(&self) -> &NSView {
        &self.parent
    }
}

fn mount_after_webview_transparency<T>(
    configure_transparency: impl FnOnce() -> Result<(), SurfaceError>,
    attach_surface: impl FnOnce() -> Result<T, SurfaceError>,
) -> Result<T, SurfaceError> {
    configure_transparency()?;
    attach_surface()
}

fn configure_transparent_webview(webview: &NSView) -> Result<(), SurfaceError> {
    if !webview.respondsToSelector(objc2::sel!(setUnderPageBackgroundColor:)) {
        return Err(SurfaceError::WebviewTransparencyUnavailable);
    }
    let clear = NSColor::clearColor();
    // SAFETY: `mount` obtains this view from Tauri/Wry's documented WKWebView
    // slot, verifies the public selector, and invokes it on AppKit's main
    // thread. The copied NSColor remains valid after this call returns.
    unsafe {
        let _: () = msg_send![webview, setUnderPageBackgroundColor: &*clear];
    }
    Ok(())
}

impl RenderTarget for MacVideoSurface {
    fn framebuffer(&self) -> i32 {
        0
    }

    fn pixel_size(&self) -> (i32, i32) {
        let backing = self.view.convertRectToBacking(self.view.bounds());
        (
            backing.size.width.round() as i32,
            backing.size.height.round() as i32,
        )
    }

    fn scale_factor(&self) -> f64 {
        self.view
            .window()
            .map(|window| window.backingScaleFactor())
            .unwrap_or(1.0)
    }
}

impl Drop for MacVideoSurface {
    fn drop(&mut self) {
        if MainThreadMarker::new().is_some() {
            let _ = self.unmount();
            let _ = self.clear_gl_context();
        } else {
            debug_assert!(
                !self.is_mounted(),
                "mounted native video surface dropped away from AppKit main thread"
            );
        }
    }
}

fn frame_in_parent(webview: &NSView, rect: SurfaceRect) -> Option<NSRect> {
    let local = appkit_frame(rect, webview.bounds().size.height)?;
    let webview_frame = webview.frame();
    Some(NSRect::new(
        NSPoint::new(
            webview_frame.origin.x + local.x,
            webview_frame.origin.y + local.y,
        ),
        NSSize::new(local.width, local.height),
    ))
}

unsafe extern "C" fn get_proc_address(_context: *mut c_void, name: *const c_char) -> *mut c_void {
    if name.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: libmpv supplies a NUL-terminated OpenGL symbol name. NSOpenGL has
    // loaded the system framework before the render context is constructed.
    unsafe { libc::dlsym(libc::RTLD_DEFAULT, name) }
}

pub fn appkit_frame(rect: SurfaceRect, container_height: f64) -> Option<AppKitFrame> {
    let values = [rect.x, rect.y, rect.width, rect.height, container_height];
    if values.iter().any(|value| !value.is_finite())
        || rect.x < 0.0
        || rect.y < 0.0
        || rect.width <= 0.0
        || rect.height <= 0.0
        || container_height <= 0.0
    {
        return None;
    }

    let y = container_height - rect.y - rect.height;
    if y < 0.0 {
        return None;
    }

    Some(AppKitFrame {
        x: rect.x,
        y,
        width: rect.width,
        height: rect.height,
    })
}

pub fn backing_pixels(size: (f64, f64), scale: f64) -> Option<(i32, i32)> {
    if !size.0.is_finite()
        || !size.1.is_finite()
        || !scale.is_finite()
        || size.0 <= 0.0
        || size.1 <= 0.0
        || scale <= 0.0
    {
        return None;
    }

    let width = (size.0 * scale).round();
    let height = (size.1 * scale).round();
    if width > i32::MAX as f64 || height > i32::MAX as f64 {
        return None;
    }
    Some((width as i32, height as i32))
}

#[cfg(test)]
mod tests {
    use super::{SurfaceError, mount_after_webview_transparency};
    use std::cell::{Cell, RefCell};

    #[test]
    fn configures_webview_transparency_before_attaching_native_surface() {
        let order = RefCell::new(Vec::new());

        let result = mount_after_webview_transparency(
            || {
                order.borrow_mut().push("transparent");
                Ok(())
            },
            || {
                order.borrow_mut().push("attach");
                Ok("mounted")
            },
        );

        assert_eq!(result, Ok("mounted"));
        assert_eq!(*order.borrow(), ["transparent", "attach"]);
    }

    #[test]
    fn transparency_failure_is_typed_and_prevents_surface_attachment() {
        let attached = Cell::new(false);

        let result = mount_after_webview_transparency(
            || Err(SurfaceError::WebviewTransparencyUnavailable),
            || {
                attached.set(true);
                Ok(())
            },
        );

        assert_eq!(result, Err(SurfaceError::WebviewTransparencyUnavailable));
        assert!(!attached.get());
    }
}
