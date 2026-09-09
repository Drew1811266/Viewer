use viewer_render_core::{LogicalSize, PhysicalSize, SceneRevision};
use viewer_render_wgpu::{
    FrameRing, FrameState, RendererDescriptor, SurfaceAcquireFailure, SurfaceRecovery,
    WgpuImageRenderer, surface_recovery,
};

#[test]
fn clean_frame_state_does_not_submit_work() {
    let mut state = FrameState::default();

    assert!(state.take_request().is_none());
}

#[test]
fn camera_scene_resource_and_surface_changes_coalesce_into_one_request() {
    let mut state = FrameState::default();
    state.invalidate_camera();
    state.invalidate_scene(SceneRevision(7));
    state.invalidate_resource();
    state.invalidate_surface();

    let request = state.take_request().expect("dirty state must render");

    assert_eq!(request.scene_revision, SceneRevision(7));
    assert!(request.reasons.camera);
    assert!(request.reasons.scene);
    assert!(request.reasons.resource);
    assert!(request.reasons.surface);
    assert!(state.take_request().is_none());
}

#[test]
fn three_frame_ring_reuses_only_completed_slots() {
    let mut ring = FrameRing::default();
    let first = ring.acquire().unwrap();
    let second = ring.acquire().unwrap();
    let third = ring.acquire().unwrap();

    assert!(ring.acquire().is_none());
    ring.complete(second);
    assert_eq!(ring.acquire(), Some(second));
    assert_ne!(first, third);
}

#[test]
fn surface_failures_have_explicit_recovery_actions() {
    assert_eq!(
        surface_recovery(SurfaceAcquireFailure::Outdated),
        SurfaceRecovery::Reconfigure
    );
    assert_eq!(
        surface_recovery(SurfaceAcquireFailure::Lost),
        SurfaceRecovery::RecreateSurface
    );
    assert_eq!(
        surface_recovery(SurfaceAcquireFailure::Timeout),
        SurfaceRecovery::RetryNextFrame
    );
    assert_eq!(
        surface_recovery(SurfaceAcquireFailure::Occluded),
        SurfaceRecovery::WaitUntilVisible
    );
    assert_eq!(
        surface_recovery(SurfaceAcquireFailure::Validation),
        SurfaceRecovery::ReportValidation
    );
    assert_eq!(
        surface_recovery(SurfaceAcquireFailure::OutOfMemory),
        SurfaceRecovery::Terminate
    );
}

#[test]
fn descriptor_rejects_invalid_scale_before_gpu_initialization() {
    let result = RendererDescriptor::headless(
        LogicalSize::new(800.0, 600.0).unwrap(),
        PhysicalSize {
            width: 1_600,
            height: 1_200,
        },
        0.0,
    );

    assert!(result.is_err());
}

#[test]
fn metal_adapter_smoke_test_is_explicitly_opt_in() {
    if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
        return;
    }
    if !cfg!(target_os = "macos") {
        return;
    }

    let descriptor = RendererDescriptor::headless(
        LogicalSize::new(64.0, 64.0).unwrap(),
        PhysicalSize {
            width: 64,
            height: 64,
        },
        1.0,
    )
    .unwrap();
    let renderer = WgpuImageRenderer::new(descriptor).expect("Metal adapter should initialize");

    assert_eq!(renderer.adapter_backend(), wgpu::Backend::Metal);
}
