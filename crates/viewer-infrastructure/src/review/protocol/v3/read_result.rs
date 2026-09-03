use super::super::common::ReviewProtocolError;
pub use super::history_result::*;
use super::{EvidenceBinding, ReviewStateRecord, validate, wire};
use crate::review::protocol::ContinuousReviewProtocol;
use serde::{Deserialize, Serialize};
use viewer_domain::review::AssetVersion;
use viewer_domain::review::continuous::{
    ContinuousReviewState, HistoryRef, SnapshotRef, SourceCheck, SourceCheckStatus, TargetDelta,
    VersionedFeedback, project_current,
};
use viewer_domain::{
    AssetVersionId, ProjectId, ReviewCommandId, ReviewSnapshotId, ReviewStreamId, ReviewTargetId,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ReviewReadResult {
    Current(CurrentReadResult),
    History(HistoryReadResult),
    NoReviewState(NoReviewStateResult),
    Error(ReadErrorResult),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CurrentReadResult {
    protocol_version: ContinuousReviewProtocol,
    status: OkStatus,
    role: CurrentRole,
    #[serde(deserialize_with = "wire::canonical_id")]
    pub project_id: ProjectId,
    #[serde(deserialize_with = "wire::canonical_id")]
    pub review_stream_id: ReviewStreamId,
    #[serde(with = "wire::snapshot")]
    pub snapshot_ref: SnapshotRef,
    #[serde(with = "wire::assets")]
    pub assets: Vec<AssetVersion>,
    #[serde(with = "wire::feedback")]
    pub feedback: Vec<VersionedFeedback>,
    pub evidence: Vec<EvidenceBinding>,
    pub source_checks: Vec<ReadSourceCheck>,
    #[serde(deserialize_with = "wire::id_list")]
    pub actionable: Vec<ReviewTargetId>,
    #[serde(deserialize_with = "wire::id_list")]
    pub needs_confirmation: Vec<ReviewTargetId>,
    #[serde(with = "wire::histories")]
    pub history_refs: Vec<HistoryRef>,
    pub delta: ReadDelta,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NoReviewStateResult {
    protocol_version: ContinuousReviewProtocol,
    status: NoStateStatus,
    role: CurrentRole,
    #[serde(deserialize_with = "wire::canonical_id")]
    pub project_id: ProjectId,
    #[serde(deserialize_with = "wire::optional_id")]
    pub review_stream_id: Option<ReviewStreamId>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadErrorResult {
    protocol_version: ContinuousReviewProtocol,
    status: ErrorStatus,
    pub code: ReadErrorCode,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum OkStatus {
    #[serde(rename = "ok")]
    Ok,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum NoStateStatus {
    #[serde(rename = "no_review_state")]
    NoState,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum ErrorStatus {
    #[serde(rename = "error")]
    Error,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum CurrentRole {
    #[serde(rename = "current")]
    Current,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) enum HistoryRole {
    #[serde(rename = "history")]
    History,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadErrorCode {
    PublicationPending,
    MigrationRequired,
    UnsupportedVersion,
    Integrity,
    UnsafePath,
    LimitExceeded,
    AmbiguousStream,
    UnknownStream,
    UnknownHistory,
    Io,
}
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HistoryLimitation {
    LegacyEvidenceAbsent,
    LegacyUsageUnknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadSourceCheck {
    #[serde(deserialize_with = "wire::canonical_id")]
    pub asset_version_id: AssetVersionId,
    pub checked_at_ms: i64,
    pub status: ReadSourceStatus,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReadSourceStatus {
    Match,
    Changed,
    Missing,
    Unreadable,
    Unverified,
}
impl From<&ReadSourceCheck> for SourceCheck {
    fn from(v: &ReadSourceCheck) -> Self {
        Self {
            asset_version_id: v.asset_version_id,
            checked_at_ms: v.checked_at_ms,
            status: match v.status {
                ReadSourceStatus::Match => SourceCheckStatus::Match,
                ReadSourceStatus::Changed => SourceCheckStatus::Changed,
                ReadSourceStatus::Missing => SourceCheckStatus::Missing,
                ReadSourceStatus::Unreadable => SourceCheckStatus::Unreadable,
                ReadSourceStatus::Unverified => SourceCheckStatus::Unverified,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ReadDelta {
    NotRequested {},
    Unavailable {
        reason: DeltaUnavailableReason,
    },
    Available {
        #[serde(deserialize_with = "wire::canonical_id")]
        since_snapshot_id: ReviewSnapshotId,
        #[serde(deserialize_with = "wire::canonical_id")]
        current_snapshot_id: ReviewSnapshotId,
        #[serde(with = "wire::deltas")]
        targets: Vec<TargetDelta>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeltaUnavailableReason {
    UnknownSnapshot,
    WrongContext,
    LimitExceeded,
    Integrity,
}

impl ReviewReadResult {
    pub(super) fn set_protocol(&mut self, protocol: ContinuousReviewProtocol) {
        match self {
            Self::Current(value) => value.protocol_version = protocol,
            Self::History(value) => value.set_protocol(protocol),
            Self::NoReviewState(value) => value.protocol_version = protocol,
            Self::Error(value) => value.protocol_version = protocol,
        }
    }

    pub(in crate::review) fn validate(&self) -> Result<(), ReviewProtocolError> {
        use ReviewProtocolError::*;
        let (project_id, stream_id, reference, assets, feedback, evidence) = match self {
            Self::Current(v) => (
                v.project_id,
                v.review_stream_id,
                v.snapshot_ref,
                &v.assets,
                &v.feedback,
                &v.evidence,
            ),
            Self::History(v) => return v.validate(),
            Self::NoReviewState(_) => return Ok(()),
            Self::Error(v) => {
                return if v.message.trim().is_empty() {
                    Err(InvalidData)
                } else if v.message.len() > 4096 {
                    Err(LimitExceeded)
                } else {
                    Ok(())
                };
            }
        };
        let state = ContinuousReviewState {
            project_id,
            stream_id,
            snapshot_id: reference.snapshot_id,
            parent: None,
            assets: assets.clone(),
            feedback: feedback.clone(),
        };
        validate::state(&ReviewStateRecord {
            state: state.clone(),
            command_id: ReviewCommandId::from_u128(0),
            payload_digest: [0; 32],
            changes: vec![],
            evidence: evidence.clone(),
        })?;
        match self {
            Self::Current(v) => {
                if v.source_checks
                    .iter()
                    .any(|check| check.checked_at_ms as u64 > validate::MAX_SAFE_INTEGER)
                {
                    return Err(LimitExceeded);
                }
                if v.history_refs.len() > 10_000 {
                    return Err(LimitExceeded);
                }
                let projected = project_current(
                    &state,
                    &v.source_checks.iter().map(Into::into).collect::<Vec<_>>(),
                )
                .map_err(validate::domain_error)?;
                if projected.actionable != v.actionable
                    || projected.needs_confirmation != v.needs_confirmation
                {
                    return Err(InvalidData);
                }
                let expected: Vec<_> = feedback
                    .iter()
                    .filter_map(|f| f.history_ref.clone())
                    .collect();
                let mut unique = Vec::new();
                for item in expected {
                    if !unique.contains(&item) {
                        unique.push(item);
                    }
                }
                if v.history_refs != unique {
                    return Err(InvalidData);
                }
                if let ReadDelta::Available {
                    current_snapshot_id,
                    targets,
                    ..
                } = &v.delta
                {
                    if *current_snapshot_id != reference.snapshot_id {
                        return Err(InvalidData);
                    }
                    if targets.len() > 100_000_000 {
                        return Err(LimitExceeded);
                    }
                    let mut seen = std::collections::HashSet::new();
                    for delta in targets {
                        if !seen.insert(delta.target_id)
                            || delta.after != state.target_key(delta.target_id)
                        {
                            return Err(InvalidData);
                        }
                        if [delta.before, delta.after]
                            .iter()
                            .flatten()
                            .any(|key| key.target_id != delta.target_id)
                        {
                            return Err(InvalidData);
                        }
                        if let (Some(before), Some(after)) = (delta.before, delta.after)
                            && before.feedback_id != after.feedback_id
                        {
                            return Err(InvalidData);
                        }
                        let removed = delta.before.is_some() && delta.after.is_none();
                        if removed != delta.removal_reason.is_some() || delta.removal_reason.is_some_and(|k| !matches!(k, viewer_domain::review::continuous::ReviewChangeKind::Withdrawn | viewer_domain::review::continuous::ReviewChangeKind::Archived)) { return Err(InvalidData); }
                    }
                }
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    pub(super) fn validate_for(
        &self,
        protocol: ContinuousReviewProtocol,
    ) -> Result<(), ReviewProtocolError> {
        self.validate()?;
        match self {
            Self::Current(value) => validate::feedback_for(protocol, &value.feedback),
            Self::History(value) => value.validate_anchors_for(protocol),
            Self::NoReviewState(_) | Self::Error(_) => Ok(()),
        }
    }
}

impl CurrentReadResult {
    pub(in crate::review) fn from_verified(
        snapshot_ref: SnapshotRef,
        record: ReviewStateRecord,
        source_checks: Vec<ReadSourceCheck>,
        delta: ReadDelta,
    ) -> Result<Self, ReviewProtocolError> {
        let projected = project_current(
            &record.state,
            &source_checks.iter().map(Into::into).collect::<Vec<_>>(),
        )
        .map_err(validate::domain_error)?;
        let mut history_refs = vec![];
        for reference in record
            .state
            .feedback
            .iter()
            .filter_map(|f| f.history_ref.as_ref())
        {
            if !history_refs.contains(reference) {
                history_refs.push(reference.clone());
            }
        }
        Ok(Self {
            protocol_version: ContinuousReviewProtocol::V3,
            status: OkStatus::Ok,
            role: CurrentRole::Current,
            project_id: record.state.project_id,
            review_stream_id: record.state.stream_id,
            snapshot_ref,
            assets: record.state.assets,
            feedback: record.state.feedback,
            evidence: record.evidence,
            source_checks,
            actionable: projected.actionable,
            needs_confirmation: projected.needs_confirmation,
            history_refs,
            delta,
        })
    }
}
impl NoReviewStateResult {
    pub(in crate::review) fn new(
        project_id: ProjectId,
        review_stream_id: Option<ReviewStreamId>,
    ) -> Self {
        Self {
            protocol_version: ContinuousReviewProtocol::V3,
            status: NoStateStatus::NoState,
            role: CurrentRole::Current,
            project_id,
            review_stream_id,
        }
    }
}
