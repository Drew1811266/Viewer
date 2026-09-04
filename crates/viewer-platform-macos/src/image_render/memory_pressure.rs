use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::Arc;

use block2::RcBlock;
use dispatch2::{
    _dispatch_source_type_memorypressure, DispatchObject, DispatchQueue, DispatchRetained,
    DispatchSource, dispatch_source_memorypressure_flags_t,
};
use viewer_render_core::PressureLevel;

pub type MemoryPressureSink = Arc<dyn Fn(PressureLevel) + Send + Sync + 'static>;

pub const fn pressure_level_for_flags(flags: usize) -> PressureLevel {
    if flags & dispatch_source_memorypressure_flags_t::DISPATCH_MEMORYPRESSURE_CRITICAL.0 as usize
        != 0
    {
        PressureLevel::Critical
    } else if flags
        & dispatch_source_memorypressure_flags_t::DISPATCH_MEMORYPRESSURE_WARN.0 as usize
        != 0
    {
        PressureLevel::Warning
    } else {
        PressureLevel::Normal
    }
}

pub struct MacMemoryPressureMonitor {
    sources: Vec<DispatchRetained<DispatchSource>>,
}

impl MacMemoryPressureMonitor {
    pub fn install(sink: MemoryPressureSink) -> Self {
        let queue = DispatchQueue::new("com.viewer.image-render.memory-pressure", None);
        let events = [
            dispatch_source_memorypressure_flags_t::DISPATCH_MEMORYPRESSURE_NORMAL.0 as usize,
            dispatch_source_memorypressure_flags_t::DISPATCH_MEMORYPRESSURE_WARN.0 as usize,
            dispatch_source_memorypressure_flags_t::DISPATCH_MEMORYPRESSURE_CRITICAL.0 as usize,
        ];
        let mut sources = Vec::with_capacity(events.len());
        for flags in events {
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
            let event_sink = Arc::clone(&sink);
            let handler = RcBlock::new(move || {
                let _ = catch_unwind(AssertUnwindSafe(|| {
                    event_sink(pressure_level_for_flags(flags));
                }));
            });
            // SAFETY: libdispatch copies and retains the block until the source
            // is cancelled and released by this monitor.
            unsafe {
                source.set_event_handler_with_block(RcBlock::as_ptr(&handler));
            }
            source.activate();
            sources.push(source);
        }
        Self { sources }
    }
}

impl Drop for MacMemoryPressureMonitor {
    fn drop(&mut self) {
        for source in &self.sources {
            source.cancel();
        }
    }
}
