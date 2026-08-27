use super::common::{
    MAX_REVIEW_DOCUMENT_BYTES, MAX_REVIEW_INDEX_BYTES, ReviewProtocolError, decode_document,
    encode_digest, encode_document, parse_digest,
};
use super::{REVIEW_PROTOCOL_V1, REVIEW_PROTOCOL_V2, v1};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::str::FromStr;
use viewer_application::{
    MAX_COMPLETED_ROUNDS_PER_STREAM, MAX_REVIEW_ARTIFACT_PIXELS, MAX_REVIEW_STREAMS, ReviewCatalog,
    ReviewProtocolVersion, ReviewRecordLocation, ReviewRoundRecord, ReviewStreamHead,
};
use viewer_domain::review::{
    Feedback, FeedbackAnchor, FeedbackTarget, ImageStroke, NormalizedPoint, NormalizedRect,
    ProductionId, ProductionScope, ReviewDraft, ReviewRoundError, ReviewSnapshot, ReviewValueError,
};
use viewer_domain::{AssetVersionId, FeedbackId, ReviewRoundId, ReviewStreamId};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct V2CompletedDocument {
    pub snapshot: ReviewSnapshot,
    pub artifacts: Vec<V2ArtifactRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct V2ArtifactRecord {
    pub asset_version_id: AssetVersionId,
    pub relative_path: String,
    pub blake3: [u8; 32],
    pub media_type: String,
    pub width: u32,
    pub height: u32,
    pub annotations: Vec<V2ArtifactAnnotation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct V2ArtifactAnnotation {
    pub ordinal: u32,
    pub feedback_id: FeedbackId,
}

pub fn decode_catalog(bytes: &[u8]) -> Result<ReviewCatalog, ReviewProtocolError> {
    let stored: StoredCatalogV2 =
        decode_document(bytes, MAX_REVIEW_INDEX_BYTES, REVIEW_PROTOCOL_V2)?;
    parse_catalog(stored)
}

pub fn encode_catalog(catalog: &ReviewCatalog) -> Result<Vec<u8>, ReviewProtocolError> {
    let stored = stored_catalog(catalog)?;
    encode_document(&stored, MAX_REVIEW_INDEX_BYTES)
}

pub fn decode_draft(bytes: &[u8]) -> Result<ReviewDraft, ReviewProtocolError> {
    let value: Value = decode_document(bytes, MAX_REVIEW_DOCUMENT_BYTES, REVIEW_PROTOCOL_V2)?;
    let stored: StoredDraftV2 =
        serde_json::from_value(value.clone()).map_err(|_| ReviewProtocolError::InvalidData)?;
    let shell = v1_shell(value, &stored.feedback, false)?;
    let decoded = v1::decode_draft(&shell)?;
    rebuild_draft(decoded, stored.feedback)
}

pub fn encode_draft(draft: &ReviewDraft) -> Result<Vec<u8>, ReviewProtocolError> {
    let validated = validated_draft(draft)?;
    let shell = v1_safe_draft(&validated)?;
    let mut value: Value = serde_json::from_slice(&v1::encode_draft(&shell)?)
        .map_err(|_| ReviewProtocolError::InvalidData)?;
    set_protocol(&mut value, REVIEW_PROTOCOL_V2)?;
    set_feedback(&mut value, &validated.feedback)?;
    encode_document(&value, MAX_REVIEW_DOCUMENT_BYTES)
}

pub fn decode_completed(bytes: &[u8]) -> Result<ReviewSnapshot, ReviewProtocolError> {
    Ok(decode_completed_document(bytes)?.snapshot)
}

pub(crate) fn decode_completed_document(
    bytes: &[u8],
) -> Result<V2CompletedDocument, ReviewProtocolError> {
    let value: Value = decode_document(bytes, MAX_REVIEW_DOCUMENT_BYTES, REVIEW_PROTOCOL_V2)?;
    let stored: StoredCompletedV2 =
        serde_json::from_value(value.clone()).map_err(|_| ReviewProtocolError::InvalidData)?;
    let shell = v1_shell(value, &stored.feedback, true)?;
    let decoded = v1::decode_completed(&shell)?;
    let snapshot = rebuild_snapshot(decoded, stored.feedback)?;
    validate_artifacts(&stored.artifacts, &snapshot)?;
    let artifacts = stored
        .artifacts
        .into_iter()
        .map(V2ArtifactRecord::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(V2CompletedDocument {
        snapshot,
        artifacts,
    })
}

pub fn encode_completed(snapshot: &ReviewSnapshot) -> Result<Vec<u8>, ReviewProtocolError> {
    encode_completed_with_artifacts(snapshot, &[])
}

pub(crate) fn encode_completed_with_artifacts(
    snapshot: &ReviewSnapshot,
    artifacts: &[V2ArtifactRecord],
) -> Result<Vec<u8>, ReviewProtocolError> {
    let validated = validated_snapshot(snapshot)?;
    let shell = v1_safe_snapshot(&validated)?;
    let mut value: Value = serde_json::from_slice(&v1::encode_completed(&shell)?)
        .map_err(|_| ReviewProtocolError::InvalidData)?;
    set_protocol(&mut value, REVIEW_PROTOCOL_V2)?;
    set_feedback(&mut value, &validated.feedback)?;
    let stored_artifacts = artifacts
        .iter()
        .map(StoredArtifactV2::from_record)
        .collect::<Vec<_>>();
    validate_artifacts(&stored_artifacts, &validated)?;
    value
        .as_object_mut()
        .ok_or(ReviewProtocolError::InvalidData)?
        .insert(
            "artifacts".to_owned(),
            serde_json::to_value(stored_artifacts).map_err(|_| ReviewProtocolError::InvalidData)?,
        );
    encode_document(&value, MAX_REVIEW_DOCUMENT_BYTES)
}

fn v1_shell(
    mut value: Value,
    feedback: &[StoredFeedbackV2],
    completed: bool,
) -> Result<Vec<u8>, ReviewProtocolError> {
    set_protocol(&mut value, REVIEW_PROTOCOL_V1)?;
    let object = value
        .as_object_mut()
        .ok_or(ReviewProtocolError::InvalidData)?;
    if completed {
        object.remove("artifacts");
    }
    object.insert(
        "feedback".to_owned(),
        Value::Array(feedback.iter().map(v1_feedback_shell).collect()),
    );
    serde_json::to_vec(&value).map_err(|_| ReviewProtocolError::InvalidData)
}

fn set_protocol(value: &mut Value, protocol: &str) -> Result<(), ReviewProtocolError> {
    value
        .as_object_mut()
        .ok_or(ReviewProtocolError::InvalidData)?
        .insert(
            "protocolVersion".to_owned(),
            Value::String(protocol.to_owned()),
        );
    Ok(())
}

fn set_feedback(value: &mut Value, feedback: &[Feedback]) -> Result<(), ReviewProtocolError> {
    let stored = feedback
        .iter()
        .map(StoredFeedbackV2::from_domain)
        .collect::<Result<Vec<_>, _>>()?;
    value
        .as_object_mut()
        .ok_or(ReviewProtocolError::InvalidData)?
        .insert(
            "feedback".to_owned(),
            serde_json::to_value(stored).map_err(|_| ReviewProtocolError::InvalidData)?,
        );
    Ok(())
}

fn v1_feedback_shell(feedback: &StoredFeedbackV2) -> Value {
    let mut asset_ids = HashSet::new();
    let targets = feedback
        .targets
        .iter()
        .filter(|target| asset_ids.insert(target.asset_version_id.as_str()))
        .map(|target| {
            json!({
                "assetVersionId": target.asset_version_id,
                "anchor": { "kind": "asset" }
            })
        })
        .collect::<Vec<_>>();
    json!({
        "feedbackId": feedback.feedback_id,
        "text": feedback.text,
        "createdAtMs": feedback.created_at_ms,
        "targets": targets,
    })
}

fn rebuild_draft(
    decoded: ReviewDraft,
    feedback: Vec<StoredFeedbackV2>,
) -> Result<ReviewDraft, ReviewProtocolError> {
    let mut rebuilt = ReviewDraft::new(
        decoded.project_id,
        decoded.review_stream_id,
        decoded.review_round_id,
        decoded.production,
        decoded.previous_completed_round_id,
        decoded.created_at_ms,
        decoded.assets,
    )
    .map_err(map_round_error)?;
    for stored in feedback {
        rebuilt
            .upsert_feedback(stored.into_domain()?)
            .map_err(map_round_error)?;
    }
    for item in decoded.unreviewable {
        rebuilt
            .mark_unreviewable(item.asset_version_id, item.failure)
            .map_err(map_round_error)?;
    }
    Ok(rebuilt)
}

fn rebuild_snapshot(
    decoded: ReviewSnapshot,
    feedback: Vec<StoredFeedbackV2>,
) -> Result<ReviewSnapshot, ReviewProtocolError> {
    let expected_outcomes = decoded.outcomes.clone();
    let completed_at_ms = decoded.completed_at_ms;
    let mut draft = ReviewDraft::new(
        decoded.project_id,
        decoded.review_stream_id,
        decoded.review_round_id,
        decoded.production,
        decoded.previous_completed_round_id,
        decoded.created_at_ms,
        decoded.assets,
    )
    .map_err(map_round_error)?;
    for stored in feedback {
        draft
            .upsert_feedback(stored.into_domain()?)
            .map_err(map_round_error)?;
    }
    for outcome in &expected_outcomes {
        if let Some(failure) = outcome.failure {
            draft
                .mark_unreviewable(outcome.asset_version_id, failure)
                .map_err(map_round_error)?;
        }
    }
    let rebuilt = draft.complete(completed_at_ms).map_err(map_round_error)?;
    if rebuilt.outcomes != expected_outcomes {
        return Err(ReviewProtocolError::InvalidData);
    }
    Ok(rebuilt)
}

fn validated_draft(draft: &ReviewDraft) -> Result<ReviewDraft, ReviewProtocolError> {
    let mut validated = ReviewDraft::new(
        draft.project_id,
        draft.review_stream_id,
        draft.review_round_id,
        draft.production.clone(),
        draft.previous_completed_round_id,
        draft.created_at_ms,
        draft.assets.clone(),
    )
    .map_err(map_round_error)?;
    let mut ids = HashSet::new();
    for feedback in &draft.feedback {
        if !ids.insert(feedback.id) {
            return Err(ReviewProtocolError::InvalidData);
        }
        validated
            .upsert_feedback(feedback.clone())
            .map_err(map_round_error)?;
    }
    let mut unreviewable_ids = HashSet::new();
    for item in &draft.unreviewable {
        if !unreviewable_ids.insert(item.asset_version_id) {
            return Err(ReviewProtocolError::InvalidData);
        }
        validated
            .mark_unreviewable(item.asset_version_id, item.failure)
            .map_err(map_round_error)?;
    }
    if &validated != draft {
        return Err(ReviewProtocolError::InvalidData);
    }
    Ok(validated)
}

fn validated_snapshot(snapshot: &ReviewSnapshot) -> Result<ReviewSnapshot, ReviewProtocolError> {
    let mut draft = ReviewDraft::new(
        snapshot.project_id,
        snapshot.review_stream_id,
        snapshot.review_round_id,
        snapshot.production.clone(),
        snapshot.previous_completed_round_id,
        snapshot.created_at_ms,
        snapshot.assets.clone(),
    )
    .map_err(map_round_error)?;
    let mut ids = HashSet::new();
    for feedback in &snapshot.feedback {
        if !ids.insert(feedback.id) {
            return Err(ReviewProtocolError::InvalidData);
        }
        draft
            .upsert_feedback(feedback.clone())
            .map_err(map_round_error)?;
    }
    for outcome in &snapshot.outcomes {
        if let Some(failure) = outcome.failure {
            draft
                .mark_unreviewable(outcome.asset_version_id, failure)
                .map_err(map_round_error)?;
        }
    }
    let validated = draft
        .complete(snapshot.completed_at_ms)
        .map_err(map_round_error)?;
    if &validated != snapshot {
        return Err(ReviewProtocolError::InvalidData);
    }
    Ok(validated)
}

fn v1_safe_draft(draft: &ReviewDraft) -> Result<ReviewDraft, ReviewProtocolError> {
    let mut safe = draft.clone();
    safe.feedback = draft
        .feedback
        .iter()
        .map(v1_safe_feedback)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(safe)
}

fn v1_safe_snapshot(snapshot: &ReviewSnapshot) -> Result<ReviewSnapshot, ReviewProtocolError> {
    let mut safe = snapshot.clone();
    safe.feedback = snapshot
        .feedback
        .iter()
        .map(v1_safe_feedback)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(safe)
}

fn v1_safe_feedback(feedback: &Feedback) -> Result<Feedback, ReviewProtocolError> {
    let mut asset_ids = HashSet::new();
    let targets = feedback
        .targets
        .iter()
        .filter(|target| asset_ids.insert(target.asset_version_id))
        .map(|target| FeedbackTarget {
            asset_version_id: target.asset_version_id,
            anchor: FeedbackAnchor::Asset,
        })
        .collect();
    Feedback::new(
        feedback.id,
        feedback.text.clone(),
        feedback.created_at_ms,
        targets,
    )
    .map_err(map_value_error)
}

fn validate_artifacts(
    artifacts: &[StoredArtifactV2],
    snapshot: &ReviewSnapshot,
) -> Result<(), ReviewProtocolError> {
    let mut asset_ids = HashSet::new();
    for artifact in artifacts {
        let asset_id: AssetVersionId = parse_id(&artifact.asset_version_id)?;
        if !asset_ids.insert(asset_id)
            || artifact.relative_path != format!("artifacts/{asset_id}-annotation.png")
            || artifact.media_type != "image/png"
            || artifact.width == 0
            || artifact.height == 0
            || u64::from(artifact.width) * u64::from(artifact.height) > MAX_REVIEW_ARTIFACT_PIXELS
        {
            return Err(ReviewProtocolError::InvalidData);
        }
        parse_digest(&artifact.blake3)?;
        if !snapshot.assets.iter().any(|asset| asset.id == asset_id) {
            return Err(ReviewProtocolError::InvalidData);
        }
        let mut expected = snapshot
            .feedback
            .iter()
            .filter(|feedback| {
                feedback.targets.iter().any(|target| {
                    target.asset_version_id == asset_id
                        && matches!(
                            target.anchor,
                            FeedbackAnchor::ImageRect(_) | FeedbackAnchor::ImageStroke(_)
                        )
                })
            })
            .collect::<Vec<_>>();
        expected.sort_by_key(|feedback| (feedback.created_at_ms, feedback.id.to_string()));
        if expected.is_empty() {
            return Err(ReviewProtocolError::InvalidData);
        }
        let mut ordinals = HashSet::new();
        let mut feedback_ids = HashSet::new();
        for annotation in &artifact.annotations {
            let feedback_id = parse_id(&annotation.feedback_id)?;
            if annotation.ordinal == 0
                || !ordinals.insert(annotation.ordinal)
                || !feedback_ids.insert(feedback_id)
                || expected
                    .get(annotation.ordinal.saturating_sub(1) as usize)
                    .map(|feedback| feedback.id)
                    != Some(feedback_id)
            {
                return Err(ReviewProtocolError::InvalidData);
            }
        }
        if feedback_ids.len() != expected.len()
            || (1..=artifact.annotations.len() as u32).any(|ordinal| !ordinals.contains(&ordinal))
        {
            return Err(ReviewProtocolError::InvalidData);
        }
    }
    let expected_assets = snapshot
        .feedback
        .iter()
        .flat_map(|feedback| &feedback.targets)
        .filter(|target| {
            matches!(
                target.anchor,
                FeedbackAnchor::ImageRect(_) | FeedbackAnchor::ImageStroke(_)
            )
        })
        .map(|target| target.asset_version_id)
        .collect::<HashSet<_>>();
    if asset_ids != expected_assets {
        return Err(ReviewProtocolError::InvalidData);
    }
    Ok(())
}

fn parse_catalog(stored: StoredCatalogV2) -> Result<ReviewCatalog, ReviewProtocolError> {
    if stored.streams.len() > MAX_REVIEW_STREAMS {
        return Err(ReviewProtocolError::LimitExceeded);
    }
    let project_id = parse_id(&stored.project_id)?;
    let mut stream_ids = HashSet::with_capacity(stored.streams.len());
    let mut production_scopes = HashSet::new();
    let mut round_ids = HashSet::new();
    let mut streams = Vec::with_capacity(stored.streams.len());
    for stream in stored.streams {
        let review_stream_id: ReviewStreamId = parse_id(&stream.review_stream_id)?;
        if !stream_ids.insert(review_stream_id) {
            return Err(ReviewProtocolError::InvalidData);
        }
        let production = parse_catalog_production(stream.task_id, stream.batch_id)?;
        if production
            .as_ref()
            .is_some_and(|scope| !production_scopes.insert(scope.clone()))
        {
            return Err(ReviewProtocolError::InvalidData);
        }
        if stream.completed_rounds.len() > MAX_COMPLETED_ROUNDS_PER_STREAM {
            return Err(ReviewProtocolError::LimitExceeded);
        }
        let completed_rounds = stream
            .completed_rounds
            .into_iter()
            .map(|record| {
                let review_round_id: ReviewRoundId = parse_id(&record.review_round_id)?;
                if !round_ids.insert(review_round_id) {
                    return Err(ReviewProtocolError::InvalidData);
                }
                let protocol_version = parse_record_protocol(&record.protocol_version)?;
                let location = ReviewRecordLocation::new(record.location)
                    .map_err(|_| ReviewProtocolError::InvalidData)?;
                if location.as_str() != canonical_record_location(review_round_id, protocol_version)
                {
                    return Err(ReviewProtocolError::InvalidData);
                }
                Ok(ReviewRoundRecord {
                    review_round_id,
                    protocol_version,
                    location,
                    blake3: parse_digest(&record.blake3)?,
                })
            })
            .collect::<Result<Vec<_>, ReviewProtocolError>>()?;
        let latest_completed_round_id = stream
            .latest_completed_round_id
            .as_deref()
            .map(parse_id)
            .transpose()?;
        if completed_rounds.last().map(|record| record.review_round_id) != latest_completed_round_id
        {
            return Err(ReviewProtocolError::InvalidData);
        }
        streams.push(ReviewStreamHead {
            review_stream_id,
            production,
            completed_rounds,
            latest_completed_round_id,
        });
    }
    Ok(ReviewCatalog {
        project_id,
        streams,
    })
}

fn stored_catalog(catalog: &ReviewCatalog) -> Result<StoredCatalogV2, ReviewProtocolError> {
    let candidate = StoredCatalogV2 {
        project_id: catalog.project_id.to_string(),
        protocol_version: REVIEW_PROTOCOL_V2.to_owned(),
        streams: catalog
            .streams
            .iter()
            .map(|stream| StoredStreamV2 {
                batch_id: stream
                    .production
                    .as_ref()
                    .map(|scope| scope.batch_id.as_str().to_owned()),
                completed_rounds: stream
                    .completed_rounds
                    .iter()
                    .map(|record| StoredRoundRecordV2 {
                        blake3: encode_digest(record.blake3),
                        location: record.location.as_str().to_owned(),
                        protocol_version: record_protocol(record.protocol_version).to_owned(),
                        review_round_id: record.review_round_id.to_string(),
                    })
                    .collect(),
                latest_completed_round_id: stream
                    .latest_completed_round_id
                    .map(|id| id.to_string()),
                review_stream_id: stream.review_stream_id.to_string(),
                task_id: stream
                    .production
                    .as_ref()
                    .map(|scope| scope.task_id.as_str().to_owned()),
            })
            .collect(),
    };
    parse_catalog(candidate.clone())?;
    Ok(candidate)
}

fn parse_catalog_production(
    task_id: Option<String>,
    batch_id: Option<String>,
) -> Result<Option<ProductionScope>, ReviewProtocolError> {
    match (task_id, batch_id) {
        (None, None) => Ok(None),
        (Some(task_id), Some(batch_id)) => Ok(Some(ProductionScope {
            task_id: ProductionId::parse(&task_id).map_err(map_value_error)?,
            batch_id: ProductionId::parse(&batch_id).map_err(map_value_error)?,
        })),
        _ => Err(ReviewProtocolError::InvalidData),
    }
}

fn parse_record_protocol(value: &str) -> Result<ReviewProtocolVersion, ReviewProtocolError> {
    match value {
        REVIEW_PROTOCOL_V1 => Ok(ReviewProtocolVersion::V1),
        REVIEW_PROTOCOL_V2 => Ok(ReviewProtocolVersion::V2),
        _ => Err(ReviewProtocolError::UnsupportedVersion),
    }
}

fn record_protocol(value: ReviewProtocolVersion) -> &'static str {
    match value {
        ReviewProtocolVersion::V1 => REVIEW_PROTOCOL_V1,
        ReviewProtocolVersion::V2 => REVIEW_PROTOCOL_V2,
    }
}

fn canonical_record_location(
    round_id: ReviewRoundId,
    protocol_version: ReviewProtocolVersion,
) -> String {
    match protocol_version {
        ReviewProtocolVersion::V1 => format!("rounds/{round_id}.json"),
        ReviewProtocolVersion::V2 => format!("rounds/{round_id}/round.json"),
    }
}

fn parse_id<T>(value: &str) -> Result<T, ReviewProtocolError>
where
    T: FromStr,
{
    T::from_str(value).map_err(|_| ReviewProtocolError::InvalidData)
}

fn map_value_error(error: ReviewValueError) -> ReviewProtocolError {
    match error {
        ReviewValueError::LimitExceeded => ReviewProtocolError::LimitExceeded,
        _ => ReviewProtocolError::InvalidData,
    }
}

fn map_round_error(error: ReviewRoundError) -> ReviewProtocolError {
    match error {
        ReviewRoundError::LimitExceeded => ReviewProtocolError::LimitExceeded,
        _ => ReviewProtocolError::InvalidData,
    }
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredDraftV2 {
    protocol_version: String,
    status: StoredDraftStatus,
    project_id: String,
    review_stream_id: String,
    review_round_id: String,
    task_id: Option<String>,
    batch_id: Option<String>,
    previous_completed_round_id: Option<String>,
    created_at_ms: i64,
    assets: Vec<Value>,
    feedback: Vec<StoredFeedbackV2>,
    unreviewable: Vec<Value>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredCompletedV2 {
    protocol_version: String,
    status: StoredCompletedStatus,
    project_id: String,
    review_stream_id: String,
    review_round_id: String,
    task_id: Option<String>,
    batch_id: Option<String>,
    previous_completed_round_id: Option<String>,
    created_at_ms: i64,
    completed_at_ms: i64,
    assets: Vec<Value>,
    feedback: Vec<StoredFeedbackV2>,
    outcomes: Vec<Value>,
    artifacts: Vec<StoredArtifactV2>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum StoredDraftStatus {
    Draft,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum StoredCompletedStatus {
    Completed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredFeedbackV2 {
    feedback_id: String,
    text: String,
    created_at_ms: i64,
    targets: Vec<StoredTargetV2>,
}

impl StoredFeedbackV2 {
    fn from_domain(feedback: &Feedback) -> Result<Self, ReviewProtocolError> {
        Ok(Self {
            feedback_id: feedback.id.to_string(),
            text: feedback.text.clone(),
            created_at_ms: feedback.created_at_ms,
            targets: feedback
                .targets
                .iter()
                .map(StoredTargetV2::from_domain)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    fn into_domain(self) -> Result<Feedback, ReviewProtocolError> {
        Feedback::new(
            parse_id(&self.feedback_id)?,
            self.text,
            self.created_at_ms,
            self.targets
                .into_iter()
                .map(StoredTargetV2::into_domain)
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(map_value_error)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredTargetV2 {
    asset_version_id: String,
    anchor: StoredAnchorV2,
}

impl StoredTargetV2 {
    fn from_domain(target: &FeedbackTarget) -> Result<Self, ReviewProtocolError> {
        Ok(Self {
            asset_version_id: target.asset_version_id.to_string(),
            anchor: StoredAnchorV2::from_domain(&target.anchor),
        })
    }

    fn into_domain(self) -> Result<FeedbackTarget, ReviewProtocolError> {
        Ok(FeedbackTarget {
            asset_version_id: parse_id(&self.asset_version_id)?,
            anchor: self.anchor.into_domain()?,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum StoredAnchorV2 {
    Asset,
    ImageRect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    ImageStroke {
        points: Vec<StoredPointV2>,
    },
    VideoPoint {
        position_us: u64,
    },
    VideoRange {
        start_us: u64,
        end_us: u64,
    },
}

impl StoredAnchorV2 {
    fn from_domain(anchor: &FeedbackAnchor) -> Self {
        match anchor {
            FeedbackAnchor::Asset => Self::Asset,
            FeedbackAnchor::ImageRect(rect) => Self::ImageRect {
                x: rect.x(),
                y: rect.y(),
                width: rect.width(),
                height: rect.height(),
            },
            FeedbackAnchor::ImageStroke(stroke) => Self::ImageStroke {
                points: stroke
                    .points()
                    .iter()
                    .map(|point| StoredPointV2 {
                        x: point.x(),
                        y: point.y(),
                    })
                    .collect(),
            },
            FeedbackAnchor::VideoPoint { position_us } => Self::VideoPoint {
                position_us: *position_us,
            },
            FeedbackAnchor::VideoRange { start_us, end_us } => Self::VideoRange {
                start_us: *start_us,
                end_us: *end_us,
            },
        }
    }

    fn into_domain(self) -> Result<FeedbackAnchor, ReviewProtocolError> {
        match self {
            Self::Asset => Ok(FeedbackAnchor::Asset),
            Self::ImageRect {
                x,
                y,
                width,
                height,
            } => Ok(FeedbackAnchor::ImageRect(
                NormalizedRect::new(x, y, width, height).map_err(map_value_error)?,
            )),
            Self::ImageStroke { points } => Ok(FeedbackAnchor::ImageStroke(
                ImageStroke::new(
                    points
                        .into_iter()
                        .map(|point| {
                            NormalizedPoint::new(point.x, point.y).map_err(map_value_error)
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                )
                .map_err(map_value_error)?,
            )),
            Self::VideoPoint { position_us } => Ok(FeedbackAnchor::VideoPoint { position_us }),
            Self::VideoRange { start_us, end_us } => {
                Ok(FeedbackAnchor::VideoRange { start_us, end_us })
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredPointV2 {
    x: f64,
    y: f64,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredArtifactV2 {
    asset_version_id: String,
    relative_path: String,
    blake3: String,
    media_type: String,
    width: u32,
    height: u32,
    annotations: Vec<StoredArtifactAnnotationV2>,
}

impl StoredArtifactV2 {
    fn from_record(record: &V2ArtifactRecord) -> Self {
        Self {
            asset_version_id: record.asset_version_id.to_string(),
            relative_path: record.relative_path.clone(),
            blake3: encode_digest(record.blake3),
            media_type: record.media_type.clone(),
            width: record.width,
            height: record.height,
            annotations: record
                .annotations
                .iter()
                .map(|annotation| StoredArtifactAnnotationV2 {
                    ordinal: annotation.ordinal,
                    feedback_id: annotation.feedback_id.to_string(),
                })
                .collect(),
        }
    }
}

impl TryFrom<StoredArtifactV2> for V2ArtifactRecord {
    type Error = ReviewProtocolError;

    fn try_from(stored: StoredArtifactV2) -> Result<Self, Self::Error> {
        Ok(Self {
            asset_version_id: parse_id(&stored.asset_version_id)?,
            relative_path: stored.relative_path,
            blake3: parse_digest(&stored.blake3)?,
            media_type: stored.media_type,
            width: stored.width,
            height: stored.height,
            annotations: stored
                .annotations
                .into_iter()
                .map(|annotation| {
                    Ok(V2ArtifactAnnotation {
                        ordinal: annotation.ordinal,
                        feedback_id: parse_id(&annotation.feedback_id)?,
                    })
                })
                .collect::<Result<Vec<_>, ReviewProtocolError>>()?,
        })
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredArtifactAnnotationV2 {
    ordinal: u32,
    feedback_id: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredCatalogV2 {
    project_id: String,
    protocol_version: String,
    streams: Vec<StoredStreamV2>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredStreamV2 {
    #[serde(skip_serializing_if = "Option::is_none")]
    batch_id: Option<String>,
    completed_rounds: Vec<StoredRoundRecordV2>,
    latest_completed_round_id: Option<String>,
    review_stream_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_id: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredRoundRecordV2 {
    blake3: String,
    location: String,
    protocol_version: String,
    review_round_id: String,
}
