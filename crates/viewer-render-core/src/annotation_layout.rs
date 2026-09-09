//! Screen-space controls shared by native drawing and hit testing.
use crate::{AnnotationGeometry, LogicalPoint, LogicalSize, NormalizedPoint, TransformSnapshot};

pub const ORDINAL_RADIUS_PX: f64 = 14.0;
pub const HANDLE_RADIUS_PX: f64 = 12.0;
pub const ORDINAL_CLEARANCE_PX: f64 = ORDINAL_RADIUS_PX + 2.0 + 4.0;
pub const BOX_ORDINAL_CLEARANCE_PX: f64 = 24.0;

/// The old review canvas' ordered, bounded candidates. A badge is omitted if no
/// candidate clears its geometry; it never covers an endpoint or corner handle.
pub fn annotation_ordinal_position(
    geometry: &AnnotationGeometry,
    transform: &TransformSnapshot,
) -> Option<LogicalPoint> {
    ordinal_position(
        geometry,
        |point| transform.image_to_view(point),
        transform.viewport().logical_size,
        2.0,
        true,
    )
}

pub fn annotation_ordinal_position_projected(
    geometry: &AnnotationGeometry,
    project: impl Fn(NormalizedPoint) -> LogicalPoint,
    size: LogicalSize,
    line_width: f64,
) -> Option<LogicalPoint> {
    ordinal_position(geometry, project, size, line_width, false)
}

fn ordinal_position(
    geometry: &AnnotationGeometry,
    project: impl Fn(NormalizedPoint) -> LogicalPoint,
    size: LogicalSize,
    line_width: f64,
    avoid_handles: bool,
) -> Option<LogicalPoint> {
    let box_geometry = matches!(
        geometry,
        AnnotationGeometry::Rectangle { .. } | AnnotationGeometry::Ellipse { .. }
    );
    let distance = if avoid_handles && box_geometry {
        BOX_ORDINAL_CLEARANCE_PX
    } else {
        ORDINAL_RADIUS_PX + line_width + 4.0
    };
    let handles = if avoid_handles {
        crate::annotation_handles(geometry)
            .into_iter()
            .filter(|(handle, _)| *handle != crate::AnnotationHandle::Point)
            .map(|(_, point)| project(point))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let mut path = Vec::new();
    let mut bounds = None;
    let reference = match geometry {
        AnnotationGeometry::Point { position } => project(*position),
        AnnotationGeometry::Arrow { tail, head } => {
            path = vec![project(*tail), project(*head)];
            path[0]
        }
        AnnotationGeometry::Stroke { points } => {
            path = points.iter().copied().map(project).collect();
            *path.last()?
        }
        AnnotationGeometry::Rectangle { rect } | AnnotationGeometry::Ellipse { rect } => {
            let start = project(crate::NormalizedPoint {
                x: rect.x,
                y: rect.y,
            });
            let end = project(crate::NormalizedPoint {
                x: rect.x + rect.width,
                y: rect.y + rect.height,
            });
            bounds = Some((
                start.x.min(end.x),
                start.y.min(end.y),
                start.x.max(end.x),
                start.y.max(end.y),
            ));
            start
        }
    };
    let candidates = if let Some((left, top, right, bottom)) = bounds {
        [
            (right + distance, top - distance),
            (left - distance, top - distance),
            (right + distance, bottom + distance),
            (left - distance, bottom + distance),
            (right + distance, (top + bottom) / 2.0),
            (left - distance, (top + bottom) / 2.0),
            ((left + right) / 2.0, top - distance),
            ((left + right) / 2.0, bottom + distance),
        ]
    } else {
        let (x, y) = (reference.x, reference.y);
        [
            (x + distance, y - distance),
            (x - distance, y - distance),
            (x + distance, y + distance),
            (x - distance, y + distance),
            (x, y - distance),
            (x + distance, y),
            (x, y + distance),
            (x - distance, y),
        ]
    };
    candidates
        .into_iter()
        .map(|(x, y)| LogicalPoint { x, y })
        .find(|candidate| {
            if handles.iter().any(|handle| {
                let dx = (candidate.x - handle.x).abs();
                let dy = (candidate.y - handle.y).abs();
                if box_geometry {
                    (dx - HANDLE_RADIUS_PX)
                        .max(0.0)
                        .hypot((dy - HANDLE_RADIUS_PX).max(0.0))
                        < ORDINAL_RADIUS_PX + 2.0
                } else {
                    dx.hypot(dy) < ORDINAL_RADIUS_PX + HANDLE_RADIUS_PX + 2.0
                }
            }) {
                return false;
            }
            if candidate.x < ORDINAL_RADIUS_PX
                || candidate.y < ORDINAL_RADIUS_PX
                || candidate.x > size.width - ORDINAL_RADIUS_PX
                || candidate.y > size.height - ORDINAL_RADIUS_PX
            {
                return false;
            }
            if let Some((left, top, right, bottom)) = bounds {
                return !(candidate.x > left - distance
                    && candidate.x < right + distance
                    && candidate.y > top - distance
                    && candidate.y < bottom + distance);
            }
            if path.is_empty() {
                return (candidate.x - reference.x).hypot(candidate.y - reference.y)
                    >= distance + 7.0;
            }
            path.windows(2)
                .all(|segment| segment_distance(*candidate, segment[0], segment[1]) >= distance)
        })
}

fn segment_distance(point: LogicalPoint, start: LogicalPoint, end: LogicalPoint) -> f64 {
    let (dx, dy) = (end.x - start.x, end.y - start.y);
    let squared = dx * dx + dy * dy;
    let progress = if squared == 0.0 {
        0.0
    } else {
        (((point.x - start.x) * dx + (point.y - start.y) * dy) / squared).clamp(0.0, 1.0)
    };
    (point.x - start.x - progress * dx).hypot(point.y - start.y - progress * dy)
}
