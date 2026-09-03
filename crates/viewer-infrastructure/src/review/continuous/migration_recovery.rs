use super::{
    faults::NoReviewCommitFaults,
    migration_inspect,
    owned_io::Directory,
    recovery,
    repository::{ContinuousReviewRepository, VersionedReviewIndex, View},
};
use std::{path::Path, sync::Arc};
use viewer_application::review_workspace::*;
use viewer_domain::ProjectId;

pub(in crate::review) fn save(
    path: &Path,
    project: ProjectId,
    draft: &RecoveryDraft,
) -> Result<(), ReviewCommitError> {
    if draft.editor_input.migration.is_none() || draft.expected_snapshot_id.is_some() {
        return Err(ReviewCommitError::Integrity);
    }
    let repository =
        ContinuousReviewRepository::open_migration(path, project, Arc::new(NoReviewCommitFaults))?;
    let writer = repository
        .writer
        .as_ref()
        .ok_or(ReviewCommitError::ReadOnly)?;
    let _guard = writer.gate.lock().map_err(|_| ReviewCommitError::Io)?;
    let directory = repository
        .checked_directory()?
        .ok_or(ReviewCommitError::Integrity)?;
    let view = match legacy_view(directory, project)? {
        Some(view) => view,
        None => repository.view()?.ok_or(ReviewCommitError::Integrity)?,
    };
    // A stale inspection may retain raw input, but can never authorize migration.
    recovery::save_view(&view, draft)?;
    repository.checked_directory()?;
    Ok(())
}

pub(in crate::review) fn load(
    path: &Path,
    project: ProjectId,
) -> Result<Vec<RecoveryDraft>, ReviewCommitError> {
    let root = Directory::open(path)?;
    let Some(viewer) = root.child(".viewer", false)? else {
        return Ok(vec![]);
    };
    let Some(directory) = viewer.child("reviews", false)? else {
        return Ok(vec![]);
    };
    if let Some(view) = legacy_view(directory, project)? {
        recovery::load_view(&view)
    } else {
        ContinuousReviewRepository::open(path, project, false)?.load_recovery()
    }
}

fn legacy_view(
    directory: Directory,
    project: ProjectId,
) -> Result<Option<View>, ReviewCommitError> {
    Ok(
        migration_inspect::scan(&directory, project)?.map(|legacy| View {
            directory,
            index: VersionedReviewIndex {
                protocol: ReviewPublicationProtocol::V3,
                record: legacy.index,
            },
            index_bytes: Some(legacy.index_bytes),
            ancestry: Default::default(),
        }),
    )
}
