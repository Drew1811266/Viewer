#![allow(deprecated)]

use std::{
    ffi::c_void,
    ptr::{NonNull, null_mut},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use objc2_core_foundation::CFRetained;
use objc2_core_video::{
    CVDisplayLink, CVGetHostClockFrequency, CVOptionFlags, CVReturn, CVTimeStamp,
};

use super::SurfaceError;

#[derive(Clone, Debug)]
pub struct DisplayTickSignal {
    latest_ns: Arc<AtomicU64>,
    pending: Arc<AtomicBool>,
    closed: Arc<AtomicBool>,
}

impl Default for DisplayTickSignal {
    fn default() -> Self {
        Self {
            latest_ns: Arc::new(AtomicU64::new(0)),
            pending: Arc::new(AtomicBool::new(false)),
            closed: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl DisplayTickSignal {
    pub fn publish(&self, timestamp_ns: u64) {
        if !self.closed.load(Ordering::Acquire) {
            self.latest_ns.store(timestamp_ns, Ordering::Release);
            self.pending.store(true, Ordering::Release);
        }
    }

    pub fn take_latest(&self) -> Option<u64> {
        loop {
            if self.closed.load(Ordering::Acquire) || !self.pending.swap(false, Ordering::AcqRel) {
                return None;
            }
            let timestamp_ns = self.latest_ns.load(Ordering::Acquire);
            if self.closed.load(Ordering::Acquire) {
                return None;
            }
            if !self.pending.load(Ordering::Acquire) {
                return Some(timestamp_ns);
            }
        }
    }

    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.pending.store(false, Ordering::Release);
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}

struct CallbackState {
    signal: DisplayTickSignal,
    host_clock_hz: f64,
}

pub struct MacDisplayLink {
    link: Option<CFRetained<CVDisplayLink>>,
    callback: Option<NonNull<CallbackState>>,
    stopped: bool,
}

impl MacDisplayLink {
    pub fn start(signal: DisplayTickSignal) -> Result<Self, SurfaceError> {
        Self::start_for_display(signal, None)
    }

    pub fn start_for_display(
        signal: DisplayTickSignal,
        display_id: Option<u32>,
    ) -> Result<Self, SurfaceError> {
        let mut raw = null_mut();
        // SAFETY: `raw` is a valid out pointer. Core Video returns a +1 object
        // on success, transferred immediately into CFRetained below.
        let status = unsafe {
            match display_id {
                Some(display_id) => {
                    CVDisplayLink::create_with_cg_display(display_id, NonNull::from(&mut raw))
                }
                None => CVDisplayLink::create_with_active_cg_displays(NonNull::from(&mut raw)),
            }
        };
        ensure_success(status)?;
        let raw = NonNull::new(raw).ok_or(SurfaceError::DisplayLink(status))?;
        // SAFETY: a successful Create call follows the Core Foundation create
        // rule and returns one owned retain count.
        let link = unsafe { CFRetained::from_raw(raw) };
        let host_clock_hz = CVGetHostClockFrequency();
        if !host_clock_hz.is_finite() || host_clock_hz <= 0.0 {
            return Err(SurfaceError::DisplayLink(-1));
        }
        let callback = Box::new(CallbackState {
            signal,
            host_clock_hz,
        });
        let callback = NonNull::from(Box::leak(callback));
        // SAFETY: callback storage is boxed and remains at a stable address
        // until a successful synchronous stop. If Core Video cannot stop, Drop
        // intentionally leaks both objects rather than freeing live callback
        // storage. The callback only touches atomics.
        let status = unsafe {
            link.set_output_callback(Some(display_link_callback), callback.as_ptr().cast())
        };
        if let Err(error) = ensure_success(status) {
            // SAFETY: Core Video rejected the callback registration, so the
            // leaked allocation was never published and is exclusively owned.
            unsafe { drop(Box::from_raw(callback.as_ptr())) };
            return Err(error);
        }
        if let Err(error) = ensure_success(link.start()) {
            // The display link never started, so it cannot be executing the
            // callback while the local allocation is reclaimed.
            // SAFETY: callback ownership is still exclusive to this function.
            unsafe { drop(Box::from_raw(callback.as_ptr())) };
            return Err(error);
        }
        Ok(Self {
            link: Some(link),
            callback: Some(callback),
            stopped: false,
        })
    }

    pub fn stop(&mut self) -> Result<(), SurfaceError> {
        if self.stopped {
            return Ok(());
        }
        let link = self
            .link
            .as_ref()
            .expect("a running display link retains its Core Video object");
        ensure_success(link.stop())?;
        self.stopped = true;
        if let Some(callback) = self.callback.take() {
            // SAFETY: CVDisplayLinkStop completed synchronously, so no callback
            // can still dereference this Box and ownership returns here.
            unsafe { drop(Box::from_raw(callback.as_ptr())) };
        }
        self.link.take();
        Ok(())
    }

    pub fn refresh_period_seconds(&self) -> Option<f64> {
        let period = self.link.as_ref()?.actual_output_video_refresh_period();
        (period.is_finite() && period > 0.0).then_some(period)
    }
}

impl Drop for MacDisplayLink {
    fn drop(&mut self) {
        if self.stop().is_err() {
            // A failed stop means Core Video may still execute the callback.
            // Leaking the retained display link and callback allocation is the
            // only safe recovery; freeing either could become a use-after-free.
            if let Some(link) = self.link.take() {
                std::mem::forget(link);
            }
            self.callback.take();
        }
    }
}

unsafe extern "C-unwind" fn display_link_callback(
    _display_link: NonNull<CVDisplayLink>,
    _now: NonNull<CVTimeStamp>,
    output: NonNull<CVTimeStamp>,
    _flags_in: CVOptionFlags,
    _flags_out: NonNull<CVOptionFlags>,
    user_info: *mut c_void,
) -> CVReturn {
    let Some(callback) = NonNull::new(user_info.cast::<CallbackState>()) else {
        return 0;
    };
    // SAFETY: `start` installs a pointer to stable Box storage and `stop`
    // synchronizes callback shutdown before that Box is dropped.
    let callback = unsafe { callback.as_ref() };
    // SAFETY: Core Video supplies a valid output timestamp for this callback.
    let host_time = unsafe { output.as_ref() }.hostTime;
    let timestamp_ns = ((host_time as f64 / callback.host_clock_hz) * 1_000_000_000.0)
        .clamp(0.0, u64::MAX as f64) as u64;
    callback.signal.publish(timestamp_ns);
    0
}

fn ensure_success(status: CVReturn) -> Result<(), SurfaceError> {
    if status == 0 {
        Ok(())
    } else {
        Err(SurfaceError::DisplayLink(status))
    }
}
