use super::{ReviewProtocolError, wire};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use viewer_application::review_workspace::{MigrationBinding, MigrationChoice, MigrationPlan};
use viewer_domain::{
    review::{
        FeedbackAnchor, MAX_IMAGE_STROKE_POINTS_PER_ROUND, MAX_TARGETS_PER_FEEDBACK,
        continuous::LegacyTargetRef,
    },
    *,
};

#[derive(Serialize, Deserialize)]
#[serde(
    remote = "LegacyTargetRef",
    rename_all = "camelCase",
    deny_unknown_fields
)]
struct LegacyTarget {
    #[serde(deserialize_with = "wire::canonical_id")]
    round_id: ReviewRoundId,
    #[serde(deserialize_with = "wire::canonical_id")]
    feedback_id: FeedbackId,
    target_index: u32,
}
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "MigrationBinding",
    rename_all = "camelCase",
    deny_unknown_fields
)]
struct Binding {
    #[serde(with = "LegacyTarget")]
    legacy_target: LegacyTargetRef,
    #[serde(deserialize_with = "wire::canonical_id")]
    new_asset_version_id: AssetVersionId,
    #[serde(with = "wire::anchor")]
    anchor: FeedbackAnchor,
    position_confirmed: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "MigrationChoice",
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Choice {
    KeepHistoryOnly,
    ContinueSelected {
        #[serde(with = "legacy_targets")]
        legacy_targets: Vec<LegacyTargetRef>,
        #[serde(with = "bindings")]
        bindings: Vec<MigrationBinding>,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "MigrationPlan",
    rename_all = "camelCase",
    deny_unknown_fields
)]
struct Plan {
    #[serde(with = "wire::digest")]
    inspection_digest: [u8; 32],
    #[serde(with = "Choice")]
    choice: MigrationChoice,
}
macro_rules! list {
    ($module:ident, $domain:ty, $dto:literal) => {
        mod $module {
            use super::*;
            #[derive(Serialize, Deserialize)]
            struct Value(#[serde(with = $dto)] $domain);
            pub fn serialize<S: Serializer>(v: &[$domain], s: S) -> Result<S::Ok, S::Error> {
                use serde::ser::SerializeSeq;
                let mut seq = s.serialize_seq(Some(v.len()))?;
                for item in v {
                    seq.serialize_element(&Value(item.clone()))?;
                }
                seq.end()
            }
            pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<$domain>, D::Error> {
                Ok(Vec::<Value>::deserialize(d)?
                    .into_iter()
                    .map(|v| v.0)
                    .collect())
            }
        }
    };
}
list!(legacy_targets, LegacyTargetRef, "LegacyTarget");
list!(bindings, MigrationBinding, "Binding");
#[derive(Serialize, Deserialize)]
struct Value(#[serde(with = "Plan")] MigrationPlan);
pub(super) fn serialize<S: Serializer>(v: &Option<MigrationPlan>, s: S) -> Result<S::Ok, S::Error> {
    v.clone().map(Value).serialize(s)
}
pub(super) fn deserialize<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Option<MigrationPlan>, D::Error> {
    Ok(Option::<Value>::deserialize(d)?.map(|v| v.0))
}
pub(super) fn validate(plan: Option<&MigrationPlan>) -> Result<(), ReviewProtocolError> {
    let Some(MigrationPlan {
        choice:
            MigrationChoice::ContinueSelected {
                legacy_targets,
                bindings,
            },
        ..
    }) = plan
    else {
        return Ok(());
    };
    if legacy_targets.len() > MAX_TARGETS_PER_FEEDBACK || bindings.len() > MAX_TARGETS_PER_FEEDBACK
    {
        return Err(ReviewProtocolError::LimitExceeded);
    }
    let mut selected = std::collections::HashSet::new();
    let mut bound = std::collections::HashSet::new();
    if legacy_targets
        .iter()
        .any(|key| key.target_index as usize >= MAX_TARGETS_PER_FEEDBACK || !selected.insert(*key))
        || bindings.iter().any(|b| {
            b.legacy_target.target_index as usize >= MAX_TARGETS_PER_FEEDBACK
                || !bound.insert(b.legacy_target)
        })
    {
        return Err(ReviewProtocolError::InvalidData);
    }
    let points: usize = bindings
        .iter()
        .map(|b| match &b.anchor {
            FeedbackAnchor::ImageStroke(s) => s.points().len(),
            _ => 0,
        })
        .sum();
    if points > MAX_IMAGE_STROKE_POINTS_PER_ROUND {
        return Err(ReviewProtocolError::LimitExceeded);
    }
    Ok(())
}
