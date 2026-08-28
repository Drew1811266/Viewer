use super::super::{
    MAX_REVIEW_DOCUMENT_BYTES, atomic::atomic_replace_at, protocol::v3::recovery_wire,
};
use super::{
    faults::ReviewCommitFaultPoint,
    history,
    owned_io::Directory,
    repository::{ContinuousReviewRepository, View, protocol_error},
};
use viewer_application::review_workspace::*;
use viewer_domain::ReviewCommandId;

const MAX_RECOVERY_FILES: usize = 10_000;

pub(super) fn save(
    repository: &ContinuousReviewRepository,
    draft: &RecoveryDraft,
) -> Result<(), ReviewCommitError> {
    let writer = repository
        .writer
        .as_ref()
        .ok_or(ReviewCommitError::ReadOnly)?;
    let _guard = writer.gate.lock().map_err(|_| ReviewCommitError::Io)?;
    let view = repository.view()?.ok_or(ReviewCommitError::Integrity)?;
    let bytes = recovery_wire::encode(repository.project_id, draft).map_err(protocol_error)?;
    let directory = view
        .directory
        .child("recovery", true)?
        .ok_or(ReviewCommitError::Integrity)?;
    if let Some(existing) = read(&view, &directory, draft.command_id)?
        && (existing.stream_id != draft.stream_id
            || existing.payload_digest != draft.payload_digest
            || existing.expected_snapshot_id != draft.expected_snapshot_id
            || existing.editor_input != draft.editor_input)
    {
        return Err(ReviewCommitError::CommandConflict);
    }
    atomic_replace_at(
        &directory.file,
        &format!("{}.json", draft.command_id),
        &bytes,
    )
    .map_err(|_| ReviewCommitError::Io)?;
    directory.verify()?;
    repository.view()?;
    repository
        .faults
        .check(ReviewCommitFaultPoint::AfterRecovery)
}

pub(super) fn load(
    repository: &ContinuousReviewRepository,
) -> Result<Vec<RecoveryDraft>, ReviewCommitError> {
    let Some(view) = repository.view()? else {
        return Ok(vec![]);
    };
    let Some(directory) = view.directory.child("recovery", false)? else {
        return Ok(vec![]);
    };
    let mut names = directory.entries(MAX_RECOVERY_FILES)?;
    names.sort();
    let mut drafts = vec![];
    let mut total = 0_u64;
    for name in names {
        if name.starts_with(".viewer-review-") && name.ends_with(".tmp") {
            continue;
        }
        let text = name
            .strip_suffix(".json")
            .ok_or(ReviewCommitError::Integrity)?;
        let id: ReviewCommandId = text.parse().map_err(|_| ReviewCommitError::Integrity)?;
        if id.to_string() != text {
            return Err(ReviewCommitError::Integrity);
        }
        let bytes = directory
            .read(&name, MAX_REVIEW_DOCUMENT_BYTES - total)?
            .ok_or(ReviewCommitError::Integrity)?;
        total += bytes.len() as u64;
        let draft = recovery_wire::decode(view.index.project_id, &bytes).map_err(protocol_error)?;
        if draft.command_id != id {
            return Err(ReviewCommitError::Integrity);
        }
        drafts.push(draft);
    }
    Ok(drafts)
}

pub(super) fn resolve(
    repository: &ContinuousReviewRepository,
    command_id: ReviewCommandId,
) -> Result<CommandLookup, ReviewCommitError> {
    let Some(view) = repository.view()? else {
        return Ok(CommandLookup::Unavailable);
    };
    let Some(directory) = view.directory.child("recovery", false)? else {
        return Ok(CommandLookup::Unavailable);
    };
    let Some(draft) = read(&view, &directory, command_id)? else {
        return Ok(CommandLookup::Unavailable);
    };
    let result = history::find_command(&view, draft.stream_id, command_id)?;
    if matches!(&result,CommandLookup::Found(receipt) if receipt.payload_digest!=draft.payload_digest)
    {
        return Err(ReviewCommitError::CommandConflict);
    }
    Ok(result)
}

fn read(
    view: &View,
    directory: &Directory,
    id: ReviewCommandId,
) -> Result<Option<RecoveryDraft>, ReviewCommitError> {
    let Some(bytes) = directory.read(&format!("{id}.json"), MAX_REVIEW_DOCUMENT_BYTES)? else {
        return Ok(None);
    };
    let draft = recovery_wire::decode(view.index.project_id, &bytes).map_err(protocol_error)?;
    if draft.command_id != id {
        return Err(ReviewCommitError::Integrity);
    }
    Ok(Some(draft))
}
