use super::VideoDisplayGeometry;
use objc2::{MainThreadMarker, Message, rc::Retained};
use objc2_app_kit::NSWindow;
use objc2_foundation::{NSPoint, NSRect, NSSize};
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
        validated_aspect_rect(previous_frame).ok_or(WindowAspectError::InvalidWindowGeometry)?;
        let current_content_frame = window.contentRectForFrameRect(previous_frame);
        let current_content = validated_aspect_rect(current_content_frame)
            .ok_or(WindowAspectError::InvalidWindowGeometry)?;
        let screen = window.screen().ok_or(WindowAspectError::MissingScreen)?;
        let visible_frame = screen.visibleFrame();
        let visible =
            validated_aspect_rect(visible_frame).ok_or(WindowAspectError::InvalidWindowGeometry)?;
        let visible_content_frame = window.contentRectForFrameRect(visible_frame);
        let visible_content = validated_aspect_rect(visible_content_frame)
            .ok_or(WindowAspectError::InvalidWindowGeometry)?;
        let fitted_content_size = best_fit_content_size(
            AspectSize::new(current_content.width, current_content.height),
            AspectSize::new(visible_content.width, visible_content.height),
            display.width / display.height,
        )
        .ok_or(WindowAspectError::InvalidWindowGeometry)?;
        let target_content = AspectRect::new(
            current_content.x,
            current_content.y,
            fitted_content_size.width,
            fitted_content_size.height,
        );
        if !target_content.is_valid() {
            return Err(WindowAspectError::InvalidWindowGeometry);
        }
        let converted_target_frame = window.frameRectForContentRect(appkit_rect(target_content));
        let converted_target = validated_aspect_rect(converted_target_frame)
            .ok_or(WindowAspectError::InvalidWindowGeometry)?;
        let placed_target = place_frame_inside_visible(converted_target, visible)
            .ok_or(WindowAspectError::InvalidWindowGeometry)?;
        let target_frame = appkit_rect(placed_target);
        validated_aspect_rect(target_frame).ok_or(WindowAspectError::InvalidWindowGeometry)?;
        let previous_aspect = window.contentAspectRatio();
        let previous_resize_increments = window.contentResizeIncrements();
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
                restore_content_sizing_policy(
                    AspectSize::new(previous_aspect.width, previous_aspect.height),
                    AspectSize::new(
                        previous_resize_increments.width,
                        previous_resize_increments.height,
                    ),
                    |aspect| {
                        restore_window
                            .setContentAspectRatio(NSSize::new(aspect.width, aspect.height));
                    },
                    |increments| {
                        restore_window.setContentResizeIncrements(NSSize::new(
                            increments.width,
                            increments.height,
                        ));
                    },
                );
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

fn validated_aspect_rect(rect: NSRect) -> Option<AspectRect> {
    let rect = AspectRect::new(
        rect.origin.x,
        rect.origin.y,
        rect.size.width,
        rect.size.height,
    );
    rect.is_valid().then_some(rect)
}

fn appkit_rect(rect: AspectRect) -> NSRect {
    NSRect::new(
        NSPoint::new(rect.x, rect.y),
        NSSize::new(rect.width, rect.height),
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AspectSize {
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AspectRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl AspectRect {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn is_valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
    }
}

impl AspectSize {
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    fn is_valid(self) -> bool {
        self.width.is_finite() && self.height.is_finite() && self.width > 0.0 && self.height > 0.0
    }
}

fn restore_content_sizing_policy(
    previous_aspect: AspectSize,
    previous_resize_increments: AspectSize,
    set_aspect: impl FnOnce(AspectSize),
    set_resize_increments: impl FnOnce(AspectSize),
) {
    if previous_aspect.is_valid() {
        set_aspect(previous_aspect);
    } else {
        set_resize_increments(previous_resize_increments);
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

pub fn place_frame_inside_visible(frame: AspectRect, visible: AspectRect) -> Option<AspectRect> {
    if !frame.is_valid()
        || !visible.is_valid()
        || frame.width > visible.width
        || frame.height > visible.height
    {
        return None;
    }

    let visible_max_x = visible.x + visible.width;
    let visible_max_y = visible.y + visible.height;
    if !visible_max_x.is_finite() || !visible_max_y.is_finite() {
        return None;
    }
    let max_origin_x = visible_max_x - frame.width;
    let max_origin_y = visible_max_y - frame.height;
    let placed = AspectRect::new(
        frame.x.clamp(visible.x, max_origin_x),
        frame.y.clamp(visible.y, max_origin_y),
        frame.width,
        frame.height,
    );
    placed.is_valid().then_some(placed)
}

#[cfg(test)]
mod tests {
    use super::{AspectSize, WindowAspectError, WindowAspectLease, restore_content_sizing_policy};
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

    #[test]
    fn an_unconstrained_window_is_restored_without_calling_the_zero_aspect_setter() {
        let calls = RefCell::new(Vec::new());

        restore_content_sizing_policy(
            AspectSize::new(0.0, 0.0),
            AspectSize::new(1.0, 1.0),
            |aspect| calls.borrow_mut().push(("aspect", aspect)),
            |increments| calls.borrow_mut().push(("increments", increments)),
        );

        assert_eq!(
            &*calls.borrow(),
            &[("increments", AspectSize::new(1.0, 1.0))]
        );
    }
}
