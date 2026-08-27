#[path = "support/continuous.rs"]
mod support;
use support::*;
use viewer_domain::review::continuous::*;
use viewer_domain::*;

fn check(id: u128, status: SourceCheckStatus) -> SourceCheck {
    SourceCheck {
        asset_version_id: AssetVersionId::from_u128(id),
        checked_at_ms: 30,
        status,
    }
}
fn change(
    before: Option<TargetVersionKey>,
    after: Option<TargetVersionKey>,
    kind: ReviewChangeKind,
) -> ReviewChange {
    ReviewChange {
        target_id: before.or(after).unwrap().target_id,
        before,
        after,
        kind,
        archive_id: None,
        historical_key: None,
    }
}

#[test]
fn reading_changed_source_only_changes_projection_not_saved_state_or_delta() {
    let state = state();
    let saved = state.clone();
    let projected = project_current(
        &state,
        &[
            check(1, SourceCheckStatus::Changed),
            check(2, SourceCheckStatus::Match),
        ],
    )
    .unwrap();
    assert_eq!(projected.actionable, vec![ReviewTargetId::from_u128(12)]);
    assert_eq!(
        projected.needs_confirmation,
        vec![ReviewTargetId::from_u128(11)]
    );
    assert_eq!(state, saved);
    assert!(diff_review(&saved, &state, &[]).unwrap().targets.is_empty());
}

#[test]
fn unknown_missing_or_unreadable_source_is_never_actionable() {
    let state = state();
    for status in [
        SourceCheckStatus::Changed,
        SourceCheckStatus::Missing,
        SourceCheckStatus::Unreadable,
        SourceCheckStatus::Unverified,
    ] {
        let projected = project_current(&state, &[check(1, status)]).unwrap();
        assert!(projected.actionable.is_empty());
        assert_eq!(
            projected.needs_confirmation,
            vec![ReviewTargetId::from_u128(11), ReviewTargetId::from_u128(12)]
        );
    }
    assert_eq!(
        project_current(&state, &[])
            .unwrap()
            .needs_confirmation
            .len(),
        2
    );
}

#[test]
fn matching_bytes_do_not_override_a_saved_applicability_confirmation_requirement() {
    let mut state = state();
    state.feedback[0].targets[0].availability =
        ReviewAvailability::NeedsConfirmation(vec![ReviewPendingReason::ApplicabilityUnconfirmed]);
    let projected = project_current(
        &state,
        &[
            check(1, SourceCheckStatus::Match),
            check(2, SourceCheckStatus::Match),
        ],
    )
    .unwrap();
    assert_eq!(projected.actionable, vec![ReviewTargetId::from_u128(12)]);
    assert_eq!(
        projected.needs_confirmation,
        vec![ReviewTargetId::from_u128(11)]
    );
}

#[test]
fn invalid_source_checks_fail_instead_of_last_write_wins() {
    let state = state();
    assert_eq!(
        project_current(
            &state,
            &[
                check(1, SourceCheckStatus::Match),
                check(1, SourceCheckStatus::Changed)
            ]
        ),
        Err(ContinuousReviewError::DuplicateIdentity)
    );
    assert_eq!(
        project_current(&state, &[check(99, SourceCheckStatus::Match)]),
        Err(ContinuousReviewError::MissingReference)
    );
    let mut invalid = check(1, SourceCheckStatus::Match);
    invalid.checked_at_ms = -1;
    assert_eq!(
        project_current(&state, &[invalid]),
        Err(ContinuousReviewError::InvalidData)
    );
}

#[test]
fn undo_does_not_make_an_overwritten_source_executable() {
    let (before, current, archive) = archived();
    let decisions = vec![RestoreDecision {
        historical_key: key(&before, 11),
        choice: RestoreChoice::UseHistorical,
    }];
    let (restored, _) = apply_restore(
        &current,
        &archive,
        &[before],
        &decisions,
        ReviewSnapshotId::from_u128(5),
    )
    .unwrap();
    let projected = project_current(
        &restored,
        &[
            check(1, SourceCheckStatus::Changed),
            check(2, SourceCheckStatus::Match),
        ],
    )
    .unwrap();
    assert_eq!(projected.actionable, vec![ReviewTargetId::from_u128(12)]);
    assert_eq!(
        projected.needs_confirmation,
        vec![ReviewTargetId::from_u128(11)]
    );
}

#[test]
fn text_edit_then_revert_has_no_net_task_even_with_new_revision_ids() {
    let before = state();
    let mut middle = before.clone();
    middle.snapshot_id = ReviewSnapshotId::from_u128(4);
    middle.feedback[0] = update_feedback_text(
        &before.feedback[0],
        ReviewTextRevisionId::from_u128(30),
        "临时改成另一要求",
    )
    .unwrap();
    let mut after = middle.clone();
    after.snapshot_id = ReviewSnapshotId::from_u128(5);
    after.feedback[0] = update_feedback_text(
        &middle.feedback[0],
        ReviewTextRevisionId::from_u128(31),
        &before.feedback[0].text,
    )
    .unwrap();
    let changes = vec![
        change(
            Some(key(&before, 11)),
            Some(key(&middle, 11)),
            ReviewChangeKind::Edited,
        ),
        change(
            Some(key(&middle, 11)),
            Some(key(&after, 11)),
            ReviewChangeKind::Edited,
        ),
    ];
    assert!(
        diff_review(&before, &after, &changes)
            .unwrap()
            .targets
            .is_empty()
    );
}

#[test]
fn added_then_withdrawn_has_no_net_task_or_reverse_instruction() {
    let before = state();
    let middle = add_feedback(
        &before,
        feedback(20, &[(21, 1)]),
        ReviewSnapshotId::from_u128(4),
    )
    .unwrap();
    let after = withdraw_targets(
        &middle,
        &[ReviewTargetId::from_u128(21)],
        ReviewSnapshotId::from_u128(5),
    )
    .unwrap();
    let added_key = key(&middle, 21);
    let changes = vec![
        change(None, Some(added_key), ReviewChangeKind::Added),
        change(Some(added_key), None, ReviewChangeKind::Withdrawn),
    ];
    assert!(
        diff_review(&before, &after, &changes)
            .unwrap()
            .targets
            .is_empty()
    );
}

#[test]
fn one_target_can_report_text_geometry_binding_and_availability_changes_together() {
    let before = state();
    let mut after = before.clone();
    after.snapshot_id = ReviewSnapshotId::from_u128(4);
    after.feedback[0] = update_feedback_text(
        &before.feedback[0],
        ReviewTextRevisionId::from_u128(30),
        "保留布料质感",
    )
    .unwrap();
    after.feedback[0] = replace_target(
        &after.feedback[0],
        ReviewTargetId::from_u128(11),
        ReviewTargetRevisionId::from_u128(30),
        rect(),
    )
    .unwrap();
    after.feedback[0].targets[0].asset_version_id = AssetVersionId::from_u128(2);
    after.feedback[0].targets[0].availability =
        ReviewAvailability::NeedsConfirmation(vec![ReviewPendingReason::ApplicabilityUnconfirmed]);
    let delta = diff_review(&before, &after, &[]).unwrap();
    assert_eq!(delta.since_snapshot_id, before.snapshot_id);
    assert_eq!(delta.current_snapshot_id, after.snapshot_id);
    assert_eq!(
        delta.targets.len(),
        2,
        "a shared text edit affects both current targets"
    );
    assert!(
        delta.targets[0].text_changed
            && delta.targets[0].anchor_changed
            && delta.targets[0].binding_changed
            && delta.targets[0].availability_changed
    );
    assert!(delta.targets[1].text_changed);
    assert!(
        !delta.targets[1].anchor_changed
            && !delta.targets[1].binding_changed
            && !delta.targets[1].availability_changed
    );
}

#[test]
fn redraw_only_is_not_misreported_as_a_text_edit() {
    let before = state();
    let mut after = before.clone();
    after.snapshot_id = ReviewSnapshotId::from_u128(4);
    after.feedback[0] = replace_target(
        &after.feedback[0],
        ReviewTargetId::from_u128(11),
        ReviewTargetRevisionId::from_u128(30),
        rect(),
    )
    .unwrap();
    let delta = diff_review(&before, &after, &[]).unwrap();
    assert_eq!(delta.targets.len(), 1);
    assert!(delta.targets[0].anchor_changed);
    assert!(!delta.targets[0].text_changed);
}

#[test]
fn partial_archive_and_withdrawal_have_distinct_removal_reasons() {
    let (before, archived, archive) = archived();
    let archived_change = ReviewChange {
        target_id: ReviewTargetId::from_u128(11),
        before: Some(key(&before, 11)),
        after: None,
        kind: ReviewChangeKind::Archived,
        archive_id: Some(archive.archive_id),
        historical_key: Some(key(&before, 11)),
    };
    let delta = diff_review(&before, &archived, &[archived_change]).unwrap();
    assert_eq!(delta.targets.len(), 1);
    assert_eq!(
        delta.targets[0].removal_reason,
        Some(ReviewChangeKind::Archived)
    );
    assert!(delta.targets[0].after.is_none());
    let withdrawn = withdraw_targets(
        &before,
        &[ReviewTargetId::from_u128(11)],
        ReviewSnapshotId::from_u128(5),
    )
    .unwrap();
    let delta = diff_review(
        &before,
        &withdrawn,
        &[change(
            Some(key(&before, 11)),
            None,
            ReviewChangeKind::Withdrawn,
        )],
    )
    .unwrap();
    assert_eq!(
        delta.targets[0].removal_reason,
        Some(ReviewChangeKind::Withdrawn)
    );
    assert!(
        !delta.targets[0].text_changed,
        "withdrawal is not reverse editing text"
    );
}

#[test]
fn removals_need_a_connected_typed_transition_not_an_unrelated_last_event() {
    let before = state();
    let after = withdraw_targets(
        &before,
        &[ReviewTargetId::from_u128(11)],
        ReviewSnapshotId::from_u128(4),
    )
    .unwrap();
    assert_eq!(
        diff_review(&before, &after, &[]),
        Err(ContinuousReviewError::MissingReference)
    );
    let wrong = TargetVersionKey {
        text_revision_id: ReviewTextRevisionId::from_u128(99),
        ..key(&before, 11)
    };
    assert_eq!(
        diff_review(
            &before,
            &after,
            &[change(Some(wrong), None, ReviewChangeKind::Withdrawn)]
        ),
        Err(ContinuousReviewError::SelectionConflict)
    );
    assert_eq!(
        diff_review(
            &before,
            &after,
            &[change(
                Some(key(&before, 11)),
                Some(wrong),
                ReviewChangeKind::Edited
            )]
        ),
        Err(ContinuousReviewError::SelectionConflict)
    );
}

#[test]
fn archiving_an_already_absent_old_key_does_not_replace_the_withdrawal_reason() {
    let before = state();
    let after = withdraw_targets(
        &before,
        &[ReviewTargetId::from_u128(11)],
        ReviewSnapshotId::from_u128(4),
    )
    .unwrap();
    let changes = vec![
        change(Some(key(&before, 11)), None, ReviewChangeKind::Withdrawn),
        ReviewChange {
            target_id: ReviewTargetId::from_u128(11),
            before: None,
            after: None,
            kind: ReviewChangeKind::Archived,
            archive_id: Some(ReviewArchiveId::from_u128(1)),
            historical_key: Some(key(&before, 11)),
        },
    ];
    assert_eq!(
        diff_review(&before, &after, &changes).unwrap().targets[0].removal_reason,
        Some(ReviewChangeKind::Withdrawn)
    );
}

#[test]
fn pending_reason_order_is_not_a_semantic_change() {
    let mut before = state();
    before.feedback[0].targets[0].availability = ReviewAvailability::NeedsConfirmation(vec![
        ReviewPendingReason::SourceChanged,
        ReviewPendingReason::ApplicabilityUnconfirmed,
    ]);
    let mut after = before.clone();
    after.snapshot_id = ReviewSnapshotId::from_u128(4);
    after.feedback[0].targets[0].revision_id = ReviewTargetRevisionId::from_u128(30);
    after.feedback[0].targets[0].availability = ReviewAvailability::NeedsConfirmation(vec![
        ReviewPendingReason::ApplicabilityUnconfirmed,
        ReviewPendingReason::SourceChanged,
    ]);
    assert!(
        diff_review(&before, &after, &[])
            .unwrap()
            .targets
            .is_empty()
    );
}

#[test]
fn identities_cannot_hide_changes_under_an_old_snapshot_or_revision() {
    let before = state();
    let mut after = before.clone();
    after.feedback[0].text = "伪造版本内容".into();
    assert_eq!(
        diff_review(&before, &after, &[]),
        Err(ContinuousReviewError::DuplicateIdentity)
    );
    after.snapshot_id = ReviewSnapshotId::from_u128(4);
    assert_eq!(
        diff_review(&before, &after, &[]),
        Err(ContinuousReviewError::DuplicateIdentity)
    );
    after = before.clone();
    after.stream_id = ReviewStreamId::from_u128(99);
    assert_eq!(
        diff_review(&before, &after, &[]),
        Err(ContinuousReviewError::InvalidData)
    );
}

#[test]
fn newly_added_targets_are_reported_in_stable_current_order() {
    let before = state();
    let after = add_feedback(
        &before,
        feedback(20, &[(23, 1), (21, 2)]),
        ReviewSnapshotId::from_u128(4),
    )
    .unwrap();
    let delta = diff_review(&before, &after, &[]).unwrap();
    assert_eq!(
        delta
            .targets
            .iter()
            .map(|target| target.target_id)
            .collect::<Vec<_>>(),
        vec![ReviewTargetId::from_u128(23), ReviewTargetId::from_u128(21)]
    );
    assert!(
        delta
            .targets
            .iter()
            .all(|target| target.before.is_none() && target.after.is_some())
    );
}
