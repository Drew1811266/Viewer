use viewer_render_core::{
    AnnotationGeometry as Geometry, AnnotationId, AnnotationNode, CameraState,
    InteractionController, InteractionEvent as Event, InteractionMode, LogicalPoint, LogicalSize,
    Modifiers, NativeInput, NormalizedPoint as Point, NormalizedRect, PointerButton, PointerPhase,
    PointerSample, Rotation, SceneRevision, SceneSnapshot, SourceSize, TransformSnapshot,
    ViewportLayout,
};

fn transform(rotation: Rotation) -> TransformSnapshot {
    TransformSnapshot::new(
        SourceSize::new(1000, 500).unwrap(),
        ViewportLayout::new(LogicalSize::new(1000.0, 1000.0).unwrap(), 2.0, 1.0).unwrap(),
        CameraState::fit(rotation),
    )
    .unwrap()
}
fn point(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}
fn scene(geometry: Geometry, selected: bool) -> SceneSnapshot {
    let mut node = AnnotationNode::new(AnnotationId::new("item").unwrap(), 1, geometry).unwrap();
    node.selected = selected;
    SceneSnapshot::new(SceneRevision(1), vec![node], None).unwrap()
}
fn input(phase: PointerPhase, position: LogicalPoint, space: bool) -> NativeInput {
    NativeInput::Pointer(PointerSample {
        phase,
        location: position,
        button: PointerButton::Primary,
        pressure: 0.0,
        modifiers: Modifiers {
            space,
            ..Modifiers::default()
        },
        timestamp_ns: 1,
    })
}
fn drag(scene: &SceneSnapshot, start: Point, end: Point, rotation: Rotation) -> Vec<Event> {
    let transform = transform(rotation);
    let mut controller = InteractionController::new(InteractionMode::Browse);
    controller.handle_input(
        input(PointerPhase::Down, transform.image_to_view(start), false),
        &transform,
        scene,
    );
    controller.handle_input(
        input(PointerPhase::Up, transform.image_to_view(end), false),
        &transform,
        scene,
    )
}
fn completed(geometry: Geometry) -> Vec<Event> {
    vec![Event::GeometryEditCompleted {
        annotation_id: AnnotationId::new("item").unwrap(),
        geometry,
    }]
}

#[test]
fn selected_point_moves_in_normalized_coordinates_after_rotation() {
    for rotation in [
        Rotation::Deg0,
        Rotation::Deg90,
        Rotation::Deg180,
        Rotation::Deg270,
    ] {
        assert_eq!(
            drag(
                &scene(
                    Geometry::Point {
                        position: point(0.2, 0.3)
                    },
                    true
                ),
                point(0.2, 0.3),
                point(0.4, 0.5),
                rotation
            ),
            completed(Geometry::Point {
                position: point(0.4, 0.5)
            })
        );
    }
}

#[test]
fn first_click_selects_unselected_geometry_without_editing_or_panning() {
    let scene = scene(
        Geometry::Point {
            position: point(0.2, 0.3),
        },
        false,
    );
    let transform = transform(Rotation::Deg0);
    let mut controller = InteractionController::new(InteractionMode::Browse);
    let down = controller.handle_input(
        input(
            PointerPhase::Down,
            transform.image_to_view(point(0.2, 0.3)),
            false,
        ),
        &transform,
        &scene,
    );
    assert_eq!(
        down,
        vec![Event::SelectionChanged(Some(
            AnnotationId::new("item").unwrap()
        ))]
    );
    let up = controller.handle_input(
        input(
            PointerPhase::Up,
            transform.image_to_view(point(0.4, 0.5)),
            false,
        ),
        &transform,
        &scene,
    );
    assert!(up.is_empty());
}

#[test]
fn rectangle_and_ellipse_interiors_move_without_changing_size() {
    for ellipse in [false, true] {
        let original = NormalizedRect::new(0.1, 0.2, 0.4, 0.5).unwrap();
        let expected = NormalizedRect::new(0.3, 0.3, 0.4, 0.5).unwrap();
        let shape = |rect| {
            if ellipse {
                Geometry::Ellipse { rect }
            } else {
                Geometry::Rectangle { rect }
            }
        };
        assert_eq!(
            drag(
                &scene(shape(original), true),
                point(0.3, 0.4),
                point(0.5, 0.5),
                Rotation::Deg0
            ),
            completed(shape(expected))
        );
    }
}

#[test]
fn every_box_corner_resizes_about_the_opposite_corner_including_crossing() {
    for ellipse in [false, true] {
        let shape = |rect| {
            if ellipse {
                Geometry::Ellipse { rect }
            } else {
                Geometry::Rectangle { rect }
            }
        };
        for (start, end, expected) in [
            ((0.2, 0.2), (0.1, 0.1), (0.1, 0.1, 0.5, 0.5)),
            ((0.6, 0.2), (0.8, 0.1), (0.2, 0.1, 0.6, 0.5)),
            ((0.6, 0.6), (0.8, 0.9), (0.2, 0.2, 0.6, 0.7)),
            ((0.2, 0.6), (0.1, 0.8), (0.1, 0.2, 0.5, 0.6)),
            ((0.2, 0.2), (0.8, 0.9), (0.6, 0.6, 0.2, 0.3)),
        ] {
            assert_eq!(
                drag(
                    &scene(
                        shape(NormalizedRect::new(0.2, 0.2, 0.4, 0.4).unwrap()),
                        true
                    ),
                    point(start.0, start.1),
                    point(end.0, end.1),
                    Rotation::Deg0
                ),
                completed(shape(
                    NormalizedRect::new(expected.0, expected.1, expected.2, expected.3).unwrap()
                ))
            );
        }
    }
}

#[test]
fn arrow_body_moves_as_a_unit_and_each_endpoint_edits_independently() {
    let original = Geometry::Arrow {
        tail: point(0.2, 0.2),
        head: point(0.6, 0.6),
    };
    for (start, end, tail, head) in [
        ((0.4, 0.4), (0.5, 0.6), (0.3, 0.4), (0.7, 0.8)),
        ((0.2, 0.2), (0.1, 0.4), (0.1, 0.4), (0.6, 0.6)),
        ((0.6, 0.6), (0.8, 0.5), (0.2, 0.2), (0.8, 0.5)),
    ] {
        assert_eq!(
            drag(
                &scene(original.clone(), true),
                point(start.0, start.1),
                point(end.0, end.1),
                Rotation::Deg0
            ),
            completed(Geometry::Arrow {
                tail: point(tail.0, tail.1),
                head: point(head.0, head.1)
            })
        );
    }
}

#[test]
fn moving_past_the_image_edge_clamps_the_whole_shape() {
    let scene = scene(
        Geometry::Rectangle {
            rect: NormalizedRect::new(0.2, 0.2, 0.4, 0.4).unwrap(),
        },
        true,
    );
    let transform = transform(Rotation::Deg90);
    let mut controller = InteractionController::new(InteractionMode::Browse);
    controller.handle_input(
        input(
            PointerPhase::Down,
            transform.image_to_view(point(0.4, 0.4)),
            false,
        ),
        &transform,
        &scene,
    );
    let events = controller.handle_input(
        input(
            PointerPhase::Up,
            transform.image_to_view(Point { x: 1.8, y: -0.3 }),
            false,
        ),
        &transform,
        &scene,
    );
    assert_eq!(
        events,
        completed(Geometry::Rectangle {
            rect: NormalizedRect::new(0.6, 0.0, 0.4, 0.4).unwrap()
        })
    );
}

#[test]
fn cancellation_and_subpixel_click_leave_the_original_geometry() {
    let scene = scene(
        Geometry::Point {
            position: point(0.2, 0.3),
        },
        true,
    );
    let transform = transform(Rotation::Deg0);
    for cancelled in [false, true] {
        let mut controller = InteractionController::new(InteractionMode::Browse);
        let start = transform.image_to_view(point(0.2, 0.3));
        controller.handle_input(input(PointerPhase::Down, start, false), &transform, &scene);
        let finish = if cancelled {
            NativeInput::Cancel
        } else {
            input(
                PointerPhase::Up,
                LogicalPoint {
                    x: start.x + 0.5,
                    y: start.y,
                },
                false,
            )
        };
        assert_eq!(
            controller.handle_input(finish, &transform, &scene),
            vec![Event::GeometryEditCancelled {
                annotation_id: AnnotationId::new("item").unwrap()
            }]
        );
    }
}

#[test]
fn zero_area_resize_and_collapsed_arrow_keep_the_last_valid_candidate() {
    let arrow = Geometry::Arrow {
        tail: point(0.2, 0.2),
        head: point(0.6, 0.6),
    };
    let rect = Geometry::Rectangle {
        rect: NormalizedRect::new(0.2, 0.2, 0.4, 0.4).unwrap(),
    };
    for geometry in [arrow, rect] {
        assert_eq!(
            drag(
                &scene(geometry, true),
                point(0.2, 0.2),
                point(0.6, 0.6),
                Rotation::Deg0
            ),
            vec![Event::GeometryEditCancelled {
                annotation_id: AnnotationId::new("item").unwrap()
            }]
        );
    }
}

#[test]
fn space_pans_selected_geometry_and_does_not_change_selection_on_click() {
    let scene = scene(
        Geometry::Point {
            position: point(0.2, 0.3),
        },
        true,
    );
    let transform = transform(Rotation::Deg0);
    let mut controller = InteractionController::new(InteractionMode::Browse);
    let start = transform.image_to_view(point(0.2, 0.3));
    assert!(
        controller
            .handle_input(input(PointerPhase::Down, start, true), &transform, &scene)
            .is_empty()
    );
    assert!(
        controller
            .handle_input(input(PointerPhase::Up, start, true), &transform, &scene)
            .is_empty()
    );
    controller.handle_input(input(PointerPhase::Down, start, true), &transform, &scene);
    assert!(matches!(
        controller
            .handle_input(
                input(
                    PointerPhase::Move,
                    LogicalPoint {
                        x: start.x + 10.0,
                        y: start.y
                    },
                    true
                ),
                &transform,
                &scene
            )
            .as_slice(),
        [Event::CameraChanged(_)]
    ));
}

#[test]
fn scene_staging_and_repeated_tool_commands_do_not_drop_edit_capture() {
    let scene = scene(
        Geometry::Point {
            position: point(0.2, 0.3),
        },
        true,
    );
    let transform = transform(Rotation::Deg0);
    let mut controller = InteractionController::new(InteractionMode::Browse);
    controller.handle_input(
        input(
            PointerPhase::Down,
            transform.image_to_view(point(0.2, 0.3)),
            false,
        ),
        &transform,
        &scene,
    );
    controller.set_mode(InteractionMode::Browse);
    controller.set_mode(InteractionMode::Point);
    assert_eq!(
        controller.handle_input(
            input(
                PointerPhase::Up,
                transform.image_to_view(point(0.4, 0.5)),
                false
            ),
            &transform,
            &scene
        ),
        completed(Geometry::Point {
            position: point(0.4, 0.5)
        })
    );
}

#[test]
fn selected_handles_remain_editable_when_the_controller_keeps_the_shape_tool_active() {
    let scene = scene(
        Geometry::Rectangle {
            rect: NormalizedRect::new(0.2, 0.2, 0.4, 0.4).unwrap(),
        },
        true,
    );
    let transform = transform(Rotation::Deg0);
    let mut controller = InteractionController::new(InteractionMode::Rectangle);
    assert!(matches!(
        controller
            .handle_input(
                input(
                    PointerPhase::Down,
                    transform.image_to_view(point(0.2, 0.2)),
                    false
                ),
                &transform,
                &scene
            )
            .as_slice(),
        [Event::GeometryEditStarted {
            handle: Some(viewer_render_core::AnnotationHandle::NorthWest),
            ..
        }]
    ));
}

#[test]
fn pending_geometry_save_blocks_editing_another_annotation() {
    let mut pending = AnnotationNode::new(
        AnnotationId::new("pending").unwrap(),
        2,
        Geometry::Point {
            position: point(0.8, 0.8),
        },
    )
    .unwrap();
    pending.draft = true;
    let mut annotations = scene(
        Geometry::Point {
            position: point(0.2, 0.3),
        },
        true,
    )
    .annotations()
    .to_vec();
    annotations.push(pending);
    let scene = SceneSnapshot::new(SceneRevision(2), annotations, None).unwrap();
    let transform = transform(Rotation::Deg0);
    let mut controller = InteractionController::new(InteractionMode::Browse);
    assert!(
        !controller
            .handle_input(
                input(
                    PointerPhase::Down,
                    transform.image_to_view(point(0.2, 0.3)),
                    false
                ),
                &transform,
                &scene
            )
            .iter()
            .any(|event| matches!(event, Event::GeometryEditStarted { .. }))
    );
}

#[test]
fn shift_constrains_new_ellipses_in_source_pixels_but_does_not_constrain_rectangle_creation() {
    let transform = transform(Rotation::Deg0);
    let scene = SceneSnapshot::empty(SceneRevision(0));
    for mode in [InteractionMode::Ellipse, InteractionMode::Rectangle] {
        let mut controller = InteractionController::new(mode);
        controller.handle_input(
            input(
                PointerPhase::Down,
                transform.image_to_view(point(0.2, 0.2)),
                false,
            ),
            &transform,
            &scene,
        );
        let NativeInput::Pointer(mut sample) = input(
            PointerPhase::Up,
            transform.image_to_view(point(0.8, 0.6)),
            false,
        ) else {
            unreachable!()
        };
        sample.modifiers.shift = true;
        let events = controller.handle_input(NativeInput::Pointer(sample), &transform, &scene);
        let [
            Event::DraftCompleted(geometry),
            Event::EditorPlacementChanged(_),
        ] = events.as_slice()
        else {
            panic!("shape completion expected")
        };
        let expected = if mode == InteractionMode::Ellipse {
            Geometry::Ellipse {
                rect: NormalizedRect::new(0.2, 0.2, 0.2, 0.4).unwrap(),
            }
        } else {
            Geometry::Rectangle {
                rect: NormalizedRect::new(0.2, 0.2, 0.6, 0.4).unwrap(),
            }
        };
        match (geometry, expected) {
            (Geometry::Ellipse { rect: actual }, Geometry::Ellipse { rect: expected })
            | (Geometry::Rectangle { rect: actual }, Geometry::Rectangle { rect: expected }) => {
                assert!((actual.width - expected.width).abs() < 1e-12);
                assert!((actual.height - expected.height).abs() < 1e-12);
            }
            _ => panic!("shape kind changed"),
        }
    }
}

#[test]
fn shape_creation_rejects_drags_shorter_than_six_screen_pixels() {
    let transform = transform(Rotation::Deg0);
    let scene = SceneSnapshot::empty(SceneRevision(0));
    for mode in [
        InteractionMode::Arrow,
        InteractionMode::Rectangle,
        InteractionMode::Ellipse,
    ] {
        let mut controller = InteractionController::new(mode);
        controller.handle_input(
            input(
                PointerPhase::Down,
                LogicalPoint { x: 200.0, y: 350.0 },
                false,
            ),
            &transform,
            &scene,
        );
        assert_eq!(
            controller.handle_input(
                input(PointerPhase::Up, LogicalPoint { x: 202.0, y: 351.0 }, false),
                &transform,
                &scene
            ),
            vec![Event::DraftCancelled]
        );
    }
}

#[test]
fn geometry_edits_retain_the_existing_twelve_decimal_coordinate_precision() {
    assert_eq!(
        drag(
            &scene(
                Geometry::Point {
                    position: point(0.2, 0.3)
                },
                true
            ),
            point(0.2, 0.3),
            point(0.456789123456, 0.5),
            Rotation::Deg0
        ),
        completed(Geometry::Point {
            position: point(0.456789123456, 0.5)
        })
    );
}

#[test]
fn readonly_annotations_can_be_selected_but_never_create_edit_captures() {
    let transform = transform(Rotation::Deg0);
    for selected in [true, false] {
        let scene = scene(
            Geometry::Point {
                position: point(0.2, 0.3),
            },
            selected,
        )
        .with_annotations_editable(false);
        let mut controller = InteractionController::new(InteractionMode::Browse);
        let down = controller.handle_input(
            input(
                PointerPhase::Down,
                transform.image_to_view(point(0.2, 0.3)),
                false,
            ),
            &transform,
            &scene,
        );
        assert!(
            !down
                .iter()
                .any(|event| matches!(event, Event::GeometryEditStarted { .. }))
        );
        if !selected {
            assert_eq!(
                down,
                vec![Event::SelectionChanged(Some(
                    AnnotationId::new("item").unwrap()
                ))]
            );
        }
        let moved = controller.handle_input(
            input(
                PointerPhase::Move,
                transform.image_to_view(point(0.4, 0.5)),
                false,
            ),
            &transform,
            &scene,
        );
        assert!(
            !moved
                .iter()
                .any(|event| matches!(event, Event::GeometryEditChanged { .. }))
        );
    }
    let scene = SceneSnapshot::empty(SceneRevision(1)).with_annotations_editable(false);
    let mut controller = InteractionController::new(InteractionMode::Rectangle);
    assert!(
        controller
            .handle_input(
                input(
                    PointerPhase::Down,
                    transform.image_to_view(point(0.2, 0.3)),
                    false
                ),
                &transform,
                &scene
            )
            .is_empty()
    );
}
