use super::super::v3;
use super::{history, repository::View};
use viewer_application::review_workspace::ReviewCommitError;
use viewer_domain::review::continuous::*;

/// Validate each basis separately: a 10,000-node history is never collected as 10,000 full states.
pub(super) fn verify_checkpoint(
    view: &View,
    checkpoint: &ArchiveCheckpoint,
    before: &ContinuousReviewState,
) -> Result<ArchivePlan, ReviewCommitError> {
    if checkpoint.project_id != view.index.project_id
        || checkpoint.stream_id != before.stream_id
        || checkpoint.before.snapshot_id != before.snapshot_id
    {
        return Err(ReviewCommitError::Integrity);
    }
    let mut combined = ArchivePlan {
        expected_snapshot_id: before.snapshot_id,
        groups: vec![],
        removed: vec![],
        retained: vec![],
        already_covered: vec![],
    };
    for group in &checkpoint.groups {
        let bases = match &group.basis {
            ArchiveBasis::Known { snapshot, .. } => {
                vec![history::reachable(view, before.stream_id, snapshot)?.state]
            }
            ArchiveBasis::Unknown => vec![],
        };
        let selection = ArchiveSelection {
            expected_snapshot_id: before.snapshot_id,
            groups: vec![group.clone()],
        };
        let plan = plan_archive(before, &bases, &selection, &[])
            .map_err(|_| ReviewCommitError::Integrity)?;
        combined.groups.extend(plan.groups);
        combined.removed.extend(plan.removed);
        combined.retained.extend(plan.retained);
    }
    let derived = ArchiveCheckpoint::from_plan(
        before,
        checkpoint.before,
        &combined,
        checkpoint.archive_id,
        checkpoint.created_at_ms,
    )
    .map_err(|_| ReviewCommitError::Integrity)?;
    if derived != *checkpoint {
        return Err(ReviewCommitError::Integrity);
    }
    Ok(combined)
}

pub(super) fn validate_commit(
    view: &View,
    next: &v3::ReviewStateRecord,
    checkpoints: &[ArchiveCheckpoint],
) -> Result<(), ReviewCommitError> {
    if checkpoints.is_empty() {
        return Ok(());
    }
    let previous_ref = next.state.parent.ok_or(ReviewCommitError::Integrity)?;
    let before = history::read_state(view, next.state.stream_id, &previous_ref)?.state;
    let mut ids = std::collections::HashSet::new();
    let mut targets = vec![];
    let mut changes = vec![];
    for checkpoint in checkpoints {
        if checkpoint.before != previous_ref || !ids.insert(checkpoint.archive_id) {
            return Err(ReviewCommitError::Integrity);
        }
        let plan = verify_checkpoint(view, checkpoint, &before)?;
        targets.extend(plan.removed.iter().map(|key| key.target_id));
        changes.extend(plan.changes(checkpoint.archive_id));
    }
    let mut expected = if targets.is_empty() {
        before.clone()
    } else {
        withdraw_targets(&before, &targets, next.state.snapshot_id)
            .map_err(|_| ReviewCommitError::Integrity)?
    };
    expected.snapshot_id = next.state.snapshot_id;
    expected.parent = next.state.parent;
    if expected != next.state || changes != next.changes {
        return Err(ReviewCommitError::Integrity);
    }
    Ok(())
}
