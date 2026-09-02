use super::*;
use crate::{ReviewArtifactError, ReviewAssetError, ReviewTaskCancellation};
use std::collections::{HashMap, HashSet};
use viewer_domain::{
    ReviewStreamId, ReviewTargetId,
    review::continuous::{
        ContinuousReviewState, ReviewChange, ReviewChangeKind, TargetVersionKey, VersionedFeedback,
        VersionedTarget, diff_review,
    },
};

const MAX_FLUSHED_REVISIONS: usize = 10_000;

/// Fold a contiguous, barrier-free authoring segment into one public transition. Every original
/// journal is validated first; the result is then validated against the exact public endpoints.
pub fn fold_public_changes(
    published: &ContinuousReviewState,
    targets: &[StoredAuthoringState],
) -> Result<Vec<ReviewChange>, ReviewWorkspaceError> {
    published.validate()?;
    let Some(final_target) = targets.last() else {
        return Ok(vec![]);
    };
    if targets.len() > MAX_FLUSHED_REVISIONS {
        return Err(ReviewCommitError::LimitExceeded.into());
    }
    let mut previous = published;
    let mut expected_sequence = targets[0].head.sequence;
    for (index, target) in targets.iter().enumerate() {
        if target.barrier != ReviewBarrierKind::None
            || target.head.sequence != expected_sequence
            || target.state.snapshot_id != target.head.snapshot_id
            || target.state.project_id != published.project_id
            || target.state.stream_id != published.stream_id
            || (index > 0
                && target.state.parent.map(|parent| parent.snapshot_id)
                    != Some(previous.snapshot_id))
            || (index == 0
                && target
                    .state
                    .parent
                    .is_some_and(|parent| parent.snapshot_id != published.snapshot_id))
        {
            return Err(ReviewCommitError::Integrity.into());
        }
        diff_review(previous, &target.state, &target.changes)?;
        previous = &target.state;
        expected_sequence = expected_sequence
            .checked_add(1)
            .ok_or(ReviewCommitError::LimitExceeded)?;
    }

    let before = target_index(published);
    let after = target_index(&final_target.state);
    let mut order = Vec::with_capacity(before.len().saturating_add(after.len()));
    let mut seen = HashSet::new();
    for id in published
        .feedback
        .iter()
        .flat_map(|feedback| feedback.targets.iter().map(|target| target.id))
        .chain(
            final_target
                .state
                .feedback
                .iter()
                .flat_map(|feedback| feedback.targets.iter().map(|target| target.id)),
        )
    {
        if seen.insert(id) {
            order.push(id);
        }
    }

    let mut folded = Vec::new();
    for id in order {
        let old = before.get(&id).copied();
        let new = after.get(&id).copied();
        let before_key = old.map(key);
        let after_key = new.map(key);
        if before_key == after_key {
            continue;
        }
        let change = match (old, new) {
            (None, Some(_)) => ReviewChange {
                target_id: id,
                before: None,
                after: after_key,
                kind: ReviewChangeKind::Added,
                archive_id: None,
                historical_key: None,
            },
            (Some(_), None) => {
                let removal = targets
                    .iter()
                    .flat_map(|target| target.changes.iter())
                    .rev()
                    .find(|change| {
                        change.target_id == id && change.before.is_some() && change.after.is_none()
                    })
                    .ok_or(ReviewCommitError::Integrity)?;
                ReviewChange {
                    target_id: id,
                    before: before_key,
                    after: None,
                    kind: removal.kind,
                    archive_id: removal.archive_id,
                    historical_key: removal.historical_key,
                }
            }
            (Some((old_feedback, old_target)), Some((new_feedback, new_target))) => {
                let kind = if old_target.asset_version_id != new_target.asset_version_id {
                    ReviewChangeKind::Rebound
                } else if old_target.availability != new_target.availability {
                    ReviewChangeKind::AvailabilityChanged
                } else {
                    let _ = (old_feedback, new_feedback);
                    ReviewChangeKind::Edited
                };
                ReviewChange {
                    target_id: id,
                    before: before_key,
                    after: after_key,
                    kind,
                    archive_id: None,
                    historical_key: None,
                }
            }
            (None, None) => continue,
        };
        change.validate()?;
        folded.push(change);
    }
    diff_review(published, &final_target.state, &folded)?;
    Ok(folded)
}

type IndexedTarget<'a> = (&'a VersionedFeedback, &'a VersionedTarget);

fn target_index(state: &ContinuousReviewState) -> HashMap<ReviewTargetId, IndexedTarget<'_>> {
    state
        .feedback
        .iter()
        .flat_map(|feedback| {
            feedback
                .targets
                .iter()
                .map(move |target| (target.id, (feedback, target)))
        })
        .collect()
}

fn key((feedback, target): IndexedTarget<'_>) -> TargetVersionKey {
    TargetVersionKey {
        feedback_id: feedback.id,
        text_revision_id: feedback.text_revision_id,
        target_id: target.id,
        target_revision_id: target.revision_id,
    }
}

impl ReviewMaterializationService {
    /// Synchronously reaches an exact authoring basis before a history-sensitive command is
    /// prepared. No barrier command is inserted unless the selected basis is public.
    pub async fn flush_through(
        &self,
        stream: ReviewStreamId,
        head: ReviewAuthoringHead,
        cancellation: ReviewTaskCancellation,
    ) -> Result<(), ReviewWorkspaceError> {
        for _ in 0..MAX_FLUSHED_REVISIONS {
            if cancellation.is_cancelled() {
                return Err(ReviewWorkspaceError::Cancelled);
            }
            let queue = self.queue.clone();
            let heads = super::service::io(move || queue.heads(stream)).await?;
            let authoring = heads.authoring.ok_or(ReviewCommitError::StaleSnapshot)?;
            if authoring.sequence < head.sequence
                || (authoring.sequence == head.sequence
                    && authoring.snapshot_id != head.snapshot_id)
            {
                return Err(ReviewCommitError::StaleSnapshot.into());
            }
            if heads.published.is_some_and(|published| {
                published.sequence > head.sequence
                    || (published.sequence == head.sequence
                        && published.snapshot_id == head.snapshot_id)
            }) {
                return Ok(());
            }
            match self.run_one(cancellation.clone()).await? {
                ReviewMaterializationOutcome::Published { .. } => continue,
                ReviewMaterializationOutcome::Blocked { .. }
                | ReviewMaterializationOutcome::Retrying { .. }
                | ReviewMaterializationOutcome::Idle => {}
            }

            // `run_one` serves the process-wide queue and may have handled another stream. Never
            // infer this stream's state from the returned target sequence alone: sequences are
            // stream-local and can collide. Re-read the requested stream before deciding.
            let queue = self.queue.clone();
            let status = super::service::io(move || queue.status(stream)).await?;
            if let ReviewPublicationStatus::Blocked { code } = status {
                return Err(materialization_failure(code));
            }
            return Err(ReviewCommitError::LeaseBusy.into());
        }
        Err(ReviewCommitError::LimitExceeded.into())
    }
}

pub(crate) fn materialization_failure(code: ReviewMaterializationFailure) -> ReviewWorkspaceError {
    match code {
        ReviewMaterializationFailure::SourceChanged => ReviewAssetError::SourceChanged.into(),
        ReviewMaterializationFailure::SourceMissing => ReviewAssetError::NotFound.into(),
        ReviewMaterializationFailure::SourceUnreadable => ReviewAssetError::Unavailable.into(),
        ReviewMaterializationFailure::RenderFailed => ReviewArtifactError::Unavailable.into(),
        ReviewMaterializationFailure::Integrity => ReviewCommitError::Integrity.into(),
        ReviewMaterializationFailure::LimitExceeded => ReviewCommitError::LimitExceeded.into(),
        ReviewMaterializationFailure::Io => ReviewCommitError::Io.into(),
    }
}
