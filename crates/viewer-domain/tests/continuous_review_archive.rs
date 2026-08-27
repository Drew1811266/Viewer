#[path = "support/continuous.rs"]
mod support;
use support::*;
use viewer_domain::review::continuous::*;
use viewer_domain::*;

fn selection(
    current: &ContinuousReviewState,
    basis: &ContinuousReviewState,
    ids: &[u128],
) -> ArchiveSelection {
    ArchiveSelection {
        expected_snapshot_id: current.snapshot_id,
        groups: vec![ArchiveGroup {
            basis: ArchiveBasis::Known {
                snapshot: reference(basis),
                source: ArchiveBasisSource::UserSelected,
            },
            targets: ids.iter().map(|&id| key(basis, id)).collect(),
        }],
    }
}

#[test]
fn exact_version_comparison_protects_later_edits() {
    let b = state();
    let old = key(&b, 11);
    let new = TargetVersionKey {
        text_revision_id: ReviewTextRevisionId::from_u128(30),
        ..old
    };
    assert_eq!(
        classify_archive(Some(old), old),
        ArchiveDisposition::RemoveCurrent
    );
    assert_eq!(
        classify_archive(Some(new), old),
        ArchiveDisposition::RetainLaterEdit
    );
    assert_eq!(
        classify_archive(None, old),
        ArchiveDisposition::AlreadyAbsent
    );
}

#[test]
fn archive_records_the_old_basis_but_keeps_later_shared_text_and_new_feedback() {
    let b = state();
    let mut c = add_feedback(&b, feedback(20, &[(21, 1)]), ReviewSnapshotId::from_u128(4)).unwrap();
    c.feedback[0] = update_feedback_text(
        &c.feedback[0],
        ReviewTextRevisionId::from_u128(30),
        "保留褶皱，袖口再收紧一点",
    )
    .unwrap();
    let (next, plan) = apply_archive(
        &c,
        std::slice::from_ref(&b),
        &selection(&c, &b, &[11, 12]),
        &[],
        ReviewSnapshotId::from_u128(5),
    )
    .unwrap();
    assert!(plan.removed.is_empty());
    assert_eq!(plan.retained.len(), 2);
    assert_eq!(plan.groups[0].targets, vec![key(&b, 11), key(&b, 12)]);
    assert_eq!(plan.retained[0].basis, key(&b, 11));
    assert_eq!(plan.retained[0].current, Some(key(&c, 11)));
    assert_eq!(next.feedback, c.feedback);
    assert_eq!(next.snapshot_id, ReviewSnapshotId::from_u128(5));
    assert_eq!(b.feedback[0].text, "袖口收紧，保留褶皱");
}

#[test]
fn partial_archive_removes_only_the_selected_target_of_shared_feedback() {
    let b = state();
    let (next, plan) = apply_archive(
        &b,
        std::slice::from_ref(&b),
        &selection(&b, &b, &[11]),
        &[],
        ReviewSnapshotId::from_u128(4),
    )
    .unwrap();
    assert_eq!(plan.removed, vec![key(&b, 11)]);
    assert_eq!(next.feedback[0].targets.len(), 1);
    assert_eq!(
        next.target_key(ReviewTargetId::from_u128(12)),
        Some(key(&b, 12))
    );
    assert_eq!(
        next.feedback[0].text_revision_id,
        b.feedback[0].text_revision_id
    );
}

#[test]
fn redraw_and_withdrawal_are_not_lost_or_resurrected_by_archiving_old_basis() {
    let b = state();
    let mut c = withdraw_targets(
        &b,
        &[ReviewTargetId::from_u128(12)],
        ReviewSnapshotId::from_u128(4),
    )
    .unwrap();
    c.feedback[0] = replace_target(
        &c.feedback[0],
        ReviewTargetId::from_u128(11),
        ReviewTargetRevisionId::from_u128(30),
        rect(),
    )
    .unwrap();
    let (next, plan) = apply_archive(
        &c,
        std::slice::from_ref(&b),
        &selection(&c, &b, &[11, 12]),
        &[],
        ReviewSnapshotId::from_u128(5),
    )
    .unwrap();
    assert_eq!(next.feedback, c.feedback);
    assert_eq!(
        plan.retained[0].disposition,
        ArchiveDisposition::RetainLaterEdit
    );
    assert_eq!(
        plan.retained[1].disposition,
        ArchiveDisposition::AlreadyAbsent
    );
    assert!(plan.retained[1].current.is_none());
}

#[test]
fn unknown_usage_archives_only_the_confirmed_current_keys_with_real_evidence_reference() {
    let c = state();
    let selection = ArchiveSelection {
        expected_snapshot_id: c.snapshot_id,
        groups: vec![ArchiveGroup {
            basis: ArchiveBasis::Unknown,
            targets: vec![key(&c, 11)],
        }],
    };
    let plan = plan_archive(&c, &[], &selection, &[]).unwrap();
    let checkpoint =
        ArchiveCheckpoint::from_plan(&c, reference(&c), &plan, ReviewArchiveId::from_u128(1), 100)
            .unwrap();
    assert_eq!(checkpoint.before, reference(&c));
    assert_eq!(checkpoint.groups[0].basis, ArchiveBasis::Unknown);
    let mut stale = selection;
    stale.groups[0].targets[0].text_revision_id = ReviewTextRevisionId::from_u128(99);
    assert_eq!(
        plan_archive(&c, &[], &stale, &[]),
        Err(ContinuousReviewError::SelectionConflict)
    );
}

#[test]
fn duplicate_active_coverage_is_a_noop_but_reversed_coverage_can_be_archived_again() {
    let b = state();
    let selected = selection(&b, &b, &[11]);
    let mut coverage = vec![ArchiveCoverage {
        archive_id: ReviewArchiveId::from_u128(1),
        key: key(&b, 11),
        active: true,
    }];
    let (next, plan) = apply_archive(
        &b,
        std::slice::from_ref(&b),
        &selected,
        &coverage,
        ReviewSnapshotId::from_u128(4),
    )
    .unwrap();
    assert!(plan.is_noop());
    assert!(plan.groups.is_empty());
    assert_eq!(plan.already_covered, vec![key(&b, 11)]);
    assert_eq!(next, b);
    coverage[0].active = false;
    assert_eq!(
        plan_archive(&b, std::slice::from_ref(&b), &selected, &coverage)
            .unwrap()
            .removed,
        vec![key(&b, 11)]
    );
}

#[test]
fn empty_archive_does_not_create_a_checkpoint() {
    let c = state();
    let empty = ArchiveSelection {
        expected_snapshot_id: c.snapshot_id,
        groups: vec![],
    };
    let plan = plan_archive(&c, &[], &empty, &[]).unwrap();
    assert!(plan.is_noop());
    assert_eq!(
        ArchiveCheckpoint::from_plan(&c, reference(&c), &plan, ReviewArchiveId::from_u128(1), 100),
        Err(ContinuousReviewError::InvalidData)
    );
}

#[test]
fn stale_previews_missing_bases_and_forged_keys_are_rejected() {
    let c = state();
    let mut selected = selection(&c, &c, &[11]);
    selected.expected_snapshot_id = ReviewSnapshotId::from_u128(2);
    assert_eq!(
        plan_archive(&c, std::slice::from_ref(&c), &selected, &[]),
        Err(ContinuousReviewError::StaleSnapshot)
    );
    selected.expected_snapshot_id = c.snapshot_id;
    assert_eq!(
        plan_archive(&c, &[], &selected, &[]),
        Err(ContinuousReviewError::MissingReference)
    );
    selected.groups[0].targets[0].target_revision_id = ReviewTargetRevisionId::from_u128(99);
    assert_eq!(
        plan_archive(&c, std::slice::from_ref(&c), &selected, &[]),
        Err(ContinuousReviewError::MissingReference)
    );
}

#[test]
fn competing_bases_for_one_target_require_user_resolution_even_if_keys_match() {
    let c = state();
    let mut selected = selection(&c, &c, &[11]);
    selected.groups.push(ArchiveGroup {
        basis: ArchiveBasis::Unknown,
        targets: vec![key(&c, 11)],
    });
    assert_eq!(
        plan_archive(&c, std::slice::from_ref(&c), &selected, &[]),
        Err(ContinuousReviewError::SelectionConflict)
    );
}

#[test]
fn cross_context_and_reused_immutable_identities_cannot_be_hidden_in_a_basis() {
    let c = state();
    let mut b = c.clone();
    b.project_id = ProjectId::from_u128(99);
    assert_eq!(
        plan_archive(&c, &[b], &selection(&c, &c, &[11]), &[]),
        Err(ContinuousReviewError::InvalidData)
    );
    let mut b = c.clone();
    b.snapshot_id = ReviewSnapshotId::from_u128(2);
    b.feedback[0].text = "同一版本 ID 下伪造的旧文字".into();
    assert_eq!(
        plan_archive(&c, std::slice::from_ref(&b), &selection(&c, &b, &[11]), &[]),
        Err(ContinuousReviewError::DuplicateIdentity)
    );
}

#[test]
fn inconsistent_hash_references_and_duplicate_coverage_are_invalid() {
    let c = state();
    let mut selected = selection(&c, &c, &[11]);
    selected.groups.push(ArchiveGroup {
        basis: ArchiveBasis::Known {
            snapshot: SnapshotRef {
                blake3: [9; 32],
                ..reference(&c)
            },
            source: ArchiveBasisSource::UserSelected,
        },
        targets: vec![key(&c, 12)],
    });
    assert_eq!(
        plan_archive(&c, std::slice::from_ref(&c), &selected, &[]),
        Err(ContinuousReviewError::InvalidData)
    );
    let coverage = ArchiveCoverage {
        archive_id: ReviewArchiveId::from_u128(1),
        key: key(&c, 11),
        active: true,
    };
    assert_eq!(
        plan_archive(
            &c,
            std::slice::from_ref(&c),
            &selection(&c, &c, &[11]),
            &[coverage.clone(), coverage]
        ),
        Err(ContinuousReviewError::DuplicateIdentity)
    );
}

#[test]
fn an_archive_that_only_records_old_history_still_validates_the_resulting_snapshot() {
    let mut b = state();
    b.feedback[0].history_ref = Some(HistoryRef {
        project_id: b.project_id,
        stream_id: b.stream_id,
        source: HistorySource::Snapshot {
            snapshot: SnapshotRef {
                snapshot_id: ReviewSnapshotId::from_u128(10),
                blake3: [4; 32],
            },
            keys: vec![key(&b, 11)],
        },
    });
    let mut c = b.clone();
    c.snapshot_id = ReviewSnapshotId::from_u128(4);
    c.feedback[0] = update_feedback_text(
        &c.feedback[0],
        ReviewTextRevisionId::from_u128(30),
        "后补内容",
    )
    .unwrap();
    assert_eq!(
        apply_archive(
            &c,
            std::slice::from_ref(&b),
            &selection(&c, &b, &[11]),
            &[],
            ReviewSnapshotId::from_u128(10)
        ),
        Err(ContinuousReviewError::InvalidData)
    );
}

#[test]
fn a_manually_constructed_unknown_basis_cannot_claim_noncurrent_history() {
    let c = state();
    let wrong = TargetVersionKey {
        text_revision_id: ReviewTextRevisionId::from_u128(99),
        ..key(&c, 11)
    };
    let forged = ArchivePlan {
        expected_snapshot_id: c.snapshot_id,
        groups: vec![ArchiveGroup {
            basis: ArchiveBasis::Unknown,
            targets: vec![wrong],
        }],
        removed: vec![],
        retained: vec![ArchiveRetention {
            basis: wrong,
            current: Some(key(&c, 11)),
            disposition: ArchiveDisposition::RetainLaterEdit,
        }],
        already_covered: vec![],
    };
    assert_eq!(
        ArchiveCheckpoint::from_plan(
            &c,
            reference(&c),
            &forged,
            ReviewArchiveId::from_u128(1),
            30
        ),
        Err(ContinuousReviewError::SelectionConflict)
    );
}
