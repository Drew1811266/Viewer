use super::{
    Failure, ReadErrorCode,
    project::Project,
    request::{Operation, Request},
};
use crate::review::{MAX_REVIEW_DOCUMENT_BYTES, continuous::legacy, protocol};
use serde_json::{Value, json};
use viewer_application::{ReviewCatalogError, ReviewProtocolVersion};

pub(super) fn read(project: &Project, request: &Request) -> Result<Value, Failure> {
    let (directory, index_bytes) = project.required()?;
    let catalog = protocol::decode_catalog_versioned(index_bytes)?;
    if request.operation == Operation::LegacyList {
        return Ok(Value::Array(catalog.value.streams.iter().map(|stream| {
            let mut value = json!({"reviewStreamId": stream.review_stream_id, "latestCompletedRoundId": stream.latest_completed_round_id});
            if let Some(scope) = &stream.production {
                value["taskId"] = json!(scope.task_id.as_str());
                value["batchId"] = json!(scope.batch_id.as_str());
            }
            value
        }).collect()));
    }
    let locator = request.locator()?;
    let stream = catalog.value.resolve_stream(locator.as_ref()).map_err(|e| match e {
        ReviewCatalogError::NotFound => Failure::new(ReadErrorCode::UnknownStream, "review stream was not found"),
        ReviewCatalogError::Ambiguous => Failure::new(ReadErrorCode::AmbiguousStream, if request.task_id.is_some() { "multiple review streams match task and batch" } else { "multiple review streams; provide --stream or --task and --batch; use --list to inspect streams" }),
    })?;
    let round = stream.latest_completed_round_id.ok_or_else(|| {
        Failure::new(
            ReadErrorCode::UnknownHistory,
            "selected review stream has no completed review head",
        )
    })?;
    let record = stream
        .completed_rounds
        .iter()
        .find(|r| r.review_round_id == round)
        .ok_or_else(|| Failure::integrity("selected review stream head is not indexed"))?;
    let rounds = directory.required_child("rounds")?;
    let (bundle, name) = match record.protocol_version {
        ReviewProtocolVersion::V1 => (rounds, format!("{round}.json")),
        ReviewProtocolVersion::V2 => (
            rounds.required_child(&round.to_string())?,
            "round.json".into(),
        ),
    };
    let bytes = bundle
        .read(&name, MAX_REVIEW_DOCUMENT_BYTES)?
        .ok_or_else(|| Failure::integrity("completed round is missing"))?;
    if catalog.version == ReviewProtocolVersion::V2
        && blake3::hash(&bytes).as_bytes() != &record.blake3
    {
        return Err(Failure::integrity(
            "completed round digest does not match the review index",
        ));
    }
    let decoded = protocol::decode_completed_versioned(&bytes)?;
    if decoded.version != record.protocol_version {
        return Err(Failure::integrity(
            "completed round protocol does not match index",
        ));
    }
    let snapshot = decoded.value;
    if snapshot.project_id != catalog.value.project_id {
        return Err(Failure::integrity(
            "completed round project identity does not match index",
        ));
    }
    if snapshot.review_stream_id != stream.review_stream_id {
        return Err(Failure::integrity(
            "completed round stream identity does not match index",
        ));
    }
    if snapshot.review_round_id != round {
        return Err(Failure::integrity(
            "completed round identity does not match index",
        ));
    }
    if snapshot.production != stream.production {
        return Err(Failure::integrity(
            "completed round production identity does not match index",
        ));
    }
    if decoded.version == ReviewProtocolVersion::V2 {
        let document = protocol::v2::decode_completed_document(&bytes)?;
        legacy::verify_artifacts(&bundle, &document.artifacts)?;
    }
    project.root.verify()?;
    serde_json::from_slice(&bytes)
        .map_err(|_| Failure::integrity("completed round is not valid JSON"))
}
