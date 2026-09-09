use std::sync::{Arc, Barrier};
use viewer_render_core::{
    AllocationClass as Class, AllocationPhase as Phase, AssetGeneration, ImageMemoryCoordinator,
    ImageMemoryPolicy, LiveMemoryLimits, MemoryAdmissionError as Denied, PressureLevel,
};

fn limits(combined_bytes: u64, gpu_bytes: u64) -> LiveMemoryLimits {
    LiveMemoryLimits {
        combined_bytes,
        gpu_bytes,
    }
}

fn coordinator() -> ImageMemoryCoordinator {
    ImageMemoryCoordinator::new(
        ImageMemoryPolicy::new(limits(100, 60), limits(50, 30), limits(25, 15)).unwrap(),
    )
}

#[test]
fn clients_share_combined_and_gpu_admission_including_upload_staging() {
    let cpu = coordinator();
    let gpu = cpu.clone();
    let _decode = cpu
        .try_reserve(Class::NativeDecode, 45, AssetGeneration(1))
        .unwrap();
    let _texture = gpu
        .try_reserve(Class::GpuTexture, 40, AssetGeneration(2))
        .unwrap();
    let before = cpu.snapshot();
    assert_eq!(
        gpu.try_reserve(Class::DecodedPixels, 16, AssetGeneration(2))
            .unwrap_err(),
        Denied::TemporarilyBlocked
    );
    assert_eq!(cpu.snapshot(), before);
    let _upload = gpu
        .try_reserve(Class::UploadStaging, 15, AssetGeneration(2))
        .unwrap();
    assert_eq!(
        (cpu.snapshot().combined_bytes, cpu.snapshot().gpu_bytes),
        (100, 55)
    );
    drop(_decode);
    assert_eq!(
        gpu.try_reserve(Class::RendererBuffers, 6, AssetGeneration(2))
            .unwrap_err(),
        Denied::TemporarilyBlocked
    );
    let _buffers = gpu
        .try_reserve(Class::RendererBuffers, 5, AssetGeneration(2))
        .unwrap();
    assert_eq!(cpu.snapshot().gpu_bytes, 60);
}

#[test]
fn clones_count_once_but_independent_copies_reserve_separately() {
    let memory = coordinator();
    let pixels = memory
        .try_reserve(Class::DecodedPixels, 20, AssetGeneration(7))
        .unwrap();
    let clone = pixels.clone();
    let copy = memory
        .try_reserve(Class::DecodedPixels, 20, AssetGeneration(7))
        .unwrap();
    assert_eq!(memory.snapshot().allocation_count, 2);
    drop(pixels);
    assert_eq!(memory.snapshot().combined_bytes, 40);
    drop(clone);
    assert_eq!(memory.snapshot().combined_bytes, 20);
    drop(copy);
    assert_eq!(memory.snapshot().combined_bytes, 0);
    assert_eq!(memory.snapshot().peak_combined_bytes, 40);
}

#[test]
fn committed_and_retiring_remain_charged_through_last_owner() {
    let memory = coordinator();
    let lease = memory
        .try_reserve(Class::GpuTexture, 30, AssetGeneration(1))
        .unwrap();
    let submitted = lease.clone();
    assert_eq!(lease.retire().unwrap_err(), Denied::InvalidTransition);
    assert_eq!(lease.phase(), Phase::Reserved);
    assert_eq!((lease.class(), lease.bytes()), (Class::GpuTexture, 30));
    assert_eq!(memory.snapshot().bytes_for_phase(Phase::Reserved), 30);
    lease.commit().unwrap();
    lease.commit().unwrap();
    assert_eq!(memory.snapshot().bytes_for_phase(Phase::Committed), 30);
    submitted.retire().unwrap();
    submitted.retire().unwrap();
    assert_eq!(lease.commit().unwrap_err(), Denied::InvalidTransition);
    assert_eq!(memory.snapshot().bytes_for_phase(Phase::Retiring), 30);
    assert_eq!(memory.snapshot().combined_bytes, 30);
    drop(lease);
    assert_eq!(memory.snapshot().gpu_bytes, 30);
    drop(submitted);
    assert_eq!(memory.snapshot().bytes_for_phase(Phase::Retiring), 0);
    assert_eq!(memory.snapshot().peak_gpu_bytes, 30);
}

#[test]
fn cancelled_generation_drops_only_its_own_allocation() {
    let memory = coordinator();
    let cancelled = memory
        .try_reserve(Class::NativeDecode, 20, AssetGeneration(1))
        .unwrap();
    let other = memory
        .try_reserve(Class::OpaqueAllowance, 15, AssetGeneration(2))
        .unwrap();
    assert_eq!(cancelled.generation(), AssetGeneration(1));
    drop(cancelled);
    assert_eq!(other.generation(), AssetGeneration(2));
    assert_eq!(
        memory.snapshot().bytes_for_class(Class::OpaqueAllowance),
        15
    );
    assert_eq!(memory.snapshot().allocation_count, 1);
    drop(memory); // Lease owns the shared accounting state, not a borrowed actor.
    drop(other);
}

#[test]
fn pressure_preserves_usage_blocks_admission_and_restores_capacity() {
    let memory = coordinator();
    let live = memory
        .try_reserve(Class::GpuTexture, 40, AssetGeneration(1))
        .unwrap();
    assert_eq!(memory.set_pressure(PressureLevel::Warning).unwrap(), 1);
    assert_eq!(memory.set_pressure(PressureLevel::Warning).unwrap(), 1);
    assert_eq!(memory.snapshot().limits, limits(50, 30));
    assert_eq!(memory.snapshot().gpu_bytes, 40);
    assert_eq!(
        memory
            .try_reserve(Class::DecodedPixels, 1, AssetGeneration(2))
            .unwrap_err(),
        Denied::TemporarilyBlocked
    );
    assert_eq!(memory.set_pressure(PressureLevel::Critical).unwrap(), 2);
    drop(live);
    assert_eq!(
        memory
            .try_reserve(Class::GpuTexture, 16, AssetGeneration(2))
            .unwrap_err(),
        Denied::TemporarilyBlocked
    );
    assert_eq!(
        memory
            .try_reserve(Class::GpuTexture, 61, AssetGeneration(2))
            .unwrap_err(),
        Denied::ExceedsPolicy
    );
    assert_eq!(memory.set_pressure(PressureLevel::Normal).unwrap(), 3);
    let _restored = memory
        .try_reserve(Class::GpuTexture, 60, AssetGeneration(2))
        .unwrap();
    assert_eq!(memory.snapshot().pressure_epoch, 3);
    assert_eq!(memory.snapshot().gpu_bytes, 60);
}

#[test]
fn invalid_overflow_and_failed_reservations_do_not_mutate_accounting() {
    let max = limits(u64::MAX, u64::MAX);
    let memory = ImageMemoryCoordinator::new(ImageMemoryPolicy::new(max, max, max).unwrap());
    let _live = memory
        .try_reserve(Class::DecodedPixels, u64::MAX, AssetGeneration(1))
        .unwrap();
    let before = memory.snapshot();
    assert_eq!(
        memory
            .try_reserve(Class::DecodedPixels, 1, AssetGeneration(2))
            .unwrap_err(),
        Denied::AccountingOverflow
    );
    assert_eq!(
        memory
            .try_reserve(Class::DecodedPixels, 0, AssetGeneration(2))
            .unwrap_err(),
        Denied::InvalidSize
    );
    assert_eq!(memory.snapshot(), before);
    drop(_live);
    assert_eq!(memory.snapshot().combined_bytes, 0);
    assert_eq!(
        ImageMemoryPolicy::new(limits(10, 11), limits(5, 5), limits(1, 1)).unwrap_err(),
        Denied::InvalidPolicy
    );
    assert_eq!(
        ImageMemoryPolicy::new(limits(10, 10), limits(11, 5), limits(1, 1)).unwrap_err(),
        Denied::InvalidPolicy
    );
}

#[test]
fn failed_caller_work_and_unwind_release_reservations_without_poisoning() {
    let memory = coordinator();
    let cancelled = std::panic::catch_unwind(|| {
        let _pending = memory
            .try_reserve(Class::NativeDecode, 30, AssetGeneration(1))
            .unwrap();
        panic!("decode cancelled before commit");
    });
    assert!(cancelled.is_err());
    assert_eq!(memory.snapshot().allocation_count, 0);
    let _next = memory
        .try_reserve(Class::NativeDecode, 100, AssetGeneration(2))
        .unwrap();
    assert_eq!(memory.snapshot().combined_bytes, 100);
}

#[test]
fn simultaneous_reservations_are_atomic() {
    let memory = coordinator();
    let barrier = Arc::new(Barrier::new(17));
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..16)
            .map(|generation| {
                let memory = memory.clone();
                let barrier = barrier.clone();
                scope.spawn(move || {
                    barrier.wait();
                    let lease =
                        memory.try_reserve(Class::GpuTexture, 10, AssetGeneration(generation));
                    barrier.wait();
                    barrier.wait();
                    lease.is_ok()
                })
            })
            .collect();
        barrier.wait();
        barrier.wait();
        let held = memory.snapshot();
        barrier.wait();
        let granted = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .filter(|granted| *granted)
            .count();
        assert_eq!(held.gpu_bytes, 60);
        assert_eq!(held.allocation_count, 6);
        assert_eq!(granted, 6);
    });
    assert_eq!(memory.snapshot().combined_bytes, 0);
    assert_eq!(memory.snapshot().peak_gpu_bytes, 60);
}

#[test]
fn baseline_policy_is_explicit_and_pressure_dependent() {
    let memory = ImageMemoryCoordinator::new(ImageMemoryPolicy::baseline_8gb());
    assert_eq!(memory.snapshot().limits, limits(512 << 20, 256 << 20));
    memory.set_pressure(PressureLevel::Warning).unwrap();
    assert_eq!(memory.snapshot().limits, limits(256 << 20, 128 << 20));
    memory.set_pressure(PressureLevel::Critical).unwrap();
    assert_eq!(memory.snapshot().limits, limits(128 << 20, 64 << 20));
}

#[test]
fn retiring_gpu_subset_excludes_cpu_and_remains_charged_until_last_owner() {
    let memory = ImageMemoryCoordinator::new(ImageMemoryPolicy::baseline_8gb());
    let gpu = memory
        .try_reserve(Class::GpuTexture, 32, AssetGeneration(1))
        .unwrap();
    let cpu = memory
        .try_reserve(Class::DecodedPixels, 16, AssetGeneration(1))
        .unwrap();
    gpu.commit().unwrap();
    cpu.commit().unwrap();
    gpu.retire().unwrap();
    cpu.retire().unwrap();
    assert_eq!(memory.snapshot().retiring_gpu_bytes, 32);
    assert_eq!(memory.snapshot().gpu_bytes, 32);
    drop(cpu);
    assert_eq!(memory.snapshot().retiring_gpu_bytes, 32);
    drop(gpu);
    assert_eq!(memory.snapshot().retiring_gpu_bytes, 0);
}
