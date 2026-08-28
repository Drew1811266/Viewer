#[path = "support/continuous.rs"]
mod support;
use support::*;
use viewer_domain::{review::continuous::*, *};

#[test]
fn explicit_binding_revises_one_target_and_preserves_old_assets_and_text() {
    let current = state();
    let key = current.target_key(ReviewTargetId::from_u128(11)).unwrap();
    let decision = SourceBindingDecision {
        target_key: key,
        new_asset_version_id: AssetVersionId::from_u128(2),
        anchor: rect(),
        confirmation: SourceBindingConfirmation::UserConfirmed,
    };
    let next = apply_source_binding(
        &current,
        &decision,
        ReviewTargetRevisionId::from_u128(90),
        ReviewSnapshotId::from_u128(91),
    )
    .unwrap();
    assert_eq!(next.assets, current.assets);
    assert_eq!(next.feedback[0].text, current.feedback[0].text);
    assert_eq!(
        next.feedback[0].targets[0].asset_version_id,
        AssetVersionId::from_u128(2)
    );
    assert_eq!(
        next.feedback[0].targets[0].revision_id,
        ReviewTargetRevisionId::from_u128(90)
    );
    assert_eq!(next.feedback[0].targets[1], current.feedback[0].targets[1]);
    let mut stale = decision.clone();
    stale.target_key.text_revision_id = ReviewTextRevisionId::from_u128(999);
    assert_eq!(
        apply_source_binding(
            &current,
            &stale,
            ReviewTargetRevisionId::from_u128(90),
            ReviewSnapshotId::from_u128(91)
        ),
        Err(ContinuousReviewError::StaleSnapshot)
    );
    assert_eq!(
        apply_source_binding(
            &current,
            &decision,
            key.target_revision_id,
            ReviewSnapshotId::from_u128(91)
        ),
        Err(ContinuousReviewError::DuplicateIdentity)
    );
}

#[test]
fn applicability_confirmation_cannot_clear_evidence_or_source_restrictions() {
    let mut current = state();
    let key = key(&current, 11);
    for reasons in [
        vec![ReviewPendingReason::LegacyEvidenceAbsent],
        vec![
            ReviewPendingReason::ApplicabilityUnconfirmed,
            ReviewPendingReason::LegacyEvidenceAbsent,
        ],
    ] {
        current.feedback[0].targets[0].availability =
            ReviewAvailability::NeedsConfirmation(reasons);
        assert_eq!(
            confirm_applicability(
                &current,
                key,
                rect(),
                ReviewTargetRevisionId::new(),
                ReviewSnapshotId::new()
            ),
            Err(ContinuousReviewError::NeedsConfirmation)
        );
    }
    current.feedback[0].targets[0].availability =
        ReviewAvailability::NeedsConfirmation(vec![ReviewPendingReason::ApplicabilityUnconfirmed]);
    let mut stale = key;
    stale.target_revision_id = ReviewTargetRevisionId::new();
    assert_eq!(
        confirm_applicability(
            &current,
            stale,
            rect(),
            ReviewTargetRevisionId::new(),
            ReviewSnapshotId::new()
        ),
        Err(ContinuousReviewError::StaleSnapshot)
    );
    let next = confirm_applicability(
        &current,
        key,
        rect(),
        ReviewTargetRevisionId::new(),
        ReviewSnapshotId::new(),
    )
    .unwrap();
    assert_eq!(
        next.feedback[0].targets[0].asset_version_id,
        current.feedback[0].targets[0].asset_version_id
    );
    let projection = project_current(
        &next,
        &[SourceCheck {
            asset_version_id: next.assets[0].id,
            checked_at_ms: 100,
            status: SourceCheckStatus::Changed,
        }],
    )
    .unwrap();
    assert!(projection.actionable.is_empty());
    assert!(projection.needs_confirmation.contains(&key.target_id));
}
