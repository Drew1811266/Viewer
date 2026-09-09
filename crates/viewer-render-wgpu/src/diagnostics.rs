#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceAcquireFailure {
    Timeout,
    Occluded,
    Outdated,
    Lost,
    Validation,
    OutOfMemory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceRecovery {
    RetryNextFrame,
    WaitUntilVisible,
    Reconfigure,
    RecreateSurface,
    ReportValidation,
    Terminate,
}

pub const fn surface_recovery(failure: SurfaceAcquireFailure) -> SurfaceRecovery {
    match failure {
        SurfaceAcquireFailure::Timeout => SurfaceRecovery::RetryNextFrame,
        SurfaceAcquireFailure::Occluded => SurfaceRecovery::WaitUntilVisible,
        SurfaceAcquireFailure::Outdated => SurfaceRecovery::Reconfigure,
        SurfaceAcquireFailure::Lost => SurfaceRecovery::RecreateSurface,
        SurfaceAcquireFailure::Validation => SurfaceRecovery::ReportValidation,
        SurfaceAcquireFailure::OutOfMemory => SurfaceRecovery::Terminate,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GpuFault {
    Validation,
    Internal,
    OutOfMemory,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GpuFaultState {
    severity: Arc<AtomicU8>,
}

impl GpuFaultState {
    pub(crate) fn record(&self, error: wgpu::Error) {
        let severity = match error {
            wgpu::Error::Validation { .. } => 1,
            wgpu::Error::Internal { .. } => 2,
            wgpu::Error::OutOfMemory { .. } => 3,
        };
        self.severity.fetch_max(severity, Ordering::AcqRel);
    }

    pub(crate) fn take(&self) -> Option<GpuFault> {
        match self.severity.swap(0, Ordering::AcqRel) {
            0 => None,
            1 => Some(GpuFault::Validation),
            2 => Some(GpuFault::Internal),
            _ => Some(GpuFault::OutOfMemory),
        }
    }
}
use std::collections::BTreeSet;
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

use serde::Serialize;

use crate::{FrameReceipt, GpuFrameTiming, GpuTimingSupport};

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct PerformancePercentiles {
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageRendererPerformanceWorkload {
    pub source_width: u32,
    pub source_height: u32,
    pub annotation_count: usize,
    pub display_hz: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct MeasuredImageRendererWorkload {
    pub source_width: u32,
    pub source_height: u32,
    pub annotation_count: usize,
    pub display_hz: u32,
    pub frame_samples: usize,
    pub save_samples: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ImageRendererPerformanceReceipt {
    pub schema_version: u32,
    pub measurement_mode: &'static str,
    pub frame_timing: &'static str,
    pub workload: MeasuredImageRendererWorkload,
    pub frame_ms: PerformancePercentiles,
    pub gpu_frame_ms: Option<PerformancePercentiles>,
    pub gpu_timing: &'static str,
    pub gpu_timing_support: &'static str,
    pub gpu_sample_count: usize,
    pub presented_fps: f64,
    pub first_interactive_ms: f64,
    pub warm_first_interactive_ms: f64,
    pub gpu_texture_bytes_peak: u64,
    pub dropped_input_samples: u64,
    pub recovery_count: u64,
    pub save_commit_ms: PerformancePercentiles,
}

#[derive(Clone, Debug)]
pub struct ImageRendererDiagnostics {
    started_at_ns: u64,
    first_interactive_at_ns: Option<u64>,
    warm_first_interactive_at_ns: Option<u64>,
    warm_started_at_ns: Option<u64>,
    presented_at_ns: Vec<u64>,
    gpu_frame_ns: Vec<u64>,
    gpu_timing_support: GpuTimingSupport,
    gpu_sample_ids: BTreeSet<(u64, u64)>,
    gpu_texture_bytes_peak: u64,
    dropped_input_samples: u64,
    recovery_count: u64,
    save_commit_ns: Vec<u64>,
}

impl ImageRendererDiagnostics {
    pub fn new(started_at_ns: u64) -> Self {
        Self {
            started_at_ns,
            first_interactive_at_ns: None,
            warm_first_interactive_at_ns: None,
            warm_started_at_ns: None,
            presented_at_ns: Vec::new(),
            gpu_frame_ns: Vec::new(),
            gpu_timing_support: GpuTimingSupport::Unavailable,
            gpu_sample_ids: BTreeSet::new(),
            gpu_texture_bytes_peak: 0,
            dropped_input_samples: 0,
            recovery_count: 0,
            save_commit_ns: Vec::new(),
        }
    }

    pub fn mark_first_interactive(&mut self, timestamp_ns: u64) {
        self.first_interactive_at_ns.get_or_insert(timestamp_ns);
    }

    pub fn mark_warm_first_interactive(&mut self, timestamp_ns: u64) {
        self.warm_first_interactive_at_ns
            .get_or_insert(timestamp_ns);
    }

    pub fn begin_warm_open(&mut self, timestamp_ns: u64) {
        self.warm_started_at_ns = Some(timestamp_ns);
    }

    pub fn record_frame(&mut self, receipt: FrameReceipt, presented_at_ns: u64) {
        if receipt.presented
            && self
                .presented_at_ns
                .last()
                .is_none_or(|previous| presented_at_ns > *previous)
        {
            self.presented_at_ns.push(presented_at_ns);
        }
        self.gpu_texture_bytes_peak = self.gpu_texture_bytes_peak.max(receipt.gpu_resource_bytes);
    }

    pub fn set_gpu_timing_support(&mut self, support: GpuTimingSupport) {
        self.gpu_timing_support = support;
    }

    /// Records an independently completed timestamp query. Consumers select
    /// samples for their measurement window by renderer/frame/generation first.
    pub fn record_gpu_timing(&mut self, sample: GpuFrameTiming) {
        if sample.gpu_time_ns > 0
            && self
                .gpu_sample_ids
                .insert((sample.renderer_id, sample.frame_index))
        {
            self.gpu_timing_support = GpuTimingSupport::Available;
            self.gpu_frame_ns.push(sample.gpu_time_ns);
        }
    }

    pub fn record_input_sample(&mut self, dropped: bool) {
        if dropped {
            self.dropped_input_samples = self.dropped_input_samples.saturating_add(1);
        }
    }

    pub fn record_recovery(&mut self) {
        self.recovery_count = self.recovery_count.saturating_add(1);
    }

    pub fn record_save_commit_ns(&mut self, elapsed_ns: u64) {
        self.save_commit_ns.push(elapsed_ns);
    }

    pub const fn dropped_input_samples(&self) -> u64 {
        self.dropped_input_samples
    }

    pub fn reset_frame_window(&mut self) {
        self.presented_at_ns.clear();
        self.gpu_frame_ns.clear();
        self.gpu_sample_ids.clear();
        self.gpu_texture_bytes_peak = 0;
    }

    pub fn finish(
        self,
        workload: ImageRendererPerformanceWorkload,
    ) -> ImageRendererPerformanceReceipt {
        let frame_intervals = self
            .presented_at_ns
            .windows(2)
            .map(|pair| pair[1].saturating_sub(pair[0]))
            .collect::<Vec<_>>();
        let presented_fps = match (self.presented_at_ns.first(), self.presented_at_ns.last()) {
            (Some(first), Some(last)) if last > first && self.presented_at_ns.len() > 1 => {
                (self.presented_at_ns.len() - 1) as f64 * 1_000_000_000.0
                    / last.saturating_sub(*first) as f64
            }
            _ => 0.0,
        };
        let gpu_frame_ms = (!self.gpu_frame_ns.is_empty()).then(|| percentiles(&self.gpu_frame_ns));
        ImageRendererPerformanceReceipt {
            schema_version: 1,
            measurement_mode: "native_metal_surface",
            frame_timing: "display_link_surface_submission",
            workload: MeasuredImageRendererWorkload {
                source_width: workload.source_width,
                source_height: workload.source_height,
                annotation_count: workload.annotation_count,
                display_hz: workload.display_hz,
                frame_samples: self.presented_at_ns.len(),
                save_samples: self.save_commit_ns.len(),
            },
            frame_ms: percentiles(&frame_intervals),
            gpu_frame_ms,
            gpu_timing_support: match self.gpu_timing_support {
                GpuTimingSupport::Available => "available",
                GpuTimingSupport::Unavailable => "unavailable",
            },
            gpu_sample_count: self.gpu_frame_ns.len(),
            gpu_timing: if self.gpu_frame_ns.is_empty() {
                "unavailable"
            } else {
                "available"
            },
            presented_fps,
            first_interactive_ms: elapsed_ms(self.started_at_ns, self.first_interactive_at_ns),
            warm_first_interactive_ms: elapsed_ms(
                self.warm_started_at_ns
                    .or(self.first_interactive_at_ns)
                    .unwrap_or(self.started_at_ns),
                self.warm_first_interactive_at_ns,
            ),
            gpu_texture_bytes_peak: self.gpu_texture_bytes_peak,
            dropped_input_samples: self.dropped_input_samples,
            recovery_count: self.recovery_count,
            save_commit_ms: percentiles(&self.save_commit_ns),
        }
    }
}

fn elapsed_ms(start: u64, end: Option<u64>) -> f64 {
    end.map_or(0.0, |value| {
        value.saturating_sub(start) as f64 / 1_000_000.0
    })
}

fn percentiles(samples_ns: &[u64]) -> PerformancePercentiles {
    let mut samples = samples_ns.to_vec();
    samples.sort_unstable();
    PerformancePercentiles {
        p50: percentile(&samples, 50) as f64 / 1_000_000.0,
        p95: percentile(&samples, 95) as f64 / 1_000_000.0,
        p99: percentile(&samples, 99) as f64 / 1_000_000.0,
    }
}

fn percentile(sorted: &[u64], percentile: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = percentile.saturating_mul(sorted.len()).div_ceil(100);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}
