use viewer_render_core::{
    AllocationClass, AssetGeneration, ImageMemoryCoordinator, ImageMemoryPolicy, LiveMemoryLimits,
    MemoryAdmissionError, SharedPixels,
};

fn memory() -> ImageMemoryCoordinator {
    let limits = LiveMemoryLimits {
        combined_bytes: 64,
        gpu_bytes: 32,
    };
    ImageMemoryCoordinator::new(ImageMemoryPolicy::new(limits, limits, limits).unwrap())
}

#[test]
fn shared_pixel_storage_preserves_pointer_and_releases_on_last_owner() {
    let memory = memory();
    let mut pixels = SharedPixels::try_zeroed(&memory, AssetGeneration(1), 16).unwrap();
    pixels.get_mut().unwrap()[0] = 7;
    let pointer = pixels.as_ptr();
    let shared = pixels.clone();
    assert!(pixels.get_mut().is_none());
    assert_eq!(shared.as_ptr(), pointer);
    assert_eq!(
        memory
            .snapshot()
            .bytes_for_class(AllocationClass::DecodedPixels),
        16
    );
    drop(pixels);
    assert_eq!(shared[0], 7);
    assert_eq!(memory.snapshot().combined_bytes, 16);
    drop(shared);
    assert_eq!(memory.snapshot().combined_bytes, 0);
}

#[test]
fn independent_copy_is_charged_and_failed_admission_does_not_allocate() {
    let memory = memory();
    let pixels = SharedPixels::try_zeroed(&memory, AssetGeneration(1), 32).unwrap();
    let copy = SharedPixels::try_copy_from_slice(&memory, AssetGeneration(2), &pixels).unwrap();
    assert_ne!(pixels.as_ptr(), copy.as_ptr());
    assert_eq!(memory.snapshot().combined_bytes, 64);
    assert_eq!(
        SharedPixels::try_zeroed(&memory, AssetGeneration(3), 1).unwrap_err(),
        MemoryAdmissionError::TemporarilyBlocked
    );
    drop(copy);
    assert_eq!(memory.snapshot().combined_bytes, 32);
}

#[test]
fn cancelled_or_panicking_initialization_drops_storage_and_lease() {
    let memory = memory();
    let result = std::panic::catch_unwind(|| {
        let _pixels = SharedPixels::try_zeroed(&memory, AssetGeneration(1), 16).unwrap();
        assert_eq!(memory.snapshot().combined_bytes, 16);
        panic!("conversion cancelled");
    });
    assert!(result.is_err());
    assert_eq!(memory.snapshot().combined_bytes, 0);
    assert_eq!(memory.snapshot().peak_combined_bytes, 16);
    assert!(SharedPixels::try_zeroed(&memory, AssetGeneration(1), u64::MAX).is_err());
}

#[test]
fn impossible_vec_capacity_returns_allocation_error_and_releases_reservation() {
    let limits = LiveMemoryLimits {
        combined_bytes: u64::MAX,
        gpu_bytes: u64::MAX,
    };
    let memory =
        ImageMemoryCoordinator::new(ImageMemoryPolicy::new(limits, limits, limits).unwrap());
    // Deterministic capacity overflow before any actual large allocation.
    assert_eq!(
        SharedPixels::try_zeroed(&memory, AssetGeneration(1), u64::MAX).unwrap_err(),
        MemoryAdmissionError::AllocationFailed
    );
    assert_eq!(memory.snapshot().combined_bytes, 0);
    assert_eq!(memory.snapshot().allocation_count, 0);
}
