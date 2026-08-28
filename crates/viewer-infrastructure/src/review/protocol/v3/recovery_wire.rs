//! Viewer-only recovery data; deliberately not part of the public current/history protocol.
use super::{
    MAX_REVIEW_DOCUMENT_BYTES, ReviewProtocolError, bounded::encode_document, decode_document, wire,
};
use serde::{Deserialize, Serialize};
use viewer_application::review_workspace::{
    RecoveryDraft, RecoveryEditorInput, ReviewRecoveryFailure,
};
use viewer_domain::review::continuous::{
    HistoryRef, HistorySource, ReviewAvailability, VersionedTarget,
};
use viewer_domain::review::{
    FeedbackAnchor, MAX_FEEDBACK_TEXT_BYTES, MAX_IMAGE_STROKE_POINTS_PER_ROUND,
    MAX_TARGETS_PER_FEEDBACK,
};
use viewer_domain::{FeedbackId, ProjectId, ReviewCommandId, ReviewSnapshotId, ReviewStreamId};

const PROTOCOL: &str = "viewer.review.recovery/1";
#[derive(Serialize, Deserialize)]
enum Protocol {
    #[serde(rename = "viewer.review.recovery/1")]
    V1,
}
#[derive(Serialize, Deserialize)]
enum Kind {
    #[serde(rename = "recovery")]
    Recovery,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Recovery {
    protocol_version: Protocol,
    kind: Kind,
    #[serde(deserialize_with = "wire::canonical_id")]
    project_id: ProjectId,
    #[serde(deserialize_with = "wire::canonical_id")]
    review_stream_id: ReviewStreamId,
    #[serde(deserialize_with = "wire::canonical_id")]
    command_id: ReviewCommandId,
    #[serde(deserialize_with = "wire::optional_id")]
    expected_snapshot_id: Option<ReviewSnapshotId>,
    #[serde(with = "wire::digest")]
    payload_digest: [u8; 32],
    editor_input: Input,
    failure: Failure,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    #[serde(default, with = "super::recovery_migration")]
    migration: Option<viewer_application::review_workspace::MigrationPlan>,
    #[serde(default, with = "super::recovery_selection")]
    selections: Vec<viewer_application::review_workspace::RecoveryTargetSelection>,
    text: String,
    #[serde(deserialize_with = "wire::optional_id")]
    feedback_id: Option<FeedbackId>,
    #[serde(with = "wire::targets")]
    targets: Vec<VersionedTarget>,
    #[serde(with = "wire::optional_history")]
    history_ref: Option<HistoryRef>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Failure {
    RenderFailed,
    SourceChanged,
    WriteFailed,
    CommitUnknown,
    Cancelled,
    StaleSnapshot,
}
impl From<ReviewRecoveryFailure> for Failure {
    fn from(value: ReviewRecoveryFailure) -> Self {
        match value {
            ReviewRecoveryFailure::RenderFailed => Self::RenderFailed,
            ReviewRecoveryFailure::SourceChanged => Self::SourceChanged,
            ReviewRecoveryFailure::WriteFailed => Self::WriteFailed,
            ReviewRecoveryFailure::CommitUnknown => Self::CommitUnknown,
            ReviewRecoveryFailure::Cancelled => Self::Cancelled,
            ReviewRecoveryFailure::StaleSnapshot => Self::StaleSnapshot,
        }
    }
}
impl From<Failure> for ReviewRecoveryFailure {
    fn from(value: Failure) -> Self {
        match value {
            Failure::RenderFailed => Self::RenderFailed,
            Failure::SourceChanged => Self::SourceChanged,
            Failure::WriteFailed => Self::WriteFailed,
            Failure::CommitUnknown => Self::CommitUnknown,
            Failure::Cancelled => Self::Cancelled,
            Failure::StaleSnapshot => Self::StaleSnapshot,
        }
    }
}

pub(in crate::review) fn encode(
    project_id: ProjectId,
    draft: &RecoveryDraft,
) -> Result<Vec<u8>, ReviewProtocolError> {
    validate(project_id, draft)?;
    let input = &draft.editor_input;
    let wire = Recovery {
        protocol_version: Protocol::V1,
        kind: Kind::Recovery,
        project_id,
        review_stream_id: draft.stream_id,
        command_id: draft.command_id,
        expected_snapshot_id: draft.expected_snapshot_id,
        payload_digest: draft.payload_digest,
        editor_input: Input {
            migration: input.migration.clone(),
            selections: input.selections.clone(),
            text: input.text.clone(),
            feedback_id: input.feedback_id,
            targets: input.targets.clone(),
            history_ref: input.history_ref.clone(),
        },
        failure: draft.failure.into(),
    };
    encode_document(&wire, MAX_REVIEW_DOCUMENT_BYTES)
}

pub(in crate::review) fn decode(
    project_id: ProjectId,
    bytes: &[u8],
) -> Result<RecoveryDraft, ReviewProtocolError> {
    let wire: Recovery = decode_document(bytes, MAX_REVIEW_DOCUMENT_BYTES, PROTOCOL)?;
    if wire.project_id != project_id {
        return Err(ReviewProtocolError::InvalidData);
    }
    let draft = RecoveryDraft {
        stream_id: wire.review_stream_id,
        command_id: wire.command_id,
        expected_snapshot_id: wire.expected_snapshot_id,
        payload_digest: wire.payload_digest,
        editor_input: RecoveryEditorInput {
            migration: wire.editor_input.migration,
            selections: wire.editor_input.selections,
            text: wire.editor_input.text,
            feedback_id: wire.editor_input.feedback_id,
            targets: wire.editor_input.targets,
            history_ref: wire.editor_input.history_ref,
        },
        failure: wire.failure.into(),
    };
    validate(project_id, &draft)?;
    Ok(draft)
}

fn validate(project: ProjectId, draft: &RecoveryDraft) -> Result<(), ReviewProtocolError> {
    use std::collections::HashSet;
    let input = &draft.editor_input;
    super::recovery_migration::validate(input.migration.as_ref())?;
    super::recovery_selection::validate(input)?;
    if input.migration.is_some()
        && (draft.expected_snapshot_id.is_some()
            || !input.selections.is_empty()
            || !input.targets.is_empty()
            || input.history_ref.is_some()
            || input.feedback_id.is_some()
            || !input.text.is_empty())
    {
        return Err(ReviewProtocolError::InvalidData);
    }
    if input.text.len() > MAX_FEEDBACK_TEXT_BYTES
        || input.targets.len() > MAX_TARGETS_PER_FEEDBACK
        || input.selections.len() > MAX_TARGETS_PER_FEEDBACK
    {
        return Err(ReviewProtocolError::LimitExceeded);
    }
    let mut ids = HashSet::new();
    let mut revisions = HashSet::new();
    let mut points = 0;
    for target in &input.targets {
        if !ids.insert(target.id) || !revisions.insert(target.revision_id) {
            return Err(ReviewProtocolError::InvalidData);
        }
        if let FeedbackAnchor::ImageStroke(stroke) = &target.anchor {
            points += stroke.points().len();
        }
        if let ReviewAvailability::NeedsConfirmation(reasons) = &target.availability
            && (reasons.is_empty()
                || reasons.len() > 7
                || reasons.iter().collect::<HashSet<_>>().len() != reasons.len())
        {
            return Err(ReviewProtocolError::InvalidData);
        }
    }
    if points > MAX_IMAGE_STROKE_POINTS_PER_ROUND {
        return Err(ReviewProtocolError::LimitExceeded);
    }
    if let Some(history) = &input.history_ref {
        if history.project_id != project || history.stream_id != draft.stream_id {
            return Err(ReviewProtocolError::InvalidData);
        }
        let count = match &history.source {
            HistorySource::Snapshot { keys, .. } => keys.len(),
            HistorySource::Legacy { targets, .. } => targets.len(),
        };
        if count > MAX_TARGETS_PER_FEEDBACK {
            return Err(ReviewProtocolError::LimitExceeded);
        }
    }
    Ok(())
}
