use viewer_render_core::SceneRevision;
use viewer_render_wgpu::FrameScheduler;

#[test]
fn high_frequency_input_is_coalesced_to_one_frame_per_display_tick() {
    let mut scheduler = FrameScheduler::default();
    for _ in 0..240 {
        scheduler.frame_state().invalidate_camera();
    }

    let first = scheduler.on_display_tick(1_000_000).unwrap();
    assert!(first.reasons.camera);
    assert!(scheduler.on_display_tick(1_000_000).is_none());

    scheduler.frame_state().invalidate_camera();
    assert!(scheduler.on_display_tick(1_000_000).is_none());
    assert!(scheduler.on_display_tick(9_333_333).is_some());
}

#[test]
fn scheduler_uses_supplied_60_and_120_hz_ticks_without_a_fixed_interval() {
    let mut sixty_hz = FrameScheduler::default();
    sixty_hz.frame_state().invalidate_scene(SceneRevision(12));
    assert!(sixty_hz.on_display_tick(16_666_667).is_some());
    sixty_hz.frame_state().invalidate_camera();
    assert!(sixty_hz.on_display_tick(33_333_334).is_some());

    let mut one_twenty_hz = FrameScheduler::default();
    one_twenty_hz
        .frame_state()
        .invalidate_scene(SceneRevision(12));
    assert!(one_twenty_hz.on_display_tick(8_333_333).is_some());
    one_twenty_hz.frame_state().invalidate_camera();
    assert!(one_twenty_hz.on_display_tick(16_666_666).is_some());
}

#[test]
fn idle_ticks_do_not_create_render_work() {
    let mut scheduler = FrameScheduler::default();

    assert!(scheduler.on_display_tick(8_333_333).is_none());
    assert!(scheduler.on_display_tick(16_666_666).is_none());
    assert!(scheduler.on_display_tick(33_333_333).is_none());
}

#[test]
fn stale_or_out_of_order_display_ticks_do_not_consume_pending_work() {
    let mut scheduler = FrameScheduler::default();
    scheduler.frame_state().invalidate_camera();
    assert!(scheduler.on_display_tick(20).is_some());

    scheduler.frame_state().invalidate_resource();
    assert!(scheduler.on_display_tick(19).is_none());
    assert!(scheduler.on_display_tick(20).is_none());
    assert!(scheduler.on_display_tick(21).unwrap().reasons.resource);
}
