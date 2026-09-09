use viewer_platform_macos::image_render::{
    InputExclusionRect, InputRect, MacInputRouter, RouteDecision, WindowInput,
};
use viewer_render_core::{
    HoverSample, InteractionMode, LogicalPoint, MagnifySample, Modifiers, NativeInput,
    PointerButton, PointerPhase, PointerSample, ScrollSample,
};

#[allow(dead_code)]
fn native_input_monitor_is_owned_by_the_render_host(
    host: &mut viewer_platform_macos::image_render::MacImageRenderHost,
) -> Result<(), viewer_platform_macos::image_render::SurfaceError> {
    let sink: viewer_platform_macos::image_render::NativeInputSink = std::sync::Arc::new(|_| {});
    host.start_input_monitor(Vec::new(), InteractionMode::Browse, sink)?;
    host.set_input_tool(InteractionMode::Brush)?;
    host.set_input_exclusions(Vec::new())?;
    host.stop_input_monitor()
}

const fn point(x: f64, y: f64) -> LogicalPoint {
    LogicalPoint { x, y }
}

fn router(mode: InteractionMode) -> MacInputRouter {
    MacInputRouter::new(
        InputRect::new(100.0, 80.0, 800.0, 600.0).unwrap(),
        vec![InputExclusionRect::new(700.0, 90.0, 180.0, 120.0).unwrap()],
        mode,
    )
}

fn pointer(
    phase: PointerPhase,
    location: LogicalPoint,
    button: PointerButton,
    timestamp_ns: u64,
) -> WindowInput {
    WindowInput::Pointer {
        phase,
        location,
        button,
        pressure: 0.75,
        shift: false,
        timestamp_ns,
    }
}

#[test]
fn stage_routes_to_renderer_while_chrome_and_editor_stay_in_webview() {
    for mode in [
        InteractionMode::Browse,
        InteractionMode::Point,
        InteractionMode::Arrow,
        InteractionMode::Brush,
        InteractionMode::Rectangle,
        InteractionMode::Ellipse,
    ] {
        let mut router = router(mode);
        assert!(matches!(
            router.classify(pointer(
                PointerPhase::Move,
                point(300.0, 300.0),
                PointerButton::None,
                1,
            )),
            RouteDecision::Render(NativeInput::Pointer(_))
        ));
        assert_eq!(
            router.classify(pointer(
                PointerPhase::Move,
                point(720.0, 120.0),
                PointerButton::None,
                2,
            )),
            RouteDecision::Observe(NativeInput::Hover(HoverSample {
                location: point(620.0, 40.0),
                active: false,
                timestamp_ns: 2,
            })),
            "editor exclusion must win in {mode:?} mode"
        );
        assert_eq!(
            router.classify(pointer(
                PointerPhase::Move,
                point(30.0, 30.0),
                PointerButton::None,
                3,
            )),
            RouteDecision::Observe(NativeInput::Hover(HoverSample {
                location: point(-70.0, -50.0),
                active: false,
                timestamp_ns: 3,
            })),
            "window chrome must stay in WebView in {mode:?} mode"
        );
    }
}

#[test]
fn renderer_capture_survives_leaving_the_stage_until_pointer_up() {
    let mut router = router(InteractionMode::Brush);
    assert_eq!(
        router.classify(pointer(
            PointerPhase::Down,
            point(200.0, 200.0),
            PointerButton::Primary,
            10,
        )),
        RouteDecision::Render(NativeInput::Pointer(PointerSample {
            phase: PointerPhase::Down,
            location: point(100.0, 120.0),
            button: PointerButton::Primary,
            pressure: 0.75,
            modifiers: Modifiers::default(),
            timestamp_ns: 10,
        }))
    );
    assert!(matches!(
        router.classify(pointer(
            PointerPhase::Move,
            point(950.0, 740.0),
            PointerButton::Primary,
            11,
        )),
        RouteDecision::Render(NativeInput::Pointer(_))
    ));
    assert!(matches!(
        router.classify(pointer(
            PointerPhase::Up,
            point(950.0, 740.0),
            PointerButton::Primary,
            12,
        )),
        RouteDecision::Render(NativeInput::Pointer(_))
    ));
    assert_eq!(
        router.classify(pointer(
            PointerPhase::Move,
            point(950.0, 740.0),
            PointerButton::None,
            13,
        )),
        RouteDecision::Observe(NativeInput::Hover(HoverSample {
            location: point(850.0, 660.0),
            active: false,
            timestamp_ns: 13,
        }))
    );
}

#[test]
fn webview_capture_is_not_stolen_when_pointer_crosses_into_stage() {
    let mut router = router(InteractionMode::Rectangle);
    assert_eq!(
        router.classify(pointer(
            PointerPhase::Down,
            point(720.0, 120.0),
            PointerButton::Primary,
            20,
        )),
        RouteDecision::WebView
    );
    assert_eq!(
        router.classify(pointer(
            PointerPhase::Move,
            point(300.0, 300.0),
            PointerButton::Primary,
            21,
        )),
        RouteDecision::Observe(NativeInput::Hover(HoverSample {
            location: point(200.0, 220.0),
            active: false,
            timestamp_ns: 21,
        }))
    );
    assert_eq!(
        router.classify(pointer(
            PointerPhase::Up,
            point(300.0, 300.0),
            PointerButton::Primary,
            22,
        )),
        RouteDecision::WebView
    );
    assert!(matches!(
        router.classify(pointer(
            PointerPhase::Move,
            point(300.0, 300.0),
            PointerButton::None,
            23,
        )),
        RouteDecision::Render(NativeInput::Pointer(_))
    ));
}

#[test]
fn space_temporarily_marks_stage_pointer_samples_for_pan() {
    let mut router = router(InteractionMode::Ellipse);
    assert_eq!(
        router.classify(WindowInput::Space { pressed: true }),
        RouteDecision::WebView
    );
    assert_eq!(
        router.classify(pointer(
            PointerPhase::Down,
            point(250.0, 250.0),
            PointerButton::Primary,
            30,
        )),
        RouteDecision::Render(NativeInput::Pointer(PointerSample {
            phase: PointerPhase::Down,
            location: point(150.0, 170.0),
            button: PointerButton::Primary,
            pressure: 0.75,
            modifiers: Modifiers {
                space: true,
                ..Modifiers::default()
            },
            timestamp_ns: 30,
        }))
    );
    assert_eq!(
        router.classify(WindowInput::Space { pressed: false }),
        RouteDecision::WebView
    );
}

#[test]
fn pointer_samples_preserve_shift_without_stealing_capture_on_repeated_tool_updates() {
    let mut router = router(InteractionMode::Ellipse);
    let WindowInput::Pointer {
        phase,
        location,
        button,
        pressure,
        timestamp_ns,
        ..
    } = pointer(
        PointerPhase::Down,
        point(250.0, 250.0),
        PointerButton::Primary,
        30,
    )
    else {
        unreachable!()
    };
    let result = router.classify(WindowInput::Pointer {
        phase,
        location,
        button,
        pressure,
        timestamp_ns,
        shift: true,
    });
    let RouteDecision::Render(NativeInput::Pointer(sample)) = result else {
        panic!("renderer owns stage pointer")
    };
    assert!(sample.modifiers.shift);
    assert_eq!(router.set_tool(InteractionMode::Ellipse), None);
    assert!(matches!(
        router.classify(pointer(
            PointerPhase::Up,
            point(950.0, 740.0),
            PointerButton::Primary,
            31
        )),
        RouteDecision::Render(NativeInput::Pointer(_))
    ));
}

#[test]
fn scroll_and_magnify_bypass_webview_only_over_the_stage() {
    let mut router = router(InteractionMode::Browse);
    assert_eq!(
        router.classify(WindowInput::Scroll {
            location: point(400.0, 400.0),
            delta: point(3.0, -8.0),
            timestamp_ns: 40,
        }),
        RouteDecision::Render(NativeInput::Scroll(ScrollSample {
            location: point(300.0, 320.0),
            delta: point(3.0, -8.0),
            modifiers: Modifiers::default(),
            timestamp_ns: 40,
        }))
    );
    assert_eq!(
        router.classify(WindowInput::Magnify {
            location: point(400.0, 400.0),
            factor: 1.125,
            timestamp_ns: 41,
        }),
        RouteDecision::Render(NativeInput::Magnify(MagnifySample {
            location: point(300.0, 320.0),
            factor: 1.125,
            timestamp_ns: 41,
        }))
    );
    assert_eq!(
        router.classify(WindowInput::Scroll {
            location: point(720.0, 120.0),
            delta: point(3.0, -8.0),
            timestamp_ns: 42,
        }),
        RouteDecision::WebView
    );
}

#[test]
fn focus_loss_escape_tool_change_and_unmount_cancel_renderer_capture() {
    let mut router = router(InteractionMode::Brush);
    router.classify(pointer(
        PointerPhase::Down,
        point(200.0, 200.0),
        PointerButton::Primary,
        50,
    ));
    assert_eq!(
        router.classify(WindowInput::FocusChanged { focused: false }),
        RouteDecision::Render(NativeInput::Cancel)
    );
    assert_eq!(
        router.classify(pointer(
            PointerPhase::Move,
            point(200.0, 200.0),
            PointerButton::None,
            51,
        )),
        RouteDecision::Ignore
    );
    assert_eq!(
        router.classify(WindowInput::FocusChanged { focused: true }),
        RouteDecision::Ignore
    );

    router.classify(pointer(
        PointerPhase::Down,
        point(200.0, 200.0),
        PointerButton::Primary,
        52,
    ));
    assert_eq!(
        router.classify(WindowInput::Escape),
        RouteDecision::Render(NativeInput::Cancel)
    );
    assert_eq!(router.classify(WindowInput::Escape), RouteDecision::WebView);

    router.classify(pointer(
        PointerPhase::Down,
        point(200.0, 200.0),
        PointerButton::Primary,
        53,
    ));
    assert_eq!(
        router.set_tool(InteractionMode::Arrow),
        Some(NativeInput::Cancel)
    );
    assert_eq!(router.tool(), InteractionMode::Arrow);

    router.classify(pointer(
        PointerPhase::Down,
        point(200.0, 200.0),
        PointerButton::Primary,
        54,
    ));
    assert_eq!(router.cancel(), Some(NativeInput::Cancel));
    assert_eq!(router.cancel(), None);
}

#[test]
fn invalid_input_geometry_is_rejected_and_updates_preserve_capture_owner() {
    assert!(InputRect::new(0.0, 0.0, f64::NAN, 20.0).is_err());
    assert!(InputExclusionRect::new(0.0, 0.0, 0.0, 20.0).is_err());

    let mut router = router(InteractionMode::Point);
    router.classify(pointer(
        PointerPhase::Down,
        point(200.0, 200.0),
        PointerButton::Primary,
        60,
    ));
    router.set_surface(InputRect::new(120.0, 100.0, 700.0, 500.0).unwrap());
    router.set_exclusions(vec![
        InputExclusionRect::new(130.0, 110.0, 100.0, 100.0).unwrap(),
    ]);
    assert!(matches!(
        router.classify(pointer(
            PointerPhase::Up,
            point(900.0, 700.0),
            PointerButton::Primary,
            61,
        )),
        RouteDecision::Render(NativeInput::Pointer(_))
    ));
}

#[test]
fn secondary_clicks_remain_available_to_the_webview() {
    let mut router = router(InteractionMode::Browse);
    assert_eq!(
        router.classify(pointer(
            PointerPhase::Down,
            point(300.0, 300.0),
            PointerButton::Secondary,
            70,
        )),
        RouteDecision::WebView
    );
}
