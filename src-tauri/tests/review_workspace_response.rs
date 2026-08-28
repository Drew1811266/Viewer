use std::sync::Arc;
use viewer_application::review_workspace::ReviewCommitReceipt;
use viewer_desktop::dto::review_workspace::*;
use viewer_domain::{
    review::{continuous::*, *},
    *,
};

fn shared_restore_plan() -> RestorePlan {
    let feedback = Arc::new(RestoreFeedbackContent {
        id: FeedbackId::from_u128(1),
        text_revision_id: ReviewTextRevisionId::from_u128(2),
        text: Arc::from("x".repeat(65_536)),
        created_at_ms: 1,
        history_ref: None,
    });
    let asset = Arc::new(AssetVersion {
        id: AssetVersionId::from_u128(3),
        source_entity_id: Some(EntityId::from_u128(4)),
        relative_path: RelativePath::parse("image.png").unwrap(),
        evidence: AssetEvidence {
            size_bytes: 20,
            modified_ns: 1,
            blake3: Some([1; 32]),
        },
        media: ReviewMedia::Image {
            width: Some(1),
            height: Some(1),
        },
        producer_asset_id: None,
        parent_asset_version_id: None,
    });
    RestorePlan {
        expected_snapshot_id: ReviewSnapshotId::from_u128(5),
        restored: (0..10_000)
            .map(|i| {
                let key = TargetVersionKey {
                    feedback_id: feedback.id,
                    text_revision_id: feedback.text_revision_id,
                    target_id: ReviewTargetId::from_u128(1_000 + i),
                    target_revision_id: ReviewTargetRevisionId::from_u128(20_000 + i),
                };
                RestoredFeedback {
                    historical_key: key,
                    feedback: feedback.clone(),
                    target: VersionedTarget {
                        id: key.target_id,
                        revision_id: key.target_revision_id,
                        asset_version_id: asset.id,
                        anchor: FeedbackAnchor::Asset,
                        availability: ReviewAvailability::Ready,
                    },
                    asset: asset.clone(),
                    continued_as_new: false,
                }
            })
            .collect(),
        conflicts: vec![],
        coverage_reversals: vec![],
        requires_source_check: vec![],
    }
}

#[test]
fn review_workspace_response_bounds_restore_expansion_and_preserves_known_receipts() {
    let plan = shared_restore_plan();
    let error = review_response(ReviewWire::from(plan.clone()), None).unwrap_err();
    assert_eq!(error.code, ReviewWorkspaceErrorCode::LimitExceeded);
    assert!(error.committed_receipt.is_none());
    let receipt = ReviewCommitReceipt {
        command_id: ReviewCommandId::from_u128(6),
        payload_digest: [7; 32],
        snapshot: SnapshotRef {
            snapshot_id: ReviewSnapshotId::from_u128(8),
            blake3: [9; 32],
        },
    };
    let error = review_response(ReviewWire::from(plan), Some(receipt)).unwrap_err();
    assert_eq!(
        error.code,
        ReviewWorkspaceErrorCode::CommittedViewUnavailable
    );
    assert_eq!(error.committed_receipt.unwrap().0, receipt);

    let mut small = shared_restore_plan();
    small.restored.truncate(1);
    let wire = ReviewWire::from(small);
    let expected = serde_json::to_value(&wire).unwrap();
    assert_eq!(
        serde_json::to_value(review_response(wire, None).unwrap()).unwrap(),
        expected
    );
}

#[test]
fn review_workspace_legacy_sized_video_anchors_never_round_in_javascript() {
    const EXACT: u64 = 9_007_199_254_740_991;
    for anchor in [
        FeedbackAnchor::VideoPoint {
            position_us: EXACT + 2,
        },
        FeedbackAnchor::VideoRange {
            start_us: 0,
            end_us: EXACT + 2,
        },
    ] {
        // Legacy media with unknown duration can contain these u64 anchors. The desktop must
        // reject them before emitting a JSON number that JavaScript would silently round.
        let error = review_response(ReviewWire::from(anchor), None).unwrap_err();
        assert_eq!(error.code, ReviewWorkspaceErrorCode::InvalidData);
    }
    let value = review_response(
        ReviewWire::from(FeedbackAnchor::VideoPoint { position_us: EXACT }),
        None,
    )
    .unwrap();
    assert_eq!(serde_json::to_value(value).unwrap()["positionUs"], EXACT);
}
