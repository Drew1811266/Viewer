use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use block2::RcBlock;
use dispatch2::{
    _dispatch_source_type_memorypressure, DispatchObject, DispatchQueue, DispatchRetained,
    DispatchSource, dispatch_source_memorypressure_flags_t,
};
use viewer_render_core::PressureLevel;

pub type MemoryPressureSink = Arc<dyn Fn(PressureLevel) + Send + Sync + 'static>;

pub const fn pressure_level_for_flags(flags: usize) -> Option<PressureLevel> {
    if flags & dispatch_source_memorypressure_flags_t::DISPATCH_MEMORYPRESSURE_CRITICAL.0 as usize
        != 0
    {
        Some(PressureLevel::Critical)
    } else if flags
        & dispatch_source_memorypressure_flags_t::DISPATCH_MEMORYPRESSURE_WARN.0 as usize
        != 0
    {
        Some(PressureLevel::Warning)
    } else if flags
        == dispatch_source_memorypressure_flags_t::DISPATCH_MEMORYPRESSURE_NORMAL.0 as usize
    {
        Some(PressureLevel::Normal)
    } else {
        None
    }
}

pub struct MacMemoryPressureMonitor {
    source: DispatchRetained<DispatchSource>,
    active: Arc<AtomicBool>,
}

impl MacMemoryPressureMonitor {
    pub fn install(sink: MemoryPressureSink) -> Self {
        let queue = DispatchQueue::new("com.viewer.image-render.memory-pressure", None);
        let flags = (dispatch_source_memorypressure_flags_t::DISPATCH_MEMORYPRESSURE_NORMAL.0
            | dispatch_source_memorypressure_flags_t::DISPATCH_MEMORYPRESSURE_WARN.0
            | dispatch_source_memorypressure_flags_t::DISPATCH_MEMORYPRESSURE_CRITICAL.0)
            as usize;
        // SAFETY: The global dispatch source type is process-lifetime
        // immutable state. Memory-pressure sources require a zero handle
        // and a mask containing only documented pressure flags.
        let source = unsafe {
            DispatchSource::new(
                ptr::addr_of!(_dispatch_source_type_memorypressure).cast_mut(),
                0,
                flags,
                Some(&queue),
            )
        };
        let active = Arc::new(AtomicBool::new(true));
        let handler_active = active.clone();
        // Do not capture a retain: source -> block -> source would cycle.
        // libdispatch owns the source throughout its non-reentrant handler.
        let address = DispatchRetained::as_ptr(&source).as_ptr() as usize;
        let handler = RcBlock::new(move || {
            let _ = catch_unwind(AssertUnwindSafe(|| {
                if !handler_active.load(Ordering::Acquire) {
                    return;
                }
                // SAFETY: read pending data only inside this source's own
                // callback, while libdispatch guarantees its lifetime.
                let data = unsafe { &*(address as *const DispatchSource) }.data();
                // OR-coalesced flags do not reveal temporal order. This is
                // conservative effective pressure, not a current-OS query.
                if let Some(pressure) = pressure_level_for_flags(data) {
                    sink(pressure);
                }
            }));
        });
        // SAFETY: libdispatch copies and retains the block until the source
        // is cancelled and released by this monitor.
        unsafe {
            source.set_event_handler_with_block(RcBlock::as_ptr(&handler));
        }
        source.activate();
        Self { source, active }
    }
}

impl Drop for MacMemoryPressureMonitor {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
        self.source.cancel();
    }
}
