use std::collections::HashSet;

use super::{ContinuousReviewError, ContinuousReviewState, TargetVersionKey, VersionedFeedback};
use crate::review::{FeedbackAnchor, MAX_FEEDBACK_TEXT_BYTES};
use crate::{
    ReviewArchiveId, ReviewSnapshotId, ReviewTargetId, ReviewTargetRevisionId, ReviewTextRevisionId,
};

/// Natural language is preserved byte-for-byte; a real edit needs a new text revision.
pub fn update_feedback_text(
    feedback: &VersionedFeedback,
    revision_id: ReviewTextRevisionId,
    text: &str,
) -> Result<VersionedFeedback, ContinuousReviewError> {
    feedback.validate()?;
    if text.len() > MAX_FEEDBACK_TEXT_BYTES {
        return Err(ContinuousReviewError::LimitExceeded);
    }
    if feedback.text == text {
        return Ok(feedback.clone());
    }
    if feedback.text_revision_id == revision_id {
        return Err(ContinuousReviewError::DuplicateIdentity);
    }
    let mut next = feedback.clone();
    next.text = text.to_owned();
    next.text_revision_id = revision_id;
    next.validate()?;
    Ok(next)
}

/// Media-specific bounds are checked by `ContinuousReviewState::validate` before publication.
pub fn replace_target(
    feedback: &VersionedFeedback,
    target_id: ReviewTargetId,
    revision_id: ReviewTargetRevisionId,
    anchor: FeedbackAnchor,
) -> Result<VersionedFeedback, ContinuousReviewError> {
    feedback.validate()?;
    let target = feedback
        .targets
        .iter()
        .find(|target| target.id == target_id)
        .ok_or(ContinuousReviewError::MissingReference)?;
    if target.anchor == anchor {
        return Ok(feedback.clone());
    }
    if feedback
        .targets
        .iter()
        .any(|target| target.revision_id == revision_id)
    {
        return Err(ContinuousReviewError::DuplicateIdentity);
    }
    let mut next = feedback.clone();
    let target = next
        .targets
        .iter_mut()
        .find(|target| target.id == target_id)
        .expect("validated target");
    target.revision_id = revision_id;
    target.anchor = anchor;
    next.validate()?;
    Ok(next)
}

pub fn add_feedback(
    current: &ContinuousReviewState,
    feedback: VersionedFeedback,
    snapshot_id: ReviewSnapshotId,
) -> Result<ContinuousReviewState, ContinuousReviewError> {
    current.validate()?;
    let mut next = prepare_next(current, snapshot_id)?;
    next.feedback.push(feedback);
    next.validate()?;
    Ok(next)
}

pub fn withdraw_targets(
    current: &ContinuousReviewState,
    target_ids: &[ReviewTargetId],
    snapshot_id: ReviewSnapshotId,
) -> Result<ContinuousReviewState, ContinuousReviewError> {
    current.validate()?;
    if target_ids.is_empty() {
        return Ok(current.clone());
    }
    let available: HashSet<_> = current
        .feedback
        .iter()
        .flat_map(|feedback| &feedback.targets)
        .map(|target| target.id)
        .collect();
    if target_ids.len() > available.len()
        && target_ids.len() > crate::review::MAX_TARGETS_PER_FEEDBACK
    {
        return Err(ContinuousReviewError::LimitExceeded);
    }
    let mut selected = HashSet::new();
    for id in target_ids {
        if !selected.insert(*id) {
            return Err(ContinuousReviewError::DuplicateIdentity);
        }
        if !available.contains(id) {
            return Err(ContinuousReviewError::MissingReference);
        }
    }
    let mut next = prepare_next(current, snapshot_id)?;
    for feedback in &mut next.feedback {
        feedback
            .targets
            .retain(|target| !selected.contains(&target.id));
    }
    next.feedback
        .retain(|feedback| !feedback.targets.is_empty());
    next.validate()?;
    Ok(next)
}

pub(super) fn prepare_next(
    current: &ContinuousReviewState,
    snapshot_id: ReviewSnapshotId,
) -> Result<ContinuousReviewState, ContinuousReviewError> {
    if current.snapshot_id == snapshot_id {
        return Err(ContinuousReviewError::DuplicateIdentity);
    }
    let mut next = current.clone();
    next.snapshot_id = snapshot_id;
    // Only the repository can supply the digest of the actual committed predecessor.
    next.parent = None;
    Ok(next)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewChangeKind {
    Added,
    Edited,
    Withdrawn,
    Archived,
    Restored,
    Rebound,
    AvailabilityChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewChange {
    pub target_id: ReviewTargetId,
    pub before: Option<TargetVersionKey>,
    pub after: Option<TargetVersionKey>,
    pub kind: ReviewChangeKind,
    pub archive_id: Option<ReviewArchiveId>,
    pub historical_key: Option<TargetVersionKey>,
}

impl ReviewChange {
    pub fn validate(&self) -> Result<(), ContinuousReviewError> {
        use ReviewChangeKind::*;
        let invalid = || Err(ContinuousReviewError::InvalidData);
        if [self.before, self.after, self.historical_key]
            .into_iter()
            .flatten()
            .any(|key| key.target_id != self.target_id)
        {
            return invalid();
        }
        if let (Some(before), Some(after)) = (self.before, self.after)
            && before.feedback_id != after.feedback_id
        {
            return invalid();
        }
        let has_archive = self.archive_id.is_some() && self.historical_key.is_some();
        if !matches!(self.kind, Archived | Restored)
            && (self.archive_id.is_some() || self.historical_key.is_some())
        {
            return invalid();
        }
        let valid = match self.kind {
            Added => self.before.is_none() && self.after.is_some(),
            Withdrawn => self.before.is_some() && self.after.is_none(),
            Edited | Rebound | AvailabilityChanged => {
                self.before.is_some() && self.after.is_some() && self.before != self.after
            }
            Archived => {
                has_archive
                    && (self.before == self.after
                        || (self.before == self.historical_key && self.after.is_none()))
            }
            Restored => has_archive && self.after == self.historical_key,
        };
        if valid { Ok(()) } else { invalid() }
    }
}
