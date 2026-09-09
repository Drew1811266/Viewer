//! Admission accounting for application-controlled allocations, not process RSS
//! or physical Metal residency. Share one coordinator across providers/actors.
use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    sync::{Arc, Mutex, MutexGuard},
};

use crate::{AssetGeneration, PressureLevel};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocationClass {
    NativeDecode,
    DecodedPixels,
    UploadStaging,
    GpuTexture,
    RendererBuffers,
    OpaqueAllowance,
}

impl AllocationClass {
    const fn is_gpu(self) -> bool {
        matches!(
            self,
            Self::UploadStaging | Self::GpuTexture | Self::RendererBuffers
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocationPhase {
    Reserved,
    Committed,
    Retiring,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveMemoryLimits {
    pub combined_bytes: u64,
    pub gpu_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageMemoryPolicy {
    normal: LiveMemoryLimits,
    warning: LiveMemoryLimits,
    critical: LiveMemoryLimits,
}

impl ImageMemoryPolicy {
    pub fn new(
        normal: LiveMemoryLimits,
        warning: LiveMemoryLimits,
        critical: LiveMemoryLimits,
    ) -> Result<Self, MemoryAdmissionError> {
        let valid = |limits: LiveMemoryLimits| {
            limits.gpu_bytes > 0 && limits.gpu_bytes <= limits.combined_bytes
        };
        if ![normal, warning, critical].into_iter().all(valid)
            || warning.combined_bytes > normal.combined_bytes
            || warning.gpu_bytes > normal.gpu_bytes
            || critical.combined_bytes > warning.combined_bytes
            || critical.gpu_bytes > warning.gpu_bytes
        {
            return Err(MemoryAdmissionError::InvalidPolicy);
        }
        Ok(Self {
            normal,
            warning,
            critical,
        })
    }
    pub const fn baseline_8gb() -> Self {
        Self {
            normal: LiveMemoryLimits {
                combined_bytes: 512 << 20,
                gpu_bytes: 256 << 20,
            },
            warning: LiveMemoryLimits {
                combined_bytes: 256 << 20,
                gpu_bytes: 128 << 20,
            },
            critical: LiveMemoryLimits {
                combined_bytes: 128 << 20,
                gpu_bytes: 64 << 20,
            },
        }
    }

    const fn limits(self, pressure: PressureLevel) -> LiveMemoryLimits {
        match pressure {
            PressureLevel::Normal => self.normal,
            PressureLevel::Warning => self.warning,
            PressureLevel::Critical => self.critical,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryAdmissionError {
    InvalidPolicy,
    InvalidSize,
    TemporarilyBlocked,
    ExceedsPolicy,
    AccountingOverflow,
    AllocationFailed,
    InvalidTransition,
}

impl fmt::Display for MemoryAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidPolicy => "invalid live-memory pressure policy",
            Self::InvalidSize => "allocation size must be nonzero",
            Self::TemporarilyBlocked => "live allocations or pressure temporarily block admission",
            Self::ExceedsPolicy => "allocation exceeds even the normal memory ceiling",
            Self::AccountingOverflow => "live-memory accounting overflow",
            Self::AllocationFailed => "pixel storage allocation failed",
            Self::InvalidTransition => "invalid allocation lifetime transition",
        })
    }
}

impl Error for MemoryAdmissionError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemorySnapshot {
    pub pressure: PressureLevel,
    pub pressure_epoch: u64,
    pub limits: LiveMemoryLimits,
    pub combined_bytes: u64,
    pub gpu_bytes: u64,
    pub peak_combined_bytes: u64,
    pub retiring_gpu_bytes: u64,
    pub peak_gpu_bytes: u64,
    pub allocation_count: usize,
    class_bytes: [u64; 6],
    phase_bytes: [u64; 3],
}
impl MemorySnapshot {
    pub fn bytes_for_class(&self, class: AllocationClass) -> u64 {
        self.class_bytes[class as usize]
    }
    pub fn bytes_for_phase(&self, phase: AllocationPhase) -> u64 {
        self.phase_bytes[phase as usize]
    }
}

#[derive(Debug)]
struct AllocationRecord {
    bytes: u64,
    class: AllocationClass,
    phase: AllocationPhase,
}

#[derive(Debug)]
struct State {
    policy: ImageMemoryPolicy,
    snapshot: MemorySnapshot,
    next_id: u64,
    allocations: BTreeMap<u64, AllocationRecord>,
}

// All mutations below are internal and leave accounting coherent before releasing
// the lock. No caller callbacks, allocation work or GPU waits execute under it.
fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(Clone, Debug)]
pub struct ImageMemoryCoordinator {
    state: Arc<Mutex<State>>,
}
impl ImageMemoryCoordinator {
    pub fn new(policy: ImageMemoryPolicy) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                policy,
                snapshot: MemorySnapshot {
                    pressure: PressureLevel::Normal,
                    pressure_epoch: 0,
                    limits: policy.normal,
                    combined_bytes: 0,
                    gpu_bytes: 0,
                    retiring_gpu_bytes: 0,
                    peak_combined_bytes: 0,
                    peak_gpu_bytes: 0,
                    allocation_count: 0,
                    class_bytes: [0; 6],
                    phase_bytes: [0; 3],
                },
                next_id: 0,
                allocations: BTreeMap::new(),
            })),
        }
    }

    /// Reserve BEFORE allocating. Ok grants ownership; a normal-policy excess is
    /// permanent for this allocation, while pressure/occupancy can later clear.
    /// All phases remain charged. This does not allocate the requested payload.
    pub fn try_reserve(
        &self,
        class: AllocationClass,
        bytes: u64,
        generation: AssetGeneration,
    ) -> Result<MemoryLease, MemoryAdmissionError> {
        if bytes == 0 {
            return Err(MemoryAdmissionError::InvalidSize);
        }
        let mut state = lock(&self.state);
        let gpu_bytes = if class.is_gpu() { bytes } else { 0 };
        if bytes > state.policy.normal.combined_bytes || gpu_bytes > state.policy.normal.gpu_bytes {
            return Err(MemoryAdmissionError::ExceedsPolicy);
        }
        let combined = state
            .snapshot
            .combined_bytes
            .checked_add(bytes)
            .ok_or(MemoryAdmissionError::AccountingOverflow)?;
        let gpu = state
            .snapshot
            .gpu_bytes
            .checked_add(gpu_bytes)
            .ok_or(MemoryAdmissionError::AccountingOverflow)?;
        // An already exceeded GPU ceiling also blocks CPU admission: first drain
        // real live ownership to satisfy both current ceilings.
        if combined > state.snapshot.limits.combined_bytes || gpu > state.snapshot.limits.gpu_bytes
        {
            return Err(MemoryAdmissionError::TemporarilyBlocked);
        }
        let id = state.next_id;
        let next_id = id
            .checked_add(1)
            .ok_or(MemoryAdmissionError::AccountingOverflow)?;
        let count = state
            .snapshot
            .allocation_count
            .checked_add(1)
            .ok_or(MemoryAdmissionError::AccountingOverflow)?;
        state.allocations.insert(
            id,
            AllocationRecord {
                bytes,
                class,
                phase: AllocationPhase::Reserved,
            },
        );
        state.next_id = next_id;
        let snapshot = &mut state.snapshot;
        snapshot.combined_bytes = combined;
        snapshot.gpu_bytes = gpu;
        snapshot.peak_combined_bytes = snapshot.peak_combined_bytes.max(combined);
        snapshot.peak_gpu_bytes = snapshot.peak_gpu_bytes.max(gpu);
        snapshot.allocation_count = count;
        // Each category is a subset of the checked combined total above.
        snapshot.class_bytes[class as usize] += bytes;
        snapshot.phase_bytes[AllocationPhase::Reserved as usize] += bytes;
        Ok(MemoryLease {
            inner: Arc::new(LeaseInner {
                state: self.state.clone(),
                id,
                bytes,
                class,
                generation,
            }),
        })
    }

    pub fn snapshot(&self) -> MemorySnapshot {
        lock(&self.state).snapshot.clone()
    }

    /// The permanent ceiling for compound-operation preflight, independent of
    /// temporary pressure and other callers' currently live ownership.
    pub fn normal_limits(&self) -> LiveMemoryLimits {
        lock(&self.state).policy.normal
    }

    /// Pressure changes admission ceilings only, never existing ownership/usage.
    pub fn set_pressure(&self, pressure: PressureLevel) -> Result<u64, MemoryAdmissionError> {
        let mut state = lock(&self.state);
        if state.snapshot.pressure != pressure {
            let epoch = state
                .snapshot
                .pressure_epoch
                .checked_add(1)
                .ok_or(MemoryAdmissionError::AccountingOverflow)?;
            state.snapshot.limits = state.policy.limits(pressure);
            state.snapshot.pressure = pressure;
            state.snapshot.pressure_epoch = epoch;
        }
        Ok(state.snapshot.pressure_epoch)
    }
}

/// Clone alongside the SAME allocation's Arc owners. A pixel copy requires a
/// separate reservation. Dropping the last lease releases its accounting; keep
/// a retiring GPU lease alive until submission completion, not just registry removal.
#[derive(Clone, Debug)]
pub struct MemoryLease {
    inner: Arc<LeaseInner>,
}
impl MemoryLease {
    pub fn is_accounted_by(&self, memory: &ImageMemoryCoordinator) -> bool {
        Arc::ptr_eq(&self.inner.state, &memory.state)
    }
    /// Diagnostic attribution only; generations never bulk-release allocations.
    pub fn generation(&self) -> AssetGeneration {
        self.inner.generation
    }
    pub fn bytes(&self) -> u64 {
        self.inner.bytes
    }
    pub fn class(&self) -> AllocationClass {
        self.inner.class
    }
    pub fn phase(&self) -> AllocationPhase {
        lock(&self.inner.state).allocations[&self.inner.id].phase
    }
    pub fn commit(&self) -> Result<(), MemoryAdmissionError> {
        self.transition(AllocationPhase::Committed)
    }
    pub fn retire(&self) -> Result<(), MemoryAdmissionError> {
        self.transition(AllocationPhase::Retiring)
    }

    fn transition(&self, target: AllocationPhase) -> Result<(), MemoryAdmissionError> {
        let mut state = lock(&self.inner.state);
        let record = state
            .allocations
            .get_mut(&self.inner.id)
            .expect("live lease has allocation record");
        let previous = record.phase;
        if previous == target {
            return Ok(());
        }
        if !matches!(
            (previous, target),
            (AllocationPhase::Reserved, AllocationPhase::Committed)
                | (AllocationPhase::Committed, AllocationPhase::Retiring)
        ) {
            return Err(MemoryAdmissionError::InvalidTransition);
        }
        record.phase = target;
        state.snapshot.phase_bytes[previous as usize] -= self.inner.bytes;
        state.snapshot.phase_bytes[target as usize] += self.inner.bytes;
        if target == AllocationPhase::Retiring && self.inner.class.is_gpu() {
            state.snapshot.retiring_gpu_bytes += self.inner.bytes;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct LeaseInner {
    state: Arc<Mutex<State>>,
    id: u64,
    bytes: u64,
    class: AllocationClass,
    generation: AssetGeneration,
}

impl Drop for LeaseInner {
    fn drop(&mut self) {
        let mut state = lock(&self.state);
        let record = state
            .allocations
            .remove(&self.id)
            .expect("last lease owns allocation record");
        let snapshot = &mut state.snapshot;
        snapshot.combined_bytes -= record.bytes;
        if record.class.is_gpu() {
            snapshot.gpu_bytes -= record.bytes;
            if record.phase == AllocationPhase::Retiring {
                snapshot.retiring_gpu_bytes -= record.bytes;
            }
        }
        snapshot.class_bytes[record.class as usize] -= record.bytes;
        snapshot.phase_bytes[record.phase as usize] -= record.bytes;
        snapshot.allocation_count -= 1;
    }
}
