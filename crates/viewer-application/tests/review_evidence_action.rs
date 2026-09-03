use viewer_application::review_workspace::{
    CURRENT_REVIEW_EVIDENCE_ACTION_POLICY, GeneratedReviewIds, ReviewAuthoringHead,
    ReviewBarrierKind, ReviewEvidenceActionPolicy, ReviewPublicationProtocol, StoredAuthoringState,
    dirty_evidence_assets, evidence_action_key,
};
use viewer_domain::{
    AssetVersionId, FeedbackId, ProjectId, RelativePath, ReviewArchiveId, ReviewCommandId,
    ReviewSnapshotId, ReviewStreamId, ReviewTargetId, ReviewTargetRevisionId, ReviewTextRevisionId,
    review::{
        AssetEvidence, AssetVersion, FeedbackAnchor, NormalizedArrow, NormalizedPoint,
        NormalizedRect, ReviewMedia,
        continuous::{
            ContinuousReviewState, ReviewAvailability, VersionedFeedback, VersionedTarget,
        },
    },
};

fn policy(renderer_version: u32, output_policy_version: u32) -> ReviewEvidenceActionPolicy {
    ReviewEvidenceActionPolicy {
        renderer_version,
        output_policy_version,
    }
}

#[test]
fn current_policy_invalidates_evidence_from_the_previous_renderer() {
    assert_eq!(CURRENT_REVIEW_EVIDENCE_ACTION_POLICY.renderer_version, 2);
    assert_ne!(
        evidence_action_key(
            &state("文字", [1; 32], 0.1),
            AssetVersionId::from_u128(1),
            policy(1, 1),
        )
        .unwrap(),
        evidence_action_key(
            &state("文字", [1; 32], 0.1),
            AssetVersionId::from_u128(1),
            CURRENT_REVIEW_EVIDENCE_ACTION_POLICY,
        )
        .unwrap(),
    );
}

fn state(text: &str, digest: [u8; 32], rect_x: f64) -> ContinuousReviewState {
    let asset_id = AssetVersionId::from_u128(1);
    ContinuousReviewState {
        project_id: ProjectId::from_u128(2),
        stream_id: ReviewStreamId::from_u128(3),
        snapshot_id: ReviewSnapshotId::from_u128(4),
        parent: None,
        assets: vec![AssetVersion {
            id: asset_id,
            source_entity_id: None,
            relative_path: RelativePath::parse("image.png").unwrap(),
            evidence: AssetEvidence {
                size_bytes: 100,
                modified_ns: 1,
                blake3: Some(digest),
            },
            media: ReviewMedia::Image {
                width: Some(1200),
                height: Some(800),
            },
            producer_asset_id: None,
            parent_asset_version_id: None,
        }],
        feedback: vec![VersionedFeedback {
            id: FeedbackId::from_u128(5),
            text_revision_id: ReviewTextRevisionId::from_u128(if text == "原文字" {
                6
            } else {
                7
            }),
            text: text.to_owned(),
            created_at_ms: 1,
            targets: vec![VersionedTarget {
                id: ReviewTargetId::from_u128(8),
                revision_id: ReviewTargetRevisionId::from_u128(9),
                asset_version_id: asset_id,
                anchor: FeedbackAnchor::ImageRect(
                    NormalizedRect::new(rect_x, 0.2, 0.3, 0.4).unwrap(),
                ),
                availability: ReviewAvailability::Ready,
            }],
            history_ref: None,
        }],
    }
}

fn authoring(state: ContinuousReviewState) -> StoredAuthoringState {
    StoredAuthoringState {
        head: ReviewAuthoringHead {
            sequence: 1,
            snapshot_id: state.snapshot_id,
        },
        publication_protocol: ReviewPublicationProtocol::V3,
        production: None,
        command_id: ReviewCommandId::from_u128(10),
        payload_digest: [11; 32],
        generated: GeneratedReviewIds {
            snapshot_id: state.snapshot_id,
            feedback_id: FeedbackId::from_u128(12),
            text_revision_id: ReviewTextRevisionId::from_u128(13),
            archive_id: ReviewArchiveId::from_u128(14),
            targets: vec![],
            migration: vec![],
            created_at_ms: 1,
        },
        state,
        changes: vec![],
        archives: vec![],
        adopted_usage: vec![],
        barrier: ReviewBarrierKind::None,
    }
}

#[test]
fn feedback_text_does_not_change_evidence_action_key() {
    let original = state("原文字", [1; 32], 0.1);
    let edited = state("新文字", [1; 32], 0.1);

    assert_eq!(
        evidence_action_key(&original, AssetVersionId::from_u128(1), policy(1, 1)).unwrap(),
        evidence_action_key(&edited, AssetVersionId::from_u128(1), policy(1, 1)).unwrap()
    );
}

#[test]
fn source_geometry_renderer_and_output_policy_each_invalidate_the_key() {
    let base = state("文字", [1; 32], 0.1);
    let key = evidence_action_key(&base, AssetVersionId::from_u128(1), policy(1, 1)).unwrap();

    assert_ne!(
        key,
        evidence_action_key(
            &state("文字", [2; 32], 0.1),
            AssetVersionId::from_u128(1),
            policy(1, 1)
        )
        .unwrap()
    );
    assert_ne!(
        key,
        evidence_action_key(
            &state("文字", [1; 32], 0.2),
            AssetVersionId::from_u128(1),
            policy(1, 1)
        )
        .unwrap()
    );
    assert_ne!(
        key,
        evidence_action_key(&base, AssetVersionId::from_u128(1), policy(2, 1)).unwrap()
    );
    assert_ne!(
        key,
        evidence_action_key(&base, AssetVersionId::from_u128(1), policy(1, 2)).unwrap()
    );
}

#[test]
fn extended_anchor_kinds_and_geometry_have_distinct_action_keys() {
    let mut point = state("文字", [1; 32], 0.1);
    point.feedback[0].targets[0].anchor =
        FeedbackAnchor::ImagePoint(NormalizedPoint::new(0.2, 0.3).unwrap());
    let mut arrow = point.clone();
    arrow.feedback[0].targets[0].anchor = FeedbackAnchor::ImageArrow(
        NormalizedArrow::new(
            NormalizedPoint::new(0.2, 0.3).unwrap(),
            NormalizedPoint::new(0.8, 0.7).unwrap(),
        )
        .unwrap(),
    );
    let mut ellipse = point.clone();
    ellipse.feedback[0].targets[0].anchor =
        FeedbackAnchor::ImageEllipse(NormalizedRect::new(0.1, 0.2, 0.3, 0.4).unwrap());

    let keys = [&point, &arrow, &ellipse]
        .map(|value| {
            evidence_action_key(value, AssetVersionId::from_u128(1), policy(1, 1)).unwrap()
        })
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(keys.len(), 3);
}

#[test]
fn dirty_selection_ignores_text_only_edits_and_selects_changed_geometry() {
    let original = authoring(state("原文字", [1; 32], 0.1));
    let text_only = authoring(state("新文字", [1; 32], 0.1));
    let geometry = authoring(state("新文字", [1; 32], 0.2));

    assert_eq!(
        dirty_evidence_assets(Some(&original), &text_only, policy(1, 1)).unwrap(),
        vec![]
    );
    assert_eq!(
        dirty_evidence_assets(Some(&original), &geometry, policy(1, 1)).unwrap(),
        vec![AssetVersionId::from_u128(1)]
    );
}

#[test]
fn dirty_selection_over_1000_assets_returns_only_the_changed_asset() {
    let mut previous = authoring(state("原文字", [1; 32], 0.1));
    let template = previous.state.assets[0].clone();
    for index in 2..=1_000_u128 {
        let mut asset = template.clone();
        asset.id = AssetVersionId::from_u128(index);
        asset.relative_path = RelativePath::parse(&format!("image-{index}.png")).unwrap();
        previous.state.assets.push(asset);
    }
    let mut text_only = previous.clone();
    text_only.state.feedback[0].text = "新文字".into();
    text_only.state.feedback[0].text_revision_id = ReviewTextRevisionId::from_u128(99);

    assert!(
        dirty_evidence_assets(Some(&previous), &text_only, policy(1, 1))
            .unwrap()
            .is_empty()
    );

    let changed_id = AssetVersionId::from_u128(731);
    text_only
        .state
        .assets
        .iter_mut()
        .find(|asset| asset.id == changed_id)
        .unwrap()
        .evidence
        .blake3 = Some([9; 32]);
    assert_eq!(
        dirty_evidence_assets(Some(&previous), &text_only, policy(1, 1)).unwrap(),
        vec![changed_id]
    );
}
