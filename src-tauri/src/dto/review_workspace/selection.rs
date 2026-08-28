use super::{remote_wire, wire};
use serde::{Deserialize, Serialize};
use viewer_application::review_workspace::*;
use viewer_domain::{
    review::{FeedbackAnchor, ProductionId, ProductionScope, continuous::*},
    *,
};

#[derive(Serialize, Deserialize)]
#[serde(
    remote = "ProductionScope",
    rename_all = "camelCase",
    deny_unknown_fields
)]
struct Scope {
    #[serde(with = "wire")]
    task_id: ProductionId,
    #[serde(with = "wire")]
    batch_id: ProductionId,
}
remote_wire!(ProductionScope, Scope);
#[derive(Serialize, Deserialize)]
#[serde(remote = "SnapshotRef", rename_all = "camelCase", deny_unknown_fields)]
struct Snapshot {
    #[serde(with = "wire")]
    snapshot_id: ReviewSnapshotId,
    #[serde(with = "wire")]
    blake3: [u8; 32],
}
remote_wire!(SnapshotRef, Snapshot);
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "TargetVersionKey",
    rename_all = "camelCase",
    deny_unknown_fields
)]
struct Key {
    #[serde(with = "wire")]
    feedback_id: FeedbackId,
    #[serde(with = "wire")]
    text_revision_id: ReviewTextRevisionId,
    #[serde(with = "wire")]
    target_id: ReviewTargetId,
    #[serde(with = "wire")]
    target_revision_id: ReviewTargetRevisionId,
}
remote_wire!(TargetVersionKey, Key);
#[derive(Serialize, Deserialize)]
#[serde(remote = "HistoryRef", rename_all = "camelCase", deny_unknown_fields)]
struct History {
    #[serde(with = "wire")]
    project_id: ProjectId,
    #[serde(with = "wire")]
    stream_id: ReviewStreamId,
    #[serde(with = "wire")]
    source: HistorySource,
}
remote_wire!(HistoryRef, History);
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "HistorySource",
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum HistoryOrigin {
    Snapshot {
        #[serde(with = "wire")]
        snapshot: SnapshotRef,
        #[serde(with = "wire")]
        keys: Vec<TargetVersionKey>,
    },
    Legacy {
        #[serde(with = "wire")]
        round_id: ReviewRoundId,
        #[serde(with = "wire")]
        record_blake3: [u8; 32],
        #[serde(with = "wire")]
        targets: Vec<LegacyTargetRef>,
    },
}
remote_wire!(HistorySource, HistoryOrigin);
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "LegacyTargetRef",
    rename_all = "camelCase",
    deny_unknown_fields
)]
struct LegacyKey {
    #[serde(with = "wire")]
    round_id: ReviewRoundId,
    #[serde(with = "wire")]
    feedback_id: FeedbackId,
    #[serde(with = "wire")]
    target_index: u32,
}
remote_wire!(LegacyTargetRef, LegacyKey);
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "SourceBindingDecision",
    rename_all = "camelCase",
    deny_unknown_fields
)]
struct Binding {
    #[serde(with = "wire")]
    target_key: TargetVersionKey,
    #[serde(with = "wire")]
    new_asset_version_id: AssetVersionId,
    #[serde(with = "wire")]
    anchor: FeedbackAnchor,
    #[serde(with = "wire")]
    confirmation: SourceBindingConfirmation,
}
remote_wire!(SourceBindingDecision, Binding);
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "SourceBindingConfirmation",
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Confirmation {
    UserConfirmed {},
    ProducerVerifiedAndPositionConfirmed {
        #[serde(with = "wire")]
        usage_id: ReviewUsageId,
    },
}
remote_wire!(SourceBindingConfirmation, Confirmation);
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "ArchiveSelection",
    rename_all = "camelCase",
    deny_unknown_fields
)]
struct Selection {
    #[serde(with = "wire")]
    expected_snapshot_id: ReviewSnapshotId,
    #[serde(with = "wire")]
    groups: Vec<ArchiveGroup>,
}
remote_wire!(ArchiveSelection, Selection);
#[derive(Serialize, Deserialize)]
#[serde(remote = "ArchiveGroup", rename_all = "camelCase", deny_unknown_fields)]
struct Group {
    #[serde(with = "wire")]
    basis: ArchiveBasis,
    #[serde(with = "wire")]
    targets: Vec<TargetVersionKey>,
}
remote_wire!(ArchiveGroup, Group);
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "ArchiveBasis",
    tag = "kind",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum Basis {
    Known {
        #[serde(with = "wire")]
        snapshot: SnapshotRef,
        #[serde(with = "wire")]
        source: ArchiveBasisSource,
    },
    Unknown {},
}
remote_wire!(ArchiveBasis, Basis);
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "ArchiveBasisSource",
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum BasisSource {
    AgentDeclared {
        #[serde(with = "wire")]
        usage_id: ReviewUsageId,
    },
    UserSelected {},
}
remote_wire!(ArchiveBasisSource, BasisSource);
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "RestoreDecision",
    rename_all = "camelCase",
    deny_unknown_fields
)]
struct Restore {
    #[serde(with = "wire")]
    historical_key: TargetVersionKey,
    #[serde(with = "wire")]
    choice: RestoreChoice,
}
remote_wire!(RestoreDecision, Restore);
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "RestoreChoice",
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum RestoreInput {
    PreserveCurrent {},
    UseHistorical {},
    ContinueAsNew {
        #[serde(with = "wire")]
        feedback_id: FeedbackId,
        #[serde(with = "wire")]
        text_revision_id: ReviewTextRevisionId,
        #[serde(with = "wire")]
        target_id: ReviewTargetId,
        #[serde(with = "wire")]
        target_revision_id: ReviewTargetRevisionId,
        #[serde(with = "wire")]
        target_asset_version_id: AssetVersionId,
        #[serde(with = "wire")]
        confirmed_anchor: Option<FeedbackAnchor>,
        #[serde(with = "wire")]
        created_at_ms: i64,
    },
}
remote_wire!(RestoreChoice, RestoreInput);
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "MigrationPlan",
    rename_all = "camelCase",
    deny_unknown_fields
)]
struct Migration {
    #[serde(with = "wire")]
    inspection_digest: [u8; 32],
    #[serde(with = "wire")]
    choice: MigrationChoice,
}
remote_wire!(MigrationPlan, Migration);
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "MigrationChoice",
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum MigrationInput {
    ContinueSelected {
        #[serde(with = "wire")]
        legacy_targets: Vec<LegacyTargetRef>,
        #[serde(with = "wire")]
        bindings: Vec<MigrationBinding>,
    },
    KeepHistoryOnly {},
}
remote_wire!(MigrationChoice, MigrationInput);
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "MigrationBinding",
    rename_all = "camelCase",
    deny_unknown_fields
)]
struct MigrationBindingInput {
    #[serde(with = "wire")]
    legacy_target: LegacyTargetRef,
    #[serde(with = "wire")]
    new_asset_version_id: AssetVersionId,
    #[serde(with = "wire")]
    anchor: FeedbackAnchor,
    position_confirmed: bool,
}
remote_wire!(MigrationBinding, MigrationBindingInput);
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "MigrationFeedbackIds",
    rename_all = "camelCase",
    deny_unknown_fields
)]
struct MigrationIds {
    #[serde(with = "wire")]
    round_id: ReviewRoundId,
    #[serde(with = "wire")]
    legacy_feedback_id: FeedbackId,
    #[serde(with = "wire")]
    feedback_id: FeedbackId,
    #[serde(with = "wire")]
    text_revision_id: ReviewTextRevisionId,
    #[serde(with = "wire")]
    targets: Vec<(u32, ReviewTargetId, ReviewTargetRevisionId)>,
}
remote_wire!(MigrationFeedbackIds, MigrationIds);
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MigrationTarget {
    #[serde(with = "wire")]
    target_index: u32,
    #[serde(with = "wire")]
    target_id: ReviewTargetId,
    #[serde(with = "wire")]
    target_revision_id: ReviewTargetRevisionId,
}
impl wire::WireValue for (u32, ReviewTargetId, ReviewTargetRevisionId) {
    fn encode<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        MigrationTarget {
            target_index: self.0,
            target_id: self.1,
            target_revision_id: self.2,
        }
        .serialize(s)
    }
    fn decode<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = MigrationTarget::deserialize(d)?;
        Ok((v.target_index, v.target_id, v.target_revision_id))
    }
}
