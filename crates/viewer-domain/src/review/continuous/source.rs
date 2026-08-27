use std::collections::{HashMap, HashSet};

use super::{ContinuousReviewError, ContinuousReviewState, ReviewAvailability};
use crate::review::MAX_ASSETS_PER_ROUND;
use crate::{AssetVersionId, ReviewTargetId};

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
