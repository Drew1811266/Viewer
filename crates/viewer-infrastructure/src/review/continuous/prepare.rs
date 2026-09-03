//! Validates a complete transaction before publishing any immutable object.
use super::super::{v3, v4};
use super::{
    history, mapping, references,
    repository::{View, protocol_error},
};
use std::collections::HashMap;
use viewer_application::review_workspace::*;
use viewer_domain::review::continuous::{
    ArchiveCheckpoint, SnapshotRef, diff_review, validate_shared_identities,
};

pub(super) struct PreparedCommit {
    pub protocol: ReviewPublicationProtocol,
    pub record: v3::ReviewStateRecord,
    pub bytes: Vec<u8>,
    pub reference: SnapshotRef,
    pub index_bytes: Vec<u8>,
    pub archives: Vec<ArchiveCheckpoint>,
    pub usages: Vec<v3::ReviewUsageRecord>,
    pub staged: Vec<PreparedEvidenceFile>,
    pub reusable_evidence: HashMap<[u8; 32], v3::EvidenceRef>,
}

pub(super) fn prepare(
    view: &mut View,
    mut request: ReviewCommitRequest,
) -> Result<PreparedCommit, ReviewCommitError> {
    let mut stream = prepare_stream(view, &request)?;
    if request.next.state.parent.is_some() && request.next.state.parent != request.expected {
        return Err(ReviewCommitError::Integrity);
    }
    request.next.state.parent = request.expected;
    let protocol = request.next.publication_protocol;
    let record: v3::ReviewStateRecord = request.next.into();
    let bytes = match protocol {
        ReviewPublicationProtocol::V3 => v3::encode_state_v3(&record),
        ReviewPublicationProtocol::V4 => v4::encode_state_v4(&record),
    }
    .map_err(protocol_error)?;
    let previous = validate_chain(view, protocol, &record)?;
    let reusable_evidence = super::evidence::reusable_references(
        previous
            .as_ref()
            .map(|value| value.record.evidence.as_slice()),
        &record.evidence,
    );
    super::coverage::require_uncovered(view, record.state.stream_id, &request.archives)?;
    super::archives::validate_commit(view, &record, &request.archives)?;
    references::feedback_origins(view, &record)?;
    references::transitions(view, &record, &request.archives)?;
    let reference = SnapshotRef {
        snapshot_id: record.state.snapshot_id,
        blake3: *blake3::hash(&bytes).as_bytes(),
    };
    stream.current_ref = Some(reference);
    let usages: Vec<v3::ReviewUsageRecord> =
        request.adopted_usage.into_iter().map(Into::into).collect();
    for checkpoint in &request.archives {
        references::declared_usage(view, checkpoint, &usages)?;
    }
    add_usage_refs(view, &mut stream, &usages)?;
    add_archive_refs(
        &mut stream,
        &request.archives,
        reference.snapshot_id,
        protocol,
    )?;
    if let Some(existing) = view
        .index
        .streams
        .iter_mut()
        .find(|s| s.review_stream_id == stream.review_stream_id)
    {
        *existing = stream;
    } else {
        view.index.streams.push(stream);
    }
    if protocol == ReviewPublicationProtocol::V4 {
        view.index.protocol = ReviewPublicationProtocol::V4;
    }
    let index_bytes = match view.index.protocol {
        ReviewPublicationProtocol::V3 => v3::encode_index_v3(&view.index.record),
        ReviewPublicationProtocol::V4 => v4::encode_index_v4(&view.index.record),
    }
    .map_err(protocol_error)?;
    Ok(PreparedCommit {
        protocol,
        record,
        bytes,
        reference,
        index_bytes,
        archives: request.archives,
        usages,
        staged: request.staged_evidence,
        reusable_evidence,
    })
}

fn prepare_stream(
    view: &View,
    request: &ReviewCommitRequest,
) -> Result<v3::ReviewStreamV3, ReviewCommitError> {
    let id = request.next.state.stream_id;
    let previous = view.index.streams.iter().find(|s| s.review_stream_id == id);
    if previous.and_then(|s| s.current_ref) != request.expected {
        return Err(ReviewCommitError::StaleSnapshot);
    }
    if request.next.state.project_id != view.index.project_id {
        return Err(ReviewCommitError::Integrity);
    }
    if let Some(previous) = previous {
        if mapping::scope(previous)? != request.production {
            return Err(ReviewCommitError::Integrity);
        }
        return Ok(previous.clone());
    }
    Ok(v3::ReviewStreamV3 {
        review_stream_id: id,
        task_id: request
            .production
            .as_ref()
            .map(|s| s.task_id.as_str().to_owned()),
        batch_id: request
            .production
            .as_ref()
            .map(|s| s.batch_id.as_str().to_owned()),
        current_ref: None,
        archive_refs: vec![],
        legacy_refs: vec![],
        usage_refs: vec![],
    })
}

fn validate_chain(
    view: &View,
    protocol: ReviewPublicationProtocol,
    record: &v3::ReviewStateRecord,
) -> Result<Option<history::VersionedReviewState>, ReviewCommitError> {
    if let Some(expected) = record.state.parent {
        let before =
            history::read_state_for_materialization(view, record.state.stream_id, &expected)?;
        if protocol == ReviewPublicationProtocol::V3
            && before.protocol == ReviewPublicationProtocol::V4
        {
            return Err(ReviewCommitError::Integrity);
        }
        diff_review(&before.state, &record.state, &record.changes)
            .map_err(|_| ReviewCommitError::Integrity)?;
        history::walk(view, record.state.stream_id, |_, historical| {
            validate_shared_identities(&record.state, &historical.state)
                .map_err(|_| ReviewCommitError::Integrity)?;
            references::evidence_identity(record, &historical)?;
            Ok(false)
        })?;
        return Ok(Some(before));
    }
    Ok(None)
}

fn add_usage_refs(
    view: &View,
    stream: &mut v3::ReviewStreamV3,
    usages: &[v3::ReviewUsageRecord],
) -> Result<(), ReviewCommitError> {
    for usage in usages {
        let encoded = v3::encode_usage_v1(usage).map_err(protocol_error)?;
        super::usage::validate(view, stream.review_stream_id, usage)?;
        let reference = v3::UsageRecordRef {
            declaration_id: usage.declaration_id,
            location: format!("usage/{}.json", usage.declaration_id),
            blake3: *blake3::hash(&encoded).as_bytes(),
        };
        if let Some(existing) = stream
            .usage_refs
            .iter()
            .find(|r| r.declaration_id == usage.declaration_id)
        {
            if existing != &reference {
                return Err(ReviewCommitError::Integrity);
            }
        } else {
            stream.usage_refs.push(reference);
        }
    }
    Ok(())
}

fn add_archive_refs(
    stream: &mut v3::ReviewStreamV3,
    checkpoints: &[ArchiveCheckpoint],
    result_snapshot_id: viewer_domain::ReviewSnapshotId,
    protocol: ReviewPublicationProtocol,
) -> Result<(), ReviewCommitError> {
    for checkpoint in checkpoints {
        let record = v3::ReviewArchiveRecord {
            checkpoint: checkpoint.clone(),
            result_snapshot_id,
        };
        let encoded = match protocol {
            ReviewPublicationProtocol::V3 => v3::encode_archive_v3(&record),
            ReviewPublicationProtocol::V4 => v4::encode_archive_v4(&record),
        }
        .map_err(protocol_error)?;
        stream.archive_refs.push(v3::ArchiveRecordRef {
            archive_id: checkpoint.archive_id,
            location: format!("archives/{}.json", checkpoint.archive_id),
            blake3: *blake3::hash(&encoded).as_bytes(),
        });
    }
    Ok(())
}
