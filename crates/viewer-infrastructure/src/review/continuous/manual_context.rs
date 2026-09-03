use super::{migration, migration_inspect, owned_io::Directory};
use crate::review::{
    ContinuousReviewProtocol, MAX_REVIEW_INDEX_BYTES, detect_continuous_review_protocol, v3, v4,
};
use std::path::Path;
use viewer_application::review_workspace::ReviewCommitError;
use viewer_domain::{ProjectId, ReviewStreamId};

/// Stable before the first committed index, including failed-save recovery across sessions.
/// Project identity, not paths or untrusted recovery candidates, determines this empty context.
pub(in crate::review) fn empty_stream(project: ProjectId) -> ReviewStreamId {
    let digest = blake3::derive_key(
        "viewer.review.manual-stream/1",
        project.to_string().as_bytes(),
    );
    ReviewStreamId::from_u128(u128::from_be_bytes(
        digest[..16].try_into().expect("fixed digest"),
    ))
}

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
    let index = match bytes.as_deref().map(|bytes| {
        match detect_continuous_review_protocol(bytes, MAX_REVIEW_INDEX_BYTES)? {
            ContinuousReviewProtocol::V3 => v3::decode_index_v3(bytes),
            ContinuousReviewProtocol::V4 => v4::decode_index_v4(bytes),
        }
    }) {
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
