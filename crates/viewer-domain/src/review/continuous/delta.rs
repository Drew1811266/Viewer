use std::collections::HashMap;

use super::validation::{TargetIndex, ValidatedStates};
use super::{
    ContinuousReviewError, ContinuousReviewState, ReviewAvailability, ReviewChange,
    ReviewChangeKind, TargetVersionKey,
};
use crate::review::{MAX_FEEDBACK_ITEMS_PER_ROUND, MAX_TARGETS_PER_FEEDBACK};
use crate::{FeedbackId, ReviewSnapshotId, ReviewTargetId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewDelta {
    pub since_snapshot_id: ReviewSnapshotId,
    pub current_snapshot_id: ReviewSnapshotId,
    pub targets: Vec<TargetDelta>,
}

/// Net facts only. Old prose belongs to an explicit historical read, never this delta.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetDelta {
    pub target_id: ReviewTargetId,
    pub before: Option<TargetVersionKey>,
    pub after: Option<TargetVersionKey>,
    pub text_changed: bool,
    pub anchor_changed: bool,
    pub binding_changed: bool,
    pub availability_changed: bool,
    pub removal_reason: Option<ReviewChangeKind>,
}

pub fn diff_review(
    before: &ContinuousReviewState,
    after: &ContinuousReviewState,
    changes: &[ReviewChange],
) -> Result<ReviewDelta, ContinuousReviewError> {
    let states = ValidatedStates::new(after, std::slice::from_ref(before))?;
    let old = &states.targets[&before.snapshot_id];
    let new = &states.targets[&after.snapshot_id];
    let reasons = validate_transitions(old, new, changes)?;
    let ordered_ids = before
        .feedback
        .iter()
        .flat_map(|feedback| &feedback.targets)
        .map(|target| target.id)
        .chain(
            after
                .feedback
                .iter()
                .flat_map(|feedback| &feedback.targets)
                .map(|target| target.id)
                .filter(|id| !old.contains_key(id)),
        );
    let mut result = ReviewDelta {
        since_snapshot_id: before.snapshot_id,
        current_snapshot_id: after.snapshot_id,
        targets: vec![],
    };
    for id in ordered_ids {
        let previous = old.get(&id);
        let current = new.get(&id);
        let mut delta = TargetDelta {
            target_id: id,
            before: previous.map(|(feedback, target)| feedback.key(target)),
            after: current.map(|(feedback, target)| feedback.key(target)),
            text_changed: false,
            anchor_changed: false,
            binding_changed: false,
            availability_changed: false,
            removal_reason: None,
        };
        match (previous, current) {
            (Some((before_feedback, before_target)), Some((after_feedback, after_target))) => {
                delta.text_changed = before_feedback.text != after_feedback.text;
                delta.anchor_changed = before_target.anchor != after_target.anchor;
                delta.binding_changed =
                    before_target.asset_version_id != after_target.asset_version_id;
                delta.availability_changed =
                    !same_availability(&before_target.availability, &after_target.availability);
                if !(delta.text_changed
                    || delta.anchor_changed
                    || delta.binding_changed
                    || delta.availability_changed)
                {
                    continue;
                }
            }
            (Some(_), None) => {
                delta.removal_reason = Some(
                    *reasons
                        .get(&id)
                        .ok_or(ContinuousReviewError::MissingReference)?,
                )
            }
            (None, Some(_)) => {}
            (None, None) => unreachable!("union contains only existing target identities"),
        }
        result.targets.push(delta);
    }
    Ok(result)
}

fn same_availability(before: &ReviewAvailability, after: &ReviewAvailability) -> bool {
    match (before, after) {
        (ReviewAvailability::Ready, ReviewAvailability::Ready) => true,
        (
            ReviewAvailability::NeedsConfirmation(before),
            ReviewAvailability::NeedsConfirmation(after),
        ) => before.len() == after.len() && before.iter().all(|reason| after.contains(reason)),
        _ => false,
    }
}

struct TransitionCursor {
    key: Option<TargetVersionKey>,
    owner: Option<FeedbackId>,
    ever_present: bool,
}

/// Inputs may omit a journal for content-only comparisons. Every supplied per-target journal,
/// however, must connect the requested endpoints; removals always need an explicit proven cause.
fn validate_transitions(
    before: &TargetIndex<'_>,
    after: &TargetIndex<'_>,
    changes: &[ReviewChange],
) -> Result<HashMap<ReviewTargetId, ReviewChangeKind>, ContinuousReviewError> {
    use ContinuousReviewError::*;
    if changes.len() > MAX_FEEDBACK_ITEMS_PER_ROUND * MAX_TARGETS_PER_FEEDBACK {
        return Err(LimitExceeded);
    }
    let mut cursors = HashMap::new();
    let mut reasons = HashMap::new();
    for change in changes {
        change.validate()?;
        let cursor = cursors.entry(change.target_id).or_insert_with(|| {
            let key = before
                .get(&change.target_id)
                .map(|(feedback, target)| feedback.key(target));
            TransitionCursor {
                key,
                owner: key.map(|key| key.feedback_id).or_else(|| {
                    after
                        .get(&change.target_id)
                        .map(|(feedback, _)| feedback.id)
                }),
                ever_present: key.is_some(),
            }
        });
        for key in [change.before, change.after, change.historical_key]
            .into_iter()
            .flatten()
        {
            if cursor.owner.is_some_and(|owner| owner != key.feedback_id) {
                return Err(DuplicateIdentity);
            }
            cursor.owner = Some(key.feedback_id);
        }
        if cursor.key != change.before {
            return Err(SelectionConflict);
        }
        if change.kind == ReviewChangeKind::Added && cursor.ever_present {
            return Err(DuplicateIdentity);
        }
        if change.before.is_some() && change.after.is_none() {
            reasons.insert(change.target_id, change.kind);
        }
        cursor.key = change.after;
        cursor.ever_present |= change.after.is_some() || change.historical_key.is_some();
    }
    for (id, cursor) in cursors {
        if cursor.key
            != after
                .get(&id)
                .map(|(feedback, target)| feedback.key(target))
        {
            return Err(SelectionConflict);
        }
    }
    Ok(reasons)
}
