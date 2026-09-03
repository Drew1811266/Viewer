use viewer_application::review_workspace::{
    GeneratedReviewIds, ReviewAuthoringHead, ReviewBarrierKind, ReviewPublicationProtocol,
    StoredAuthoringState, fold_public_changes,
};
use viewer_domain::{
    AssetVersionId, FeedbackId, ProjectId, RelativePath, ReviewArchiveId, ReviewCommandId,
    ReviewSnapshotId, ReviewStreamId, ReviewTargetId, ReviewTargetRevisionId, ReviewTextRevisionId,
    review::{
        AssetEvidence, AssetVersion, FeedbackAnchor, ReviewMedia,
        continuous::{
            ContinuousReviewState, ReviewAvailability, ReviewChange, ReviewChangeKind,
            TargetVersionKey, VersionedFeedback, VersionedTarget,
        },
    },
};

fn asset() -> AssetVersion {
    AssetVersion {
        id: AssetVersionId::from_u128(1),
        source_entity_id: None,
        relative_path: RelativePath::parse("image.png").unwrap(),
        evidence: AssetEvidence {
            size_bytes: 1,
            modified_ns: 1,
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

fn key(text_revision: u128, target_revision: u128) -> TargetVersionKey {
    TargetVersionKey {
        feedback_id: FeedbackId::from_u128(2),
        text_revision_id: ReviewTextRevisionId::from_u128(text_revision),
        target_id: ReviewTargetId::from_u128(3),
        target_revision_id: ReviewTargetRevisionId::from_u128(target_revision),
    }
}

fn state(snapshot: u128, text_revision: u128, target_revision: u128) -> ContinuousReviewState {
    ContinuousReviewState {
        project_id: ProjectId::from_u128(4),
        stream_id: ReviewStreamId::from_u128(5),
        snapshot_id: ReviewSnapshotId::from_u128(snapshot),
        parent: None,
        assets: vec![asset()],
        feedback: vec![VersionedFeedback {
            id: FeedbackId::from_u128(2),
            text_revision_id: ReviewTextRevisionId::from_u128(text_revision),
            text: format!("文字-{text_revision}"),
            created_at_ms: 1,
            targets: vec![VersionedTarget {
                id: ReviewTargetId::from_u128(3),
                revision_id: ReviewTargetRevisionId::from_u128(target_revision),
                asset_version_id: AssetVersionId::from_u128(1),
                anchor: FeedbackAnchor::Asset,
                availability: ReviewAvailability::Ready,
            }],
            history_ref: None,
        }],
    }
}

fn with_parent(
    mut state: ContinuousReviewState,
    parent: ReviewSnapshotId,
) -> ContinuousReviewState {
    state.parent = Some(viewer_domain::review::continuous::SnapshotRef {
        snapshot_id: parent,
        blake3: [0; 32],
    });
    state
}

fn authoring(
    sequence: u64,
    state: ContinuousReviewState,
    changes: Vec<ReviewChange>,
) -> StoredAuthoringState {
    StoredAuthoringState {
        head: ReviewAuthoringHead {
            sequence,
            snapshot_id: state.snapshot_id,
        },
        publication_protocol: ReviewPublicationProtocol::V3,
        production: None,
        command_id: ReviewCommandId::from_u128(100 + u128::from(sequence)),
        payload_digest: [sequence as u8; 32],
        generated: GeneratedReviewIds {
            snapshot_id: state.snapshot_id,
            feedback_id: FeedbackId::from_u128(20),
            text_revision_id: ReviewTextRevisionId::from_u128(21),
            archive_id: ReviewArchiveId::from_u128(22),
            targets: vec![],
            migration: vec![],
            created_at_ms: sequence as i64,
        },
        state,
        changes,
        archives: vec![],
        adopted_usage: vec![],
        barrier: ReviewBarrierKind::None,
    }
}

#[test]
fn add_then_edit_folds_to_one_final_addition() {
    let published = ContinuousReviewState::empty(
        ProjectId::from_u128(4),
        ReviewStreamId::from_u128(5),
        ReviewSnapshotId::from_u128(0),
    );
    let first_key = key(10, 11);
    let final_key = key(12, 13);
    let first = authoring(
        1,
        state(30, 10, 11),
        vec![ReviewChange {
            target_id: first_key.target_id,
            before: None,
            after: Some(first_key),
            kind: ReviewChangeKind::Added,
            archive_id: None,
            historical_key: None,
        }],
    );
    let second = authoring(
        2,
        with_parent(state(31, 12, 13), ReviewSnapshotId::from_u128(30)),
        vec![ReviewChange {
            target_id: first_key.target_id,
            before: Some(first_key),
            after: Some(final_key),
            kind: ReviewChangeKind::Edited,
            archive_id: None,
            historical_key: None,
        }],
    );

    assert_eq!(
        fold_public_changes(&published, &[first, second]).unwrap(),
        vec![ReviewChange {
            target_id: final_key.target_id,
            before: None,
            after: Some(final_key),
            kind: ReviewChangeKind::Added,
            archive_id: None,
            historical_key: None,
        }]
    );
}

#[test]
fn add_then_withdraw_has_no_public_target_delta() {
    let published = ContinuousReviewState::empty(
        ProjectId::from_u128(4),
        ReviewStreamId::from_u128(5),
        ReviewSnapshotId::from_u128(0),
    );
    let added_key = key(10, 11);
    let first = authoring(
        1,
        state(30, 10, 11),
        vec![ReviewChange {
            target_id: added_key.target_id,
            before: None,
            after: Some(added_key),
            kind: ReviewChangeKind::Added,
            archive_id: None,
            historical_key: None,
        }],
    );
    let mut empty = with_parent(state(31, 10, 11), ReviewSnapshotId::from_u128(30));
    empty.feedback.clear();
    let second = authoring(
        2,
        empty,
        vec![ReviewChange {
            target_id: added_key.target_id,
            before: Some(added_key),
            after: None,
            kind: ReviewChangeKind::Withdrawn,
            archive_id: None,
            historical_key: None,
        }],
    );

    assert!(
        fold_public_changes(&published, &[first, second])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn repeated_edits_fold_from_the_public_key_to_the_final_key() {
    let published = state(20, 6, 7);
    let public_key = key(6, 7);
    let first_key = key(10, 11);
    let final_key = key(12, 13);
    let first = authoring(
        1,
        state(30, 10, 11),
        vec![ReviewChange {
            target_id: public_key.target_id,
            before: Some(public_key),
            after: Some(first_key),
            kind: ReviewChangeKind::Edited,
            archive_id: None,
            historical_key: None,
        }],
    );
    let second = authoring(
        2,
        with_parent(state(31, 12, 13), ReviewSnapshotId::from_u128(30)),
        vec![ReviewChange {
            target_id: public_key.target_id,
            before: Some(first_key),
            after: Some(final_key),
            kind: ReviewChangeKind::Edited,
            archive_id: None,
            historical_key: None,
        }],
    );

    assert_eq!(
        fold_public_changes(&published, &[first, second]).unwrap(),
        vec![ReviewChange {
            target_id: public_key.target_id,
            before: Some(public_key),
            after: Some(final_key),
            kind: ReviewChangeKind::Edited,
            archive_id: None,
            historical_key: None,
        }]
    );
}

#[test]
fn a_public_removal_keeps_its_typed_withdrawal_cause() {
    let published = state(20, 6, 7);
    let public_key = key(6, 7);
    let mut empty = state(30, 6, 7);
    empty.feedback.clear();
    let withdrawn = authoring(
        1,
        empty,
        vec![ReviewChange {
            target_id: public_key.target_id,
            before: Some(public_key),
            after: None,
            kind: ReviewChangeKind::Withdrawn,
            archive_id: None,
            historical_key: None,
        }],
    );

    assert_eq!(
        fold_public_changes(&published, &[withdrawn]).unwrap(),
        vec![ReviewChange {
            target_id: public_key.target_id,
            before: Some(public_key),
            after: None,
            kind: ReviewChangeKind::Withdrawn,
            archive_id: None,
            historical_key: None,
        }]
    );
}

#[test]
fn availability_changes_keep_their_public_transition_kind() {
    let published = state(20, 6, 7);
    let public_key = key(6, 7);
    let final_key = key(6, 8);
    let mut unavailable = state(30, 6, 8);
    unavailable.feedback[0].targets[0].availability = ReviewAvailability::NeedsConfirmation(vec![
        viewer_domain::review::continuous::ReviewPendingReason::SourceChanged,
    ]);
    let changed = authoring(
        1,
        unavailable,
        vec![ReviewChange {
            target_id: public_key.target_id,
            before: Some(public_key),
            after: Some(final_key),
            kind: ReviewChangeKind::AvailabilityChanged,
            archive_id: None,
            historical_key: None,
        }],
    );

    assert_eq!(
        fold_public_changes(&published, &[changed]).unwrap()[0].kind,
        ReviewChangeKind::AvailabilityChanged
    );
}
