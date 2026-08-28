use super::{history, repository::View};
use std::collections::HashSet;
use viewer_application::review_workspace::ReviewCommitError;
use viewer_domain::{
    ReviewStreamId,
    review::continuous::{ArchiveCheckpoint, ReviewChangeKind},
};

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
