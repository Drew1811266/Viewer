use super::{history, repository::View};
use std::collections::HashSet;
use viewer_application::review_workspace::ReviewCommitError;
use viewer_domain::{
    ReviewStreamId,
    review::continuous::{ArchiveCheckpoint, ReviewChangeKind},
};

pub(super) fn load(
    view: &View,
    stream: ReviewStreamId,
    keys: &[viewer_domain::review::continuous::TargetVersionKey],
) -> Result<Vec<viewer_domain::review::continuous::ArchiveCoverage>, ReviewCommitError> {
    use viewer_domain::review::continuous::ArchiveCoverage;
    if keys.len() > 100_000_000 {
        return Err(ReviewCommitError::LimitExceeded);
    }
    let selected: HashSet<_> = keys.iter().copied().collect();
    let mut seen = HashSet::new();
    let mut result = vec![];
    if selected.is_empty() {
        return Ok(result);
    }
    history::walk(view, stream, |_, state| {
        for change in state.changes.iter().rev() {
            let (Some(id), Some(key)) = (change.archive_id, change.historical_key) else {
                continue;
            };
            if !selected.contains(&key) || !seen.insert((id, key)) {
                continue;
            }
            let checkpoint = history::archive(view, stream, id)?.checkpoint;
            if !checkpoint.groups.iter().any(|g| g.targets.contains(&key)) {
                return Err(ReviewCommitError::Integrity);
            }
            let active = match change.kind {
                ReviewChangeKind::Archived => true,
                ReviewChangeKind::Restored => false,
                _ => return Err(ReviewCommitError::Integrity),
            };
            if result.len() >= 100_000_000 {
                return Err(ReviewCommitError::LimitExceeded);
            }
            result.push(ArchiveCoverage {
                archive_id: id,
                key,
                active,
            });
        }
        Ok(false)
    })?;
    Ok(result)
}

/// Read newest events first, retaining only the coverage keys relevant to this selection.
pub(super) fn require_uncovered(
    view: &View,
    stream: ReviewStreamId,
    checkpoints: &[ArchiveCheckpoint],
) -> Result<(), ReviewCommitError> {
    if checkpoints.is_empty() {
        return Ok(());
    }
    let mut selected = HashSet::new();
    for checkpoint in checkpoints {
        for group in &checkpoint.groups {
            for key in &group.targets {
                if !selected.insert(*key) {
                    return Err(ReviewCommitError::Integrity);
                }
            }
        }
    }
    let mut seen = HashSet::new();
    history::walk(view, stream, |_, record| {
        for change in record.changes.iter().rev() {
            let (Some(id), Some(key)) = (change.archive_id, change.historical_key) else {
                continue;
            };
            if !selected.contains(&key) || !seen.insert((id, key)) {
                continue;
            }
            let archive = history::archive(view, stream, id)?;
            if !archive
                .checkpoint
                .groups
                .iter()
                .any(|group| group.targets.contains(&key))
            {
                return Err(ReviewCommitError::Integrity);
            }
            match change.kind {
                ReviewChangeKind::Archived => return Err(ReviewCommitError::Integrity),
                ReviewChangeKind::Restored => {}
                _ => return Err(ReviewCommitError::Integrity),
            }
        }
        Ok(false)
    })
}
