use viewer_render_core::SceneRevision;
use viewer_render_wgpu::{
    FrameReceipt, ImageRendererDiagnostics, ImageRendererPerformanceWorkload,
};

fn workload() -> ImageRendererPerformanceWorkload {
    ImageRendererPerformanceWorkload {
        source_width: 7_680,
        source_height: 4_320,
        annotation_count: 500,
        display_hz: 60,
    }
}

fn frame(index: u64, presented: bool) -> FrameReceipt {
    FrameReceipt {
        frame_index: index,
        scene_revision: SceneRevision(1),
        cpu_time_ns: 1_000_000,
        gpu_time_ns: 0,
        gpu_resource_bytes: 140 * 1024 * 1024,
        presented,
        magnifier_rendered: false,
    }
}

#[test]
fn diagnostics_use_surface_slots_and_report_unmeasured_gpu_time_explicitly() {
    let mut diagnostics = ImageRendererDiagnostics::new(1_000_000_000);
    diagnostics.mark_first_interactive(1_250_000_000);
    diagnostics.begin_warm_open(2_000_000_000);
    diagnostics.mark_warm_first_interactive(2_080_000_000);
    for index in 0..5 {
        diagnostics.record_frame(frame(index, true), 3_000_000_000 + index * 16_000_000);
    }
    for save_ms in [10, 20, 30, 40, 50] {
        diagnostics.record_save_commit_ns(save_ms * 1_000_000);
    }
    let receipt = diagnostics.finish(workload());
    assert_eq!(receipt.frame_ms.p95, 16.0);
    assert_eq!(receipt.first_interactive_ms, 250.0);
    assert_eq!(receipt.warm_first_interactive_ms, 80.0);
    assert_eq!(receipt.gpu_timing, "unavailable");
    assert_eq!(receipt.gpu_frame_ms, None);
    assert_eq!(receipt.save_commit_ms.p99, 50.0);
}

#[test]
fn offscreen_frames_never_count_as_window_presentation_evidence() {
    let mut diagnostics = ImageRendererDiagnostics::new(0);
    for index in 0..240 {
        diagnostics.record_frame(frame(index, false), index * 100_000);
    }
    let receipt = diagnostics.finish(workload());
    assert_eq!(receipt.workload.frame_samples, 0);
    assert_eq!(receipt.presented_fps, 0.0);
}

#[test]
fn diagnostics_count_only_explicitly_dropped_input_boundaries() {
    let mut diagnostics = ImageRendererDiagnostics::new(0);
    diagnostics.record_input_sample(false);
    diagnostics.record_input_sample(true);
    diagnostics.record_input_sample(false);
    assert_eq!(diagnostics.dropped_input_samples(), 1);
}
