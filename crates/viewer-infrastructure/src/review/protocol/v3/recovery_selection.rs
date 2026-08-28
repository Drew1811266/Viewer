use super::wire;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use viewer_application::review_workspace::{
    RecoveryTargetConfirmation, RecoveryTargetOrigin, RecoveryTargetSelection,
};
use viewer_domain::review::continuous::TargetVersionKey;
use viewer_domain::*;

#[derive(Serialize, Deserialize)]
#[serde(
    remote = "RecoveryTargetOrigin",
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Origin {
    Current {
        #[serde(with = "wire::key")]
        key: TargetVersionKey,
    },
    Snapshot {
        #[serde(with = "wire::key")]
        key: TargetVersionKey,
    },
    Legacy {
        #[serde(deserialize_with = "wire::canonical_id")]
        round_id: ReviewRoundId,
        #[serde(deserialize_with = "wire::canonical_id")]
        feedback_id: FeedbackId,
        target_index: u32,
    },
    Archive {
        #[serde(deserialize_with = "wire::canonical_id")]
        archive_id: ReviewArchiveId,
        #[serde(with = "wire::key")]
        key: TargetVersionKey,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "RecoveryTargetConfirmation",
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Confirmation {
    Unconfirmed,
    UserConfirmed,
    ProducerVerified {
        #[serde(deserialize_with = "wire::canonical_id")]
        usage_id: ReviewUsageId,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "RecoveryTargetSelection",
    rename_all = "camelCase",
    deny_unknown_fields
)]
struct Selection {
    #[serde(with = "Origin")]
    origin: RecoveryTargetOrigin,
    #[serde(deserialize_with = "wire::canonical_id")]
    feedback_id: FeedbackId,
    #[serde(deserialize_with = "wire::canonical_id")]
    text_revision_id: ReviewTextRevisionId,
    #[serde(deserialize_with = "wire::canonical_id")]
    target_id: ReviewTargetId,
    #[serde(deserialize_with = "wire::canonical_id")]
    target_revision_id: ReviewTargetRevisionId,
    #[serde(deserialize_with = "wire::canonical_id")]
    asset_version_id: AssetVersionId,
    #[serde(with = "Confirmation")]
    confirmation: RecoveryTargetConfirmation,
}
#[derive(Serialize, Deserialize)]
struct Value(#[serde(with = "Selection")] RecoveryTargetSelection);

pub(super) fn serialize<S: Serializer>(
    values: &[RecoveryTargetSelection],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq;
    let mut seq = serializer.serialize_seq(Some(values.len()))?;
    for value in values {
        seq.serialize_element(&Value(value.clone()))?;
    }
    seq.end()
}
pub(super) fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<RecoveryTargetSelection>, D::Error> {
    Ok(Vec::<Value>::deserialize(deserializer)?
        .into_iter()
        .map(|v| v.0)
        .collect())
}

pub(super) fn validate(
    input: &viewer_application::review_workspace::RecoveryEditorInput,
) -> Result<(), super::ReviewProtocolError> {
    use super::ReviewProtocolError::InvalidData;
    use std::collections::{HashMap, HashSet};
    use viewer_domain::review::continuous::HistorySource;
    let mut ids = HashSet::new();
    let mut revisions = HashSet::new();
    let mut texts = HashMap::new();
    let targets: HashMap<_, _> = input.targets.iter().map(|t| (t.id, t)).collect();
    for s in &input.selections {
        if !ids.insert(s.target_id)
            || !revisions.insert(s.target_revision_id)
            || input.feedback_id.is_some_and(|id| id != s.feedback_id)
            || texts
                .insert(s.feedback_id, s.text_revision_id)
                .is_some_and(|id| id != s.text_revision_id)
        {
            return Err(InvalidData);
        }
        match targets.get(&s.target_id) {
            Some(t)
                if t.revision_id == s.target_revision_id
                    && t.asset_version_id == s.asset_version_id => {}
            None if s.confirmation == RecoveryTargetConfirmation::Unconfirmed => {}
            _ => return Err(InvalidData),
        }
        let valid_origin = match &s.origin {
            RecoveryTargetOrigin::Current { key } => {
                key.feedback_id == s.feedback_id
                    && key.text_revision_id == s.text_revision_id
                    && key.target_id == s.target_id
            }
            RecoveryTargetOrigin::Snapshot { key } => matches!(
                input.history_ref.as_ref().map(|h| &h.source),
                Some(HistorySource::Snapshot { keys, .. }) if keys.contains(key)),
            RecoveryTargetOrigin::Legacy {
                round_id,
                feedback_id,
                target_index,
            } => matches!(
                input.history_ref.as_ref().map(|h| &h.source),
                Some(HistorySource::Legacy { round_id: round, targets, .. }) if round == round_id
                    && targets.contains(&viewer_domain::review::continuous::LegacyTargetRef {
                        round_id: *round_id, feedback_id: *feedback_id, target_index: *target_index })),
            RecoveryTargetOrigin::Archive { .. } => true,
        };
        if !valid_origin {
            return Err(InvalidData);
        }
    }
    if !input.selections.is_empty() && input.targets.iter().any(|t| !ids.contains(&t.id)) {
        return Err(InvalidData);
    }
    Ok(())
}
