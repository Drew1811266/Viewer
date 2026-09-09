use viewer_render_core::{
    AssetGeneration, CameraState, LogicalSize, PhysicalSize, Rotation, SceneRevision, SourceSize,
    TransformSnapshot, ViewportLayout,
};
use viewer_render_wgpu::{
    DecodedResource, FrameReceipt, GpuFrameTiming, GpuTimingSupport, ImageRendererDiagnostics,
    ImageRendererPerformanceWorkload, RendererDescriptor, WgpuImageRenderer,
};

fn workload() -> ImageRendererPerformanceWorkload {
    ImageRendererPerformanceWorkload {
        source_width: 7_680,
        source_height: 4_320,
        annotation_count: 500,
        display_hz: 60,
    }
}

// Break caught: supported hardware with pending/failed samples must not be
// labelled as hardware that cannot provide timestamps.
#[test]
fn supported_gpu_without_completed_samples_is_explicit() {
    let mut diagnostics = ImageRendererDiagnostics::new(0);
    diagnostics.set_gpu_timing_support(GpuTimingSupport::Available);

    let receipt = diagnostics.finish(workload());
    assert_eq!(receipt.gpu_timing_support, "available");
    assert_eq!(receipt.gpu_sample_count, 0);
    assert_eq!(receipt.gpu_frame_ms, None);
}

// Break caught: default/failed timestamps or duplicate callbacks must not
// inflate sample count or bias GPU percentiles.
#[test]
fn completed_gpu_samples_are_deduplicated_and_zero_is_not_a_measurement() {
    let mut diagnostics = ImageRendererDiagnostics::new(0);
    let sample = GpuFrameTiming {
        renderer_id: 1,
        frame_index: 8,
        generation: AssetGeneration(2),
        scene_revision: SceneRevision(3),
        gpu_time_ns: 2_000_000,
    };
    diagnostics.record_gpu_timing(sample);
    diagnostics.record_gpu_timing(sample);
    diagnostics.record_gpu_timing(GpuFrameTiming {
        frame_index: 9,
        gpu_time_ns: 0,
        ..sample
    });
    diagnostics.record_gpu_timing(GpuFrameTiming {
        renderer_id: 2,
        gpu_time_ns: 4_000_000,
        ..sample
    });

    let receipt = diagnostics.finish(workload());
    assert_eq!(receipt.gpu_sample_count, 2);
    assert_eq!(receipt.gpu_timing_support, "available");
    assert_eq!(receipt.gpu_timing, "available");
    assert_eq!(receipt.gpu_frame_ms.unwrap().p50, 2.0);
    assert_eq!(receipt.gpu_frame_ms.unwrap().p99, 4.0);
}

// Break caught: stale GPU sample identities surviving a new measurement window.
#[test]
fn resetting_frame_window_resets_gpu_samples_but_preserves_hardware_support() {
    let mut diagnostics = ImageRendererDiagnostics::new(0);
    let sample = GpuFrameTiming {
        renderer_id: 1,
        frame_index: 8,
        generation: AssetGeneration(2),
        scene_revision: SceneRevision(3),
        gpu_time_ns: 2_000_000,
    };
    diagnostics.record_gpu_timing(sample);
    diagnostics.reset_frame_window();
    let cleared = diagnostics.clone().finish(workload());
    assert_eq!(cleared.gpu_timing_support, "available");
    assert_eq!(cleared.gpu_sample_count, 0);
    assert_eq!(cleared.gpu_frame_ms, None);
    diagnostics.record_gpu_timing(sample);
    assert_eq!(diagnostics.finish(workload()).gpu_sample_count, 1);
}

// Break caught: disabling a supported feature, growing readback resources with
// each submission, failing to recycle slots, or labelling an old generation's
// asynchronous result with the current generation/frame.
#[test]
fn metal_timestamp_queries_are_bounded_reused_and_associated_with_the_submitted_frame() {
    if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() || !cfg!(target_os = "macos") {
        return;
    }
    let adapter = timestamp_test_adapter();
    let hardware_supports_timestamps = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
    let logical = LogicalSize::new(1_024.0, 1_024.0).unwrap();
    let mut renderer = WgpuImageRenderer::new(
        RendererDescriptor::headless(
            logical,
            PhysicalSize {
                width: 1_024,
                height: 1_024,
            },
            1.0,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        renderer.gpu_timing_support(),
        if hardware_supports_timestamps {
            GpuTimingSupport::Available
        } else {
            GpuTimingSupport::Unavailable
        }
    );
    if !hardware_supports_timestamps {
        assert!(renderer.poll_gpu_timings().unwrap().is_empty());
        eprintln!("Metal adapter does not support timestamp queries");
        return;
    }
    let renderer_id = renderer.renderer_id();
    let source_size = SourceSize::new(1, 1).unwrap();
    renderer
        .upload_resource(DecodedResource::WholeImage {
            generation: AssetGeneration(7),
            level: 0,
            source_size,
            width: 1,
            height: 1,
            pixels: viewer_render_core::SharedPixels::try_copy_from_slice(
                &viewer_render_core::ImageMemoryCoordinator::new(
                    viewer_render_core::ImageMemoryPolicy::baseline_8gb(),
                ),
                AssetGeneration(1),
                &[0, 0, 255, 255],
            )
            .unwrap(),
        })
        .unwrap();
    renderer
        .set_transform(
            TransformSnapshot::new(
                source_size,
                ViewportLayout::new(logical, 1.0, 1.0).unwrap(),
                CameraState::fit(Rotation::Deg0),
            )
            .unwrap(),
        )
        .unwrap();
    let request = viewer_render_wgpu::FrameRequest {
        scene_revision: SceneRevision(13),
        reasons: viewer_render_wgpu::FrameReasons {
            camera: true,
            ..Default::default()
        },
    };
    for index in 0..3 {
        let receipt = renderer.render(request).unwrap();
        assert_eq!(receipt.frame_index, index);
        assert_eq!(
            receipt.gpu_time_ns, 0,
            "submission must not masquerade as GPU completion"
        );
        assert!(
            !receipt.presented,
            "offscreen timestamps are not presentation evidence"
        );
    }
    renderer.begin_asset_generation(AssetGeneration(8)).unwrap();
    renderer
        .upload_resource(DecodedResource::WholeImage {
            generation: AssetGeneration(8),
            level: 0,
            source_size,
            width: 1,
            height: 1,
            pixels: viewer_render_core::SharedPixels::try_copy_from_slice(
                &viewer_render_core::ImageMemoryCoordinator::new(
                    viewer_render_core::ImageMemoryPolicy::baseline_8gb(),
                ),
                AssetGeneration(1),
                &[0, 255, 0, 255],
            )
            .unwrap(),
        })
        .unwrap();
    renderer
        .set_transform(
            TransformSnapshot::new(
                source_size,
                ViewportLayout::new(logical, 1.0, 1.0).unwrap(),
                CameraState::fit(Rotation::Deg0),
            )
            .unwrap(),
        )
        .unwrap();
    // No readback has been consumed: completed but undrained slots are still
    // reserved, and rendering must continue while GPU sampling is saturated.
    for _ in 0..5 {
        renderer.render(request).unwrap();
    }
    let first = await_samples(&mut renderer, 3);
    assert_eq!(
        first
            .iter()
            .map(|sample| sample.frame_index)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert!(first.iter().all(|sample| sample.renderer_id == renderer_id
        && sample.generation == AssetGeneration(7)
        && sample.scene_revision == SceneRevision(13)
        && sample.gpu_time_ns > 0));
    assert!(renderer.poll_gpu_timings().unwrap().is_empty());
    for index in 8..11 {
        assert_eq!(renderer.render(request).unwrap().frame_index, index);
    }
    let second = await_samples(&mut renderer, 3);
    assert_eq!(
        second
            .iter()
            .map(|sample| sample.frame_index)
            .collect::<Vec<_>>(),
        vec![8, 9, 10]
    );
    assert!(
        second
            .iter()
            .all(|sample| sample.generation == AssetGeneration(8) && sample.gpu_time_ns > 0)
    );
    eprintln!("Metal timestamp samples: first={first:?}, reused={second:?}");
}

fn await_samples(renderer: &mut WgpuImageRenderer, count: usize) -> Vec<GpuFrameTiming> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut samples = Vec::new();
    while samples.len() < count && std::time::Instant::now() < deadline {
        samples.extend(renderer.poll_gpu_timings().unwrap());
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(
        samples.len(),
        count,
        "real GPU queries must complete within the test deadline; samples={samples:?}"
    );
    samples.sort_by_key(|sample| sample.frame_index);
    samples
}

fn timestamp_test_adapter() -> wgpu::Adapter {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::METAL,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    test_block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap()
}

fn test_block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    struct WakeThread(std::thread::Thread);
    impl std::task::Wake for WakeThread {
        fn wake(self: std::sync::Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = std::task::Waker::from(std::sync::Arc::new(WakeThread(std::thread::current())));
    let mut context = std::task::Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(value) => return value,
            std::task::Poll::Pending => std::thread::park(),
        }
    }
}

// Break caught: treating submission-receipt fields as completed timestamp
// queries can attribute a previous frame's time to the currently submitted one.
#[test]
fn submission_receipts_do_not_supply_completed_gpu_measurements() {
    let mut diagnostics = ImageRendererDiagnostics::new(0);
    diagnostics.record_frame(
        FrameReceipt {
            frame_index: 8,
            scene_revision: SceneRevision(2),
            cpu_time_ns: 1_000_000,
            gpu_time_ns: 9_000_000,
            gpu_resource_bytes: 4,
            presented: true,
            magnifier_rendered: false,
        },
        16_000_000,
    );

    let receipt = diagnostics.finish(workload());
    assert_eq!(receipt.gpu_frame_ms, None);
    assert_eq!(receipt.gpu_timing, "unavailable");
}
