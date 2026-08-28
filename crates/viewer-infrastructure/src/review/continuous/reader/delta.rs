use super::{Failure, request::parse_id};
use crate::review::{
    continuous::{history, repository::View},
    v3::{self, DeltaUnavailableReason as Reason, ReadDelta},
};
use std::collections::{HashMap, HashSet};
use viewer_application::review_workspace::ReviewCommitError;
use viewer_domain::{
    ReviewSnapshotId,
    review::continuous::{ReviewChange, ReviewChangeKind, diff_review},
};

pub(super) fn read(
    view: &View,
    current: &v3::ReviewStateRecord,
    since: Option<&str>,
) -> Result<ReadDelta, Failure> {
    let Some(since) = since else {
        return Ok(ReadDelta::NotRequested {});
    };
    let since: ReviewSnapshotId = parse_id(since)?;
    let value =
        compute(view, current, since).unwrap_or_else(|reason| ReadDelta::Unavailable { reason });
    Ok(value)
}

fn compute(
    view: &View,
    current: &v3::ReviewStateRecord,
    since: ReviewSnapshotId,
) -> Result<ReadDelta, Reason> {
    let mut basis = None;
    let mut changes = vec![];
    let mut visited = 0;
    history::walk(view, current.state.stream_id, |reference, record| {
        visited += 1;
        if reference.snapshot_id == since {
            basis = Some(record.state);
            return Ok(true);
        }
        verify_archival_changes(view, &record)?;
        append_changes(&mut changes, record.changes, 64 * 1024 * 1024)?;
        Ok(false)
    })
    .map_err(reason)?;
    let Some(basis) = basis else {
        // Distinguish another indexed stream without scanning uncommitted files. One total budget.
        for stream in &view.index.streams {
            if stream.review_stream_id == current.state.stream_id {
                continue;
            }
            let mut found = false;
            history::walk(view, stream.review_stream_id, |reference, _| {
                visited += 1;
                if visited > 10_000 {
                    return Err(ReviewCommitError::LimitExceeded);
                }
                found = reference.snapshot_id == since;
                Ok(found)
            })
            .map_err(reason)?;
            if found {
                return Err(Reason::WrongContext);
            }
        }
        return Err(Reason::UnknownSnapshot);
    };
    changes.reverse();
    let delta = diff_review(&basis, &current.state, &changes).map_err(|_| Reason::Integrity)?;
    Ok(ReadDelta::Available {
        since_snapshot_id: since,
        current_snapshot_id: current.state.snapshot_id,
        targets: delta.targets,
    })
}

fn append_changes(
    journal: &mut Vec<ReviewChange>,
    changes: Vec<ReviewChange>,
    limit: usize,
) -> Result<(), ReviewCommitError> {
    journal
        .len()
        .checked_add(changes.len())
        .and_then(|n| n.checked_mul(std::mem::size_of::<ReviewChange>()))
        .filter(|n| *n <= limit)
        .ok_or(ReviewCommitError::LimitExceeded)?;
    journal
        .try_reserve_exact(changes.len())
        .map_err(|_| ReviewCommitError::LimitExceeded)?;
    // Walking head-to-basis reverses both group and intra-group order; one final reverse restores it.
    journal.extend(changes.into_iter().rev());
    Ok(())
}

fn verify_archival_changes(
    view: &View,
    record: &v3::ReviewStateRecord,
) -> Result<(), ReviewCommitError> {
    let mut grouped = HashMap::<_, Vec<_>>::new();
    for change in &record.changes {
        if let Some(id) = change.archive_id {
            grouped.entry(id).or_default().push(change);
        }
    }
    for (id, changes) in grouped {
        // Verify one checkpoint at a time; do not retain all archive bodies in the delta walk.
        let (archive, plan) = history::archive_with_plan(view, record.state.stream_id, id)?;
        let expected: HashMap<_, _> = plan
            .changes(id)
            .into_iter()
            .map(|c| (c.target_id, c))
            .collect();
        let covered: HashSet<_> = archive
            .checkpoint
            .groups
            .iter()
            .flat_map(|g| &g.targets)
            .collect();
        for change in changes {
            match change.kind {
                ReviewChangeKind::Archived
                    if archive.result_snapshot_id == record.state.snapshot_id
                        && expected.get(&change.target_id) == Some(change) => {}
                ReviewChangeKind::Restored
                    if change.historical_key.is_some_and(|k| covered.contains(&k)) => {}
                _ => return Err(ReviewCommitError::Integrity),
            }
        }
    }
    Ok(())
}

fn reason(error: ReviewCommitError) -> Reason {
    if error == ReviewCommitError::LimitExceeded {
        Reason::LimitExceeded
    } else {
        Reason::Integrity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viewer_domain::review::continuous::TargetVersionKey;
    use viewer_domain::{FeedbackId, ReviewTargetId, ReviewTargetRevisionId, ReviewTextRevisionId};
    fn change(id: u128) -> ReviewChange {
        let target_id = ReviewTargetId::from_u128(id);
        ReviewChange {
            target_id,
            before: None,
            after: Some(TargetVersionKey {
                feedback_id: FeedbackId::from_u128(1),
                text_revision_id: ReviewTextRevisionId::from_u128(2),
                target_id,
                target_revision_id: ReviewTargetRevisionId::from_u128(id + 100),
            }),
            kind: ReviewChangeKind::Added,
            archive_id: None,
            historical_key: None,
        }
    }
    #[test]
    fn journal_reverses_in_place_and_refuses_excess_retained_payload_before_extending() {
        let mut journal = vec![];
        let limit = 4 * std::mem::size_of::<ReviewChange>();
        append_changes(&mut journal, vec![change(3), change(4)], limit).unwrap();
        append_changes(&mut journal, vec![change(1), change(2)], limit).unwrap();
        assert_eq!(
            append_changes(&mut journal, vec![change(5)], limit),
            Err(ReviewCommitError::LimitExceeded)
        );
        journal.reverse();
        assert_eq!(journal, vec![change(1), change(2), change(3), change(4)]);
    }
}
