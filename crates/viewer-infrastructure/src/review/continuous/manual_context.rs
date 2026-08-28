use super::{migration, migration_inspect, owned_io::Directory};
use crate::review::{MAX_REVIEW_INDEX_BYTES, v3};
use std::path::Path;
use viewer_application::review_workspace::ReviewCommitError;
use viewer_domain::{ProjectId, ReviewStreamId};

/// Read-only selection from one index observation, including a legacy manual active draft.
/// Absence is not permission to adopt a producer stream or migrate a project.
pub(in crate::review) fn resolve(
    root: &Path,
    project: ProjectId,
) -> Result<Option<ReviewStreamId>, ReviewCommitError> {
    let root = Directory::open(root)?;
    let Some(viewer) = root.child(".viewer", false)? else {
        return Ok(None);
    };
    let Some(reviews) = viewer.child("reviews", false)? else {
        return Ok(None);
    };
    let bytes = reviews.read("index.json", MAX_REVIEW_INDEX_BYTES)?;
    let index = match bytes.as_deref().map(v3::decode_index_v3) {
        Some(Ok(index)) => {
            if let Some(backup) = &index.legacy_index {
                migration::verify_backup(&reviews, backup, project)?;
            }
            index
        }
        _ => match migration_inspect::scan_index(&reviews, project, bytes)? {
            Some(legacy) => legacy.index,
            None => return Ok(None),
        },
    };
    if index.project_id != project {
        return Err(ReviewCommitError::Integrity);
    }
    let mut manual = index
        .streams
        .iter()
        .filter(|s| s.task_id.is_none() && s.batch_id.is_none());
    let selected = manual.next().map(|s| s.review_stream_id);
    if manual.next().is_some() {
        return Err(ReviewCommitError::Integrity);
    }
    reviews.verify()?;
    Ok(selected)
}
