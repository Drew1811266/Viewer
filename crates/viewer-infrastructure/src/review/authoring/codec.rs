use crate::review::{ReviewProtocolError, v3};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fmt::Display, str::FromStr};
use viewer_application::review_workspace::{
    GeneratedReviewIds, MigrationFeedbackIds, ReviewBarrierKind, ReviewCommitError,
    StoredAuthoringState,
};
use viewer_domain::{
    FeedbackId, ProjectId, ReviewRoundId, ReviewSnapshotId, ReviewStreamId, ReviewTargetId,
    ReviewTargetRevisionId, ReviewTextRevisionId,
    review::{ProductionId, ProductionScope},
};

const TRANSITION_PROTOCOL_V1: &str = "viewer.review.authoring.transition/1";
pub(super) const MAX_AUTHORING_STATE_BYTES: u64 = 16 * 1024 * 1024;
pub(super) const MAX_AUTHORING_TRANSITION_BYTES: u64 = 64 * 1024;

pub(super) struct EncodedAuthoringState {
    pub logical_state: Vec<u8>,
    pub transition: Vec<u8>,
    pub generated_ids: Vec<u8>,
    pub production_scope: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AuthoringRecordV1 {
    protocol_version: AuthoringProtocol,
    project_id: String,
    review_stream_id: String,
    sequence: u64,
    snapshot_id: String,
    production: Option<ProductionWire>,
    state: serde_json::Value,
    generated: GeneratedIdsWire,
    archives: Vec<serde_json::Value>,
    adopted_usage: Vec<serde_json::Value>,
    barrier_kind: BarrierWire,
}

#[derive(Serialize, Deserialize)]
enum AuthoringProtocol {
    #[serde(rename = "viewer.review.authoring/1")]
    V1,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProductionWire {
    task_id: String,
    batch_id: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GeneratedIdsWire {
    snapshot_id: String,
    feedback_id: String,
    text_revision_id: String,
    archive_id: String,
    targets: Vec<GeneratedTargetWire>,
    migration: Vec<MigrationFeedbackWire>,
    created_at_ms: i64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GeneratedTargetWire {
    target_id: String,
    target_revision_id: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MigrationFeedbackWire {
    round_id: String,
    legacy_feedback_id: String,
    feedback_id: String,
    text_revision_id: String,
    targets: Vec<MigrationTargetWire>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MigrationTargetWire {
    legacy_index: u32,
    target_id: String,
    target_revision_id: String,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum BarrierWire {
    None,
    Archive,
    Restore,
    Migration,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TransitionRecordV1<'a> {
    protocol_version: &'static str,
    command_id: String,
    payload_digest: String,
    generated: &'a GeneratedIdsWire,
    changes: &'a serde_json::Value,
    archives: &'a [serde_json::Value],
    adopted_usage: &'a [serde_json::Value],
    barrier_kind: BarrierWire,
}

pub(super) fn encode(
    project_id: ProjectId,
    stream_id: ReviewStreamId,
    value: &StoredAuthoringState,
) -> Result<EncodedAuthoringState, ReviewCommitError> {
    validate_context(project_id, stream_id, value)?;
    let state_bytes = v3::encode_authoring_state(&v3::ReviewStateRecord {
        state: value.state.clone(),
        command_id: value.command_id,
        payload_digest: value.payload_digest,
        changes: value.changes.clone(),
        evidence: vec![],
    })
    .map_err(protocol_error)?;
    let state: serde_json::Value =
        serde_json::from_slice(&state_bytes).map_err(|_| ReviewCommitError::Integrity)?;
    let generated = GeneratedIdsWire::from(&value.generated);
    validate_generated(&value.generated)?;
    let archives = value
        .archives
        .iter()
        .map(|checkpoint| {
            v3::encode_archive_v3(&v3::ReviewArchiveRecord {
                checkpoint: checkpoint.clone(),
                result_snapshot_id: value.head.snapshot_id,
            })
            .map_err(protocol_error)
            .and_then(json_value)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let adopted_usage = value
        .adopted_usage
        .iter()
        .cloned()
        .map(v3::ReviewUsageRecord::from)
        .map(|record| {
            v3::encode_usage_v1(&record)
                .map_err(protocol_error)
                .and_then(json_value)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let barrier_kind = value.barrier.into();
    let record = AuthoringRecordV1 {
        protocol_version: AuthoringProtocol::V1,
        project_id: project_id.to_string(),
        review_stream_id: stream_id.to_string(),
        sequence: value.head.sequence,
        snapshot_id: value.head.snapshot_id.to_string(),
        production: value.production.as_ref().map(ProductionWire::from),
        state,
        generated,
        archives,
        adopted_usage,
        barrier_kind,
    };
    let changes = record
        .state
        .get("changes")
        .ok_or(ReviewCommitError::Integrity)?;
    let transition = encode_bounded(
        &TransitionRecordV1 {
            protocol_version: TRANSITION_PROTOCOL_V1,
            command_id: value.command_id.to_string(),
            payload_digest: encode_digest(value.payload_digest),
            generated: &record.generated,
            changes,
            archives: &record.archives,
            adopted_usage: &record.adopted_usage,
            barrier_kind,
        },
        MAX_AUTHORING_TRANSITION_BYTES,
    )?;
    let generated_ids = encode_bounded(&record.generated, MAX_AUTHORING_TRANSITION_BYTES)?;
    let production_scope = encode_bounded(&record.production, 1024)?;
    let logical_state = encode_bounded(&record, MAX_AUTHORING_STATE_BYTES)?;
    Ok(EncodedAuthoringState {
        logical_state,
        transition,
        generated_ids,
        production_scope,
    })
}

pub(super) fn encode_production_scope(
    value: Option<&ProductionScope>,
) -> Result<Vec<u8>, ReviewCommitError> {
    encode_bounded(&value.map(ProductionWire::from), 1024)
}

pub(super) fn decode(
    expected_project: ProjectId,
    expected_stream: ReviewStreamId,
    bytes: &[u8],
) -> Result<StoredAuthoringState, ReviewCommitError> {
    if bytes.len() as u64 > MAX_AUTHORING_STATE_BYTES {
        return Err(ReviewCommitError::LimitExceeded);
    }
    let record: AuthoringRecordV1 =
        serde_json::from_slice(bytes).map_err(|_| ReviewCommitError::Integrity)?;
    let project_id: ProjectId = parse_id(&record.project_id)?;
    let stream_id: ReviewStreamId = parse_id(&record.review_stream_id)?;
    let snapshot_id: ReviewSnapshotId = parse_id(&record.snapshot_id)?;
    if project_id != expected_project || stream_id != expected_stream || record.sequence == 0 {
        return Err(ReviewCommitError::Integrity);
    }
    let state_record = v3::decode_authoring_state(
        &serde_json::to_vec(&record.state).map_err(|_| ReviewCommitError::Integrity)?,
    )
    .map_err(protocol_error)?;
    let generated = record.generated.try_into_domain()?;
    validate_generated(&generated)?;
    let archive_records = record
        .archives
        .into_iter()
        .map(|value| {
            v3::decode_archive_v3(
                &serde_json::to_vec(&value).map_err(|_| ReviewCommitError::Integrity)?,
            )
            .map_err(protocol_error)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if archive_records
        .iter()
        .any(|archive| archive.result_snapshot_id != snapshot_id)
    {
        return Err(ReviewCommitError::Integrity);
    }
    let adopted_usage = record
        .adopted_usage
        .into_iter()
        .map(|value| {
            v3::decode_usage_v1(
                &serde_json::to_vec(&value).map_err(|_| ReviewCommitError::Integrity)?,
            )
            .map(Into::into)
            .map_err(protocol_error)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let value = StoredAuthoringState {
        head: viewer_application::review_workspace::ReviewAuthoringHead {
            sequence: record.sequence,
            snapshot_id,
        },
        production: record
            .production
            .map(ProductionWire::try_into_domain)
            .transpose()?,
        state: state_record.state,
        command_id: state_record.command_id,
        payload_digest: state_record.payload_digest,
        generated,
        changes: state_record.changes,
        archives: archive_records
            .into_iter()
            .map(|record| record.checkpoint)
            .collect(),
        adopted_usage,
        barrier: record.barrier_kind.into(),
    };
    validate_context(expected_project, expected_stream, &value)?;
    Ok(value)
}

fn validate_context(
    project_id: ProjectId,
    stream_id: ReviewStreamId,
    value: &StoredAuthoringState,
) -> Result<(), ReviewCommitError> {
    if value.head.sequence == 0
        || value.head.snapshot_id != value.state.snapshot_id
        || value.head.snapshot_id != value.generated.snapshot_id
        || value.state.project_id != project_id
        || value.state.stream_id != stream_id
        || value.generated.created_at_ms < 0
    {
        return Err(ReviewCommitError::Integrity);
    }
    if value.archives.iter().any(|archive| {
        archive.project_id != project_id
            || archive.stream_id != stream_id
            || archive.created_at_ms < 0
    }) || value
        .adopted_usage
        .iter()
        .any(|usage| usage.project_id != project_id || usage.stream_id != stream_id)
    {
        return Err(ReviewCommitError::Integrity);
    }
    unique(value.archives.iter().map(|value| value.archive_id))?;
    unique(value.adopted_usage.iter().map(|value| value.id))?;
    Ok(())
}

fn validate_generated(value: &GeneratedReviewIds) -> Result<(), ReviewCommitError> {
    unique(value.targets.iter().map(|value| value.0))?;
    unique(value.targets.iter().map(|value| value.1))?;
    unique(value.migration.iter().map(|value| value.feedback_id))?;
    unique(value.migration.iter().map(|value| value.text_revision_id))?;
    for migration in &value.migration {
        unique(migration.targets.iter().map(|value| value.0))?;
        unique(migration.targets.iter().map(|value| value.1))?;
        unique(migration.targets.iter().map(|value| value.2))?;
    }
    Ok(())
}

fn unique<T: Eq + std::hash::Hash>(
    values: impl Iterator<Item = T>,
) -> Result<(), ReviewCommitError> {
    let mut found = HashSet::new();
    if values.into_iter().all(|value| found.insert(value)) {
        Ok(())
    } else {
        Err(ReviewCommitError::Integrity)
    }
}

fn encode_bounded<T: Serialize>(value: &T, limit: u64) -> Result<Vec<u8>, ReviewCommitError> {
    v3::encode_bounded_json(value, limit).map_err(protocol_error)
}

fn json_value(bytes: Vec<u8>) -> Result<serde_json::Value, ReviewCommitError> {
    serde_json::from_slice(&bytes).map_err(|_| ReviewCommitError::Integrity)
}

fn protocol_error(error: ReviewProtocolError) -> ReviewCommitError {
    match error {
        ReviewProtocolError::LimitExceeded => ReviewCommitError::LimitExceeded,
        _ => ReviewCommitError::Integrity,
    }
}

fn parse_id<T>(value: &str) -> Result<T, ReviewCommitError>
where
    T: FromStr + Display,
{
    if value.len() != 36 {
        return Err(ReviewCommitError::Integrity);
    }
    let parsed: T = value.parse().map_err(|_| ReviewCommitError::Integrity)?;
    if parsed.to_string() != value {
        return Err(ReviewCommitError::Integrity);
    }
    Ok(parsed)
}

fn encode_digest(value: [u8; 32]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

impl From<&ProductionScope> for ProductionWire {
    fn from(value: &ProductionScope) -> Self {
        Self {
            task_id: value.task_id.as_str().to_owned(),
            batch_id: value.batch_id.as_str().to_owned(),
        }
    }
}

impl ProductionWire {
    fn try_into_domain(self) -> Result<ProductionScope, ReviewCommitError> {
        Ok(ProductionScope {
            task_id: ProductionId::parse(&self.task_id)
                .map_err(|_| ReviewCommitError::Integrity)?,
            batch_id: ProductionId::parse(&self.batch_id)
                .map_err(|_| ReviewCommitError::Integrity)?,
        })
    }
}

impl From<&GeneratedReviewIds> for GeneratedIdsWire {
    fn from(value: &GeneratedReviewIds) -> Self {
        Self {
            snapshot_id: value.snapshot_id.to_string(),
            feedback_id: value.feedback_id.to_string(),
            text_revision_id: value.text_revision_id.to_string(),
            archive_id: value.archive_id.to_string(),
            targets: value
                .targets
                .iter()
                .map(|(target_id, target_revision_id)| GeneratedTargetWire {
                    target_id: target_id.to_string(),
                    target_revision_id: target_revision_id.to_string(),
                })
                .collect(),
            migration: value
                .migration
                .iter()
                .map(MigrationFeedbackWire::from)
                .collect(),
            created_at_ms: value.created_at_ms,
        }
    }
}

impl GeneratedIdsWire {
    fn try_into_domain(self) -> Result<GeneratedReviewIds, ReviewCommitError> {
        Ok(GeneratedReviewIds {
            snapshot_id: parse_id(&self.snapshot_id)?,
            feedback_id: parse_id(&self.feedback_id)?,
            text_revision_id: parse_id(&self.text_revision_id)?,
            archive_id: parse_id(&self.archive_id)?,
            targets: self
                .targets
                .into_iter()
                .map(|value| {
                    Ok((
                        parse_id(&value.target_id)?,
                        parse_id(&value.target_revision_id)?,
                    ))
                })
                .collect::<Result<_, ReviewCommitError>>()?,
            migration: self
                .migration
                .into_iter()
                .map(MigrationFeedbackWire::try_into_domain)
                .collect::<Result<_, _>>()?,
            created_at_ms: self.created_at_ms,
        })
    }
}

impl From<&MigrationFeedbackIds> for MigrationFeedbackWire {
    fn from(value: &MigrationFeedbackIds) -> Self {
        Self {
            round_id: value.round_id.to_string(),
            legacy_feedback_id: value.legacy_feedback_id.to_string(),
            feedback_id: value.feedback_id.to_string(),
            text_revision_id: value.text_revision_id.to_string(),
            targets: value
                .targets
                .iter()
                .map(
                    |(legacy_index, target_id, target_revision_id)| MigrationTargetWire {
                        legacy_index: *legacy_index,
                        target_id: target_id.to_string(),
                        target_revision_id: target_revision_id.to_string(),
                    },
                )
                .collect(),
        }
    }
}

impl MigrationFeedbackWire {
    fn try_into_domain(self) -> Result<MigrationFeedbackIds, ReviewCommitError> {
        Ok(MigrationFeedbackIds {
            round_id: parse_id::<ReviewRoundId>(&self.round_id)?,
            legacy_feedback_id: parse_id::<FeedbackId>(&self.legacy_feedback_id)?,
            feedback_id: parse_id::<FeedbackId>(&self.feedback_id)?,
            text_revision_id: parse_id::<ReviewTextRevisionId>(&self.text_revision_id)?,
            targets: self
                .targets
                .into_iter()
                .map(|value| {
                    Ok((
                        value.legacy_index,
                        parse_id::<ReviewTargetId>(&value.target_id)?,
                        parse_id::<ReviewTargetRevisionId>(&value.target_revision_id)?,
                    ))
                })
                .collect::<Result<_, ReviewCommitError>>()?,
        })
    }
}

impl From<ReviewBarrierKind> for BarrierWire {
    fn from(value: ReviewBarrierKind) -> Self {
        match value {
            ReviewBarrierKind::None => Self::None,
            ReviewBarrierKind::Archive => Self::Archive,
            ReviewBarrierKind::Restore => Self::Restore,
            ReviewBarrierKind::Migration => Self::Migration,
        }
    }
}

impl From<BarrierWire> for ReviewBarrierKind {
    fn from(value: BarrierWire) -> Self {
        match value {
            BarrierWire::None => Self::None,
            BarrierWire::Archive => Self::Archive,
            BarrierWire::Restore => Self::Restore,
            BarrierWire::Migration => Self::Migration,
        }
    }
}
