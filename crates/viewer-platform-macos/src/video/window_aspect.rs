use super::VideoDisplayGeometry;
use objc2::{MainThreadMarker, Message, rc::Retained};
use objc2_app_kit::NSWindow;
use objc2_foundation::{NSRect, NSSize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WindowAspectError {
    #[error("native window aspect operations must run on the AppKit main thread")]
    NotMainThread,
    #[error("the video display geometry cannot produce a window aspect ratio")]
    InvalidMediaGeometry,
    #[error("the native window is not associated with a visible screen")]
    MissingScreen,
    #[error("the native window or visible screen geometry is invalid")]
    InvalidWindowGeometry,
}

type RestoreAction<'a> = Box<dyn FnMut() -> Result<(), WindowAspectError> + 'a>;

struct WindowAspectLease<'a> {
    restore_action: Option<RestoreAction<'a>>,
}

impl<'a> WindowAspectLease<'a> {
    fn new(
        install: impl FnOnce() -> Result<(), WindowAspectError>,
        restore: impl FnMut() -> Result<(), WindowAspectError> + 'a,
    ) -> Result<Self, WindowAspectError> {
        let mut lease = Self {
            restore_action: Some(Box::new(restore)),
        };
        if let Err(error) = install() {
            lease.restore()?;
            return Err(error);
        }
        Ok(lease)
    }

    #[cfg(test)]
    fn new_for_test(
        install: impl FnOnce(),
        mut restore: impl FnMut() + 'a,
    ) -> Result<Self, WindowAspectError> {
        Self::new(
            || {
                install();
                Ok(())
            },
            move || {
                restore();
                Ok(())
            },
        )
    }

    fn restore(&mut self) -> Result<(), WindowAspectError> {
        let Some(restore) = self.restore_action.as_mut() else {
            return Ok(());
        };
        restore()?;
        self.restore_action.take();
        Ok(())
    }
}

impl Drop for WindowAspectLease<'_> {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

pub struct VideoWindowAspectSession {
    lease: WindowAspectLease<'static>,
}

impl VideoWindowAspectSession {
    pub fn install(
        window: &NSWindow,
        media: VideoDisplayGeometry,
    ) -> Result<Self, WindowAspectError> {
        MainThreadMarker::new().ok_or(WindowAspectError::NotMainThread)?;
        let display = display_dimensions(media).ok_or(WindowAspectError::InvalidMediaGeometry)?;
        let previous_frame = window.frame();
        let current_content_frame = window.contentRectForFrameRect(previous_frame);
        let screen = window.screen().ok_or(WindowAspectError::MissingScreen)?;
        let visible_frame = screen.visibleFrame();
        let visible_content_frame = window.contentRectForFrameRect(visible_frame);
        let fitted_content_size = best_fit_content_size(
            AspectSize::new(
                current_content_frame.size.width,
                current_content_frame.size.height,
            ),
            AspectSize::new(
                visible_content_frame.size.width,
                visible_content_frame.size.height,
            ),
            display.width / display.height,
        )
        .ok_or(WindowAspectError::InvalidWindowGeometry)?;
        let target_content_frame = NSRect::new(
            current_content_frame.origin,
            NSSize::new(fitted_content_size.width, fitted_content_size.height),
        );
        let target_frame = window.constrainFrameRect_toScreen(
            window.frameRectForContentRect(target_content_frame),
            Some(&screen),
        );
        let previous_aspect = window.contentAspectRatio();
        let installed_aspect = NSSize::new(display.width, display.height);
        let restore_window: Retained<NSWindow> = window.retain();
        let lease = WindowAspectLease::new(
            || {
                window.setContentAspectRatio(installed_aspect);
                window.setFrame_display(target_frame, true);
                Ok(())
            },
            move || {
                MainThreadMarker::new().ok_or(WindowAspectError::NotMainThread)?;
                restore_window.setContentAspectRatio(previous_aspect);
                restore_window.setFrame_display(previous_frame, true);
                Ok(())
            },
        )?;

        Ok(Self { lease })
    }

    pub fn restore(&mut self) -> Result<(), WindowAspectError> {
        self.lease.restore()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AspectSize {
    pub width: f64,
    pub height: f64,
}

impl AspectSize {
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    fn is_valid(self) -> bool {
        self.width.is_finite() && self.height.is_finite() && self.width > 0.0 && self.height > 0.0
    }
}

pub fn display_aspect(media: VideoDisplayGeometry) -> Option<f64> {
    let display = display_dimensions(media)?;
    Some(display.width / display.height)
}

fn display_dimensions(media: VideoDisplayGeometry) -> Option<AspectSize> {
    if media.width == 0 || media.height == 0 {
        return None;
    }

    let (width, height) = match media.rotation_degrees.rem_euclid(360) {
        0 | 180 => (media.width, media.height),
        90 | 270 => (media.height, media.width),
        _ => return None,
    };

    Some(AspectSize::new(f64::from(width), f64::from(height)))
}

pub fn best_fit_content_size(
    current: AspectSize,
    visible: AspectSize,
    aspect: f64,
) -> Option<AspectSize> {
    if !current.is_valid() || !visible.is_valid() || !aspect.is_finite() || aspect <= 0.0 {
        return None;
    }

    let width_bound = current.width.min(visible.width);
    let height_bound = current.height.min(visible.height);
    let (width, height) = if width_bound / height_bound > aspect {
        (height_bound * aspect, height_bound)
    } else {
        (width_bound, width_bound / aspect)
    };
    let fit = AspectSize::new(width, height);
    fit.is_valid().then_some(fit)
}

#[cfg(test)]
mod tests {
    use super::{WindowAspectError, WindowAspectLease};
    use std::cell::RefCell;

    #[test]
    fn restore_is_idempotent_and_restores_the_prior_policy_once() {
        let calls = RefCell::new(Vec::new());
        let mut lease = WindowAspectLease::new_for_test(
            || calls.borrow_mut().push("install"),
            || calls.borrow_mut().push("restore"),
        )
        .expect("install");

        lease.restore().expect("first restore");
        lease.restore().expect("second restore");

        assert_eq!(&*calls.borrow(), &["install", "restore"]);
    }

    #[test]
    fn dropping_an_active_lease_restores_the_prior_policy() {
        let calls = RefCell::new(Vec::new());
        {
            let _lease = WindowAspectLease::new_for_test(
                || calls.borrow_mut().push("install"),
                || calls.borrow_mut().push("restore"),
            )
            .expect("install");
        }

        assert_eq!(&*calls.borrow(), &["install", "restore"]);
    }

    #[test]
    fn an_install_failure_restores_any_partially_applied_policy() {
        let calls = RefCell::new(Vec::new());
        let result = WindowAspectLease::new(
            || {
                calls.borrow_mut().push("install");
                Err(WindowAspectError::InvalidWindowGeometry)
            },
            || {
                calls.borrow_mut().push("restore");
                Ok(())
            },
        );

        assert_eq!(result.err(), Some(WindowAspectError::InvalidWindowGeometry));
        assert_eq!(&*calls.borrow(), &["install", "restore"]);
    }
}
