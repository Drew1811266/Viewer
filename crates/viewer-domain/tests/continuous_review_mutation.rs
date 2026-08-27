#[path = "support/continuous.rs"]
mod support;
use support::*;
use viewer_domain::review::FeedbackAnchor;
use viewer_domain::review::continuous::*;
use viewer_domain::*;

#[test]
fn text_edit_replaces_one_revision_without_rewriting_the_original_or_target_ids() {
    let original = feedback(10, &[(11, 1), (12, 2)]);
    let changed = update_feedback_text(
        &original,
        ReviewTextRevisionId::from_u128(20),
        "  袖口收紧\n但保留原材质。 ",
    )
    .unwrap();
    assert_eq!(changed.id, FeedbackId::from_u128(10));
    assert_eq!(changed.targets, original.targets);
    assert_eq!(
        changed.text_revision_id,
        ReviewTextRevisionId::from_u128(20)
    );
    assert_eq!(changed.text, "  袖口收紧\n但保留原材质。 ");
    assert_eq!(original.text, "袖口收紧，保留褶皱");
    assert_eq!(changed.created_at_ms, 10);
}

#[test]
fn reanchoring_only_revises_the_selected_target() {
    let original = feedback(10, &[(11, 1), (12, 2)]);
    let changed = replace_target(
        &original,
        ReviewTargetId::from_u128(11),
        ReviewTargetRevisionId::from_u128(20),
        rect(),
    )
    .unwrap();
    assert_eq!(changed.targets[0].anchor, rect());
    assert_eq!(
        changed.targets[0].revision_id,
        ReviewTargetRevisionId::from_u128(20)
    );
    assert_eq!(changed.targets[1], original.targets[1]);
    assert_eq!(changed.text_revision_id, original.text_revision_id);
    assert_eq!(original.targets[0].anchor, FeedbackAnchor::Asset);
}

#[test]
fn identity_cannot_be_reused_for_different_content() {
    let original = feedback(10, &[(11, 1), (12, 2)]);
    assert_eq!(
        update_feedback_text(&original, original.text_revision_id, "其他要求"),
        Err(ContinuousReviewError::DuplicateIdentity)
    );
    assert_eq!(
        replace_target(
            &original,
            ReviewTargetId::from_u128(11),
            ReviewTargetRevisionId::from_u128(11),
            rect()
        ),
        Err(ContinuousReviewError::DuplicateIdentity)
    );
    assert_eq!(
        replace_target(
            &original,
            ReviewTargetId::from_u128(11),
            ReviewTargetRevisionId::from_u128(12),
            rect()
        ),
        Err(ContinuousReviewError::DuplicateIdentity)
    );
    let current = state();
    assert_eq!(
        withdraw_targets(
            &current,
            &[ReviewTargetId::from_u128(11)],
            current.snapshot_id
        ),
        Err(ContinuousReviewError::DuplicateIdentity)
    );
}

#[test]
fn invalid_edits_leave_original_untouched() {
    let original = feedback(10, &[(11, 1)]);
    assert_eq!(
        update_feedback_text(&original, ReviewTextRevisionId::from_u128(20), " \n "),
        Err(ContinuousReviewError::InvalidData)
    );
    assert_eq!(
        replace_target(
            &original,
            ReviewTargetId::from_u128(99),
            ReviewTargetRevisionId::from_u128(20),
            rect()
        ),
        Err(ContinuousReviewError::MissingReference)
    );
    assert_eq!(
        replace_target(
            &original,
            ReviewTargetId::from_u128(11),
            ReviewTargetRevisionId::from_u128(20),
            FeedbackAnchor::VideoRange {
                start_us: 2,
                end_us: 1
            }
        ),
        Err(ContinuousReviewError::InvalidData)
    );
    assert_eq!(original.text, "袖口收紧，保留褶皱");
    assert_eq!(original.targets[0].anchor, FeedbackAnchor::Asset);
}

#[test]
fn partial_withdrawal_preserves_shared_text_and_other_targets() {
    let original = state();
    let changed = withdraw_targets(
        &original,
        &[ReviewTargetId::from_u128(11)],
        ReviewSnapshotId::from_u128(4),
    )
    .unwrap();
    assert_eq!(changed.snapshot_id, ReviewSnapshotId::from_u128(4));
    assert_eq!(changed.feedback.len(), 1);
    assert_eq!(changed.feedback[0].targets.len(), 1);
    assert_eq!(
        changed.feedback[0].targets[0].id,
        ReviewTargetId::from_u128(12)
    );
    assert_eq!(changed.feedback[0].text, original.feedback[0].text);
    assert_eq!(original.feedback[0].targets.len(), 2);
    let empty = withdraw_targets(
        &changed,
        &[ReviewTargetId::from_u128(12)],
        ReviewSnapshotId::from_u128(5),
    )
    .unwrap();
    assert!(empty.feedback.is_empty());
    assert_eq!(empty.assets, original.assets);
    assert_eq!(empty.validate(), Ok(()));
}

#[test]
fn empty_operations_do_not_mint_new_snapshots_or_revisions() {
    let original = state();
    assert_eq!(
        withdraw_targets(&original, &[], ReviewSnapshotId::from_u128(4)).unwrap(),
        original
    );
    let f = &original.feedback[0];
    assert_eq!(
        update_feedback_text(f, ReviewTextRevisionId::from_u128(50), &f.text).unwrap(),
        *f
    );
    assert_eq!(
        replace_target(
            f,
            ReviewTargetId::from_u128(11),
            ReviewTargetRevisionId::from_u128(50),
            FeedbackAnchor::Asset
        )
        .unwrap(),
        *f
    );
}

#[test]
fn withdrawal_requires_an_exact_unique_selection() {
    let original = state();
    assert_eq!(
        withdraw_targets(
            &original,
            &[ReviewTargetId::from_u128(99)],
            ReviewSnapshotId::from_u128(4)
        ),
        Err(ContinuousReviewError::MissingReference)
    );
    assert_eq!(
        withdraw_targets(
            &original,
            &[ReviewTargetId::from_u128(11); 2],
            ReviewSnapshotId::from_u128(4)
        ),
        Err(ContinuousReviewError::DuplicateIdentity)
    );
}

#[test]
fn adding_new_feedback_preserves_existing_work_and_registered_asset_versions() {
    let mut original = state();
    original.assets.push(asset(3));
    let added = add_feedback(
        &original,
        feedback(20, &[(21, 3)]),
        ReviewSnapshotId::from_u128(4),
    )
    .unwrap();
    assert_eq!(added.feedback.len(), 2);
    assert_eq!(added.feedback[0], original.feedback[0]);
    assert_eq!(
        added.feedback[1].targets[0].asset_version_id,
        AssetVersionId::from_u128(3)
    );
    assert_eq!(
        add_feedback(
            &original,
            feedback(10, &[(21, 3)]),
            ReviewSnapshotId::from_u128(4)
        ),
        Err(ContinuousReviewError::DuplicateIdentity)
    );
    assert_eq!(
        add_feedback(
            &original,
            feedback(20, &[(11, 3)]),
            ReviewSnapshotId::from_u128(4)
        ),
        Err(ContinuousReviewError::DuplicateIdentity)
    );
    assert_eq!(
        add_feedback(
            &original,
            feedback(20, &[(21, 99)]),
            ReviewSnapshotId::from_u128(4)
        ),
        Err(ContinuousReviewError::MissingReference)
    );
}

#[test]
fn pure_mutations_do_not_forge_a_parent_digest_or_keep_a_grandparent_reference() {
    let mut original = state();
    original.parent = Some(SnapshotRef {
        snapshot_id: ReviewSnapshotId::from_u128(2),
        blake3: [5; 32],
    });
    let next = add_feedback(
        &original,
        feedback(20, &[(21, 2)]),
        ReviewSnapshotId::from_u128(4),
    )
    .unwrap();
    assert!(next.parent.is_none());
    assert!(original.parent.is_some());
}

#[test]
fn change_reasons_are_typed_and_cannot_claim_a_withdrawal_is_an_archive() {
    let current = state();
    let key = key(&current, 11);
    let withdrawn = ReviewChange {
        target_id: key.target_id,
        before: Some(key),
        after: None,
        kind: ReviewChangeKind::Withdrawn,
        archive_id: None,
        historical_key: None,
    };
    assert_eq!(withdrawn.validate(), Ok(()));
    assert_eq!(
        ReviewChange {
            kind: ReviewChangeKind::Archived,
            ..withdrawn.clone()
        }
        .validate(),
        Err(ContinuousReviewError::InvalidData)
    );
    assert_eq!(
        ReviewChange {
            after: Some(key),
            ..withdrawn.clone()
        }
        .validate(),
        Err(ContinuousReviewError::InvalidData)
    );
    assert_eq!(
        ReviewChange {
            target_id: ReviewTargetId::from_u128(99),
            ..withdrawn
        }
        .validate(),
        Err(ContinuousReviewError::InvalidData)
    );
}

#[test]
fn archive_evidence_cannot_change_the_feedback_owner_of_a_retained_target() {
    let key = key(&state(), 11);
    let event = ReviewChange {
        target_id: key.target_id,
        before: Some(key),
        after: Some(key),
        kind: ReviewChangeKind::Archived,
        archive_id: Some(ReviewArchiveId::from_u128(1)),
        historical_key: Some(TargetVersionKey {
            feedback_id: FeedbackId::from_u128(99),
            ..key
        }),
    };
    assert_eq!(event.validate(), Err(ContinuousReviewError::InvalidData));
}
