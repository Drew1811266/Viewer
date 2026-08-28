use std::collections::{HashMap, HashSet};

use super::{ContinuousReviewError, ContinuousReviewState, ReviewAvailability};
use crate::review::MAX_ASSETS_PER_ROUND;
use crate::{AssetVersionId, ReviewTargetId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceBindingConfirmation {
    UserConfirmed,
    /// Application must verify the producer declaration and receive the user's
    /// position confirmation before constructing this pure decision.
    ProducerVerifiedAndPositionConfirmed {
        usage_id: crate::ReviewUsageId,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub struct SourceBindingDecision {
    pub target_key: super::TargetVersionKey,
    pub new_asset_version_id: AssetVersionId,
    pub anchor: crate::review::FeedbackAnchor,
    pub confirmation: SourceBindingConfirmation,
}

pub fn apply_source_binding(
    current: &ContinuousReviewState,
    decision: &SourceBindingDecision,
    revision: crate::ReviewTargetRevisionId,
    snapshot: crate::ReviewSnapshotId,
) -> Result<ContinuousReviewState, ContinuousReviewError> {
    use ContinuousReviewError::*;
    current.validate()?;
    if current.target_key(decision.target_key.target_id) != Some(decision.target_key) {
        return Err(StaleSnapshot);
    }
    if current
        .feedback
        .iter()
        .flat_map(|f| &f.targets)
        .any(|t| t.revision_id == revision)
    {
        return Err(DuplicateIdentity);
    }
    if !current
        .assets
        .iter()
        .any(|a| a.id == decision.new_asset_version_id)
    {
        return Err(MissingReference);
    }
    let mut next = super::mutation::prepare_next(current, snapshot)?;
    let target = next
        .feedback
        .iter_mut()
        .flat_map(|f| &mut f.targets)
        .find(|t| t.id == decision.target_key.target_id)
        .ok_or(MissingReference)?;
    if target.asset_version_id == decision.new_asset_version_id {
        return Err(InvalidData);
    }
    target.asset_version_id = decision.new_asset_version_id;
    target.revision_id = revision;
    target.anchor = decision.anchor.clone();
    target.availability = ReviewAvailability::Ready;
    next.validate()?;
    Ok(next)
}

/// Confirms a provisional anchor on the same asset, without claiming source replacement or
/// clearing missing evidence/source restrictions. Fresh read-time source checks still apply.
pub fn confirm_applicability(
    current: &ContinuousReviewState,
    key: super::TargetVersionKey,
    anchor: crate::review::FeedbackAnchor,
    revision: crate::ReviewTargetRevisionId,
    snapshot: crate::ReviewSnapshotId,
) -> Result<ContinuousReviewState, ContinuousReviewError> {
    use ContinuousReviewError::*;
    current.validate()?;
    if current.target_key(key.target_id) != Some(key) {
        return Err(StaleSnapshot);
    }
    if current
        .feedback
        .iter()
        .flat_map(|f| &f.targets)
        .any(|t| t.revision_id == revision)
    {
        return Err(DuplicateIdentity);
    }
    let mut next = super::mutation::prepare_next(current, snapshot)?;
    let target = next
        .feedback
        .iter_mut()
        .flat_map(|f| &mut f.targets)
        .find(|t| t.id == key.target_id)
        .ok_or(MissingReference)?;
    if target.availability
        != ReviewAvailability::NeedsConfirmation(vec![
            super::ReviewPendingReason::ApplicabilityUnconfirmed,
        ])
    {
        return Err(NeedsConfirmation);
    }
    target.anchor = anchor;
    target.revision_id = revision;
    target.availability = ReviewAvailability::Ready;
    next.validate()?;
    Ok(next)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceCheckStatus {
    Match,
    Changed,
    Missing,
    Unreadable,
    Unverified,
}

/// Ephemeral evidence from a fresh, safe source check performed by the application/reader.
/// It is not a saved state transition or a claim that a producer executed the feedback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCheck {
    pub asset_version_id: AssetVersionId,
    pub checked_at_ms: i64,
    pub status: SourceCheckStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentReviewProjection {
    pub actionable: Vec<ReviewTargetId>,
    pub needs_confirmation: Vec<ReviewTargetId>,
}

pub fn project_current(
    state: &ContinuousReviewState,
    checks: &[SourceCheck],
) -> Result<CurrentReviewProjection, ContinuousReviewError> {
    use ContinuousReviewError::*;
    state.validate()?;
    if checks.len() > MAX_ASSETS_PER_ROUND {
        return Err(LimitExceeded);
    }
    let assets: HashSet<_> = state.assets.iter().map(|asset| asset.id).collect();
    let mut by_asset = HashMap::new();
    for check in checks {
        if check.checked_at_ms < 0 {
            return Err(InvalidData);
        }
        if !assets.contains(&check.asset_version_id) {
            return Err(MissingReference);
        }
        if by_asset
            .insert(check.asset_version_id, check.status)
            .is_some()
        {
            return Err(DuplicateIdentity);
        }
    }
    let mut result = CurrentReviewProjection {
        actionable: vec![],
        needs_confirmation: vec![],
    };
    for target in state.feedback.iter().flat_map(|feedback| &feedback.targets) {
        if matches!(target.availability, ReviewAvailability::Ready)
            && by_asset.get(&target.asset_version_id) == Some(&SourceCheckStatus::Match)
        {
            result.actionable.push(target.id);
        } else {
            result.needs_confirmation.push(target.id);
        }
    }
    Ok(result)
}
