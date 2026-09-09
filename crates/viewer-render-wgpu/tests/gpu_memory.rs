use viewer_render_core::{
    AllocationClass, ImageMemoryCoordinator, ImageMemoryPolicy, LiveMemoryLimits, LogicalSize,
    PhysicalSize,
};
use viewer_render_wgpu::{RendererDescriptor, WgpuImageRenderer};

fn descriptor(memory: ImageMemoryCoordinator) -> RendererDescriptor {
    RendererDescriptor::headless(
        LogicalSize::new(32.0, 32.0).unwrap(),
        PhysicalSize {
            width: 32,
            height: 32,
        },
        1.0,
    )
    .unwrap()
    .with_memory(memory)
}

#[test]
fn pressure_reclaim_removes_prefetch_then_nonvisible_lru_and_keeps_preview_main_and_lens() {
    if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
        return;
    }
    use viewer_render_core::{AllocationPhase, AssetGeneration, SharedPixels, SourceSize};
    use viewer_render_wgpu::{DecodedResource, ResourceKey};
    let memory = ImageMemoryCoordinator::new(ImageMemoryPolicy::baseline_8gb());
    let mut renderer = WgpuImageRenderer::new(descriptor(memory.clone())).unwrap();
    let keys: Vec<_> = (0..6)
        .map(|level| ResourceKey::WholeImage { level })
        .collect();
    for level in 0..6 {
        let side = if level == 5 { 1 } else { 512 };
        let handle = renderer
            .upload_resource(DecodedResource::WholeImage {
                generation: AssetGeneration(1),
                level,
                source_size: SourceSize::new(4096, 4096).unwrap(),
                width: side,
                height: side,
                pixels: SharedPixels::try_zeroed(
                    &memory,
                    AssetGeneration(1),
                    u64::from(side * side * 4),
                )
                .unwrap(),
            })
            .unwrap();
        while !renderer.resource_is_ready(handle) {
            renderer.poll_gpu_timings().unwrap();
        }
    }
    renderer.touch_resources(&[keys[1]]);
    renderer.reclaim_resources(
        &[keys[2], keys[3]],
        Some(keys[5]),
        &[keys[4]],
        (3 << 20) + 4,
    );
    assert!(
        !renderer.resource_key_is_ready(keys[4]),
        "prefetch is first to retire"
    );
    assert!(
        !renderer.resource_key_is_ready(keys[0]),
        "least-recent nonvisible detail retires before a touched peer"
    );
    for key in [keys[1], keys[2], keys[3], keys[5]] {
        assert!(renderer.resource_key_is_ready(key));
    }
    assert!(memory.snapshot().bytes_for_phase(AllocationPhase::Retiring) >= 2 << 20);
    let before = memory.snapshot().gpu_bytes;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while memory.snapshot().bytes_for_phase(AllocationPhase::Retiring) != 0 {
        assert!(std::time::Instant::now() < deadline);
        renderer.poll_gpu_timings().unwrap();
    }
    assert!(memory.snapshot().gpu_bytes <= before - (2 << 20));
}

#[test]
fn initialization_reserves_three_upload_slots_and_fixed_gpu_buffers() {
    if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
        return;
    }
    let memory = ImageMemoryCoordinator::new(ImageMemoryPolicy::baseline_8gb());
    let renderer = WgpuImageRenderer::new(descriptor(memory.clone())).unwrap();
    assert_eq!(
        memory
            .snapshot()
            .bytes_for_class(AllocationClass::UploadStaging),
        3 << 20
    );
    assert!(
        memory
            .snapshot()
            .bytes_for_class(AllocationClass::RendererBuffers)
            > 0
    );
    drop(renderer);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while memory.snapshot().combined_bytes != 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "renderer teardown must complete real GPU retirement"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

#[test]
fn requested_retirement_progress_completes_without_display_poll_and_then_stops_waking() {
    if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
        return;
    }
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use viewer_render_core::{AllocationPhase, AssetGeneration, SharedPixels, SourceSize};
    use viewer_render_wgpu::DecodedResource;
    let memory = ImageMemoryCoordinator::new(ImageMemoryPolicy::baseline_8gb());
    let mut renderer = WgpuImageRenderer::new(descriptor(memory.clone())).unwrap();
    renderer
        .upload_resource(DecodedResource::WholeImage {
            generation: AssetGeneration(1),
            level: 0,
            source_size: SourceSize::new(1, 1).unwrap(),
            width: 1,
            height: 1,
            pixels: SharedPixels::try_zeroed(&memory, AssetGeneration(1), 4).unwrap(),
        })
        .unwrap();
    renderer.retain_resources(&[]);
    assert!(memory.snapshot().bytes_for_phase(AllocationPhase::Retiring) > 0);
    let wakes = Arc::new(AtomicUsize::new(0));
    let counter = wakes.clone();
    let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        counter.fetch_add(1, Ordering::Relaxed);
    });
    renderer.wake_when_gpu_progress(wake.clone());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while wakes.load(Ordering::Relaxed) == 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "retirement must wake an occluded actor without display ticks"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(
        memory.snapshot().bytes_for_phase(AllocationPhase::Retiring),
        0
    );
    let completed_wakes = wakes.load(Ordering::Relaxed);
    for _ in 0..100 {
        renderer.wake_when_gpu_progress(wake.clone());
    }
    std::thread::sleep(std::time::Duration::from_millis(40));
    assert_eq!(
        wakes.load(Ordering::Relaxed),
        completed_wakes,
        "no pending ownership means no permanent polling/waking"
    );
    // Another renderer/device's retirement must not turn this empty queue into
    // an immediately completing wake loop. Its service job owns that progress.
    let foreign = memory
        .try_reserve(AllocationClass::GpuTexture, 4, AssetGeneration(2))
        .unwrap();
    foreign.commit().unwrap();
    foreign.retire().unwrap();
    renderer.wake_when_gpu_progress(wake);
    std::thread::sleep(std::time::Duration::from_millis(40));
    assert_eq!(
        wakes.load(Ordering::Relaxed),
        completed_wakes,
        "foreign retirement cannot poll an empty current queue"
    );
}

#[test]
fn insufficient_shared_gpu_capacity_rejects_before_pool_allocation() {
    if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
        return;
    }
    let limits = LiveMemoryLimits {
        combined_bytes: 4 << 20,
        gpu_bytes: 2 << 20,
    };
    let memory =
        ImageMemoryCoordinator::new(ImageMemoryPolicy::new(limits, limits, limits).unwrap());
    assert!(WgpuImageRenderer::new(descriptor(memory.clone())).is_err());
    assert!(memory.snapshot().peak_gpu_bytes <= 2 << 20);
}

#[test]
fn large_row_bands_stay_hidden_until_ordered_and_retirement_needs_idle_poll_without_timestamps() {
    if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
        return;
    }
    use viewer_render_core::{
        AllocationPhase, AssetGeneration, CameraState, Rotation, SharedPixels, SourceSize,
        TransformSnapshot, ViewportLayout,
    };
    use viewer_render_wgpu::{DecodedResource, GpuTimingSupport};
    let memory = ImageMemoryCoordinator::new(ImageMemoryPolicy::baseline_8gb());
    let mut renderer =
        WgpuImageRenderer::new(descriptor(memory.clone()).without_timestamp_queries()).unwrap();
    assert_eq!(renderer.gpu_timing_support(), GpuTimingSupport::Unavailable);
    let base_gpu = memory.snapshot().gpu_bytes;
    let source = SourceSize::new(1025, 1025).unwrap();
    let mut pixels =
        SharedPixels::try_zeroed(&memory, AssetGeneration(1), 1025 * 1025 * 4).unwrap();
    for (row, bytes) in pixels.get_mut().unwrap().chunks_mut(1025 * 4).enumerate() {
        for pixel in bytes.chunks_mut(4) {
            pixel.copy_from_slice(if row < 512 {
                &[0, 0, 255, 255]
            } else {
                &[255, 0, 0, 255]
            });
        }
    }
    let shared = pixels.clone();
    let handle = renderer
        .upload_resource(DecodedResource::WholeImage {
            generation: AssetGeneration(1),
            level: 0,
            source_size: source,
            width: 1025,
            height: 1025,
            pixels,
        })
        .unwrap();
    assert!(!renderer.resource_is_ready(handle));
    assert!(
        renderer
            .retained_scene_resources()
            .image_handles()
            .is_empty()
    );
    assert_eq!(
        memory
            .snapshot()
            .bytes_for_class(AllocationClass::DecodedPixels),
        shared.len() as u64
    );
    assert_eq!(
        memory
            .snapshot()
            .bytes_for_class(AllocationClass::UploadStaging),
        3 << 20
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !renderer.resource_is_ready(handle) {
        assert!(std::time::Instant::now() < deadline);
        renderer.poll_gpu_timings().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    renderer
        .set_transform(
            TransformSnapshot::new(
                source,
                ViewportLayout::new(LogicalSize::new(32.0, 32.0).unwrap(), 1.0, 1.0).unwrap(),
                CameraState::fit(Rotation::Deg0),
            )
            .unwrap(),
        )
        .unwrap();
    let request = renderer.on_display_tick(1).unwrap();
    let (_, capture) = renderer.render_headless_capture(request).unwrap();
    let top = &capture[(4 * 32 + 16) * 4..(4 * 32 + 16) * 4 + 4];
    let bottom = &capture[(28 * 32 + 16) * 4..(28 * 32 + 16) * 4 + 4];
    assert!(top[2] > 240 && top[0] < 10, "{top:?}");
    assert!(bottom[0] > 240 && bottom[2] < 10, "{bottom:?}");
    renderer.retain_resources(&[]);
    eprintln!(
        "row-band retirement: retiring={} live_gpu={} peak_gpu={}",
        memory.snapshot().bytes_for_phase(AllocationPhase::Retiring),
        memory.snapshot().gpu_bytes,
        memory.snapshot().peak_gpu_bytes
    );
    assert!(memory.snapshot().bytes_for_phase(AllocationPhase::Retiring) >= 1025 * 1025 * 4);
    assert!(memory.snapshot().gpu_bytes > base_gpu);
    while memory.snapshot().gpu_bytes > base_gpu {
        assert!(std::time::Instant::now() < deadline);
        renderer.poll_gpu_timings().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(
        memory
            .snapshot()
            .bytes_for_class(AllocationClass::UploadStaging),
        3 << 20
    );
    drop(shared);
    assert_eq!(
        memory
            .snapshot()
            .bytes_for_class(AllocationClass::DecodedPixels),
        0
    );
}
