use super::super::{
    ContinuousReviewProtocol, MAX_REVIEW_DOCUMENT_BYTES, detect_continuous_review_protocol, v3, v4,
};
use super::repository::{View, protocol_error};
use std::collections::HashSet;
use viewer_application::review_workspace::{
    CommandLookup, ReviewCommitError, ReviewCommitReceipt, ReviewPublicationProtocol,
};
use viewer_domain::review::continuous::SnapshotRef;
use viewer_domain::{ReviewArchiveId, ReviewCommandId, ReviewStreamId};

const MAX_HISTORY_NODES: usize = 10_000;

#[derive(Debug, PartialEq)]
pub(super) struct VersionedReviewState {
    pub protocol: ReviewPublicationProtocol,
    pub record: v3::ReviewStateRecord,
}

impl std::ops::Deref for VersionedReviewState {
    type Target = v3::ReviewStateRecord;

    fn deref(&self) -> &Self::Target {
        &self.record
    }
}

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
) -> Result<VersionedReviewState, ReviewCommitError> {
    let record = read_document(view, stream_id, reference)?;
    super::evidence::verify(view, &record.evidence)?;
    Ok(record)
}

pub(super) fn read_state_for_materialization(
    view: &View,
    stream_id: ReviewStreamId,
    reference: &SnapshotRef,
) -> Result<VersionedReviewState, ReviewCommitError> {
    read_document(view, stream_id, reference)
}

fn read_document(
    view: &View,
    stream_id: ReviewStreamId,
    reference: &SnapshotRef,
) -> Result<VersionedReviewState, ReviewCommitError> {
    #[cfg(test)]
    DOCUMENT_READS.with(|count| count.set(count.get() + 1));
    let states = view.directory.required_child("states")?;
    let bytes = states
        .read(
            &format!("{}.json", reference.snapshot_id),
            MAX_REVIEW_DOCUMENT_BYTES,
        )?
        .ok_or(ReviewCommitError::Integrity)?;
    verify_digest(&bytes, &reference.blake3)?;
    let protocol = detect_continuous_review_protocol(&bytes, MAX_REVIEW_DOCUMENT_BYTES)
        .map_err(protocol_error)?;
    let record = match protocol {
        ContinuousReviewProtocol::V3 => v3::decode_state_v3(&bytes),
        ContinuousReviewProtocol::V4 => v4::decode_state_v4(&bytes),
    }
    .map_err(protocol_error)?;
    if record.state.project_id != view.index.project_id
        || record.state.stream_id != stream_id
        || record.state.snapshot_id != reference.snapshot_id
    {
        return Err(ReviewCommitError::Integrity);
    }
    let mut ancestry = view.ancestry.borrow_mut();
    let key = (stream_id, *reference);
    if ancestry
        .get(&key)
        .is_some_and(|parent| *parent != record.state.parent)
    {
        return Err(ReviewCommitError::Integrity);
    }
    // Bounded metadata only, not 10,000 complete states. Full cache falls back to reads.
    if ancestry.len() < MAX_HISTORY_NODES {
        ancestry.insert(key, record.state.parent);
    }
    Ok(VersionedReviewState {
        protocol: protocol.into(),
        record,
    })
}

#[cfg(test)]
thread_local! { static DOCUMENT_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

#[cfg(test)]
#[path = "archive_group_tests.rs"]
mod archive_group_tests;
#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;

pub(super) fn verify_digest(bytes: &[u8], digest: &[u8; 32]) -> Result<(), ReviewCommitError> {
    if blake3::hash(bytes).as_bytes() != digest {
        return Err(ReviewCommitError::Integrity);
    }
    Ok(())
}

pub(super) fn walk(
    view: &View,
    stream_id: ReviewStreamId,
    visit: impl FnMut(SnapshotRef, VersionedReviewState) -> Result<bool, ReviewCommitError>,
) -> Result<(), ReviewCommitError> {
    walk_from(view, stream_id, stream(view, stream_id)?.current_ref, visit)
}

fn walk_from(
    view: &View,
    stream_id: ReviewStreamId,
    mut next: Option<SnapshotRef>,
    mut visit: impl FnMut(SnapshotRef, VersionedReviewState) -> Result<bool, ReviewCommitError>,
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
) -> Result<VersionedReviewState, ReviewCommitError> {
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
) -> Result<VersionedReviewState, ReviewCommitError> {
    let found = find_reference(view, stream_id, start, requested.snapshot_id)?;
    if found != *requested {
        return Err(ReviewCommitError::Integrity);
    }
    read_state(view, stream_id, &found)
}

fn find_reference(
    view: &View,
    stream: ReviewStreamId,
    mut next: Option<SnapshotRef>,
    requested: viewer_domain::ReviewSnapshotId,
) -> Result<SnapshotRef, ReviewCommitError> {
    let mut seen = HashSet::new();
    while let Some(reference) = next {
        if seen.len() >= MAX_HISTORY_NODES {
            return Err(ReviewCommitError::LimitExceeded);
        }
        if !seen.insert(reference.snapshot_id) {
            return Err(ReviewCommitError::Integrity);
        }
        if reference.snapshot_id == requested {
            return Ok(reference);
        }
        let cached = view.ancestry.borrow().get(&(stream, reference)).copied();
        next = match cached {
            Some(parent) => parent,
            None => read_document(view, stream, &reference)?.state.parent,
        };
    }
    Err(ReviewCommitError::Integrity)
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
    archive_with_plan(view, stream_id, archive_id).map(|(record, _)| record)
}

pub(super) fn archive_with_plan(
    view: &View,
    stream_id: ReviewStreamId,
    archive_id: ReviewArchiveId,
) -> Result<
    (
        v3::ReviewArchiveRecord,
        viewer_domain::review::continuous::ArchivePlan,
    ),
    ReviewCommitError,
> {
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
    let protocol = detect_continuous_review_protocol(&bytes, MAX_REVIEW_DOCUMENT_BYTES)
        .map_err(protocol_error)?;
    let record = match protocol {
        ContinuousReviewProtocol::V3 => v3::decode_archive_v3(&bytes),
        ContinuousReviewProtocol::V4 => v4::decode_archive_v4(&bytes),
    }
    .map_err(protocol_error)?;
    if record.checkpoint.project_id != view.index.project_id
        || record.checkpoint.stream_id != stream_id
        || record.checkpoint.archive_id != archive_id
    {
        return Err(ReviewCommitError::Integrity);
    }
    let before = reachable(view, stream_id, &record.checkpoint.before)?;
    let plan = super::archives::verify_checkpoint(view, &record.checkpoint, &before.state)?;
    super::references::declared_usage(view, &record.checkpoint, &[])?;
    let result_ref = find_reference(
        view,
        stream_id,
        stream(view, stream_id)?.current_ref,
        record.result_snapshot_id,
    )?;
    let result = read_document(view, stream_id, &result_ref)?;
    if result.state.parent != Some(record.checkpoint.before) {
        return Err(ReviewCommitError::Integrity);
    }
    for change in plan.changes(archive_id) {
        if !result.changes.contains(&change) {
            return Err(ReviewCommitError::Integrity);
        }
    }
    Ok((record, plan))
}
