use crate::{AnnotationGeometry, NormalizedPoint, NormalizedRect};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnnotationHandle {
    Point,
    Tail,
    Head,
    NorthWest,
    NorthEast,
    SouthEast,
    SouthWest,
}

/// Source-space handle locations shared by hit testing and rendering.
pub fn annotation_handles(
    geometry: &AnnotationGeometry,
) -> Vec<(AnnotationHandle, NormalizedPoint)> {
    use AnnotationHandle::*;
    match geometry {
        AnnotationGeometry::Point { position } => vec![(Point, *position)],
        AnnotationGeometry::Arrow { tail, head } => vec![(Tail, *tail), (Head, *head)],
        AnnotationGeometry::Rectangle { rect } | AnnotationGeometry::Ellipse { rect } => vec![
            (
                NorthWest,
                NormalizedPoint {
                    x: rect.x,
                    y: rect.y,
                },
            ),
            (
                NorthEast,
                NormalizedPoint {
                    x: rect.x + rect.width,
                    y: rect.y,
                },
            ),
            (
                SouthEast,
                NormalizedPoint {
                    x: rect.x + rect.width,
                    y: rect.y + rect.height,
                },
            ),
            (
                SouthWest,
                NormalizedPoint {
                    x: rect.x,
                    y: rect.y + rect.height,
                },
            ),
        ],
        AnnotationGeometry::Stroke { .. } => Vec::new(),
    }
}

pub(crate) fn edit_geometry(
    original: &AnnotationGeometry,
    handle: Option<AnnotationHandle>,
    start: NormalizedPoint,
    current: NormalizedPoint,
) -> Option<AnnotationGeometry> {
    use AnnotationGeometry::*;
    if ![start.x, start.y, current.x, current.y]
        .iter()
        .all(|value| value.is_finite())
    {
        return None;
    }
    if handle.is_none() || handle == Some(AnnotationHandle::Point) {
        let delta = NormalizedPoint {
            x: current.x - start.x,
            y: current.y - start.y,
        };
        let handles = annotation_handles(original);
        if handles.is_empty() {
            return None;
        }
        let min_x = handles.iter().map(|(_, p)| p.x).fold(1.0, f64::min);
        let min_y = handles.iter().map(|(_, p)| p.y).fold(1.0, f64::min);
        let max_x = handles.iter().map(|(_, p)| p.x).fold(0.0, f64::max);
        let max_y = handles.iter().map(|(_, p)| p.y).fold(0.0, f64::max);
        let dx = delta.x.clamp(-min_x, 1.0 - max_x);
        let dy = delta.y.clamp(-min_y, 1.0 - max_y);
        let moved = |p: NormalizedPoint| NormalizedPoint {
            x: rounded(p.x + dx),
            y: rounded(p.y + dy),
        };
        return Some(match original {
            Point { position } => Point {
                position: moved(*position),
            },
            Arrow { tail, head } => Arrow {
                tail: moved(*tail),
                head: moved(*head),
            },
            Rectangle { rect } | Ellipse { rect } => {
                let position = moved(NormalizedPoint {
                    x: rect.x,
                    y: rect.y,
                });
                let rect = NormalizedRect {
                    x: position.x,
                    y: position.y,
                    ..*rect
                };
                if matches!(original, Rectangle { .. }) {
                    Rectangle { rect }
                } else {
                    Ellipse { rect }
                }
            }
            Stroke { .. } => return None,
        });
    }
    let current = NormalizedPoint {
        x: rounded(current.x.clamp(0.0, 1.0)),
        y: rounded(current.y.clamp(0.0, 1.0)),
    };
    match (original, handle?) {
        (Arrow { head, .. }, AnnotationHandle::Tail) if current != *head => Some(Arrow {
            tail: current,
            head: *head,
        }),
        (Arrow { tail, .. }, AnnotationHandle::Head) if current != *tail => Some(Arrow {
            tail: *tail,
            head: current,
        }),
        (Rectangle { rect } | Ellipse { rect }, handle) => {
            let opposite = match handle {
                AnnotationHandle::NorthWest => NormalizedPoint {
                    x: rect.x + rect.width,
                    y: rect.y + rect.height,
                },
                AnnotationHandle::NorthEast => NormalizedPoint {
                    x: rect.x,
                    y: rect.y + rect.height,
                },
                AnnotationHandle::SouthEast => NormalizedPoint {
                    x: rect.x,
                    y: rect.y,
                },
                AnnotationHandle::SouthWest => NormalizedPoint {
                    x: rect.x + rect.width,
                    y: rect.y,
                },
                _ => return None,
            };
            let rect = NormalizedRect::new(
                rounded(opposite.x.min(current.x)),
                rounded(opposite.y.min(current.y)),
                rounded((opposite.x - current.x).abs()),
                rounded((opposite.y - current.y).abs()),
            )
            .ok()?;
            Some(if matches!(original, Rectangle { .. }) {
                Rectangle { rect }
            } else {
                Ellipse { rect }
            })
        }
        _ => None,
    }
}

fn rounded(value: f64) -> f64 {
    (value * 1_000_000_000_000.0).round() / 1_000_000_000_000.0
}
