use viewer_render_core::{
    AnnotationGeometry, CameraState, InteractionController, InteractionEvent, InteractionMode,
    LogicalPoint, LogicalSize, Modifiers, NativeInput, PointerButton, PointerPhase, PointerSample,
    Rotation, SceneRevision, SceneSnapshot, SourceSize, TransformSnapshot, ViewportLayout,
};

fn transform() -> TransformSnapshot {
    TransformSnapshot::new(
        SourceSize::new(1_000, 1_000).unwrap(),
        ViewportLayout::new(LogicalSize::new(1_000.0, 1_000.0).unwrap(), 2.0, 1.0).unwrap(),
        CameraState::fit(Rotation::Deg0),
    )
    .unwrap()
}

fn pointer(phase: PointerPhase, x: f64, y: f64, space: bool) -> NativeInput {
    NativeInput::Pointer(PointerSample {
        phase,
        location: LogicalPoint::new(x, y).unwrap(),
        button: PointerButton::Primary,
        pressure: 0.0,
        modifiers: Modifiers { space },
        timestamp_ns: 1,
    })
}

#[test]
fn browse_drag_emits_camera_change_without_a_draft() {
    let scene = SceneSnapshot::empty(SceneRevision(0));
    let mut controller = InteractionController::new(InteractionMode::Browse);

    assert!(
        controller
            .handle_input(
                pointer(PointerPhase::Down, 500.0, 500.0, false),
                &transform(),
                &scene
            )
            .is_empty()
    );
    let events = controller.handle_input(
        pointer(PointerPhase::Move, 520.0, 490.0, false),
        &transform(),
        &scene,
    );

    assert!(matches!(
        events.as_slice(),
        [InteractionEvent::CameraChanged(_)]
    ));
}

#[test]
fn space_temporarily_pans_while_a_shape_tool_is_active() {
    let scene = SceneSnapshot::empty(SceneRevision(0));
    let mut controller = InteractionController::new(InteractionMode::Rectangle);

    controller.handle_input(
        pointer(PointerPhase::Down, 500.0, 500.0, true),
        &transform(),
        &scene,
    );
    let events = controller.handle_input(
        pointer(PointerPhase::Move, 520.0, 500.0, true),
        &transform(),
        &scene,
    );

    assert!(matches!(
        events.as_slice(),
        [InteractionEvent::CameraChanged(_)]
    ));
}

#[test]
fn rectangle_drag_emits_normalized_draft_then_completion() {
    let scene = SceneSnapshot::empty(SceneRevision(0));
    let mut controller = InteractionController::new(InteractionMode::Rectangle);

    let started = controller.handle_input(
        pointer(PointerPhase::Down, 200.0, 300.0, false),
        &transform(),
        &scene,
    );
    let changed = controller.handle_input(
        pointer(PointerPhase::Move, 600.0, 700.0, false),
        &transform(),
        &scene,
    );
    let completed = controller.handle_input(
        pointer(PointerPhase::Up, 600.0, 700.0, false),
        &transform(),
        &scene,
    );

    assert!(matches!(
        started.as_slice(),
        [InteractionEvent::DraftStarted(_)]
    ));
    assert!(matches!(
        changed.as_slice(),
        [InteractionEvent::DraftChanged(_)]
    ));
    let [InteractionEvent::DraftCompleted(AnnotationGeometry::Rectangle { rect })] =
        completed.as_slice()
    else {
        panic!("expected one completed rectangle")
    };
    assert!((rect.x - 0.2).abs() < 1e-12);
    assert!((rect.y - 0.3).abs() < 1e-12);
    assert!((rect.width - 0.4).abs() < 1e-12);
    assert!((rect.height - 0.4).abs() < 1e-12);
}

#[test]
fn cancelling_a_drawing_discards_the_native_draft() {
    let scene = SceneSnapshot::empty(SceneRevision(0));
    let mut controller = InteractionController::new(InteractionMode::Brush);
    controller.handle_input(
        pointer(PointerPhase::Down, 200.0, 300.0, false),
        &transform(),
        &scene,
    );

    assert_eq!(
        controller.handle_input(NativeInput::Cancel, &transform(), &scene),
        vec![InteractionEvent::DraftCancelled]
    );
}

#[test]
fn cancelling_a_pan_does_not_report_a_draft_cancellation() {
    let scene = SceneSnapshot::empty(SceneRevision(0));
    let mut controller = InteractionController::new(InteractionMode::Browse);
    controller.handle_input(
        pointer(PointerPhase::Down, 200.0, 300.0, false),
        &transform(),
        &scene,
    );

    assert!(
        controller
            .handle_input(NativeInput::Cancel, &transform(), &scene)
            .is_empty()
    );
}
