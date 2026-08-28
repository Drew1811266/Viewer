use super::super::{
    MAX_REVIEW_DOCUMENT_BYTES,
    atomic::{AtomicCreateOnceError, atomic_create_once_at, atomic_replace_at},
    v3,
};
use super::{
    history,
    owned_io::{Directory, map_io},
    prepare::{PreparedCommit, prepare},
    repository::{ContinuousReviewRepository, View, protocol_error},
};
use viewer_application::review_workspace::*;

pub(super) fn commit(
    repository: &ContinuousReviewRepository,
    request: ReviewCommitRequest,
) -> Result<ReviewCommitReceipt, ReviewCommitError> {
    let writer = repository
        .writer
        .as_ref()
        .ok_or(ReviewCommitError::ReadOnly)?;
    let _guard = writer.gate.lock().map_err(|_| ReviewCommitError::Io)?;
    let mut view = repository.view()?.ok_or(ReviewCommitError::Integrity)?;
    let prepared = prepare(&mut view, request)?;
    install(&view, &prepared)?;
    validate_installed(&view, &prepared)?;
    let observed = repository.view()?.ok_or(ReviewCommitError::Integrity)?;
    if observed.index_bytes != view.index_bytes {
        return Err(ReviewCommitError::StaleSnapshot);
    }
    atomic_replace_at(&view.directory.file, "index.json", &prepared.index_bytes)
        .map_err(|_| ReviewCommitError::OutcomeUnknown)?;
    repository
        .view()
        .map_err(|_| ReviewCommitError::OutcomeUnknown)?;
    Ok(ReviewCommitReceipt {
        command_id: prepared.record.command_id,
        payload_digest: prepared.record.payload_digest,
        snapshot: prepared.reference,
    })
}

fn install(view: &View, prepared: &PreparedCommit) -> Result<(), ReviewCommitError> {
    super::evidence::install(view, &prepared.record.evidence, &prepared.staged)?;
    let states = view
        .directory
        .child("states", true)?
        .ok_or(ReviewCommitError::Integrity)?;
    create_record(
        &states,
        &format!("{}.json", prepared.reference.snapshot_id),
        &prepared.bytes,
    )?;
    if !prepared.archives.is_empty() {
        let directory = view
            .directory
            .child("archives", true)?
            .ok_or(ReviewCommitError::Integrity)?;
        for checkpoint in &prepared.archives {
            let record = v3::ReviewArchiveRecord {
                checkpoint: checkpoint.clone(),
                result_snapshot_id: prepared.reference.snapshot_id,
            };
            create_record(
                &directory,
                &format!("{}.json", checkpoint.archive_id),
                &v3::encode_archive_v3(&record).map_err(protocol_error)?,
            )?;
        }
    }
    if !prepared.usages.is_empty() {
        let directory = view
            .directory
            .child("usage", true)?
            .ok_or(ReviewCommitError::Integrity)?;
        for usage in &prepared.usages {
            create_record(
                &directory,
                &format!("{}.json", usage.declaration_id),
                &v3::encode_usage_v1(usage).map_err(protocol_error)?,
            )?;
        }
    }
    Ok(())
}

fn validate_installed(view: &View, prepared: &PreparedCommit) -> Result<(), ReviewCommitError> {
    let id = prepared.record.state.stream_id;
    history::read_state(view, id, &prepared.reference)?;
    for archive in &history::stream(view, id)?.archive_refs {
        history::archive(view, id, archive.archive_id)?;
    }
    for usage in &history::stream(view, id)?.usage_refs {
        super::usage::read(view, id, usage.declaration_id)?;
    }
    Ok(())
}

pub(super) fn create_record(
    directory: &Directory,
    name: &str,
    bytes: &[u8],
) -> Result<(), ReviewCommitError> {
    directory.exact_name(name)?;
    match atomic_create_once_at(&directory.file, name, bytes) {
        Ok(()) => {}
        Err(AtomicCreateOnceError::AlreadyExists) => {
            if directory.read(name, MAX_REVIEW_DOCUMENT_BYTES)?.as_deref() != Some(bytes) {
                return Err(ReviewCommitError::Integrity);
            }
        }
        Err(AtomicCreateOnceError::Io(error)) => return Err(map_io(error)),
    }
    directory.verify()
}
