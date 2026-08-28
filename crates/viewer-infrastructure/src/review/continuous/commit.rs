use super::super::{
    MAX_REVIEW_DOCUMENT_BYTES,
    atomic::{AtomicCreateOnceError, atomic_create_once_at, atomic_replace_at_with_barrier},
    v3,
};
use super::faults::ReviewCommitFaultPoint;
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
    if request.next.state.project_id != repository.project_id {
        return Err(ReviewCommitError::Integrity);
    }
    if let Some(stream) = view
        .index
        .streams
        .iter()
        .find(|s| s.review_stream_id == request.next.state.stream_id)
        && super::mapping::scope(stream)? != request.production
    {
        return Err(ReviewCommitError::Integrity);
    }
    match history::find_command(&view, request.next.state.stream_id, request.next.command_id)? {
        CommandLookup::Found(receipt) if receipt.payload_digest == request.next.payload_digest => {
            return Ok(receipt);
        }
        CommandLookup::Found(_) => return Err(ReviewCommitError::CommandConflict),
        CommandLookup::Unavailable => return Err(ReviewCommitError::LookupUnavailable),
        CommandLookup::Absent => {}
    }
    let prepared = prepare(&mut view, request)?;
    if let Some(bytes) = &view.index_bytes
        && crate::review::protocol::decode_catalog_versioned(bytes).is_ok()
    {
        // An empty legacy catalog contains no review data; its first successful save creates
        // the first state, retaining the old index just as an explicit migration would.
        super::migration::save_backup(&view.directory, bytes)?;
    }
    install(repository, &view, &prepared)?;
    validate_installed(&view, &prepared)?;
    repository
        .faults
        .check(ReviewCommitFaultPoint::BeforeIndex)?;
    let observed = repository.view()?.ok_or(ReviewCommitError::Integrity)?;
    if observed.index_bytes != view.index_bytes {
        return Err(ReviewCommitError::StaleSnapshot);
    }
    atomic_replace_at_with_barrier(
        &view.directory.file,
        "index.json",
        &prepared.index_bytes,
        || {
            repository
                .faults
                .check(ReviewCommitFaultPoint::AfterIndex)
                .map_err(|_| std::io::Error::other("injected index durability failure"))
        },
    )
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

pub(super) fn install(
    repository: &ContinuousReviewRepository,
    view: &View,
    prepared: &PreparedCommit,
) -> Result<(), ReviewCommitError> {
    super::evidence::install(view, &prepared.record.evidence, &prepared.staged)?;
    repository
        .faults
        .check(ReviewCommitFaultPoint::AfterEvidence)?;
    let states = view
        .directory
        .child("states", true)?
        .ok_or(ReviewCommitError::Integrity)?;
    create_record(
        &states,
        &format!("{}.json", prepared.reference.snapshot_id),
        &prepared.bytes,
    )?;
    repository
        .faults
        .check(ReviewCommitFaultPoint::AfterState)?;
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
    repository
        .faults
        .check(ReviewCommitFaultPoint::AfterArchive)
}

pub(super) fn validate_installed(
    view: &View,
    prepared: &PreparedCommit,
) -> Result<(), ReviewCommitError> {
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
