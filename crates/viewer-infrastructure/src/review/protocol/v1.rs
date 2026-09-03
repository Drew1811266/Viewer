use super::common::{
    MAX_REVIEW_DOCUMENT_BYTES, MAX_REVIEW_INDEX_BYTES, ReviewProtocolError, decode_document,
    encode_digest, encode_document, parse_digest,
};
use super::{PRODUCTION_PROTOCOL_V1, REVIEW_PROTOCOL_V1};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::str::FromStr;
use viewer_application::{
    MAX_COMPLETED_ROUNDS_PER_STREAM, MAX_PRODUCTION_CONTEXT_ENTRIES,
    MAX_PRODUCTION_CONTEXT_KEY_BYTES, MAX_PRODUCTION_CONTEXT_VALUE_BYTES, MAX_REVIEW_STREAMS,
    ProductionAsset, ProductionManifest, ReviewCatalog, ReviewProtocolVersion,
    ReviewRecordLocation, ReviewRoundRecord, ReviewStreamHead,
};
use viewer_domain::RelativePath;
use viewer_domain::review::{
    AssetEvidence, AssetVersion, Feedback, FeedbackAnchor, FeedbackTarget, MAX_ASSETS_PER_ROUND,
    MAX_FEEDBACK_ITEMS_PER_ROUND, MAX_TARGETS_PER_FEEDBACK, NormalizedRect, ProductionId,
    ProductionScope, ReviewAssetKind, ReviewDraft, ReviewMedia, ReviewOutcome, ReviewOutcomeKind,
    ReviewRoundError, ReviewSnapshot, ReviewValueError, ReviewabilityFailure,
};

pub fn decode_production_manifest(bytes: &[u8]) -> Result<ProductionManifest, ReviewProtocolError> {
    let stored: StoredProductionManifest =
        decode_document(bytes, MAX_REVIEW_DOCUMENT_BYTES, PRODUCTION_PROTOCOL_V1)?;
    let StoredProductionManifest {
        _protocol_version: _,
        task_id,
        batch_id,
        context: stored_context,
        assets: stored_assets,
    } = stored;
    if stored_assets.is_empty() || stored_assets.len() > MAX_ASSETS_PER_ROUND {
        return Err(limit_or_invalid(stored_assets.is_empty()));
    }
    if stored_context.len() > MAX_PRODUCTION_CONTEXT_ENTRIES {
        return Err(ReviewProtocolError::LimitExceeded);
    }

    let production = ProductionScope {
        task_id: parse_production_id(&task_id)?,
        batch_id: parse_production_id(&batch_id)?,
    };
    let mut context = BTreeMap::new();
    for (key, value) in stored_context {
        if key.len() > MAX_PRODUCTION_CONTEXT_KEY_BYTES
            || value.len() > MAX_PRODUCTION_CONTEXT_VALUE_BYTES
        {
            return Err(ReviewProtocolError::LimitExceeded);
        }
        parse_production_id(&key)?;
        context.insert(key, value);
    }

    let mut producer_ids = HashSet::with_capacity(stored_assets.len());
    let mut paths = HashSet::with_capacity(stored_assets.len());
    let mut assets = Vec::with_capacity(stored_assets.len());
    for item in stored_assets {
        let producer_asset_id = parse_production_id(&item.producer_asset_id)?;
        let relative_path = parse_relative_path(&item.relative_path)?;
        if !producer_ids.insert(producer_asset_id.clone()) || !paths.insert(relative_path.clone()) {
            return Err(ReviewProtocolError::InvalidData);
        }
        let parent_producer_asset_id = item
            .parent_producer_asset_id
            .as_deref()
            .map(parse_production_id)
            .transpose()?;
        if parent_producer_asset_id.as_ref() == Some(&producer_asset_id) {
            return Err(ReviewProtocolError::InvalidData);
        }
        assets.push(ProductionAsset {
            producer_asset_id,
            relative_path,
            kind: item.kind.into(),
            parent_producer_asset_id,
            generation: item.generation,
        });
    }

    Ok(ProductionManifest {
        production,
        context,
        assets,
    })
}

pub fn encode_draft(draft: &ReviewDraft) -> Result<Vec<u8>, ReviewProtocolError> {
    let draft = validated_draft(draft)?;
    let stored = StoredDraft {
        protocol_version: REVIEW_PROTOCOL_V1.to_owned(),
        status: StoredDraftStatus::Draft,
        project_id: draft.project_id.to_string(),
        review_stream_id: draft.review_stream_id.to_string(),
        review_round_id: draft.review_round_id.to_string(),
        task_id: draft
            .production
            .as_ref()
            .map(|scope| scope.task_id.as_str().to_owned()),
        batch_id: draft
            .production
            .as_ref()
            .map(|scope| scope.batch_id.as_str().to_owned()),
        previous_completed_round_id: draft.previous_completed_round_id.map(|id| id.to_string()),
        created_at_ms: draft.created_at_ms,
        assets: draft.assets.iter().map(stored_asset).collect(),
        feedback: draft
            .feedback
            .iter()
            .map(stored_feedback)
            .collect::<Result<Vec<_>, _>>()?,
        unreviewable: draft
            .unreviewable
            .iter()
            .map(|item| StoredUnreviewable {
                asset_version_id: item.asset_version_id.to_string(),
                failure: item.failure.into(),
            })
            .collect(),
    };
    encode_document(&stored, MAX_REVIEW_DOCUMENT_BYTES)
}

pub fn decode_draft(bytes: &[u8]) -> Result<ReviewDraft, ReviewProtocolError> {
    let stored: StoredDraft =
        decode_document(bytes, MAX_REVIEW_DOCUMENT_BYTES, REVIEW_PROTOCOL_V1)?;
    build_draft(
        stored.project_id,
        stored.review_stream_id,
        stored.review_round_id,
        stored.task_id,
        stored.batch_id,
        stored.previous_completed_round_id,
        stored.created_at_ms,
        stored.assets,
        stored.feedback,
        stored.unreviewable,
    )
}

pub fn encode_completed(snapshot: &ReviewSnapshot) -> Result<Vec<u8>, ReviewProtocolError> {
    let validated = validated_snapshot(snapshot)?;
    let stored = StoredCompleted {
        protocol_version: REVIEW_PROTOCOL_V1.to_owned(),
        status: StoredCompletedStatus::Completed,
        project_id: validated.project_id.to_string(),
        review_stream_id: validated.review_stream_id.to_string(),
        review_round_id: validated.review_round_id.to_string(),
        task_id: validated
            .production
            .as_ref()
            .map(|scope| scope.task_id.as_str().to_owned()),
        batch_id: validated
            .production
            .as_ref()
            .map(|scope| scope.batch_id.as_str().to_owned()),
        previous_completed_round_id: validated
            .previous_completed_round_id
            .map(|id| id.to_string()),
        created_at_ms: validated.created_at_ms,
        completed_at_ms: validated.completed_at_ms,
        assets: validated.assets.iter().map(stored_asset).collect(),
        feedback: validated
            .feedback
            .iter()
            .map(stored_feedback)
            .collect::<Result<Vec<_>, _>>()?,
        outcomes: validated.outcomes.iter().map(stored_outcome).collect(),
    };
    encode_document(&stored, MAX_REVIEW_DOCUMENT_BYTES)
}

pub fn decode_completed(bytes: &[u8]) -> Result<ReviewSnapshot, ReviewProtocolError> {
    let stored: StoredCompleted =
        decode_document(bytes, MAX_REVIEW_DOCUMENT_BYTES, REVIEW_PROTOCOL_V1)?;
    let StoredCompleted {
        protocol_version: _,
        status: _,
        project_id,
        review_stream_id,
        review_round_id,
        task_id,
        batch_id,
        previous_completed_round_id,
        created_at_ms,
        completed_at_ms,
        assets,
        feedback,
        outcomes,
    } = stored;

    let parsed_outcomes = parse_outcomes(outcomes)?;
    let unreviewable = parsed_outcomes
        .iter()
        .filter_map(|outcome| {
            outcome.failure.map(|failure| StoredUnreviewable {
                asset_version_id: outcome.asset_version_id.to_string(),
                failure: failure.into(),
            })
        })
        .collect();
    let draft = build_draft(
        project_id,
        review_stream_id,
        review_round_id,
        task_id,
        batch_id,
        previous_completed_round_id,
        created_at_ms,
        assets,
        feedback,
        unreviewable,
    )?;
    let snapshot = draft.complete(completed_at_ms).map_err(map_round_error)?;
    if snapshot.outcomes != parsed_outcomes {
        return Err(ReviewProtocolError::InvalidData);
    }
    Ok(snapshot)
}

pub fn encode_catalog(catalog: &ReviewCatalog) -> Result<Vec<u8>, ReviewProtocolError> {
    let stored = stored_catalog(catalog)?;
    encode_document(&stored, MAX_REVIEW_INDEX_BYTES)
}

pub fn decode_catalog(bytes: &[u8]) -> Result<ReviewCatalog, ReviewProtocolError> {
    let stored: StoredCatalog = decode_document(bytes, MAX_REVIEW_INDEX_BYTES, REVIEW_PROTOCOL_V1)?;
    parse_catalog(stored)
}

#[allow(clippy::too_many_arguments)]
fn build_draft(
    project_id: String,
    review_stream_id: String,
    review_round_id: String,
    task_id: Option<String>,
    batch_id: Option<String>,
    previous_completed_round_id: Option<String>,
    created_at_ms: i64,
    assets: Vec<StoredAsset>,
    feedback: Vec<StoredFeedback>,
    unreviewable: Vec<StoredUnreviewable>,
) -> Result<ReviewDraft, ReviewProtocolError> {
    if feedback.len() > MAX_FEEDBACK_ITEMS_PER_ROUND || unreviewable.len() > MAX_ASSETS_PER_ROUND {
        return Err(ReviewProtocolError::LimitExceeded);
    }
    let project_id = parse_id(&project_id)?;
    let review_stream_id = parse_id(&review_stream_id)?;
    let review_round_id = parse_id(&review_round_id)?;
    let production = parse_production_scope(task_id, batch_id)?;
    let previous_completed_round_id = previous_completed_round_id
        .as_deref()
        .map(parse_id)
        .transpose()?;
    let assets = assets
        .into_iter()
        .map(parse_asset)
        .collect::<Result<Vec<_>, _>>()?;
    let mut draft = ReviewDraft::new(
        project_id,
        review_stream_id,
        review_round_id,
        production,
        previous_completed_round_id,
        created_at_ms,
        assets,
    )
    .map_err(map_round_error)?;

    let mut feedback_ids = HashSet::with_capacity(feedback.len());
    for stored in feedback {
        let feedback = parse_feedback(stored)?;
        if !feedback_ids.insert(feedback.id) {
            return Err(ReviewProtocolError::InvalidData);
        }
        draft.upsert_feedback(feedback).map_err(map_round_error)?;
    }
    let mut unreviewable_ids = HashSet::with_capacity(unreviewable.len());
    for stored in unreviewable {
        let asset_version_id = parse_id(&stored.asset_version_id)?;
        if !unreviewable_ids.insert(asset_version_id) {
            return Err(ReviewProtocolError::InvalidData);
        }
        draft
            .mark_unreviewable(asset_version_id, stored.failure.into())
            .map_err(map_round_error)?;
    }
    Ok(draft)
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
    let mut feedback_ids = HashSet::new();
    for feedback in &draft.feedback {
        if !feedback_ids.insert(feedback.id) {
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
    Ok(validated)
}

fn validated_snapshot(snapshot: &ReviewSnapshot) -> Result<ReviewSnapshot, ReviewProtocolError> {
    let unreviewable = snapshot
        .outcomes
        .iter()
        .filter_map(|outcome| {
            outcome.failure.map(|failure| StoredUnreviewable {
                asset_version_id: outcome.asset_version_id.to_string(),
                failure: failure.into(),
            })
        })
        .collect();
    let feedback = snapshot
        .feedback
        .iter()
        .map(stored_feedback)
        .collect::<Result<Vec<_>, _>>()?;
    let draft = build_draft(
        snapshot.project_id.to_string(),
        snapshot.review_stream_id.to_string(),
        snapshot.review_round_id.to_string(),
        snapshot
            .production
            .as_ref()
            .map(|scope| scope.task_id.as_str().to_owned()),
        snapshot
            .production
            .as_ref()
            .map(|scope| scope.batch_id.as_str().to_owned()),
        snapshot
            .previous_completed_round_id
            .map(|id| id.to_string()),
        snapshot.created_at_ms,
        snapshot.assets.iter().map(stored_asset).collect(),
        feedback,
        unreviewable,
    )?;
    let validated = draft
        .complete(snapshot.completed_at_ms)
        .map_err(map_round_error)?;
    if &validated != snapshot {
        return Err(ReviewProtocolError::InvalidData);
    }
    Ok(validated)
}

fn parse_outcomes(stored: Vec<StoredOutcome>) -> Result<Vec<ReviewOutcome>, ReviewProtocolError> {
    if stored.is_empty() || stored.len() > MAX_ASSETS_PER_ROUND {
        return Err(limit_or_invalid(stored.is_empty()));
    }
    let mut asset_ids = HashSet::with_capacity(stored.len());
    stored
        .into_iter()
        .map(|item| {
            let asset_version_id = parse_id(&item.asset_version_id)?;
            if !asset_ids.insert(asset_version_id) {
                return Err(ReviewProtocolError::InvalidData);
            }
            if item.feedback_ids.len() > MAX_FEEDBACK_ITEMS_PER_ROUND {
                return Err(ReviewProtocolError::LimitExceeded);
            }
            let feedback_ids = item
                .feedback_ids
                .iter()
                .map(|id| parse_id(id))
                .collect::<Result<Vec<_>, _>>()?;
            let unique_feedback = feedback_ids.iter().copied().collect::<HashSet<_>>();
            if unique_feedback.len() != feedback_ids.len() {
                return Err(ReviewProtocolError::InvalidData);
            }
            let failure = item.failure.map(Into::into);
            let kind = item.kind.into();
            let combination_is_valid = match kind {
                ReviewOutcomeKind::Pass => feedback_ids.is_empty() && failure.is_none(),
                ReviewOutcomeKind::Revise => !feedback_ids.is_empty() && failure.is_none(),
                ReviewOutcomeKind::Unreviewable => feedback_ids.is_empty() && failure.is_some(),
            };
            if !combination_is_valid {
                return Err(ReviewProtocolError::InvalidData);
            }
            Ok(ReviewOutcome {
                asset_version_id,
                kind,
                feedback_ids,
                failure,
            })
        })
        .collect()
}

fn parse_catalog(stored: StoredCatalog) -> Result<ReviewCatalog, ReviewProtocolError> {
    if stored.streams.len() > MAX_REVIEW_STREAMS {
        return Err(ReviewProtocolError::LimitExceeded);
    }
    let project_id = parse_id(&stored.project_id)?;
    let mut stream_ids = HashSet::with_capacity(stored.streams.len());
    let mut production_scopes = HashSet::new();
    let mut streams = Vec::with_capacity(stored.streams.len());
    for stream in stored.streams {
        let review_stream_id = parse_id(&stream.review_stream_id)?;
        if !stream_ids.insert(review_stream_id) {
            return Err(ReviewProtocolError::InvalidData);
        }
        let production = parse_production_scope(stream.task_id, stream.batch_id)?;
        if production
            .as_ref()
            .is_some_and(|scope| !production_scopes.insert(scope.clone()))
        {
            return Err(ReviewProtocolError::InvalidData);
        }
        if stream.completed_round_ids.len() > MAX_COMPLETED_ROUNDS_PER_STREAM {
            return Err(ReviewProtocolError::LimitExceeded);
        }
        let completed_round_ids = stream
            .completed_round_ids
            .iter()
            .map(|id| parse_id(id))
            .collect::<Result<Vec<_>, _>>()?;
        if completed_round_ids
            .iter()
            .copied()
            .collect::<HashSet<_>>()
            .len()
            != completed_round_ids.len()
        {
            return Err(ReviewProtocolError::InvalidData);
        }
        let latest_completed_round_id = stream
            .latest_completed_round_id
            .as_deref()
            .map(parse_id)
            .transpose()?;
        if completed_round_ids.last().copied() != latest_completed_round_id {
            return Err(ReviewProtocolError::InvalidData);
        }
        let completed_rounds = completed_round_ids
            .iter()
            .copied()
            .map(|review_round_id| {
                Ok(ReviewRoundRecord {
                    review_round_id,
                    protocol_version: ReviewProtocolVersion::V1,
                    location: legacy_round_location(review_round_id)?,
                    blake3: [0; 32],
                })
            })
            .collect::<Result<Vec<_>, ReviewProtocolError>>()?;
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

fn stored_catalog(catalog: &ReviewCatalog) -> Result<StoredCatalog, ReviewProtocolError> {
    for stream in &catalog.streams {
        for record in &stream.completed_rounds {
            if record.protocol_version != ReviewProtocolVersion::V1
                || record.location != legacy_round_location(record.review_round_id)?
            {
                return Err(ReviewProtocolError::InvalidData);
            }
        }
    }
    let candidate = StoredCatalog {
        protocol_version: REVIEW_PROTOCOL_V1.to_owned(),
        project_id: catalog.project_id.to_string(),
        streams: catalog
            .streams
            .iter()
            .map(|stream| StoredStream {
                review_stream_id: stream.review_stream_id.to_string(),
                task_id: stream
                    .production
                    .as_ref()
                    .map(|scope| scope.task_id.as_str().to_owned()),
                batch_id: stream
                    .production
                    .as_ref()
                    .map(|scope| scope.batch_id.as_str().to_owned()),
                completed_round_ids: stream
                    .completed_round_ids()
                    .map(|id| id.to_string())
                    .collect(),
                latest_completed_round_id: stream
                    .latest_completed_round_id
                    .map(|id| id.to_string()),
            })
            .collect(),
    };
    parse_catalog(candidate.clone())?;
    Ok(candidate)
}

fn legacy_round_location(
    round_id: viewer_domain::ReviewRoundId,
) -> Result<ReviewRecordLocation, ReviewProtocolError> {
    ReviewRecordLocation::new(format!("rounds/{round_id}.json"))
        .map_err(|_| ReviewProtocolError::InvalidData)
}

fn parse_asset(stored: StoredAsset) -> Result<AssetVersion, ReviewProtocolError> {
    let modified_ns = stored
        .evidence
        .modified_ns
        .parse::<i128>()
        .map_err(|_| ReviewProtocolError::InvalidData)?;
    if modified_ns.to_string() != stored.evidence.modified_ns {
        return Err(ReviewProtocolError::InvalidData);
    }
    let blake3 = stored
        .evidence
        .blake3
        .as_deref()
        .map(parse_digest)
        .transpose()?;
    let media = match stored.media {
        StoredMedia::Image { width, height } => ReviewMedia::Image { width, height },
        StoredMedia::Video {
            duration_us,
            display_width,
            display_height,
        } => ReviewMedia::Video {
            duration_us,
            display_width,
            display_height,
        },
    };
    Ok(AssetVersion {
        id: parse_id(&stored.asset_version_id)?,
        source_entity_id: stored
            .source_entity_id
            .as_deref()
            .map(parse_id)
            .transpose()?,
        relative_path: parse_relative_path(&stored.relative_path)?,
        evidence: AssetEvidence {
            size_bytes: stored.evidence.size_bytes,
            modified_ns,
            blake3,
        },
        media,
        producer_asset_id: stored
            .producer_asset_id
            .as_deref()
            .map(parse_production_id)
            .transpose()?,
        parent_asset_version_id: stored
            .parent_asset_version_id
            .as_deref()
            .map(parse_id)
            .transpose()?,
    })
}

fn stored_asset(asset: &AssetVersion) -> StoredAsset {
    let media = match asset.media {
        ReviewMedia::Image { width, height } => StoredMedia::Image { width, height },
        ReviewMedia::Video {
            duration_us,
            display_width,
            display_height,
        } => StoredMedia::Video {
            duration_us,
            display_width,
            display_height,
        },
    };
    StoredAsset {
        asset_version_id: asset.id.to_string(),
        source_entity_id: asset.source_entity_id.map(|id| id.to_string()),
        relative_path: asset.relative_path.as_str().to_owned(),
        evidence: StoredEvidence {
            size_bytes: asset.evidence.size_bytes,
            modified_ns: asset.evidence.modified_ns.to_string(),
            blake3: asset.evidence.blake3.map(encode_digest),
        },
        media,
        producer_asset_id: asset
            .producer_asset_id
            .as_ref()
            .map(|id| id.as_str().to_owned()),
        parent_asset_version_id: asset.parent_asset_version_id.map(|id| id.to_string()),
    }
}

fn parse_feedback(stored: StoredFeedback) -> Result<Feedback, ReviewProtocolError> {
    if stored.targets.len() > MAX_TARGETS_PER_FEEDBACK {
        return Err(ReviewProtocolError::LimitExceeded);
    }
    let targets = stored
        .targets
        .into_iter()
        .map(|target| {
            let anchor = match target.anchor {
                StoredAnchor::Asset => FeedbackAnchor::Asset,
                StoredAnchor::ImageRegion {
                    x,
                    y,
                    width,
                    height,
                } => FeedbackAnchor::ImageRect(
                    NormalizedRect::new(x, y, width, height).map_err(map_value_error)?,
                ),
                StoredAnchor::VideoPoint { position_us } => {
                    FeedbackAnchor::VideoPoint { position_us }
                }
                StoredAnchor::VideoRange { start_us, end_us } => {
                    FeedbackAnchor::VideoRange { start_us, end_us }
                }
            };
            Ok(FeedbackTarget {
                asset_version_id: parse_id(&target.asset_version_id)?,
                anchor,
            })
        })
        .collect::<Result<Vec<_>, ReviewProtocolError>>()?;
    Feedback::new(
        parse_id(&stored.feedback_id)?,
        stored.text,
        stored.created_at_ms,
        targets,
    )
    .map_err(map_value_error)
}

fn stored_feedback(feedback: &Feedback) -> Result<StoredFeedback, ReviewProtocolError> {
    Ok(StoredFeedback {
        feedback_id: feedback.id.to_string(),
        text: feedback.text.clone(),
        created_at_ms: feedback.created_at_ms,
        targets: feedback
            .targets
            .iter()
            .map(|target| {
                let anchor = match &target.anchor {
                    FeedbackAnchor::Asset => StoredAnchor::Asset,
                    FeedbackAnchor::ImageRect(rect) => StoredAnchor::ImageRegion {
                        x: rect.x(),
                        y: rect.y(),
                        width: rect.width(),
                        height: rect.height(),
                    },
                    FeedbackAnchor::ImagePoint(_)
                    | FeedbackAnchor::ImageArrow(_)
                    | FeedbackAnchor::ImageStroke(_)
                    | FeedbackAnchor::ImageEllipse(_) => {
                        return Err(ReviewProtocolError::InvalidData);
                    }
                    FeedbackAnchor::VideoPoint { position_us } => StoredAnchor::VideoPoint {
                        position_us: *position_us,
                    },
                    FeedbackAnchor::VideoRange { start_us, end_us } => StoredAnchor::VideoRange {
                        start_us: *start_us,
                        end_us: *end_us,
                    },
                };
                Ok(StoredTarget {
                    asset_version_id: target.asset_version_id.to_string(),
                    anchor,
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn stored_outcome(outcome: &ReviewOutcome) -> StoredOutcome {
    StoredOutcome {
        asset_version_id: outcome.asset_version_id.to_string(),
        kind: outcome.kind.into(),
        feedback_ids: outcome
            .feedback_ids
            .iter()
            .map(ToString::to_string)
            .collect(),
        failure: outcome.failure.map(Into::into),
    }
}

fn parse_production_scope(
    task_id: Option<String>,
    batch_id: Option<String>,
) -> Result<Option<ProductionScope>, ReviewProtocolError> {
    match (task_id, batch_id) {
        (None, None) => Ok(None),
        (Some(task_id), Some(batch_id)) => Ok(Some(ProductionScope {
            task_id: parse_production_id(&task_id)?,
            batch_id: parse_production_id(&batch_id)?,
        })),
        _ => Err(ReviewProtocolError::InvalidData),
    }
}

fn parse_relative_path(value: &str) -> Result<RelativePath, ReviewProtocolError> {
    RelativePath::parse(value).map_err(|_| ReviewProtocolError::InvalidData)
}

fn parse_production_id(value: &str) -> Result<ProductionId, ReviewProtocolError> {
    ProductionId::parse(value).map_err(map_value_error)
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

fn limit_or_invalid(is_invalid: bool) -> ReviewProtocolError {
    if is_invalid {
        ReviewProtocolError::InvalidData
    } else {
        ReviewProtocolError::LimitExceeded
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredProductionManifest {
    #[serde(rename = "protocolVersion")]
    _protocol_version: String,
    task_id: String,
    batch_id: String,
    context: BTreeMap<String, String>,
    assets: Vec<StoredProductionAsset>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredProductionAsset {
    producer_asset_id: String,
    relative_path: String,
    kind: StoredAssetKind,
    parent_producer_asset_id: Option<String>,
    generation: Option<u32>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum StoredAssetKind {
    Image,
    Video,
}

impl From<StoredAssetKind> for ReviewAssetKind {
    fn from(value: StoredAssetKind) -> Self {
        match value {
            StoredAssetKind::Image => Self::Image,
            StoredAssetKind::Video => Self::Video,
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredCatalog {
    protocol_version: String,
    project_id: String,
    streams: Vec<StoredStream>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredStream {
    review_stream_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    batch_id: Option<String>,
    completed_round_ids: Vec<String>,
    latest_completed_round_id: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredDraft {
    protocol_version: String,
    status: StoredDraftStatus,
    project_id: String,
    review_stream_id: String,
    review_round_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    batch_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_completed_round_id: Option<String>,
    created_at_ms: i64,
    assets: Vec<StoredAsset>,
    feedback: Vec<StoredFeedback>,
    unreviewable: Vec<StoredUnreviewable>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredCompleted {
    protocol_version: String,
    status: StoredCompletedStatus,
    project_id: String,
    review_stream_id: String,
    review_round_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    batch_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_completed_round_id: Option<String>,
    created_at_ms: i64,
    completed_at_ms: i64,
    assets: Vec<StoredAsset>,
    feedback: Vec<StoredFeedback>,
    outcomes: Vec<StoredOutcome>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum StoredDraftStatus {
    Draft,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum StoredCompletedStatus {
    Completed,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredAsset {
    asset_version_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_entity_id: Option<String>,
    relative_path: String,
    evidence: StoredEvidence,
    media: StoredMedia,
    #[serde(skip_serializing_if = "Option::is_none")]
    producer_asset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_asset_version_id: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredEvidence {
    size_bytes: u64,
    modified_ns: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    blake3: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum StoredMedia {
    Image {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        width: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        height: Option<u32>,
    },
    Video {
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_us: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        display_width: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        display_height: Option<u32>,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredFeedback {
    feedback_id: String,
    text: String,
    created_at_ms: i64,
    targets: Vec<StoredTarget>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredTarget {
    asset_version_id: String,
    anchor: StoredAnchor,
}

#[derive(Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum StoredAnchor {
    Asset,
    ImageRegion {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    VideoPoint {
        position_us: u64,
    },
    VideoRange {
        start_us: u64,
        end_us: u64,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredUnreviewable {
    asset_version_id: String,
    failure: StoredReviewabilityFailure,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredOutcome {
    asset_version_id: String,
    kind: StoredOutcomeKind,
    feedback_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure: Option<StoredReviewabilityFailure>,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum StoredReviewabilityFailure {
    Unsupported,
    Damaged,
    Unreadable,
    PermissionDenied,
    Missing,
    DecodeFailed,
}

impl From<StoredReviewabilityFailure> for ReviewabilityFailure {
    fn from(value: StoredReviewabilityFailure) -> Self {
        match value {
            StoredReviewabilityFailure::Unsupported => Self::Unsupported,
            StoredReviewabilityFailure::Damaged => Self::Damaged,
            StoredReviewabilityFailure::Unreadable => Self::Unreadable,
            StoredReviewabilityFailure::PermissionDenied => Self::PermissionDenied,
            StoredReviewabilityFailure::Missing => Self::Missing,
            StoredReviewabilityFailure::DecodeFailed => Self::DecodeFailed,
        }
    }
}

impl From<ReviewabilityFailure> for StoredReviewabilityFailure {
    fn from(value: ReviewabilityFailure) -> Self {
        match value {
            ReviewabilityFailure::Unsupported => Self::Unsupported,
            ReviewabilityFailure::Damaged => Self::Damaged,
            ReviewabilityFailure::Unreadable => Self::Unreadable,
            ReviewabilityFailure::PermissionDenied => Self::PermissionDenied,
            ReviewabilityFailure::Missing => Self::Missing,
            ReviewabilityFailure::DecodeFailed => Self::DecodeFailed,
        }
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum StoredOutcomeKind {
    Pass,
    Revise,
    Unreviewable,
}

impl From<StoredOutcomeKind> for ReviewOutcomeKind {
    fn from(value: StoredOutcomeKind) -> Self {
        match value {
            StoredOutcomeKind::Pass => Self::Pass,
            StoredOutcomeKind::Revise => Self::Revise,
            StoredOutcomeKind::Unreviewable => Self::Unreviewable,
        }
    }
}

impl From<ReviewOutcomeKind> for StoredOutcomeKind {
    fn from(value: ReviewOutcomeKind) -> Self {
        match value {
            ReviewOutcomeKind::Pass => Self::Pass,
            ReviewOutcomeKind::Revise => Self::Revise,
            ReviewOutcomeKind::Unreviewable => Self::Unreviewable,
        }
    }
}
