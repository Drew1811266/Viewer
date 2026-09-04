use std::collections::BTreeSet;

use crate::{
    AnnotationGeometry, AnnotationId, AnnotationNode, LogicalPoint, NormalizedPoint, SceneSnapshot,
    TransformSnapshot,
};

const GRID_SIDE: usize = 32;

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
        if !tolerance_px.is_finite() || tolerance_px < 0.0 {
            return None;
        }
        let mut candidates = self.candidate_indices(point, tolerance_px, transform);
        candidates.sort_unstable_by(|left, right| right.cmp(left));
        candidates.into_iter().find_map(|index| {
            let node = &self.nodes[index];
            (node.visible && geometry_hits(&node.geometry, point, tolerance_px, transform))
                .then(|| node.id.clone())
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

fn geometry_hits(
    geometry: &AnnotationGeometry,
    point: LogicalPoint,
    tolerance: f64,
    transform: &TransformSnapshot,
) -> bool {
    match geometry {
        AnnotationGeometry::Point { position } => {
            distance(transform.image_to_view(*position), point) <= tolerance
        }
        AnnotationGeometry::Arrow { tail, head } => {
            distance_to_segment(
                point,
                transform.image_to_view(*tail),
                transform.image_to_view(*head),
            ) <= tolerance
        }
        AnnotationGeometry::Rectangle { rect } => {
            let corners = rect_corners(*rect).map(|corner| transform.image_to_view(corner));
            corners
                .into_iter()
                .zip(corners.into_iter().cycle().skip(1))
                .take(4)
                .any(|(start, end)| distance_to_segment(point, start, end) <= tolerance)
        }
        AnnotationGeometry::Ellipse { rect } => {
            let top_left = transform.image_to_view(NormalizedPoint {
                x: rect.x,
                y: rect.y,
            });
            let bottom_right = transform.image_to_view(NormalizedPoint {
                x: rect.x + rect.width,
                y: rect.y + rect.height,
            });
            let radius_x = (bottom_right.x - top_left.x).abs() / 2.0;
            let radius_y = (bottom_right.y - top_left.y).abs() / 2.0;
            if radius_x == 0.0 || radius_y == 0.0 {
                return false;
            }
            let center = LogicalPoint {
                x: (top_left.x + bottom_right.x) / 2.0,
                y: (top_left.y + bottom_right.y) / 2.0,
            };
            let radial = (((point.x - center.x) / radius_x).powi(2)
                + ((point.y - center.y) / radius_y).powi(2))
            .sqrt();
            (radial - 1.0).abs() * radius_x.min(radius_y) <= tolerance
        }
        AnnotationGeometry::Stroke { points } => points.windows(2).any(|segment| {
            distance_to_segment(
                point,
                transform.image_to_view(segment[0]),
                transform.image_to_view(segment[1]),
            ) <= tolerance
        }),
    }
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
