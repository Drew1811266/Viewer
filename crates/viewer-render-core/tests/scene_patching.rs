use viewer_render_core::{
    AnnotationGeometry, AnnotationId, AnnotationNode, NormalizedPoint, NormalizedRect, ScenePatch,
    ScenePatchDisposition, ScenePatchError, SceneRevision, SceneSnapshot,
};

fn point(id: &str, ordinal: u32, x: f64, y: f64) -> AnnotationNode {
    AnnotationNode::new(
        AnnotationId::new(id).unwrap(),
        ordinal,
        AnnotationGeometry::Point {
            position: NormalizedPoint::new(x, y).unwrap(),
        },
    )
    .unwrap()
}

#[test]
fn upsert_patch_is_idempotent_and_revision_ordered() {
    let mut scene = SceneSnapshot::empty(SceneRevision(0));
    let patch = ScenePatch::Upsert {
        base_revision: SceneRevision(0),
        revision: SceneRevision(1),
        node: point("feedback-1", 1, 0.2, 0.3),
    };

    assert_eq!(
        scene.apply_patch(&patch).unwrap(),
        ScenePatchDisposition::Applied
    );
    assert_eq!(
        scene.apply_patch(&patch).unwrap(),
        ScenePatchDisposition::IgnoredDuplicate
    );
    assert_eq!(scene.revision(), SceneRevision(1));
    assert_eq!(scene.annotations().len(), 1);
}

#[test]
fn selection_patch_selects_only_the_requested_node() {
    let mut scene = SceneSnapshot::new(
        SceneRevision(4),
        vec![
            point("feedback-1", 1, 0.2, 0.3),
            point("feedback-2", 2, 0.7, 0.8),
        ],
        None,
    )
    .unwrap();

    scene
        .apply_patch(&ScenePatch::SetSelection {
            base_revision: SceneRevision(4),
            revision: SceneRevision(5),
            id: Some(AnnotationId::new("feedback-2").unwrap()),
        })
        .unwrap();

    assert!(!scene.annotations()[0].selected);
    assert!(scene.annotations()[1].selected);
}

#[test]
fn revision_gap_is_rejected_without_mutating_the_scene() {
    let mut scene = SceneSnapshot::empty(SceneRevision(2));
    let result = scene.apply_patch(&ScenePatch::Remove {
        base_revision: SceneRevision(1),
        revision: SceneRevision(3),
        id: AnnotationId::new("feedback-1").unwrap(),
    });

    assert_eq!(
        result,
        Err(ScenePatchError::RevisionMismatch {
            current: SceneRevision(2),
            base: SceneRevision(1),
        })
    );
    assert_eq!(scene.revision(), SceneRevision(2));
}

#[test]
fn duplicate_ids_and_oversized_strokes_are_rejected() {
    let duplicate = point("feedback-1", 1, 0.2, 0.3);
    assert!(
        SceneSnapshot::new(SceneRevision(1), vec![duplicate.clone(), duplicate], None).is_err()
    );

    let stroke = AnnotationNode::new(
        AnnotationId::new("stroke-1").unwrap(),
        1,
        AnnotationGeometry::Stroke {
            points: vec![NormalizedPoint::new(0.5, 0.5).unwrap(); 2_049],
        },
    );
    assert!(stroke.is_err());
}

#[test]
fn normalized_rect_rejects_geometry_outside_the_image() {
    assert!(NormalizedRect::new(0.9, 0.9, 0.2, 0.2).is_err());
    assert!(NormalizedRect::new(0.1, 0.1, 0.4, 0.5).is_ok());
}
