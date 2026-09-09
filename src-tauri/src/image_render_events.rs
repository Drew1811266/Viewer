use std::sync::{Mutex, MutexGuard};

use tauri::Emitter;

use crate::dto::ImageRenderEventDto;

pub const IMAGE_RENDER_EVENT_NAME: &str = "viewer://image-render";

pub trait ImageRenderEventPort: Send + Sync {
    fn publish(&self, event: ImageRenderEventDto);

    /// Local diagnostics only, never serialized into the frontend event stream.
    /// The timestamp is the display-link slot whose content was submitted to
    /// the real surface; it is not a compositor scan-out or GPU-completion time.
    fn surface_frame_submitted(
        &self,
        _receipt: viewer_render_wgpu::FrameReceipt,
        _display_timestamp_ns: u64,
        _generation: u64,
        _dropped_input_samples: u64,
        _renderer_id: u64,
    ) {
    }

    fn gpu_timing_support(
        &self,
        _renderer_id: u64,
        _support: viewer_render_wgpu::GpuTimingSupport,
    ) {
    }

    fn gpu_frame_completed(&self, _timing: viewer_render_wgpu::GpuFrameTiming) {}
}

pub struct TauriImageRenderEventEmitter {
    app: tauri::AppHandle,
}

impl TauriImageRenderEventEmitter {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl ImageRenderEventPort for TauriImageRenderEventEmitter {
    fn publish(&self, event: ImageRenderEventDto) {
        if let Err(error) = self.app.emit(IMAGE_RENDER_EVENT_NAME, event) {
            eprintln!("Viewer image render event delivery failed: {error}");
        }
    }
}

#[derive(Default)]
pub struct RecordingImageRenderEvents {
    events: Mutex<Vec<ImageRenderEventDto>>,
}

impl RecordingImageRenderEvents {
    pub fn take(&self) -> Vec<ImageRenderEventDto> {
        std::mem::take(&mut *lock(&self.events))
    }
}

impl ImageRenderEventPort for RecordingImageRenderEvents {
    fn publish(&self, event: ImageRenderEventDto) {
        lock(&self.events).push(event);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
