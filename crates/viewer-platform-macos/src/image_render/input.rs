use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

use block2::RcBlock;
use objc2::{MainThreadMarker, rc::Retained, runtime::AnyObject};
use objc2_app_kit::{
    NSEvent, NSEventMask, NSEventModifierFlags, NSEventType, NSView, NSWindow,
    NSWindowDidBecomeKeyNotification, NSWindowDidResignKeyNotification,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSOperationQueue, NSPoint};
use thiserror::Error;
use viewer_render_core::{
    HoverSample, InteractionMode, LogicalPoint, MagnifySample, Modifiers, NativeInput,
    PointerButton, PointerPhase, PointerSample, ScrollSample,
};

use super::SurfaceError;

const SPACE_KEY_CODE: u16 = 49;
const ESCAPE_KEY_CODE: u16 = 53;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputRect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

impl InputRect {
    pub fn new(left: f64, top: f64, width: f64, height: f64) -> Result<Self, InputRoutingError> {
        let values = [left, top, width, height, left + width, top + height];
        if values.iter().any(|value| !value.is_finite())
            || left < 0.0
            || top < 0.0
            || width <= 0.0
            || height <= 0.0
        {
            return Err(InputRoutingError::InvalidGeometry);
        }
        Ok(Self {
            left,
            top,
            width,
            height,
        })
    }

    fn contains(self, point: LogicalPoint) -> bool {
        point.x.is_finite()
            && point.y.is_finite()
            && point.x >= self.left
            && point.x < self.left + self.width
            && point.y >= self.top
            && point.y < self.top + self.height
    }

    fn local_point(self, point: LogicalPoint) -> LogicalPoint {
        LogicalPoint {
            x: point.x - self.left,
            y: point.y - self.top,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputExclusionRect(InputRect);

impl InputExclusionRect {
    pub fn new(left: f64, top: f64, width: f64, height: f64) -> Result<Self, InputRoutingError> {
        InputRect::new(left, top, width, height).map(Self)
    }

    fn contains(self, point: LogicalPoint) -> bool {
        self.0.contains(point)
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum InputRoutingError {
    #[error("native input geometry must be finite and positive")]
    InvalidGeometry,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowInput {
    Pointer {
        phase: PointerPhase,
        location: LogicalPoint,
        button: PointerButton,
        pressure: f64,
        shift: bool,
        timestamp_ns: u64,
    },
    Scroll {
        location: LogicalPoint,
        delta: LogicalPoint,
        timestamp_ns: u64,
    },
    Magnify {
        location: LogicalPoint,
        factor: f64,
        timestamp_ns: u64,
    },
    Space {
        pressed: bool,
    },
    Escape,
    FocusChanged {
        focused: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RouteDecision {
    Render(NativeInput),
    Observe(NativeInput),
    WebView,
    Ignore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CaptureOwner {
    Renderer,
    WebView,
}

#[derive(Debug)]
pub struct MacInputRouter {
    surface: InputRect,
    exclusions: Vec<InputExclusionRect>,
    tool: InteractionMode,
    capture: Option<CaptureOwner>,
    space_held: bool,
    focused: bool,
}

impl MacInputRouter {
    pub const fn new(
        surface: InputRect,
        exclusions: Vec<InputExclusionRect>,
        tool: InteractionMode,
    ) -> Self {
        Self {
            surface,
            exclusions,
            tool,
            capture: None,
            space_held: false,
            focused: true,
        }
    }

    pub fn classify(&mut self, input: WindowInput) -> RouteDecision {
        match input {
            WindowInput::FocusChanged { focused } => self.focus_changed(focused),
            WindowInput::Escape => self.escape(),
            WindowInput::Space { pressed } => {
                if self.focused {
                    self.space_held = pressed;
                    RouteDecision::WebView
                } else {
                    RouteDecision::Ignore
                }
            }
            _ if !self.focused => RouteDecision::Ignore,
            WindowInput::Pointer {
                phase,
                location,
                button,
                pressure,
                shift,
                timestamp_ns,
            } => self.pointer(phase, location, button, pressure, shift, timestamp_ns),
            WindowInput::Scroll {
                location,
                delta,
                timestamp_ns,
            } => {
                if !delta.x.is_finite() || !delta.y.is_finite() {
                    return RouteDecision::Ignore;
                }
                let route = self.route_at(location);
                self.route_native(
                    route,
                    NativeInput::Scroll(ScrollSample {
                        location: self.surface.local_point(location),
                        delta,
                        modifiers: Modifiers {
                            space: self.space_held,
                            ..Modifiers::default()
                        },
                        timestamp_ns,
                    }),
                )
            }
            WindowInput::Magnify {
                location,
                factor,
                timestamp_ns,
            } => {
                if !factor.is_finite() || factor <= 0.0 {
                    return RouteDecision::Ignore;
                }
                let route = self.route_at(location);
                self.route_native(
                    route,
                    NativeInput::Magnify(MagnifySample {
                        location: self.surface.local_point(location),
                        factor,
                        timestamp_ns,
                    }),
                )
            }
        }
    }

    pub fn set_surface(&mut self, surface: InputRect) {
        self.surface = surface;
    }

    pub fn set_exclusions(&mut self, exclusions: Vec<InputExclusionRect>) {
        self.exclusions = exclusions;
    }

    pub fn set_tool(&mut self, tool: InteractionMode) -> Option<NativeInput> {
        if self.tool == tool {
            return None;
        }
        let cancellation = self.cancel_capture();
        self.tool = tool;
        cancellation
    }

    pub const fn tool(&self) -> InteractionMode {
        self.tool
    }

    pub fn cancel(&mut self) -> Option<NativeInput> {
        let cancellation = self.cancel_capture();
        self.space_held = false;
        cancellation
    }

    fn cancel_capture(&mut self) -> Option<NativeInput> {
        let owner = self.capture.take();
        (owner == Some(CaptureOwner::Renderer)).then_some(NativeInput::Cancel)
    }

    fn pointer(
        &mut self,
        phase: PointerPhase,
        location: LogicalPoint,
        button: PointerButton,
        pressure: f64,
        shift: bool,
        timestamp_ns: u64,
    ) -> RouteDecision {
        if phase == PointerPhase::Cancel {
            return match self.capture.take() {
                Some(CaptureOwner::Renderer) => RouteDecision::Render(NativeInput::Cancel),
                Some(CaptureOwner::WebView) => RouteDecision::WebView,
                None => RouteDecision::Ignore,
            };
        }

        let route = match (phase, self.capture) {
            (PointerPhase::Down, None) if button == PointerButton::Primary => {
                let owner = self.route_at(location);
                self.capture = Some(owner);
                owner
            }
            (PointerPhase::Down, None) => CaptureOwner::WebView,
            (_, Some(owner)) => owner,
            (_, None) => self.route_at(location),
        };
        let native = NativeInput::Pointer(PointerSample {
            phase,
            location: self.surface.local_point(location),
            button,
            pressure: if pressure.is_finite() {
                pressure.clamp(0.0, 1.0)
            } else {
                0.0
            },
            modifiers: Modifiers {
                space: self.space_held,
                shift,
            },
            timestamp_ns,
        });
        let decision = if phase == PointerPhase::Move && route == CaptureOwner::WebView {
            RouteDecision::Observe(NativeInput::Hover(HoverSample {
                location: self.surface.local_point(location),
                active: false,
                timestamp_ns,
            }))
        } else {
            self.route_native(route, native)
        };
        if phase == PointerPhase::Up && button == PointerButton::Primary {
            self.capture = None;
        }
        decision
    }

    fn focus_changed(&mut self, focused: bool) -> RouteDecision {
        self.focused = focused;
        if focused {
            return RouteDecision::Ignore;
        }
        self.cancel()
            .map(RouteDecision::Render)
            .unwrap_or(RouteDecision::Ignore)
    }

    fn escape(&mut self) -> RouteDecision {
        if !self.focused {
            return RouteDecision::Ignore;
        }
        self.cancel()
            .map(RouteDecision::Render)
            .unwrap_or(RouteDecision::WebView)
    }

    fn route_at(&self, location: LogicalPoint) -> CaptureOwner {
        if self.surface.contains(location)
            && !self
                .exclusions
                .iter()
                .any(|exclusion| exclusion.contains(location))
        {
            CaptureOwner::Renderer
        } else {
            CaptureOwner::WebView
        }
    }

    fn route_native(&self, owner: CaptureOwner, input: NativeInput) -> RouteDecision {
        match owner {
            CaptureOwner::Renderer => RouteDecision::Render(input),
            CaptureOwner::WebView => RouteDecision::WebView,
        }
    }
}

pub type NativeInputSink = Arc<dyn Fn(NativeInput) + Send + Sync + 'static>;

struct MonitorState {
    router: MacInputRouter,
}

pub struct MacInputMonitor {
    event_monitor: Option<Retained<AnyObject>>,
    focus_observers: Vec<Retained<AnyObject>>,
    state: Arc<Mutex<MonitorState>>,
    sink: NativeInputSink,
    stopped: bool,
}

impl MacInputMonitor {
    pub fn install(
        window: &NSWindow,
        webview: Retained<NSView>,
        surface: InputRect,
        exclusions: Vec<InputExclusionRect>,
        tool: InteractionMode,
        sink: NativeInputSink,
    ) -> Result<Self, SurfaceError> {
        MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;

        let state = Arc::new(Mutex::new(MonitorState {
            router: MacInputRouter::new(surface, exclusions, tool),
        }));
        let window_number = window.windowNumber();
        let event_state = Arc::clone(&state);
        let event_sink = Arc::clone(&sink);
        let event_block = RcBlock::new(move |event_pointer: NonNull<NSEvent>| {
            let original = event_pointer.as_ptr();
            // SAFETY: AppKit invokes a local event monitor with a live NSEvent
            // on the main event thread for the duration of this callback.
            let event = unsafe { event_pointer.as_ref() };
            if event.windowNumber() != window_number {
                return original;
            }
            let Ok(mut state) = event_state.lock() else {
                return original;
            };
            let Some(input) = window_input_from_event(event, &webview) else {
                return original;
            };
            match state.router.classify(input) {
                RouteDecision::Render(input) => {
                    drop(state);
                    event_sink(input);
                    std::ptr::null_mut()
                }
                RouteDecision::Observe(input) => {
                    drop(state);
                    event_sink(input);
                    original
                }
                RouteDecision::WebView | RouteDecision::Ignore => original,
            }
        });
        let mask = NSEventMask::LeftMouseDown
            | NSEventMask::LeftMouseUp
            | NSEventMask::MouseMoved
            | NSEventMask::LeftMouseDragged
            | NSEventMask::ScrollWheel
            | NSEventMask::Magnify
            | NSEventMask::KeyDown
            | NSEventMask::KeyUp
            | NSEventMask::MouseCancelled;
        // SAFETY: The callback returns either the same live NSEvent supplied by
        // AppKit or null to consume it. Its captures are owned by the copied
        // block until the monitor is removed on the main thread.
        let event_monitor =
            unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &event_block) }
                .ok_or(SurfaceError::InputMonitorUnavailable)?;

        let center = NSNotificationCenter::defaultCenter();
        let queue = NSOperationQueue::mainQueue();
        let mut focus_observers = Vec::with_capacity(2);
        // SAFETY: These are immutable AppKit notification-name constants and
        // are read while the framework is initialized on the main thread.
        let focus_notifications = unsafe {
            [
                (NSWindowDidResignKeyNotification, false),
                (NSWindowDidBecomeKeyNotification, true),
            ]
        };
        for (notification, focused) in focus_notifications {
            let focus_state = Arc::clone(&state);
            let focus_sink = Arc::clone(&sink);
            let focus_block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
                let decision = focus_state
                    .lock()
                    .map(|mut state| state.router.classify(WindowInput::FocusChanged { focused }))
                    .unwrap_or(RouteDecision::Ignore);
                if let RouteDecision::Render(input) = decision {
                    focus_sink(input);
                }
            });
            // SAFETY: The observed object is the retained window that owns the
            // host, delivery is pinned to the main queue, and the sendable block
            // owns all captured state until explicit observer removal.
            let observer = unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(notification),
                    Some(window),
                    Some(&queue),
                    &focus_block,
                )
            };
            focus_observers.push(observer.into());
        }

        Ok(Self {
            event_monitor: Some(event_monitor),
            focus_observers,
            state,
            sink,
            stopped: false,
        })
    }

    pub fn update_geometry(&self, surface: InputRect) -> Result<(), SurfaceError> {
        MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| SurfaceError::InputMonitorUnavailable)?;
            state.router.set_surface(surface);
        }
        Ok(())
    }

    pub fn set_exclusions(&self, exclusions: Vec<InputExclusionRect>) -> Result<(), SurfaceError> {
        MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
        self.state
            .lock()
            .map_err(|_| SurfaceError::InputMonitorUnavailable)?
            .router
            .set_exclusions(exclusions);
        Ok(())
    }

    pub fn set_tool(&self, tool: InteractionMode) -> Result<(), SurfaceError> {
        MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
        let cancellation = self
            .state
            .lock()
            .map_err(|_| SurfaceError::InputMonitorUnavailable)?
            .router
            .set_tool(tool);
        if let Some(input) = cancellation {
            (self.sink)(input);
        }
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), SurfaceError> {
        MainThreadMarker::new().ok_or(SurfaceError::NotMainThread)?;
        if self.stopped {
            return Ok(());
        }
        let cancellation = self
            .state
            .lock()
            .ok()
            .and_then(|mut state| state.router.cancel());
        if let Some(input) = cancellation {
            (self.sink)(input);
        }
        if let Some(monitor) = self.event_monitor.take() {
            // SAFETY: This is the exact token returned by the AppKit local
            // event-monitor registration and removal runs on the main thread.
            unsafe { NSEvent::removeMonitor(&monitor) };
        }
        let center = NSNotificationCenter::defaultCenter();
        for observer in self.focus_observers.drain(..) {
            // SAFETY: Each object is the exact observer token created above;
            // NotificationCenter requires explicit main-thread removal.
            unsafe { center.removeObserver(&observer) };
        }
        self.stopped = true;
        Ok(())
    }
}

impl Drop for MacInputMonitor {
    fn drop(&mut self) {
        if MainThreadMarker::new().is_some() {
            let _ = self.stop();
        } else {
            debug_assert!(
                self.stopped,
                "active native input monitor dropped away from AppKit main thread"
            );
        }
    }
}

fn window_input_from_event(event: &NSEvent, webview: &NSView) -> Option<WindowInput> {
    let timestamp_ns = seconds_to_nanoseconds(event.timestamp());
    let shift = event.modifierFlags().contains(NSEventModifierFlags::Shift);
    let webview_location = webview.convertPoint_fromView(event.locationInWindow(), None);
    let safe_area = webview.safeAreaInsets();
    let location = top_left_webview_location(
        webview_location,
        webview.bounds().size.height,
        webview.isFlipped(),
        safe_area.left,
        safe_area.top,
    )?;
    match event.r#type() {
        NSEventType::LeftMouseDown => Some(WindowInput::Pointer {
            phase: PointerPhase::Down,
            location,
            button: PointerButton::Primary,
            pressure: f64::from(event.pressure()),
            shift,
            timestamp_ns,
        }),
        NSEventType::LeftMouseDragged | NSEventType::MouseMoved => Some(WindowInput::Pointer {
            phase: PointerPhase::Move,
            location,
            button: if event.r#type() == NSEventType::LeftMouseDragged {
                PointerButton::Primary
            } else {
                PointerButton::None
            },
            pressure: f64::from(event.pressure()),
            shift,
            timestamp_ns,
        }),
        NSEventType::LeftMouseUp => Some(WindowInput::Pointer {
            phase: PointerPhase::Up,
            location,
            button: PointerButton::Primary,
            pressure: f64::from(event.pressure()),
            shift,
            timestamp_ns,
        }),
        NSEventType::MouseCancelled => Some(WindowInput::Pointer {
            phase: PointerPhase::Cancel,
            location,
            button: PointerButton::None,
            pressure: 0.0,
            shift,
            timestamp_ns,
        }),
        NSEventType::ScrollWheel => Some(WindowInput::Scroll {
            location,
            delta: scroll_delta_from_appkit(event.scrollingDeltaX(), event.scrollingDeltaY()),
            timestamp_ns,
        }),
        NSEventType::Magnify => Some(WindowInput::Magnify {
            location,
            factor: (1.0 + event.magnification()).max(0.01),
            timestamp_ns,
        }),
        NSEventType::KeyDown if event.keyCode() == SPACE_KEY_CODE => {
            Some(WindowInput::Space { pressed: true })
        }
        NSEventType::KeyUp if event.keyCode() == SPACE_KEY_CODE => {
            Some(WindowInput::Space { pressed: false })
        }
        NSEventType::KeyDown if event.keyCode() == ESCAPE_KEY_CODE => Some(WindowInput::Escape),
        _ => None,
    }
}

fn scroll_delta_from_appkit(x: f64, y: f64) -> LogicalPoint {
    LogicalPoint { x, y }
}

fn top_left_webview_location(
    point: NSPoint,
    webview_height: f64,
    webview_is_flipped: bool,
    content_left: f64,
    content_top: f64,
) -> Option<LogicalPoint> {
    if !point.x.is_finite()
        || !point.y.is_finite()
        || !webview_height.is_finite()
        || webview_height <= 0.0
        || !content_left.is_finite()
        || content_left < 0.0
        || !content_top.is_finite()
        || content_top < 0.0
    {
        return None;
    }
    Some(LogicalPoint {
        x: point.x - content_left,
        y: if webview_is_flipped {
            point.y - content_top
        } else {
            webview_height - point.y - content_top
        },
    })
}

fn seconds_to_nanoseconds(seconds: f64) -> u64 {
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0;
    }
    let nanoseconds = seconds * 1_000_000_000.0;
    if nanoseconds >= u64::MAX as f64 {
        u64::MAX
    } else {
        nanoseconds.round() as u64
    }
}

#[cfg(test)]
mod tests {
    use objc2_foundation::NSPoint;
    use viewer_render_core::LogicalPoint;

    use super::{scroll_delta_from_appkit, top_left_webview_location};

    #[test]
    fn preserves_appkit_horizontal_scroll_direction_for_canvas_pan() {
        assert_eq!(
            scroll_delta_from_appkit(12.0, -8.0),
            LogicalPoint { x: 12.0, y: -8.0 }
        );
    }

    #[test]
    fn maps_flipped_and_unflipped_webview_points_to_the_same_dom_coordinates() {
        assert_eq!(
            top_left_webview_location(NSPoint::new(328.0, 116.0), 932.0, true, 8.0, 32.0),
            Some(LogicalPoint { x: 320.0, y: 84.0 })
        );
        assert_eq!(
            top_left_webview_location(NSPoint::new(328.0, 816.0), 932.0, false, 8.0, 32.0),
            Some(LogicalPoint { x: 320.0, y: 84.0 })
        );
    }

    #[test]
    fn rejects_invalid_webview_coordinate_spaces() {
        assert_eq!(
            top_left_webview_location(NSPoint::new(10.0, 10.0), f64::NAN, false, 0.0, 0.0),
            None
        );
        assert_eq!(
            top_left_webview_location(NSPoint::new(f64::INFINITY, 10.0), 900.0, true, 0.0, 0.0,),
            None
        );
        assert_eq!(
            top_left_webview_location(NSPoint::new(10.0, 10.0), 900.0, true, 0.0, -1.0),
            None
        );
    }
}
