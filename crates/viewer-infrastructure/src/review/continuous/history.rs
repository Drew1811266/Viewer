use super::super::{MAX_REVIEW_DOCUMENT_BYTES, v3};
use super::repository::{View, protocol_error};
use std::collections::HashSet;
use viewer_application::review_workspace::{CommandLookup, ReviewCommitError, ReviewCommitReceipt};
use viewer_domain::review::continuous::SnapshotRef;
use viewer_domain::{ReviewArchiveId, ReviewCommandId, ReviewStreamId};

const MAX_HISTORY_NODES: usize = 10_000;

pub(super) fn stream(
    view: &View,
    id: ReviewStreamId,
) -> Result<&v3::ReviewStreamV3, ReviewCommitError> {
    view.index
        .streams
        .iter()
        .find(|s| s.review_stream_id == id)
        .ok_or(ReviewCommitError::Integrity)
}

pub(super) fn read_state(
    view: &View,
    stream_id: ReviewStreamId,
    reference: &SnapshotRef,
) -> Result<v3::ReviewStateRecord, ReviewCommitError> {
    let record = read_document(view, stream_id, reference)?;
    super::evidence::verify(view, &record.evidence)?;
    Ok(record)
}

fn read_document(
    view: &View,
    stream_id: ReviewStreamId,
    reference: &SnapshotRef,
) -> Result<v3::ReviewStateRecord, ReviewCommitError> {
    let states = view.directory.required_child("states")?;
    let bytes = states
        .read(
            &format!("{}.json", reference.snapshot_id),
            MAX_REVIEW_DOCUMENT_BYTES,
        )?
        .ok_or(ReviewCommitError::Integrity)?;
    verify_digest(&bytes, &reference.blake3)?;
    let record = v3::decode_state_v3(&bytes).map_err(protocol_error)?;
    if record.state.project_id != view.index.project_id
        || record.state.stream_id != stream_id
        || record.state.snapshot_id != reference.snapshot_id
    {
        return Err(ReviewCommitError::Integrity);
    }
    Ok(record)
}

pub(super) fn verify_digest(bytes: &[u8], digest: &[u8; 32]) -> Result<(), ReviewCommitError> {
    if blake3::hash(bytes).as_bytes() != digest {
        return Err(ReviewCommitError::Integrity);
    }
    Ok(())
}

pub(super) fn walk(
    view: &View,
    stream_id: ReviewStreamId,
    visit: impl FnMut(SnapshotRef, v3::ReviewStateRecord) -> Result<bool, ReviewCommitError>,
) -> Result<(), ReviewCommitError> {
    walk_from(view, stream_id, stream(view, stream_id)?.current_ref, visit)
}

fn walk_from(
    view: &View,
    stream_id: ReviewStreamId,
    mut next: Option<SnapshotRef>,
    mut visit: impl FnMut(SnapshotRef, v3::ReviewStateRecord) -> Result<bool, ReviewCommitError>,
) -> Result<(), ReviewCommitError> {
    let mut seen = HashSet::new();
    while let Some(reference) = next {
        if seen.len() >= MAX_HISTORY_NODES {
            return Err(ReviewCommitError::LimitExceeded);
        }
        if !seen.insert(reference.snapshot_id) {
            return Err(ReviewCommitError::Integrity);
        }
        let record = read_document(view, stream_id, &reference)?;
        next = record.state.parent;
        if visit(reference, record)? {
            return Ok(());
        }
    }
    Ok(())
}

pub(super) fn reachable(
    view: &View,
    stream_id: ReviewStreamId,
    requested: &SnapshotRef,
) -> Result<v3::ReviewStateRecord, ReviewCommitError> {
    reachable_from(
        view,
        stream_id,
        stream(view, stream_id)?.current_ref,
        requested,
    )
}

pub(super) fn reachable_from(
    view: &View,
    stream_id: ReviewStreamId,
    start: Option<SnapshotRef>,
    requested: &SnapshotRef,
) -> Result<v3::ReviewStateRecord, ReviewCommitError> {
    let mut found = None;
    walk_from(view, stream_id, start, |reference, record| {
        if reference.snapshot_id == requested.snapshot_id {
            if reference != *requested {
                return Err(ReviewCommitError::Integrity);
            }
            found = Some(record);
            return Ok(true);
        }
        Ok(false)
    })?;
    let record = found.ok_or(ReviewCommitError::Integrity)?;
    super::evidence::verify(view, &record.evidence)?;
    Ok(record)
}

pub(super) fn find_command(
    view: &View,
    stream_id: ReviewStreamId,
    command_id: ReviewCommandId,
) -> Result<CommandLookup, ReviewCommitError> {
    if !view
        .index
        .streams
        .iter()
        .any(|s| s.review_stream_id == stream_id)
    {
        return Ok(CommandLookup::Absent);
    }
    let mut found = CommandLookup::Absent;
    let result = walk(view, stream_id, |snapshot, record| {
        if record.command_id == command_id {
            super::evidence::verify(view, &record.evidence)?;
            found = CommandLookup::Found(ReviewCommitReceipt {
                command_id,
                payload_digest: record.payload_digest,
                snapshot,
            });
            return Ok(true);
        }
        Ok(false)
    });
    if result.is_err() {
        return Ok(CommandLookup::Unavailable);
    }
    Ok(found)
}

pub(super) fn archive(
    view: &View,
    stream_id: ReviewStreamId,
    archive_id: ReviewArchiveId,
) -> Result<v3::ReviewArchiveRecord, ReviewCommitError> {
    let reference = stream(view, stream_id)?
        .archive_refs
        .iter()
        .find(|v| v.archive_id == archive_id)
        .ok_or(ReviewCommitError::Integrity)?;
    let directory = view.directory.required_child("archives")?;
    let bytes = directory
        .read(&format!("{archive_id}.json"), MAX_REVIEW_DOCUMENT_BYTES)?
        .ok_or(ReviewCommitError::Integrity)?;
    verify_digest(&bytes, &reference.blake3)?;
    let record = v3::decode_archive_v3(&bytes).map_err(protocol_error)?;
    if record.checkpoint.project_id != view.index.project_id
        || record.checkpoint.stream_id != stream_id
        || record.checkpoint.archive_id != archive_id
    {
        return Err(ReviewCommitError::Integrity);
    }
    let before = reachable(view, stream_id, &record.checkpoint.before)?;
    let plan = super::archives::verify_checkpoint(view, &record.checkpoint, &before.state)?;
    super::references::declared_usage(view, &record.checkpoint, &[])?;
    let mut result_found = false;
    walk(view, stream_id, |_, result| {
        if result.state.snapshot_id == record.result_snapshot_id {
            if result.state.parent != Some(record.checkpoint.before) {
                return Err(ReviewCommitError::Integrity);
            }
            for change in plan.changes(archive_id) {
                if !result.changes.contains(&change) {
                    return Err(ReviewCommitError::Integrity);
                }
            }
            result_found = true;
            return Ok(true);
        }
        Ok(false)
    })?;
    if !result_found {
        return Err(ReviewCommitError::Integrity);
    }
    Ok(record)
}
