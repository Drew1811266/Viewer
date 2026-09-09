use crate::BOX_ORDINAL_CLEARANCE_PX;
use std::collections::BTreeSet;

use crate::{
    AnnotationGeometry, AnnotationHandle, AnnotationId, AnnotationNode, HANDLE_RADIUS_PX,
    LogicalPoint, NormalizedPoint, ORDINAL_CLEARANCE_PX, ORDINAL_RADIUS_PX, SceneSnapshot,
    TransformSnapshot, annotation_handles, annotation_ordinal_position,
};

const GRID_SIDE: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnnotationHitPart {
    Handle(AnnotationHandle),
    Ordinal,
    Outline,
    Interior,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnnotationHit {
    pub annotation_id: AnnotationId,
    pub part: AnnotationHitPart,
}

#[derive(Clone, Debug)]
pub struct HitIndex {
    nodes: Vec<AnnotationNode>,
    cells: Vec<Vec<usize>>,
}

impl HitIndex {
    pub fn rebuild(scene: &SceneSnapshot) -> Self {
        let nodes = scene.annotations().to_vec();
        let mut cells = vec![Vec::new(); GRID_SIDE * GRID_SIDE];
        for (index, node) in nodes.iter().enumerate() {
            if !node.visible {
                continue;
            }
            let bounds = geometry_bounds(&node.geometry);
            let min_x = grid_coordinate(bounds.min_x);
            let min_y = grid_coordinate(bounds.min_y);
            let max_x = grid_coordinate(bounds.max_x);
            let max_y = grid_coordinate(bounds.max_y);
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    cells[y * GRID_SIDE + x].push(index);
                }
            }
        }
        Self { nodes, cells }
    }

    pub fn hit_test(
        &self,
        point: LogicalPoint,
        tolerance_px: f64,
        transform: &TransformSnapshot,
    ) -> Option<AnnotationId> {
        self.hit_test_part(point, tolerance_px, transform)
            .map(|hit| hit.annotation_id)
    }

    /// Handles precede selected geometry, ordinals, outlines, then interiors.
    /// Equal candidates use screen distance then the visually topmost node.
    pub fn hit_test_part(
        &self,
        point: LogicalPoint,
        tolerance_px: f64,
        transform: &TransformSnapshot,
    ) -> Option<AnnotationHit> {
        if !tolerance_px.is_finite() || tolerance_px < 0.0 {
            return None;
        }
        let mut hits = Vec::new();
        let ordinal_radius = tolerance_px.max(ORDINAL_RADIUS_PX);
        // Offset ordinals sit outside the indexed geometry bounds.
        for index in self.candidate_indices(
            point,
            ordinal_radius + ORDINAL_CLEARANCE_PX.max(BOX_ORDINAL_CLEARANCE_PX),
            transform,
        ) {
            let node = &self.nodes[index];
            if !node.visible {
                continue;
            }
            for (handle, position) in annotation_handles(&node.geometry)
                .into_iter()
                .filter(|_| node.selected)
            {
                let position = transform.image_to_view(position);
                let distance = distance(point, position);
                let hit_distance = if matches!(
                    node.geometry,
                    AnnotationGeometry::Rectangle { .. } | AnnotationGeometry::Ellipse { .. }
                ) {
                    (point.x - position.x)
                        .abs()
                        .max((point.y - position.y).abs())
                } else {
                    distance
                };
                if hit_distance <= tolerance_px.max(HANDLE_RADIUS_PX) {
                    hits.push((0, distance, index, AnnotationHitPart::Handle(handle)));
                }
            }
            if let Some((part, distance)) =
                geometry_hit(&node.geometry, point, tolerance_px, transform)
            {
                let priority = if node.selected {
                    1
                } else if part == AnnotationHitPart::Outline {
                    3
                } else {
                    4
                };
                hits.push((priority, distance, index, part));
            }
            let Some(ordinal) = annotation_ordinal_position(&node.geometry, transform) else {
                continue;
            };
            let distance = distance(point, ordinal);
            if distance <= ordinal_radius {
                hits.push((
                    if node.selected { 1 } else { 2 },
                    distance,
                    index,
                    AnnotationHitPart::Ordinal,
                ));
            }
        }
        hits.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then(left.1.total_cmp(&right.1))
                .then(right.2.cmp(&left.2))
        });
        hits.first().map(|(_, _, index, part)| AnnotationHit {
            annotation_id: self.nodes[*index].id.clone(),
            part: *part,
        })
    }

    pub fn candidate_count(
        &self,
        point: LogicalPoint,
        tolerance_px: f64,
        transform: &TransformSnapshot,
    ) -> usize {
        self.candidate_indices(point, tolerance_px, transform).len()
    }

    fn candidate_indices(
        &self,
        point: LogicalPoint,
        tolerance_px: f64,
        transform: &TransformSnapshot,
    ) -> Vec<usize> {
        let source_corners = [
            LogicalPoint {
                x: point.x - tolerance_px,
                y: point.y - tolerance_px,
            },
            LogicalPoint {
                x: point.x + tolerance_px,
                y: point.y - tolerance_px,
            },
            LogicalPoint {
                x: point.x + tolerance_px,
                y: point.y + tolerance_px,
            },
            LogicalPoint {
                x: point.x - tolerance_px,
                y: point.y + tolerance_px,
            },
        ]
        .map(|corner| transform.view_to_image_clamped(corner));
        let (min_x, min_y, max_x, max_y) = source_corners.iter().fold(
            (1.0_f64, 1.0_f64, 0.0_f64, 0.0_f64),
            |(min_x, min_y, max_x, max_y), source| {
                (
                    min_x.min(source.x),
                    min_y.min(source.y),
                    max_x.max(source.x),
                    max_y.max(source.y),
                )
            },
        );
        let min_x = grid_coordinate(min_x);
        let min_y = grid_coordinate(min_y);
        let max_x = grid_coordinate(max_x);
        let max_y = grid_coordinate(max_y);
        let mut candidates = BTreeSet::new();
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                candidates.extend(self.cells[y * GRID_SIDE + x].iter().copied());
            }
        }
        candidates.into_iter().collect()
    }
}

#[derive(Clone, Copy)]
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

fn geometry_bounds(geometry: &AnnotationGeometry) -> Bounds {
    match geometry {
        AnnotationGeometry::Point { position } => Bounds {
            min_x: position.x,
            min_y: position.y,
            max_x: position.x,
            max_y: position.y,
        },
        AnnotationGeometry::Arrow { tail, head } => Bounds {
            min_x: tail.x.min(head.x),
            min_y: tail.y.min(head.y),
            max_x: tail.x.max(head.x),
            max_y: tail.y.max(head.y),
        },
        AnnotationGeometry::Rectangle { rect } | AnnotationGeometry::Ellipse { rect } => Bounds {
            min_x: rect.x,
            min_y: rect.y,
            max_x: rect.x + rect.width,
            max_y: rect.y + rect.height,
        },
        AnnotationGeometry::Stroke { points } => points.iter().fold(
            Bounds {
                min_x: 1.0,
                min_y: 1.0,
                max_x: 0.0,
                max_y: 0.0,
            },
            |bounds, point| Bounds {
                min_x: bounds.min_x.min(point.x),
                min_y: bounds.min_y.min(point.y),
                max_x: bounds.max_x.max(point.x),
                max_y: bounds.max_y.max(point.y),
            },
        ),
    }
}

fn geometry_hit(
    geometry: &AnnotationGeometry,
    point: LogicalPoint,
    tolerance: f64,
    transform: &TransformSnapshot,
) -> Option<(AnnotationHitPart, f64)> {
    use AnnotationHitPart::{Interior, Outline};
    let distance = match geometry {
        AnnotationGeometry::Rectangle { rect } | AnnotationGeometry::Ellipse { rect } => {
            let source = transform.view_to_image_unclamped(point);
            let corners = rect_corners(*rect).map(|corner| transform.image_to_view(corner));
            let distance = if matches!(geometry, AnnotationGeometry::Ellipse { .. }) {
                let radial = (((source.x - rect.x - rect.width / 2.0) / (rect.width / 2.0))
                    .powi(2)
                    + ((source.y - rect.y - rect.height / 2.0) / (rect.height / 2.0)).powi(2))
                .sqrt();
                let radius =
                    distance(corners[0], corners[1]).min(distance(corners[1], corners[2])) / 2.0;
                let boundary = (radial - 1.0).abs() * radius;
                if boundary <= tolerance {
                    return Some((Outline, boundary));
                }
                return (radial < 1.0).then_some((Interior, boundary));
            } else {
                corners
                    .into_iter()
                    .zip(corners.into_iter().cycle().skip(1))
                    .take(4)
                    .map(|(start, end)| distance_to_segment(point, start, end))
                    .fold(f64::INFINITY, f64::min)
            };
            if distance <= tolerance {
                return Some((Outline, distance));
            }
            return (source.x >= rect.x
                && source.x <= rect.x + rect.width
                && source.y >= rect.y
                && source.y <= rect.y + rect.height)
                .then_some((Interior, distance));
        }
        AnnotationGeometry::Point { position } => {
            distance(transform.image_to_view(*position), point)
        }
        AnnotationGeometry::Arrow { tail, head } => distance_to_segment(
            point,
            transform.image_to_view(*tail),
            transform.image_to_view(*head),
        ),
        AnnotationGeometry::Stroke { points } => points
            .windows(2)
            .map(|segment| {
                distance_to_segment(
                    point,
                    transform.image_to_view(segment[0]),
                    transform.image_to_view(segment[1]),
                )
            })
            .fold(f64::INFINITY, f64::min),
    };
    (distance <= tolerance).then_some((Outline, distance))
}

fn rect_corners(rect: crate::NormalizedRect) -> [NormalizedPoint; 4] {
    [
        NormalizedPoint {
            x: rect.x,
            y: rect.y,
        },
        NormalizedPoint {
            x: rect.x + rect.width,
            y: rect.y,
        },
        NormalizedPoint {
            x: rect.x + rect.width,
            y: rect.y + rect.height,
        },
        NormalizedPoint {
            x: rect.x,
            y: rect.y + rect.height,
        },
    ]
}

fn grid_coordinate(value: f64) -> usize {
    ((value.clamp(0.0, 1.0) * GRID_SIDE as f64).floor() as usize).min(GRID_SIDE - 1)
}

fn distance(left: LogicalPoint, right: LogicalPoint) -> f64 {
    (left.x - right.x).hypot(left.y - right.y)
}

fn distance_to_segment(point: LogicalPoint, start: LogicalPoint, end: LogicalPoint) -> f64 {
    let delta_x = end.x - start.x;
    let delta_y = end.y - start.y;
    let length_squared = delta_x * delta_x + delta_y * delta_y;
    if length_squared == 0.0 {
        return distance(point, start);
    }
    let projection = (((point.x - start.x) * delta_x + (point.y - start.y) * delta_y)
        / length_squared)
        .clamp(0.0, 1.0);
    distance(
        point,
        LogicalPoint {
            x: start.x + projection * delta_x,
            y: start.y + projection * delta_y,
        },
    )
}
