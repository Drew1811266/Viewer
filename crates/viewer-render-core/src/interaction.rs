use crate::{
    AnnotationGeometry, AnnotationId, CameraState, HitIndex, LogicalPoint, NormalizedPoint,
    NormalizedRect, SceneSnapshot, TransformSnapshot,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionMode {
    Browse,
    Point,
    Arrow,
    Brush,
    Rectangle,
    Ellipse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointerPhase {
    Down,
    Move,
    Up,
    Cancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointerButton {
    Primary,
    Secondary,
    None,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Modifiers {
    pub space: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerSample {
    pub phase: PointerPhase,
    pub location: LogicalPoint,
    pub button: PointerButton,
    pub pressure: f64,
    pub modifiers: Modifiers,
    pub timestamp_ns: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollSample {
    pub location: LogicalPoint,
    pub delta: LogicalPoint,
    pub modifiers: Modifiers,
    pub timestamp_ns: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MagnifySample {
    pub location: LogicalPoint,
    pub factor: f64,
    pub timestamp_ns: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HoverSample {
    pub location: LogicalPoint,
    pub active: bool,
    pub timestamp_ns: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NativeInput {
    Pointer(PointerSample),
    Hover(HoverSample),
    Scroll(ScrollSample),
    Magnify(MagnifySample),
    Cancel,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DraftGeometry {
    Point(NormalizedPoint),
    Arrow {
        tail: NormalizedPoint,
        head: NormalizedPoint,
    },
    Brush(Vec<NormalizedPoint>),
    Rectangle {
        start: NormalizedPoint,
        end: NormalizedPoint,
    },
    Ellipse {
        start: NormalizedPoint,
        end: NormalizedPoint,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum InteractionEvent {
    CameraChanged(CameraState),
    DraftStarted(DraftGeometry),
    DraftChanged(DraftGeometry),
    DraftCompleted(AnnotationGeometry),
    DraftCancelled,
    SelectionChanged(Option<AnnotationId>),
    EditorPlacementChanged(LogicalPoint),
}

#[derive(Clone, Debug)]
enum Capture {
    Pan {
        start: LogicalPoint,
        last: LogicalPoint,
        moved: bool,
    },
    Draw {
        start: NormalizedPoint,
        current: NormalizedPoint,
        points: Vec<NormalizedPoint>,
    },
}

#[derive(Clone, Debug)]
pub struct InteractionController {
    mode: InteractionMode,
    capture: Option<Capture>,
}

impl InteractionController {
    pub const fn new(mode: InteractionMode) -> Self {
        Self {
            mode,
            capture: None,
        }
    }

    pub fn set_mode(&mut self, mode: InteractionMode) {
        self.mode = mode;
        self.capture = None;
    }

    pub fn handle_input(
        &mut self,
        input: NativeInput,
        transform: &TransformSnapshot,
        scene: &SceneSnapshot,
    ) -> Vec<InteractionEvent> {
        match input {
            NativeInput::Cancel => self.cancel(),
            NativeInput::Hover(_) => Vec::new(),
            NativeInput::Scroll(sample) => transform
                .pan_by(sample.delta)
                .map(InteractionEvent::CameraChanged)
                .into_iter()
                .collect(),
            NativeInput::Magnify(sample) => transform
                .zoom_at(sample.factor, sample.location)
                .map(InteractionEvent::CameraChanged)
                .into_iter()
                .collect(),
            NativeInput::Pointer(sample) => self.handle_pointer(sample, transform, scene),
        }
    }

    fn handle_pointer(
        &mut self,
        sample: PointerSample,
        transform: &TransformSnapshot,
        scene: &SceneSnapshot,
    ) -> Vec<InteractionEvent> {
        if sample.phase == PointerPhase::Cancel {
            return self.cancel();
        }
        match sample.phase {
            PointerPhase::Down => self.pointer_down(sample, transform),
            PointerPhase::Move => self.pointer_move(sample, transform),
            PointerPhase::Up => self.pointer_up(sample, transform, scene),
            PointerPhase::Cancel => unreachable!(),
        }
    }

    fn pointer_down(
        &mut self,
        sample: PointerSample,
        transform: &TransformSnapshot,
    ) -> Vec<InteractionEvent> {
        if sample.button != PointerButton::Primary || self.capture.is_some() {
            return Vec::new();
        }
        if self.mode == InteractionMode::Browse || sample.modifiers.space {
            self.capture = Some(Capture::Pan {
                start: sample.location,
                last: sample.location,
                moved: false,
            });
            return Vec::new();
        }
        let Some(start) = transform.view_to_image(sample.location) else {
            return Vec::new();
        };
        self.capture = Some(Capture::Draw {
            start,
            current: start,
            points: vec![start],
        });
        vec![InteractionEvent::DraftStarted(draft_geometry(
            self.mode,
            start,
            start,
            &[start],
        ))]
    }

    fn pointer_move(
        &mut self,
        sample: PointerSample,
        transform: &TransformSnapshot,
    ) -> Vec<InteractionEvent> {
        match self.capture.as_mut() {
            Some(Capture::Pan { last, moved, .. }) => {
                let delta = LogicalPoint {
                    x: sample.location.x - last.x,
                    y: sample.location.y - last.y,
                };
                *last = sample.location;
                *moved |= delta.x != 0.0 || delta.y != 0.0;
                transform
                    .pan_by(delta)
                    .map(InteractionEvent::CameraChanged)
                    .into_iter()
                    .collect()
            }
            Some(Capture::Draw {
                start,
                current,
                points,
            }) => {
                *current = transform.view_to_image_clamped(sample.location);
                if self.mode == InteractionMode::Brush
                    && points.last() != Some(current)
                    && points.len() < crate::MAX_STROKE_POINTS
                {
                    points.push(*current);
                }
                vec![InteractionEvent::DraftChanged(draft_geometry(
                    self.mode, *start, *current, points,
                ))]
            }
            None => Vec::new(),
        }
    }

    fn pointer_up(
        &mut self,
        sample: PointerSample,
        transform: &TransformSnapshot,
        scene: &SceneSnapshot,
    ) -> Vec<InteractionEvent> {
        let Some(capture) = self.capture.take() else {
            return Vec::new();
        };
        match capture {
            Capture::Pan { start, moved, .. } => {
                if moved {
                    Vec::new()
                } else {
                    let hit = HitIndex::rebuild(scene).hit_test(start, 8.0, transform);
                    vec![InteractionEvent::SelectionChanged(hit)]
                }
            }
            Capture::Draw {
                start,
                current: _,
                mut points,
            } => {
                let current = transform.view_to_image_clamped(sample.location);
                if self.mode == InteractionMode::Brush
                    && points.last() != Some(&current)
                    && points.len() < crate::MAX_STROKE_POINTS
                {
                    points.push(current);
                }
                complete_geometry(self.mode, start, current, points).map_or_else(
                    || vec![InteractionEvent::DraftCancelled],
                    |geometry| {
                        let placement = transform.image_to_view(editor_anchor(&geometry));
                        vec![
                            InteractionEvent::DraftCompleted(geometry),
                            InteractionEvent::EditorPlacementChanged(placement),
                        ]
                    },
                )
            }
        }
    }

    fn cancel(&mut self) -> Vec<InteractionEvent> {
        match self.capture.take() {
            Some(Capture::Draw { .. }) => vec![InteractionEvent::DraftCancelled],
            Some(Capture::Pan { .. }) | None => Vec::new(),
        }
    }
}

fn editor_anchor(geometry: &AnnotationGeometry) -> NormalizedPoint {
    match geometry {
        AnnotationGeometry::Point { position } => *position,
        AnnotationGeometry::Arrow { head, .. } => *head,
        AnnotationGeometry::Rectangle { rect } | AnnotationGeometry::Ellipse { rect } => {
            NormalizedPoint {
                x: rect.x + rect.width / 2.0,
                y: rect.y + rect.height / 2.0,
            }
        }
        AnnotationGeometry::Stroke { points } => points
            .last()
            .copied()
            .expect("a completed stroke always contains points"),
    }
}

fn draft_geometry(
    mode: InteractionMode,
    start: NormalizedPoint,
    current: NormalizedPoint,
    points: &[NormalizedPoint],
) -> DraftGeometry {
    match mode {
        InteractionMode::Point => DraftGeometry::Point(start),
        InteractionMode::Arrow => DraftGeometry::Arrow {
            tail: start,
            head: current,
        },
        InteractionMode::Brush => DraftGeometry::Brush(points.to_vec()),
        InteractionMode::Rectangle => DraftGeometry::Rectangle {
            start,
            end: current,
        },
        InteractionMode::Ellipse => DraftGeometry::Ellipse {
            start,
            end: current,
        },
        InteractionMode::Browse => unreachable!(),
    }
}

fn complete_geometry(
    mode: InteractionMode,
    start: NormalizedPoint,
    current: NormalizedPoint,
    points: Vec<NormalizedPoint>,
) -> Option<AnnotationGeometry> {
    match mode {
        InteractionMode::Point => Some(AnnotationGeometry::Point { position: start }),
        InteractionMode::Arrow if start != current => Some(AnnotationGeometry::Arrow {
            tail: start,
            head: current,
        }),
        InteractionMode::Brush if valid_stroke(&points) => {
            Some(AnnotationGeometry::Stroke { points })
        }
        InteractionMode::Rectangle => {
            normalized_rect(start, current).map(|rect| AnnotationGeometry::Rectangle { rect })
        }
        InteractionMode::Ellipse => {
            normalized_rect(start, current).map(|rect| AnnotationGeometry::Ellipse { rect })
        }
        InteractionMode::Browse | InteractionMode::Arrow | InteractionMode::Brush => None,
    }
}

fn normalized_rect(start: NormalizedPoint, end: NormalizedPoint) -> Option<NormalizedRect> {
    NormalizedRect::new(
        start.x.min(end.x),
        start.y.min(end.y),
        (start.x - end.x).abs(),
        (start.y - end.y).abs(),
    )
    .ok()
}

fn valid_stroke(points: &[NormalizedPoint]) -> bool {
    if points.len() < 2 {
        return false;
    }
    let first = points[0];
    points
        .iter()
        .skip(1)
        .any(|point| point.x != first.x && point.y != first.y)
}
