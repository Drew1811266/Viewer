use crate::gpu_memory::{GpuBuffer, GpuMemory};
use std::sync::{
    Arc,
    atomic::{AtomicU8, AtomicU64, Ordering},
};
use viewer_render_core::{AllocationClass, MemoryAdmissionError};

use viewer_render_core::{AssetGeneration, SceneRevision};

use crate::FRAMES_IN_FLIGHT;

static NEXT_RENDERER_ID: AtomicU64 = AtomicU64::new(1);
const TIMESTAMP_BYTES: u64 = 2 * wgpu::QUERY_SIZE as u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GpuTimingSupport {
    Available,
    Unavailable,
}

/// Completed render-pass timestamp measurement. Frame indices are local to a
/// renderer instance; generation and revision refer to the submitted scene.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpuFrameTiming {
    pub renderer_id: u64,
    pub frame_index: u64,
    pub generation: AssetGeneration,
    pub scene_revision: SceneRevision,
    pub gpu_time_ns: u64,
}

/// Each slot stays reserved from encoding until its map callback has completed
/// AND the owner has consumed/unmapped the readback. Saturation skips sampling,
/// never rendering. GPU query/buffer objects are allocated only at construction.
pub(crate) struct GpuTimer {
    renderer_id: u64,
    period_ns: f32,
    slots: Option<[TimingSlot; FRAMES_IN_FLIGHT]>,
}

struct TimingSlot {
    queries: wgpu::QuerySet,
    resolve: GpuBuffer,
    readback: GpuBuffer,
    pending: Option<GpuFrameTiming>,
    // 0 = pending/idle, 1 = render completed, 2 = mapped, 3 = map failed.
    completion: Arc<AtomicU8>,
}

impl GpuTimer {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        memory: &GpuMemory,
    ) -> Result<Self, MemoryAdmissionError> {
        let period_ns = queue.get_timestamp_period();
        let supported = device.features().contains(wgpu::Features::TIMESTAMP_QUERY)
            && period_ns.is_finite()
            && period_ns > 0.0;
        let slots = if supported {
            let mut slots = Vec::with_capacity(FRAMES_IN_FLIGHT);
            for _ in 0..FRAMES_IN_FLIGHT {
                slots.push(TimingSlot {
                    queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                        label: Some("Viewer frame timestamps"),
                        ty: wgpu::QueryType::Timestamp,
                        count: 2,
                    }),
                    resolve: memory.buffer(
                        &wgpu::BufferDescriptor {
                            label: Some("Viewer timestamp resolve"),
                            size: TIMESTAMP_BYTES,
                            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                            mapped_at_creation: false,
                        },
                        AllocationClass::RendererBuffers,
                    )?,
                    readback: memory.buffer(
                        &wgpu::BufferDescriptor {
                            label: Some("Viewer timestamp readback"),
                            size: TIMESTAMP_BYTES,
                            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        },
                        AllocationClass::RendererBuffers,
                    )?,
                    pending: None,
                    completion: Arc::new(AtomicU8::new(0)),
                });
            }
            Some(slots.try_into().ok().unwrap())
        } else {
            None
        };
        Ok(Self {
            renderer_id: NEXT_RENDERER_ID.fetch_add(1, Ordering::Relaxed),
            period_ns,
            slots,
        })
    }

    pub(crate) fn renderer_id(&self) -> u64 {
        self.renderer_id
    }

    pub(crate) fn support(&self) -> GpuTimingSupport {
        if self.slots.is_some() {
            GpuTimingSupport::Available
        } else {
            GpuTimingSupport::Unavailable
        }
    }

    pub(crate) fn begin(
        &mut self,
        frame_index: u64,
        generation: AssetGeneration,
        scene_revision: SceneRevision,
    ) -> Option<usize> {
        let slots = self.slots.as_mut()?;
        let index = slots.iter().position(|slot| slot.pending.is_none())?;
        slots[index].pending = Some(GpuFrameTiming {
            renderer_id: self.renderer_id,
            frame_index,
            generation,
            scene_revision,
            gpu_time_ns: 0,
        });
        Some(index)
    }

    pub(crate) fn writes(
        &self,
        index: Option<usize>,
    ) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        let slot = &self.slots.as_ref()?[index?];
        Some(wgpu::RenderPassTimestampWrites {
            query_set: &slot.queries,
            beginning_of_pass_write_index: Some(0),
            end_of_pass_write_index: Some(1),
        })
    }

    pub(crate) fn on_render_submitted(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        index: Option<usize>,
    ) {
        let Some(slot) = index.and_then(|index| self.slots.as_ref().map(|slots| &slots[index]))
        else {
            return;
        };
        let completion = Arc::clone(&slot.completion);
        encoder.on_submitted_work_done(move || {
            completion.store(1, Ordering::Release);
        });
    }

    pub(crate) fn drain_completed(
        &mut self,
        device: &wgpu::Device,
        memory: &GpuMemory,
    ) -> Vec<GpuFrameTiming> {
        let mut completed = Vec::new();
        let Some(slots) = self.slots.as_mut() else {
            return completed;
        };
        for slot in slots {
            let result = slot.completion.swap(0, Ordering::AcqRel);
            if result == 0 {
                continue;
            }
            if result == 1 {
                // Metal stage-boundary timestamps can still be unavailable
                // when resolved in the rendering command buffer. Submit the
                // resolve only after its completion callback, without waiting
                // on the CPU, and keep the same slot reserved through mapping.
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Viewer completed timestamp resolve"),
                });
                encoder.resolve_query_set(&slot.queries, 0..2, &slot.resolve, 0);
                encoder.copy_buffer_to_buffer(&slot.resolve, 0, &slot.readback, 0, TIMESTAMP_BYTES);
                let completion = Arc::clone(&slot.completion);
                encoder.map_buffer_on_submit(
                    &slot.readback,
                    wgpu::MapMode::Read,
                    ..,
                    move |result| {
                        // Callbacks publish only an atomic state, never wait or
                        // invoke externally supplied observers.
                        completion.store(if result.is_ok() { 2 } else { 3 }, Ordering::Release);
                    },
                );
                memory.submit([encoder.finish()]);
                continue;
            }
            let Some(mut sample) = slot.pending.take() else {
                continue;
            };
            if result == 2 {
                let duration = slot
                    .readback
                    .get_mapped_range(..)
                    .ok()
                    .and_then(|mapped| timestamp_duration_ns(&mapped, self.period_ns));
                if let Some(duration) = duration {
                    sample.gpu_time_ns = duration;
                    completed.push(sample);
                }
            }
            slot.readback.unmap();
        }
        completed
    }
}

fn timestamp_duration_ns(bytes: &[u8], period_ns: f32) -> Option<u64> {
    if bytes.len() != TIMESTAMP_BYTES as usize || !period_ns.is_finite() || period_ns <= 0.0 {
        return None;
    }
    let start = u64::from_ne_bytes(bytes[0..8].try_into().ok()?);
    let end = u64::from_ne_bytes(bytes[8..16].try_into().ok()?);
    // Zero is an unwritten query. Metal also uses all-ones as an invalid
    // counter sentinel. Neither endpoint may become a fabricated duration.
    if start == 0 || end == u64::MAX {
        return None;
    }
    let elapsed = end.checked_sub(start)? as f64 * f64::from(period_ns);
    (elapsed.is_finite() && elapsed >= 1.0 && elapsed < u64::MAX as f64)
        .then(|| elapsed.round() as u64)
}

#[cfg(test)]
mod tests {
    use super::timestamp_duration_ns;

    // Break caught: nanosecond conversion must apply the adapter period to
    // the counter difference before rounding, even for large absolute values.
    #[test]
    fn timestamp_duration_uses_the_counter_delta_and_period() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(u64::MAX - 1_000).to_ne_bytes());
        bytes.extend_from_slice(&(u64::MAX - 600).to_ne_bytes());
        assert_eq!(timestamp_duration_ns(&bytes, 2.5), Some(1_000));
    }

    // Break caught: failed/invalid query readbacks cannot be converted into
    // fabricated zero durations or enormous wrapped durations.
    #[test]
    fn invalid_queries_do_not_create_gpu_measurements() {
        let bytes = |start: u64, end: u64| [start.to_ne_bytes(), end.to_ne_bytes()].concat();
        for (start, end, period) in [
            (0, 0, 1.0),
            (0, 20, 1.0),
            (u64::MAX - 1_000, u64::MAX, 1.0),
            (10, 10, 1.0),
            (20, 10, 1.0),
            (10, 20, 0.0),
            (10, 20, -1.0),
            (10, 20, f32::NAN),
            (10, 20, f32::INFINITY),
            (0, u64::MAX, f32::MAX),
        ] {
            assert_eq!(timestamp_duration_ns(&bytes(start, end), period), None);
        }
        assert_eq!(timestamp_duration_ns(&[0; 8], 1.0), None);
    }
}
