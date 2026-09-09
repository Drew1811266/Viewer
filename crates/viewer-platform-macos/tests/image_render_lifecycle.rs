use std::time::Duration;
use viewer_platform_macos::image_render::{
    DisplayTickSignal, ImageRenderHostState, MacImageRenderHost, SurfaceError, SurfaceLayout,
    system_ordinal_glyph_atlas,
};

#[allow(dead_code)]
fn native_host_api_is_scoped_to_the_tauri_window<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    layout: SurfaceLayout,
) -> Result<(), SurfaceError> {
    let mut host = MacImageRenderHost::mount(window, layout)?;
    let _surface_handles = host.surface().renderer_surface_handles()?;
    let _glyph_atlas = system_ordinal_glyph_atlas()?;
    host.set_layout(layout)?;
    host.set_visible(true)?;
    host.start_display_link(DisplayTickSignal::default())?;
    host.stop_display_link()?;
    host.unmount()
}

#[test]
fn display_switch_requires_one_refresh_link_rebuild() {
    let mut state = ImageRenderHostState::mounted();
    assert!(state.start_display_link_for(Some(41)));
    assert!(!state.rebind_display(Some(41)));

    assert!(state.rebind_display(Some(42)));
    assert_eq!(state.display_id(), Some(42));
    assert!(!state.rebind_display(Some(42)));
}

#[test]
fn host_lifecycle_stops_refresh_before_idempotent_unmount() {
    let mut state = ImageRenderHostState::mounted();
    assert!(state.set_visible(true));
    assert!(state.start_display_link());
    assert!(state.display_link_running());

    assert!(state.unmount());
    assert!(!state.is_mounted());
    assert!(!state.display_link_running());
    assert!(!state.unmount());
}

#[test]
fn unmounted_host_rejects_visibility_and_display_link_transitions() {
    let mut state = ImageRenderHostState::mounted();
    state.unmount();

    assert!(!state.set_visible(true));
    assert!(!state.start_display_link());
    assert!(!state.stop_display_link());
}

#[test]
fn display_tick_signal_is_bounded_and_keeps_only_the_latest_tick() {
    let signal = DisplayTickSignal::default();
    for timestamp in 1..=10_000 {
        signal.publish(timestamp);
    }

    assert_eq!(signal.take_latest(), Some(10_000));
    assert_eq!(signal.take_latest(), None);
}

#[test]
fn display_tick_signal_does_not_reopen_after_close() {
    let signal = DisplayTickSignal::default();
    signal.publish(10);
    signal.close();
    signal.publish(20);

    assert_eq!(signal.take_latest(), None);
    assert!(signal.is_closed());
}

#[test]
fn display_tick_signal_preserves_the_full_timestamp_range() {
    let signal = DisplayTickSignal::default();
    signal.publish(u64::MAX);

    assert_eq!(signal.take_latest(), Some(u64::MAX));
    assert_eq!(signal.take_latest(), None);
}

#[test]
fn display_tick_wakes_a_parked_renderer_without_a_polling_timer() {
    let signal = DisplayTickSignal::default();
    let worker_signal = signal.clone();
    let (ready_sender, ready_receiver) = std::sync::mpsc::sync_channel(1);
    let (result_sender, result_receiver) = std::sync::mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        assert!(worker_signal.bind_current_thread());
        ready_sender.send(()).unwrap();
        std::thread::park();
        result_sender.send(worker_signal.take_latest()).unwrap();
    });
    ready_receiver.recv_timeout(Duration::from_secs(1)).unwrap();

    signal.publish(42);

    assert_eq!(
        result_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap(),
        Some(42)
    );
    worker.join().unwrap();
}
