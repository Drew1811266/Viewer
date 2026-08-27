#[path = "support/continuous.rs"]
mod support;
use support::*;
use viewer_domain::review::FeedbackAnchor;
use viewer_domain::review::continuous::*;
use viewer_domain::*;

fn restore(key: TargetVersionKey) -> RestoreDecision {
    RestoreDecision {
        historical_key: key,
        choice: RestoreChoice::UseHistorical,
    }
}
fn continue_as_new(
    key: TargetVersionKey,
    confirmed_anchor: Option<FeedbackAnchor>,
) -> RestoreDecision {
    RestoreDecision {
        historical_key: key,
        choice: RestoreChoice::ContinueAsNew {
            feedback_id: FeedbackId::from_u128(30),
            text_revision_id: ReviewTextRevisionId::from_u128(30),
            target_id: ReviewTargetId::from_u128(31),
            target_revision_id: ReviewTargetRevisionId::from_u128(31),
            target_asset_version_id: AssetVersionId::from_u128(2),
            confirmed_anchor,
            created_at_ms: 30,
        },
    }
}

#[test]
fn restoring_an_old_key_does_not_implicitly_overwrite_a_new_one() {
    let old = key(&state(), 11);
    let new = TargetVersionKey {
        text_revision_id: ReviewTextRevisionId::from_u128(30),
        ..old
    };
    assert_eq!(classify_restore(None, old), RestoreDisposition::Restore);
    assert_eq!(
        classify_restore(Some(old), old),
        RestoreDisposition::AlreadyCurrent
    );
    assert_eq!(
        classify_restore(Some(new), old),
        RestoreDisposition::ConfirmConflict
    );
}

#[test]
fn explicit_restore_appends_current_state_and_reverses_only_selected_coverage() {
    let (before, current, archive) = archived();
    let frozen = archive.clone();
    let (next, plan) = apply_restore(
        &current,
        &archive,
        std::slice::from_ref(&before),
        &[restore(key(&before, 11))],
        ReviewSnapshotId::from_u128(5),
    )
    .unwrap();
    assert_eq!(
        next.target_key(ReviewTargetId::from_u128(11)),
        Some(key(&before, 11))
    );
    assert_eq!(
        next.target_key(ReviewTargetId::from_u128(12)),
        Some(key(&before, 12))
    );
    assert_eq!(
        plan.coverage_reversals,
        vec![ArchiveCoverage {
            archive_id: archive.archive_id,
            key: key(&before, 11),
            active: false
        }]
    );
    assert_eq!(
        plan.requires_source_check,
        vec![ReviewTargetId::from_u128(11)]
    );
    assert!(next.parent.is_none());
    assert_eq!(archive, frozen);
    assert!(current.target_key(ReviewTargetId::from_u128(11)).is_none());
}

#[test]
fn absent_or_previously_deleted_targets_are_not_restored_without_an_explicit_selection() {
    let (before, current, archive) = archived();
    let (next, plan) = apply_restore(
        &current,
        &archive,
        &[before],
        &[],
        ReviewSnapshotId::from_u128(5),
    )
    .unwrap();
    assert!(plan.restored.is_empty());
    assert!(plan.coverage_reversals.is_empty());
    assert_eq!(next, current);
}

#[test]
fn restore_is_idempotent_and_does_not_duplicate_feedback() {
    let (before, current, archive) = archived();
    let decision = restore(key(&before, 11));
    let (next, _) = apply_restore(
        &current,
        &archive,
        std::slice::from_ref(&before),
        std::slice::from_ref(&decision),
        ReviewSnapshotId::from_u128(5),
    )
    .unwrap();
    let (again, plan) = apply_restore(
        &next,
        &archive,
        &[before],
        &[decision],
        ReviewSnapshotId::from_u128(6),
    )
    .unwrap();
    assert_eq!(again, next);
    assert!(plan.restored.is_empty());
    assert!(plan.coverage_reversals.is_empty());
}

#[test]
fn a_shared_text_conflict_cannot_overwrite_an_unselected_current_target() {
    let (before, mut current, archive) = archived();
    current.feedback[0] = update_feedback_text(
        &current.feedback[0],
        ReviewTextRevisionId::from_u128(20),
        "图二的新意见，不可覆盖",
    )
    .unwrap();
    current.snapshot_id = ReviewSnapshotId::from_u128(5);
    let decisions = vec![restore(key(&before, 11))];
    let plan = plan_restore(
        &current,
        &archive,
        std::slice::from_ref(&before),
        &decisions,
    )
    .unwrap();
    assert_eq!(plan.conflicts, vec![key(&before, 11)]);
    assert_eq!(
        apply_restore(
            &current,
            &archive,
            &[before],
            &decisions,
            ReviewSnapshotId::from_u128(6)
        ),
        Err(ContinuousReviewError::NeedsConfirmation)
    );
    assert_eq!(current.feedback[0].text, "图二的新意见，不可覆盖");
}

#[test]
fn preserve_current_is_an_explicit_noop_not_a_whole_snapshot_rollback() {
    let (before, current, archive) = archived();
    let decisions = vec![RestoreDecision {
        historical_key: key(&before, 11),
        choice: RestoreChoice::PreserveCurrent,
    }];
    let (next, plan) = apply_restore(
        &current,
        &archive,
        &[before],
        &decisions,
        ReviewSnapshotId::from_u128(5),
    )
    .unwrap();
    assert_eq!(next, current);
    assert!(plan.coverage_reversals.is_empty());
}

#[test]
fn continuing_old_feedback_uses_new_identity_and_keeps_unconfirmed_geometry_pending() {
    let (before, current, archive) = archived();
    let decision = continue_as_new(key(&before, 11), None);
    let (next, plan) = apply_restore(
        &current,
        &archive,
        std::slice::from_ref(&before),
        &[decision],
        ReviewSnapshotId::from_u128(5),
    )
    .unwrap();
    assert_eq!(next.feedback[0], current.feedback[0]);
    let new = &next.feedback[1];
    assert_eq!(new.id, FeedbackId::from_u128(30));
    assert_eq!(new.text, "袖口收紧，保留褶皱");
    assert_eq!(new.created_at_ms, 30);
    assert_eq!(
        new.targets[0].availability,
        ReviewAvailability::NeedsConfirmation(vec![ReviewPendingReason::ApplicabilityUnconfirmed])
    );
    assert_eq!(
        new.history_ref,
        Some(HistoryRef {
            project_id: before.project_id,
            stream_id: before.stream_id,
            source: HistorySource::Snapshot {
                snapshot: archive.before,
                keys: vec![key(&before, 11)]
            }
        })
    );
    assert!(
        plan.coverage_reversals.is_empty(),
        "a new request does not unarchive the old request"
    );
}

#[test]
fn confirmed_new_geometry_is_used_but_source_verification_is_still_required() {
    let (before, current, archive) = archived();
    let decision = continue_as_new(key(&before, 11), Some(rect()));
    let (next, plan) = apply_restore(
        &current,
        &archive,
        std::slice::from_ref(&before),
        std::slice::from_ref(&decision),
        ReviewSnapshotId::from_u128(5),
    )
    .unwrap();
    assert_eq!(next.feedback[1].targets[0].anchor, rect());
    assert_eq!(
        plan.requires_source_check,
        vec![ReviewTargetId::from_u128(31)]
    );
    let (retry, retry_plan) = apply_restore(
        &next,
        &archive,
        &[before],
        &[decision],
        ReviewSnapshotId::from_u128(6),
    )
    .unwrap();
    assert_eq!(retry, next);
    assert!(retry_plan.restored.is_empty());
}

#[test]
fn unknown_duplicate_or_invalid_decisions_are_rejected() {
    let (before, current, archive) = archived();
    assert_eq!(
        plan_restore(
            &current,
            &archive,
            std::slice::from_ref(&before),
            &[restore(key(&before, 12))]
        ),
        Err(ContinuousReviewError::MissingReference)
    );
    let decision = restore(key(&before, 11));
    assert_eq!(
        plan_restore(
            &current,
            &archive,
            std::slice::from_ref(&before),
            &[decision.clone(), decision]
        ),
        Err(ContinuousReviewError::DuplicateIdentity)
    );
    let mut decision = continue_as_new(key(&before, 11), Some(rect()));
    if let RestoreChoice::ContinueAsNew { feedback_id, .. } = &mut decision.choice {
        *feedback_id = FeedbackId::from_u128(10);
    }
    assert_eq!(
        plan_restore(&current, &archive, &[before], &[decision]),
        Err(ContinuousReviewError::DuplicateIdentity)
    );
}

#[test]
fn history_must_be_present_and_the_checkpoint_result_must_be_consistent() {
    let (before, current, mut archive) = archived();
    assert_eq!(
        plan_restore(&current, &archive, &[], &[]),
        Err(ContinuousReviewError::MissingReference)
    );
    archive.removed.clear();
    assert_eq!(
        plan_restore(&current, &archive, &[before], &[]),
        Err(ContinuousReviewError::InvalidData)
    );
}

#[test]
fn unrelated_new_work_survives_a_selected_restore() {
    let (before, current, archive) = archived();
    let current = add_feedback(
        &current,
        feedback(40, &[(41, 2)]),
        ReviewSnapshotId::from_u128(5),
    )
    .unwrap();
    let (next, _) = apply_restore(
        &current,
        &archive,
        std::slice::from_ref(&before),
        &[restore(key(&before, 11))],
        ReviewSnapshotId::from_u128(6),
    )
    .unwrap();
    assert_eq!(next.feedback[1], current.feedback[1]);
}

#[test]
fn bulk_restore_shares_text_and_asset_payloads_up_to_the_target_limit() {
    use viewer_domain::review::{MAX_FEEDBACK_TEXT_BYTES, MAX_TARGETS_PER_FEEDBACK};
    for count in [2, MAX_TARGETS_PER_FEEDBACK] {
        let mut before = state();
        before.feedback = vec![feedback(
            10,
            &(0..count)
                .map(|i| (1000 + i as u128, 1))
                .collect::<Vec<_>>(),
        )];
        before.feedback[0].text = "a".repeat(MAX_FEEDBACK_TEXT_BYTES);
        let keys: Vec<_> = before.feedback[0]
            .targets
            .iter()
            .map(|target| TargetVersionKey {
                feedback_id: before.feedback[0].id,
                text_revision_id: before.feedback[0].text_revision_id,
                target_id: target.id,
                target_revision_id: target.revision_id,
            })
            .collect();
        let selection = ArchiveSelection {
            expected_snapshot_id: before.snapshot_id,
            groups: vec![ArchiveGroup {
                basis: ArchiveBasis::Unknown,
                targets: keys.clone(),
            }],
        };
        let (current, plan) = apply_archive(
            &before,
            &[],
            &selection,
            &[],
            ReviewSnapshotId::from_u128(4),
        )
        .unwrap();
        let archive = ArchiveCheckpoint::from_plan(
            &before,
            reference(&before),
            &plan,
            ReviewArchiveId::from_u128(1),
            20,
        )
        .unwrap();
        let decisions: Vec<_> = keys.into_iter().map(restore).collect();
        let plan = plan_restore(
            &current,
            &archive,
            std::slice::from_ref(&before),
            &decisions,
        )
        .unwrap();
        assert_eq!(plan.restored.len(), count);
        assert!(
            std::ptr::eq(
                plan.restored[0].feedback.text.as_ptr(),
                plan.restored[count - 1].feedback.text.as_ptr()
            ),
            "shared feedback text must have one allocation, not one per target"
        );
        assert!(
            std::ptr::eq(
                plan.restored[0].asset.relative_path.as_str().as_ptr(),
                plan.restored[count - 1]
                    .asset
                    .relative_path
                    .as_str()
                    .as_ptr()
            ),
            "shared asset metadata must have one allocation"
        );
        let (restored, _) = apply_restore(
            &current,
            &archive,
            std::slice::from_ref(&before),
            &decisions,
            ReviewSnapshotId::from_u128(5),
        )
        .unwrap();
        assert_eq!(restored.feedback.len(), 1);
        assert_eq!(restored.feedback[0], before.feedback[0]);
    }
}

#[test]
fn explicitly_selecting_all_affected_shared_targets_allows_restoring_old_text() {
    let before = state();
    let selection = ArchiveSelection {
        expected_snapshot_id: before.snapshot_id,
        groups: vec![ArchiveGroup {
            basis: ArchiveBasis::Unknown,
            targets: vec![key(&before, 11), key(&before, 12)],
        }],
    };
    let plan = plan_archive(&before, &[], &selection, &[]).unwrap();
    let archive = ArchiveCheckpoint::from_plan(
        &before,
        reference(&before),
        &plan,
        ReviewArchiveId::from_u128(1),
        20,
    )
    .unwrap();
    let mut current = before.clone();
    current.snapshot_id = ReviewSnapshotId::from_u128(5);
    current.feedback[0] = update_feedback_text(
        &before.feedback[0],
        ReviewTextRevisionId::from_u128(30),
        "后来的共同意见",
    )
    .unwrap();
    let decisions = vec![restore(key(&before, 11)), restore(key(&before, 12))];
    let (next, plan) = apply_restore(
        &current,
        &archive,
        std::slice::from_ref(&before),
        &decisions,
        ReviewSnapshotId::from_u128(6),
    )
    .unwrap();
    assert!(plan.conflicts.is_empty());
    assert_eq!(plan.coverage_reversals.len(), 2);
    assert_eq!(next.feedback, before.feedback);
    assert_eq!(current.feedback[0].text, "后来的共同意见");
}
