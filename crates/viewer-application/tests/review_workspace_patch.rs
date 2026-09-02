use viewer_application::{
    review_evidence::HistorySelector,
    review_workspace::{
        GeneratedReviewIds, ReviewAuthoringHead, ReviewBarrierKind, ReviewPatchError,
        ReviewWorkspaceCurrent, ReviewWorkspacePatch, StoredAuthoringState,
    },
};
use viewer_domain::{
    AssetVersionId, EntityId, FeedbackId, ProjectId, RelativePath, ReviewArchiveId,
    ReviewCommandId, ReviewSnapshotId, ReviewStreamId, ReviewTargetId, ReviewTargetRevisionId,
    ReviewTextRevisionId,
    review::{
        AssetEvidence, AssetVersion, FeedbackAnchor, ReviewMedia,
        continuous::{
            ContinuousReviewState, ReviewAvailability, SnapshotRef, VersionedFeedback,
            VersionedTarget,
        },
    },
};

fn asset(id: u128) -> AssetVersion {
    AssetVersion {
        id: AssetVersionId::from_u128(id),
        source_entity_id: Some(EntityId::from_u128(id)),
        relative_path: RelativePath::parse(&format!("asset-{id}.png")).unwrap(),
        evidence: AssetEvidence {
            size_bytes: 10,
            modified_ns: 20,
            blake3: Some([id as u8; 32]),
        },
        media: ReviewMedia::Image {
            width: Some(640),
            height: Some(480),
        },
        producer_asset_id: None,
        parent_asset_version_id: None,
    }
}

fn feedback(text: &str, text_revision: u128, asset: AssetVersionId) -> VersionedFeedback {
    VersionedFeedback {
        id: FeedbackId::from_u128(30),
        text_revision_id: ReviewTextRevisionId::from_u128(text_revision),
        text: text.into(),
        created_at_ms: 1,
        targets: vec![VersionedTarget {
            id: ReviewTargetId::from_u128(40),
            revision_id: ReviewTargetRevisionId::from_u128(text_revision),
            asset_version_id: asset,
            anchor: FeedbackAnchor::Asset,
            availability: ReviewAvailability::Ready,
        }],
        history_ref: None,
    }
}

fn workspace_current(
    sequence: u64,
    snapshot: u128,
    assets: Vec<AssetVersion>,
    feedback: Vec<VersionedFeedback>,
) -> ReviewWorkspaceCurrent {
    let snapshot_id = ReviewSnapshotId::from_u128(snapshot);
    ReviewWorkspaceCurrent {
        authoring: StoredAuthoringState {
            head: ReviewAuthoringHead {
                sequence,
                snapshot_id,
            },
            production: None,
            state: ContinuousReviewState {
                project_id: ProjectId::from_u128(1),
                stream_id: ReviewStreamId::from_u128(2),
                snapshot_id,
                parent: None,
                assets,
                feedback,
            },
            command_id: ReviewCommandId::from_u128(50 + sequence as u128),
            payload_digest: [sequence as u8; 32],
            generated: GeneratedReviewIds {
                snapshot_id,
                feedback_id: FeedbackId::from_u128(60 + sequence as u128),
                text_revision_id: ReviewTextRevisionId::from_u128(70 + sequence as u128),
                archive_id: ReviewArchiveId::from_u128(80 + sequence as u128),
                targets: vec![],
                migration: vec![],
                created_at_ms: 1,
            },
            changes: vec![],
            archives: vec![],
            adopted_usage: vec![],
            barrier: ReviewBarrierKind::None,
        },
        published_ref: Some(SnapshotRef {
            snapshot_id: ReviewSnapshotId::from_u128(9),
            blake3: [9; 32],
        }),
        evidence: vec![],
    }
}

fn workspace_current_with_feedback(text: &str) -> ReviewWorkspaceCurrent {
    let image = asset(10);
    workspace_current(
        1,
        100,
        vec![image.clone()],
        vec![feedback(text, 31, image.id)],
    )
}

#[test]
fn patch_reconstructs_the_same_logical_view_as_a_full_projection() {
    let before = workspace_current_with_feedback("原意见");
    let image = before.authoring.state.assets[0].clone();
    let after = workspace_current(
        2,
        101,
        vec![image.clone()],
        vec![feedback("新意见", 32, image.id)],
    );
    let patch = ReviewWorkspacePatch::between(Some(&before), &after, None);

    assert_eq!(patch.apply(Some(before)).unwrap(), after);
}

#[test]
fn patch_rejects_a_different_basis() {
    let before = workspace_current_with_feedback("原意见");
    let image = before.authoring.state.assets[0].clone();
    let after = workspace_current(
        2,
        101,
        vec![image.clone()],
        vec![feedback("新意见", 32, image.id)],
    );
    let mut wrong = before.clone();
    wrong.authoring.head.snapshot_id = ReviewSnapshotId::from_u128(999);
    let patch = ReviewWorkspacePatch::between(Some(&before), &after, None);

    assert_eq!(patch.apply(Some(wrong)), Err(ReviewPatchError::StaleBasis));
}

#[test]
fn patch_rejects_a_target_whose_head_and_state_disagree() {
    let before = workspace_current_with_feedback("原意见");
    let image = before.authoring.state.assets[0].clone();
    let mut after = workspace_current(
        2,
        101,
        vec![image.clone()],
        vec![feedback("新意见", 32, image.id)],
    );
    after.authoring.state.snapshot_id = ReviewSnapshotId::from_u128(777);
    let patch = ReviewWorkspacePatch::between(Some(&before), &after, None);

    assert_eq!(
        patch.apply(Some(before)),
        Err(ReviewPatchError::InvalidPatch)
    );
}

#[test]
fn additions_and_restores_upsert_whole_stable_id_values() {
    let before = workspace_current(1, 100, vec![], vec![]);
    let image = asset(10);
    let after = workspace_current(
        2,
        101,
        vec![image.clone()],
        vec![feedback("恢复", 32, image.id)],
    );
    let patch = ReviewWorkspacePatch::between(Some(&before), &after, None);

    assert_eq!(patch.upsert_assets, after.authoring.state.assets);
    assert_eq!(patch.upsert_feedback, after.authoring.state.feedback);
    assert_eq!(patch.apply(Some(before)).unwrap(), after);
}

#[test]
fn first_save_reconstructs_from_an_empty_workspace() {
    let image = asset(10);
    let mut after = workspace_current(
        1,
        101,
        vec![image.clone()],
        vec![feedback("第一条", 32, image.id)],
    );
    after.published_ref = None;
    let patch = ReviewWorkspacePatch::between(None, &after, None);

    assert_eq!(patch.apply(None).unwrap(), after);
}

#[test]
fn withdraw_and_archive_removals_are_sorted_by_stable_id() {
    let first = asset(11);
    let second = asset(10);
    let before = workspace_current(1, 100, vec![first, second], vec![]);
    let after = workspace_current(2, 101, vec![], vec![]);
    let patch = ReviewWorkspacePatch::between(Some(&before), &after, None);

    let mut expected = vec![AssetVersionId::from_u128(10), AssetVersionId::from_u128(11)];
    expected.sort_by_key(ToString::to_string);
    assert_eq!(patch.remove_asset_version_ids, expected);
    assert_eq!(patch.apply(Some(before)).unwrap(), after);
}

#[test]
fn redraw_and_text_edit_replace_existing_feedback_without_duplicates() {
    let before = workspace_current_with_feedback("原意见");
    let image = before.authoring.state.assets[0].clone();
    let after = workspace_current(
        2,
        101,
        vec![image.clone()],
        vec![feedback("重绘后", 33, image.id)],
    );
    let patch = ReviewWorkspacePatch::between(Some(&before), &after, None);

    assert_eq!(patch.upsert_feedback.len(), 1);
    let applied = patch.apply(Some(before)).unwrap();
    assert_eq!(
        applied.authoring.state.feedback,
        after.authoring.state.feedback
    );
}

#[test]
fn duplicate_patch_identities_are_rejected() {
    let before = workspace_current_with_feedback("原意见");
    let image = before.authoring.state.assets[0].clone();
    let after = workspace_current(
        2,
        101,
        vec![image.clone()],
        vec![feedback("新意见", 32, image.id)],
    );
    let mut patch = ReviewWorkspacePatch::between(Some(&before), &after, None);
    patch.upsert_feedback.push(patch.upsert_feedback[0].clone());

    assert_eq!(
        patch.apply(Some(before)),
        Err(ReviewPatchError::InvalidPatch)
    );
}

#[test]
fn history_is_replaced_only_for_an_explicit_barrier() {
    let before = workspace_current_with_feedback("原意见");
    let image = before.authoring.state.assets[0].clone();
    let after = workspace_current(
        2,
        101,
        vec![image.clone()],
        vec![feedback("新意见", 32, image.id)],
    );
    let normal = ReviewWorkspacePatch::between(Some(&before), &after, None);
    let selector = HistorySelector::Snapshot(SnapshotRef {
        snapshot_id: ReviewSnapshotId::from_u128(700),
        blake3: [7; 32],
    });
    let barrier =
        ReviewWorkspacePatch::between(Some(&before), &after, Some(vec![selector.clone()]));

    assert_eq!(normal.history_selectors, None);
    assert_eq!(barrier.history_selectors, Some(vec![selector]));
}
