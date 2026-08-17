#![allow(deprecated)]

use super::{
    TheaterBounds, VideoDisplayGeometry, VideoWindowAspectSession, WindowAspectError,
    diagnostics::{ResourceKind, ResourceLease},
    theater_viewport_frame,
};
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, msg_send, rc::Retained};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSBox, NSBoxType, NSColor, NSOpenGLContext, NSOpenGLPFAAccelerated,
    NSOpenGLPFADoubleBuffer, NSOpenGLPFAOpenGLProfile, NSOpenGLPixelFormat,
    NSOpenGLProfileVersion3_2Core, NSOpenGLView, NSTitlePosition, NSView, NSWindow,
    NSWindowOrderingMode,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};
use std::{
    cell::{Cell, RefCell},
    ffi::{c_char, c_void},
    ptr::NonNull,
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
    #[error("the Tauri window could not be prepared as an opaque video theater")]
    WindowBackgroundUnavailable,
    #[error("the Tauri window content view has no WKWebView")]
    MissingWebview,
    #[error(transparent)]
    WindowAspect(#[from] WindowAspectError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SurfaceUnmountState {
    Mounted,
    Restoring,
    Unmounted,
}

pub struct MacVideoSurface {
    view: Retained<NSOpenGLView>,
    theater: Retained<NSBox>,
    parent: Retained<NSView>,
    aspect_session: RefCell<Option<VideoWindowAspectSession>>,
    unmount_state: Cell<SurfaceUnmountState>,
    _lease: ResourceLease,
}

impl MacVideoSurface {
    /// Mounts the production video surface across the full native theater.
    /// The OpenGL view follows native content-view resizing while compact
    /// webview chrome overlays it; libmpv performs the media aspect fit inside
    /// that backing surface.
    pub fn mount_theater<R: Runtime>(
        window: &WebviewWindow<R>,
        media: VideoDisplayGeometry,
    ) -> Result<Self, SurfaceError> {
        if media.width == 0 || media.height == 0 {
            return Err(SurfaceError::InvalidGeometry);
        }
        Self::mount_with_frame(window, None, Some(media))
    }

    pub fn mount<R: Runtime>(
        window: &WebviewWindow<R>,
        rect: SurfaceRect,
    ) -> Result<Self, SurfaceError> {
        let frame = Some(rect);
        Self::mount_with_frame(window, frame, None)
    }

    fn mount_with_frame<R: Runtime>(
        window: &WebviewWindow<R>,
        rect: Option<SurfaceRect>,
        media: Option<VideoDisplayGeometry>,
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
        // view. The theater is inserted below it and retained by the surface.
        let webview = subviews.objectAtIndex(0);
        Self::mount_in_webview(native_window, &webview, rect, media)
    }

    fn mount_in_webview(
        window: &NSWindow,
        webview: &NSView,
        rect: Option<SurfaceRect>,
        media: Option<VideoDisplayGeometry>,
    ) -> Result<Self, SurfaceError> {
        let mtm = MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
        // SAFETY: the WKWebView is retained by its window and this method runs on
        // AppKit's main thread. objc2 retains the returned parent for this owner.
        let parent = unsafe { webview.superview() }.ok_or(SurfaceError::MissingParent)?;
        let explicit_frame = rect
            .map(|rect| {
                appkit_frame(rect, webview.bounds().size.height)
                    .map(|frame| {
                        NSRect::new(
                            NSPoint::new(frame.x, frame.y),
                            NSSize::new(frame.width, frame.height),
                        )
                    })
                    .ok_or(SurfaceError::InvalidGeometry)
            })
            .transpose()?;
        let theater_parent = parent.clone();
        mount_after_aspect_session_install(
            || {
                media
                    .map(|media| VideoWindowAspectSession::install(window, media))
                    .transpose()
                    .map_err(SurfaceError::from)
            },
            |aspect_session| {
                mount_after_opaque_theater_configuration(
                    || configure_opaque_window(window),
                    || configure_transparent_webview(webview),
                    || {
                        let theater = NSBox::initWithFrame(NSBox::alloc(mtm), webview.frame());
                        theater.setBoxType(NSBoxType::Custom);
                        theater.setTitlePosition(NSTitlePosition::NoTitle);
                        theater.setBorderWidth(0.0);
                        theater.setContentViewMargins(NSSize::new(0.0, 0.0));
                        theater.setFillColor(&theater_color());
                        theater.setAutoresizingMask(
                            NSAutoresizingMaskOptions::ViewWidthSizable
                                | NSAutoresizingMaskOptions::ViewHeightSizable,
                        );
                        theater_parent.addSubview_positioned_relativeTo(
                            &theater,
                            NSWindowOrderingMode::Below,
                            Some(webview),
                        );
                        Ok(theater)
                    },
                    move |theater| {
                        let frame = if let Some(frame) = explicit_frame {
                            frame
                        } else {
                            let bounds = theater.bounds();
                            let viewport = theater_viewport_frame(TheaterBounds {
                                width: bounds.size.width,
                                height: bounds.size.height,
                                scale_factor: window.backingScaleFactor(),
                            })
                            .ok_or(SurfaceError::InvalidGeometry)?;
                            NSRect::new(
                                NSPoint::new(viewport.x, viewport.y),
                                NSSize::new(viewport.width, viewport.height),
                            )
                        };
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
                        if explicit_frame.is_none() {
                            view.setAutoresizingMask(
                                NSAutoresizingMaskOptions::ViewWidthSizable
                                    | NSAutoresizingMaskOptions::ViewHeightSizable,
                            );
                        }
                        view.setHidden(true);
                        theater.addSubview(&view);
                        view.prepareOpenGL();
                        if view.openGLContext().is_none() {
                            view.removeFromSuperview();
                            theater.removeFromSuperview();
                            return Err(SurfaceError::OpenGlContextUnavailable);
                        }

                        Ok(Self {
                            view,
                            theater,
                            parent,
                            aspect_session: RefCell::new(aspect_session),
                            unmount_state: Cell::new(SurfaceUnmountState::Mounted),
                            _lease: ResourceLease::acquire(ResourceKind::Surface),
                        })
                    },
                )
            },
        )
    }

    pub fn update_geometry(&self, rect: SurfaceRect) -> Result<(), SurfaceError> {
        MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
        if !self.is_mounted() {
            return Ok(());
        }
        let frame = appkit_frame(rect, self.theater.bounds().size.height)
            .map(|frame| {
                NSRect::new(
                    NSPoint::new(frame.x, frame.y),
                    NSSize::new(frame.width, frame.height),
                )
            })
            .ok_or(SurfaceError::InvalidGeometry)?;
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
        unmount_after_aspect_restore(
            &self.unmount_state,
            || self.aspect_session.borrow_mut().take(),
            |aspect_session| {
                if let Some(session) = aspect_session.as_mut() {
                    session.restore()?;
                }
                Ok(())
            },
            |aspect_session| *self.aspect_session.borrow_mut() = aspect_session,
            || {
                self.view.setHidden(true);
                self.view.removeFromSuperview();
                self.theater.removeFromSuperview();
            },
        )
    }

    pub fn is_mounted(&self) -> bool {
        self.unmount_state.get() == SurfaceUnmountState::Mounted
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

#[cfg(test)]
fn mount_after_webview_transparency<T>(
    configure_transparency: impl FnOnce() -> Result<(), SurfaceError>,
    attach_surface: impl FnOnce() -> Result<T, SurfaceError>,
) -> Result<T, SurfaceError> {
    configure_transparency()?;
    attach_surface()
}

fn mount_after_aspect_session_install<S, T>(
    install_aspect_session: impl FnOnce() -> Result<S, SurfaceError>,
    attach_native_surface: impl FnOnce(S) -> Result<T, SurfaceError>,
) -> Result<T, SurfaceError> {
    let aspect_session = install_aspect_session()?;
    attach_native_surface(aspect_session)
}

fn unmount_after_aspect_restore<T>(
    state: &Cell<SurfaceUnmountState>,
    take_aspect_session: impl FnOnce() -> T,
    restore_aspect: impl FnOnce(&mut T) -> Result<(), SurfaceError>,
    reinsert_aspect_session: impl FnOnce(T),
    detach_native_surface: impl FnOnce(),
) -> Result<(), SurfaceError> {
    if state.get() != SurfaceUnmountState::Mounted {
        return Ok(());
    }

    state.set(SurfaceUnmountState::Restoring);
    let mut aspect_session = take_aspect_session();
    match restore_aspect(&mut aspect_session) {
        Ok(()) => {
            drop(aspect_session);
            detach_native_surface();
            state.set(SurfaceUnmountState::Unmounted);
            Ok(())
        }
        Err(error) => {
            if state.get() == SurfaceUnmountState::Restoring {
                reinsert_aspect_session(aspect_session);
                state.set(SurfaceUnmountState::Mounted);
            }
            Err(error)
        }
    }
}

fn mount_after_opaque_theater_configuration<T, U>(
    configure_window: impl FnOnce() -> Result<(), SurfaceError>,
    configure_webview: impl FnOnce() -> Result<(), SurfaceError>,
    attach_theater: impl FnOnce() -> Result<T, SurfaceError>,
    attach_video: impl FnOnce(T) -> Result<U, SurfaceError>,
) -> Result<U, SurfaceError> {
    configure_window()?;
    configure_webview()?;
    let theater = attach_theater()?;
    attach_video(theater)
}

fn configure_opaque_window(window: &NSWindow) -> Result<(), SurfaceError> {
    window.setOpaque(true);
    window.setBackgroundColor(Some(&theater_color()));
    if !window.isOpaque() {
        return Err(SurfaceError::WindowBackgroundUnavailable);
    }
    Ok(())
}

fn theater_color() -> Retained<NSColor> {
    let [red, green, blue, alpha] = theater_color_components();
    NSColor::colorWithSRGBRed_green_blue_alpha(red, green, blue, alpha)
}

fn theater_color_components() -> [f64; 4] {
    [245.0 / 255.0, 245.0 / 255.0, 243.0 / 255.0, 1.0]
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

    // DOM layout and AppKit window resizing do not commit in the same run-loop
    // turn. NSView frames may safely extend outside their parent and are clipped
    // by the parent; preserving the requested size avoids both a fatal surface
    // error and a one-frame aspect-ratio distortion during live resize.
    let y = container_height - rect.y - rect.height;

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
    use super::{
        SurfaceError, SurfaceUnmountState, mount_after_aspect_session_install,
        mount_after_opaque_theater_configuration, mount_after_webview_transparency,
        theater_color_components, unmount_after_aspect_restore,
    };
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

    #[test]
    fn configures_opaque_window_and_webview_before_attaching_theater_and_video() {
        let order = RefCell::new(Vec::new());

        let result = mount_after_opaque_theater_configuration(
            || {
                order.borrow_mut().push("window-opaque");
                Ok(())
            },
            || {
                order.borrow_mut().push("webview-transparent");
                Ok(())
            },
            || {
                order.borrow_mut().push("theater");
                Ok("native-theater")
            },
            |theater| {
                order.borrow_mut().push("video");
                Ok(theater)
            },
        );

        assert_eq!(result, Ok("native-theater"));
        assert_eq!(
            *order.borrow(),
            ["window-opaque", "webview-transparent", "theater", "video"]
        );
    }

    #[test]
    fn native_theater_uses_the_viewer_light_neutral_surface() {
        assert_eq!(
            theater_color_components(),
            [245.0 / 255.0, 245.0 / 255.0, 243.0 / 255.0, 1.0]
        );
    }

    #[test]
    fn theater_configuration_failure_prevents_video_attachment() {
        let video_attached = Cell::new(false);

        let result = mount_after_opaque_theater_configuration(
            || Ok(()),
            || Ok(()),
            || Err(SurfaceError::OpenGlViewUnavailable),
            |()| {
                video_attached.set(true);
                Ok(())
            },
        );

        assert_eq!(result, Err(SurfaceError::OpenGlViewUnavailable));
        assert!(!video_attached.get());
    }

    #[test]
    fn a_later_mount_failure_drops_and_restores_the_installed_aspect_session() {
        struct TestAspectSession<'a>(&'a RefCell<Vec<&'static str>>);

        impl Drop for TestAspectSession<'_> {
            fn drop(&mut self) {
                self.0.borrow_mut().push("restore-aspect");
            }
        }

        let order = RefCell::new(Vec::new());
        let result = mount_after_aspect_session_install(
            || {
                order.borrow_mut().push("install-aspect");
                Ok(TestAspectSession(&order))
            },
            |_session| {
                order.borrow_mut().push("attach-video");
                Err::<(), _>(SurfaceError::OpenGlContextUnavailable)
            },
        );

        assert_eq!(result, Err(SurfaceError::OpenGlContextUnavailable));
        assert_eq!(
            &*order.borrow(),
            &["install-aspect", "attach-video", "restore-aspect"]
        );
    }

    #[test]
    fn an_aspect_install_failure_prevents_native_attachment() {
        let attached = Cell::new(false);
        let result = mount_after_aspect_session_install(
            || Err::<(), _>(SurfaceError::InvalidGeometry),
            |()| {
                attached.set(true);
                Ok(())
            },
        );

        assert_eq!(result, Err(SurfaceError::InvalidGeometry));
        assert!(!attached.get());
    }

    #[test]
    fn unmount_restores_the_aspect_policy_before_detaching_native_views() {
        let order = RefCell::new(Vec::new());
        let state = Cell::new(SurfaceUnmountState::Mounted);
        let session = RefCell::new(Some("aspect-session"));

        unmount_after_aspect_restore(
            &state,
            || session.borrow_mut().take(),
            |_session| {
                order.borrow_mut().push("restore-aspect");
                Ok(())
            },
            |owned| *session.borrow_mut() = owned,
            || order.borrow_mut().push("detach-native-views"),
        )
        .expect("unmount");

        assert_eq!(&*order.borrow(), &["restore-aspect", "detach-native-views"]);
        assert_eq!(state.get(), SurfaceUnmountState::Unmounted);
        assert_eq!(*session.borrow(), None);
    }

    #[test]
    fn aspect_restore_failure_reinserts_ownership_and_prevents_detachment() {
        let detached = Cell::new(false);
        let state = Cell::new(SurfaceUnmountState::Mounted);
        let session = RefCell::new(Some("aspect-session"));

        let result = unmount_after_aspect_restore(
            &state,
            || session.borrow_mut().take(),
            |_session| Err(SurfaceError::InvalidGeometry),
            |owned| *session.borrow_mut() = owned,
            || detached.set(true),
        );

        assert_eq!(result, Err(SurfaceError::InvalidGeometry));
        assert!(!detached.get());
        assert_eq!(state.get(), SurfaceUnmountState::Mounted);
        assert_eq!(*session.borrow(), Some("aspect-session"));
    }

    #[test]
    fn reentrant_unmount_during_aspect_restore_is_a_safe_no_op() {
        let order = RefCell::new(Vec::new());
        let state = Cell::new(SurfaceUnmountState::Mounted);
        let session = RefCell::new(Some("aspect-session"));

        unmount_after_aspect_restore(
            &state,
            || session.borrow_mut().take(),
            |_session| {
                order.borrow_mut().push("outer-restore");
                unmount_after_aspect_restore(
                    &state,
                    || {
                        order.borrow_mut().push("reentrant-take");
                    },
                    |_owned| {
                        order.borrow_mut().push("reentrant-restore");
                        Ok(())
                    },
                    |()| order.borrow_mut().push("reentrant-reinsert"),
                    || order.borrow_mut().push("reentrant-detach"),
                )?;
                Ok(())
            },
            |owned| *session.borrow_mut() = owned,
            || order.borrow_mut().push("outer-detach"),
        )
        .expect("outer unmount");

        assert_eq!(&*order.borrow(), &["outer-restore", "outer-detach"]);
        assert_eq!(state.get(), SurfaceUnmountState::Unmounted);
        assert_eq!(*session.borrow(), None);
    }
}
