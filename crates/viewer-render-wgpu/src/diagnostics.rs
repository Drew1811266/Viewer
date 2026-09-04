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
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};
