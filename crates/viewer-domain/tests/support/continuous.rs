#![allow(dead_code)]

use viewer_domain::review::continuous::*;
use viewer_domain::review::{
    AssetEvidence, AssetVersion, FeedbackAnchor, NormalizedRect, ReviewMedia,
};
use viewer_domain::*;

pub fn asset(id: u128) -> AssetVersion {
    AssetVersion {
        id: AssetVersionId::from_u128(id),
        source_entity_id: None,
        relative_path: RelativePath::parse(&format!("images/{id}.png")).unwrap(),
        evidence: AssetEvidence {
            size_bytes: 100,
            modified_ns: 10,
            blake3: Some([1; 32]),
        },
        media: ReviewMedia::Image {
            width: Some(100),
            height: Some(100),
        },
        producer_asset_id: None,
        parent_asset_version_id: None,
    }
}

pub fn feedback(id: u128, targets: &[(u128, u128)]) -> VersionedFeedback {
    VersionedFeedback {
        id: FeedbackId::from_u128(id),
        text_revision_id: ReviewTextRevisionId::from_u128(id),
        text: "袖口收紧，保留褶皱".into(),
        created_at_ms: 10,
        targets: targets
            .iter()
            .map(|&(target, asset)| {
                VersionedTarget::asset(
                    ReviewTargetId::from_u128(target),
                    ReviewTargetRevisionId::from_u128(target),
                    AssetVersionId::from_u128(asset),
                )
            })
            .collect(),
        history_ref: None,
    }
}

pub fn state() -> ContinuousReviewState {
    let mut state = ContinuousReviewState::empty(
        ProjectId::from_u128(1),
        ReviewStreamId::from_u128(2),
        ReviewSnapshotId::from_u128(3),
    );
    state.assets = vec![asset(1), asset(2)];
    state.feedback = vec![feedback(10, &[(11, 1), (12, 2)])];
    state
}

pub fn key(state: &ContinuousReviewState, id: u128) -> TargetVersionKey {
    state.target_key(ReviewTargetId::from_u128(id)).unwrap()
}

pub fn reference(state: &ContinuousReviewState) -> SnapshotRef {
    // Test fixture reference only; production digests are supplied by the repository.
    SnapshotRef {
        snapshot_id: state.snapshot_id,
        blake3: [7; 32],
    }
}

pub fn rect() -> FeedbackAnchor {
    FeedbackAnchor::ImageRect(NormalizedRect::new(0.1, 0.2, 0.3, 0.4).unwrap())
}
