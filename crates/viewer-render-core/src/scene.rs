use std::{collections::BTreeSet, error::Error, fmt};

use crate::{AnnotationId, NormalizedPoint, NormalizedRect, SceneRevision};

pub const MAX_STROKE_POINTS: usize = 2_048;
pub const MAX_SCENE_ANNOTATIONS: usize = 4_096;

#[derive(Clone, Debug, PartialEq)]
pub enum AnnotationGeometry {
    Point {
        position: NormalizedPoint,
    },
    Arrow {
        tail: NormalizedPoint,
        head: NormalizedPoint,
    },
    Rectangle {
        rect: NormalizedRect,
    },
    Ellipse {
        rect: NormalizedRect,
    },
    Stroke {
        points: Vec<NormalizedPoint>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnnotationStyle {
    pub color: [f32; 4],
    pub line_width_px: f32,
    pub dashed: bool,
}

impl Default for AnnotationStyle {
    fn default() -> Self {
        Self {
            color: [0.70, 0.13, 0.09, 1.0],
            line_width_px: 2.0,
            dashed: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnnotationNode {
    pub id: AnnotationId,
    pub ordinal: u32,
    pub geometry: AnnotationGeometry,
    pub style: AnnotationStyle,
    pub selected: bool,
    pub draft: bool,
}

impl AnnotationNode {
    pub fn new(
        id: AnnotationId,
        ordinal: u32,
        geometry: AnnotationGeometry,
    ) -> Result<Self, SceneError> {
        validate_geometry(&geometry)?;
        Ok(Self {
            id,
            ordinal,
            geometry,
            style: AnnotationStyle::default(),
            selected: false,
            draft: false,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneSnapshot {
    revision: SceneRevision,
    annotations: Vec<AnnotationNode>,
    draft: Option<AnnotationNode>,
}

impl SceneSnapshot {
    pub const fn empty(revision: SceneRevision) -> Self {
        Self {
            revision,
            annotations: Vec::new(),
            draft: None,
        }
    }

    pub fn new(
        revision: SceneRevision,
        mut annotations: Vec<AnnotationNode>,
        mut draft: Option<AnnotationNode>,
    ) -> Result<Self, SceneError> {
        validate_nodes(&annotations, draft.as_ref())?;
        sort_nodes(&mut annotations);
        if let Some(node) = draft.as_mut() {
            node.draft = true;
            node.selected = false;
        }
        Ok(Self {
            revision,
            annotations,
            draft,
        })
    }

    pub const fn revision(&self) -> SceneRevision {
        self.revision
    }

    pub fn annotations(&self) -> &[AnnotationNode] {
        &self.annotations
    }

    pub fn draft(&self) -> Option<&AnnotationNode> {
        self.draft.as_ref()
    }

    pub fn apply_patch(
        &mut self,
        patch: &ScenePatch,
    ) -> Result<ScenePatchDisposition, ScenePatchError> {
        let base = patch.base_revision();
        let revision = patch.revision();
        if revision <= self.revision {
            return Ok(if revision == self.revision {
                ScenePatchDisposition::IgnoredDuplicate
            } else {
                ScenePatchDisposition::IgnoredStale
            });
        }
        if base != self.revision {
            return Err(ScenePatchError::RevisionMismatch {
                current: self.revision,
                base,
            });
        }
        if self.revision.checked_next() != Some(revision) {
            return Err(ScenePatchError::NonSequential {
                current: self.revision,
                next: revision,
            });
        }

        let mut next = self.clone();
        match patch {
            ScenePatch::Upsert { node, .. } => {
                validate_geometry(&node.geometry).map_err(ScenePatchError::InvalidScene)?;
                if let Some(existing) = next.annotations.iter_mut().find(|item| item.id == node.id)
                {
                    *existing = node.clone();
                } else {
                    if next.annotations.len() >= MAX_SCENE_ANNOTATIONS {
                        return Err(ScenePatchError::InvalidScene(
                            SceneError::TooManyAnnotations,
                        ));
                    }
                    next.annotations.push(node.clone());
                }
                sort_nodes(&mut next.annotations);
            }
            ScenePatch::Remove { id, .. } => {
                next.annotations.retain(|node| &node.id != id);
            }
            ScenePatch::ReplaceAll {
                annotations, draft, ..
            } => {
                validate_nodes(annotations, draft.as_ref())
                    .map_err(ScenePatchError::InvalidScene)?;
                next.annotations.clone_from(annotations);
                sort_nodes(&mut next.annotations);
                next.draft.clone_from(draft);
                if let Some(node) = next.draft.as_mut() {
                    node.draft = true;
                    node.selected = false;
                }
            }
            ScenePatch::SetSelection { id, .. } => {
                if let Some(id) = id
                    && !next.annotations.iter().any(|node| &node.id == id)
                {
                    return Err(ScenePatchError::MissingAnnotation(id.clone()));
                }
                for node in &mut next.annotations {
                    node.selected = id.as_ref() == Some(&node.id);
                }
            }
            ScenePatch::SetDraft { node, .. } => {
                if let Some(node) = node {
                    validate_geometry(&node.geometry).map_err(ScenePatchError::InvalidScene)?;
                }
                next.draft.clone_from(node);
                if let Some(node) = next.draft.as_mut() {
                    node.draft = true;
                    node.selected = false;
                }
            }
        }
        next.revision = revision;
        *self = next;
        Ok(ScenePatchDisposition::Applied)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScenePatch {
    Upsert {
        base_revision: SceneRevision,
        revision: SceneRevision,
        node: AnnotationNode,
    },
    Remove {
        base_revision: SceneRevision,
        revision: SceneRevision,
        id: AnnotationId,
    },
    ReplaceAll {
        base_revision: SceneRevision,
        revision: SceneRevision,
        annotations: Vec<AnnotationNode>,
        draft: Option<AnnotationNode>,
    },
    SetSelection {
        base_revision: SceneRevision,
        revision: SceneRevision,
        id: Option<AnnotationId>,
    },
    SetDraft {
        base_revision: SceneRevision,
        revision: SceneRevision,
        node: Option<AnnotationNode>,
    },
}

impl ScenePatch {
    pub const fn base_revision(&self) -> SceneRevision {
        match self {
            Self::Upsert { base_revision, .. }
            | Self::Remove { base_revision, .. }
            | Self::ReplaceAll { base_revision, .. }
            | Self::SetSelection { base_revision, .. }
            | Self::SetDraft { base_revision, .. } => *base_revision,
        }
    }

    pub const fn revision(&self) -> SceneRevision {
        match self {
            Self::Upsert { revision, .. }
            | Self::Remove { revision, .. }
            | Self::ReplaceAll { revision, .. }
            | Self::SetSelection { revision, .. }
            | Self::SetDraft { revision, .. } => *revision,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScenePatchDisposition {
    Applied,
    IgnoredDuplicate,
    IgnoredStale,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SceneError {
    DuplicateAnnotation(AnnotationId),
    InvalidArrow,
    InvalidStroke,
    TooManyAnnotations,
}

impl fmt::Display for SceneError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateAnnotation(id) => {
                write!(formatter, "duplicate annotation {}", id.as_str())
            }
            Self::InvalidArrow => formatter.write_str("arrow endpoints must differ"),
            Self::InvalidStroke => formatter.write_str("stroke geometry is invalid"),
            Self::TooManyAnnotations => formatter.write_str("scene annotation limit exceeded"),
        }
    }
}

impl Error for SceneError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScenePatchError {
    RevisionMismatch {
        current: SceneRevision,
        base: SceneRevision,
    },
    NonSequential {
        current: SceneRevision,
        next: SceneRevision,
    },
    MissingAnnotation(AnnotationId),
    InvalidScene(SceneError),
}

impl fmt::Display for ScenePatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "scene patch rejected: {self:?}")
    }
}

impl Error for ScenePatchError {}

fn validate_nodes(
    annotations: &[AnnotationNode],
    draft: Option<&AnnotationNode>,
) -> Result<(), SceneError> {
    if annotations.len() > MAX_SCENE_ANNOTATIONS {
        return Err(SceneError::TooManyAnnotations);
    }
    let mut ids = BTreeSet::new();
    for node in annotations.iter().chain(draft) {
        validate_geometry(&node.geometry)?;
        if !ids.insert(node.id.clone()) {
            return Err(SceneError::DuplicateAnnotation(node.id.clone()));
        }
    }
    Ok(())
}

fn validate_geometry(geometry: &AnnotationGeometry) -> Result<(), SceneError> {
    match geometry {
        AnnotationGeometry::Point { .. }
        | AnnotationGeometry::Rectangle { .. }
        | AnnotationGeometry::Ellipse { .. } => Ok(()),
        AnnotationGeometry::Arrow { tail, head } => {
            if tail == head {
                Err(SceneError::InvalidArrow)
            } else {
                Ok(())
            }
        }
        AnnotationGeometry::Stroke { points } => {
            if points.len() < 2 || points.len() > MAX_STROKE_POINTS {
                return Err(SceneError::InvalidStroke);
            }
            let (mut min_x, mut min_y, mut max_x, mut max_y) = (1.0_f64, 1.0_f64, 0.0_f64, 0.0_f64);
            for point in points {
                min_x = min_x.min(point.x);
                min_y = min_y.min(point.y);
                max_x = max_x.max(point.x);
                max_y = max_y.max(point.y);
            }
            if max_x > min_x && max_y > min_y {
                Ok(())
            } else {
                Err(SceneError::InvalidStroke)
            }
        }
    }
}

fn sort_nodes(nodes: &mut [AnnotationNode]) {
    nodes.sort_by(|left, right| {
        left.ordinal
            .cmp(&right.ordinal)
            .then_with(|| left.id.cmp(&right.id))
    });
}
