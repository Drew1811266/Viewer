use super::{remote_wire, wire};
use serde::{Deserialize, Serialize};
use viewer_application::review_workspace::*;
use viewer_domain::{
    review::{FeedbackAnchor, ProductionScope, continuous::*},
    *,
};

#[derive(Serialize, Deserialize)]
#[serde(
    remote = "ReviewCommandEnvelope",
    rename_all = "camelCase",
    deny_unknown_fields
)]
struct Envelope {
    #[serde(with = "wire")]
    usage_selections: Vec<PreparedUsageSelection>,
    #[serde(with = "wire")]
    context: ReviewWorkspaceContext,
    #[serde(with = "wire")]
    command_id: ReviewCommandId,
    #[serde(with = "wire")]
    expected_snapshot_id: Option<ReviewSnapshotId>,
    #[serde(with = "wire")]
    payload_digest: [u8; 32],
    #[serde(with = "wire")]
    generated: GeneratedReviewIds,
    #[serde(with = "wire")]
    command: ReviewWorkspaceCommand,
}
remote_wire!(ReviewCommandEnvelope, Envelope);
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "ReviewWorkspaceContext",
    rename_all = "camelCase",
    deny_unknown_fields
)]
struct Context {
    #[serde(with = "wire")]
    project_id: ProjectId,
    #[serde(with = "wire")]
    stream_id: ReviewStreamId,
    #[serde(with = "wire")]
    production: Option<ProductionScope>,
}
remote_wire!(ReviewWorkspaceContext, Context);
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "GeneratedReviewIds",
    rename_all = "camelCase",
    deny_unknown_fields
)]
struct Generated {
    #[serde(with = "wire")]
    snapshot_id: ReviewSnapshotId,
    #[serde(with = "wire")]
    feedback_id: FeedbackId,
    #[serde(with = "wire")]
    text_revision_id: ReviewTextRevisionId,
    #[serde(with = "wire")]
    archive_id: ReviewArchiveId,
    #[serde(with = "wire")]
    targets: Vec<(ReviewTargetId, ReviewTargetRevisionId)>,
    #[serde(with = "wire")]
    migration: Vec<MigrationFeedbackIds>,
    #[serde(with = "wire")]
    created_at_ms: i64,
}
remote_wire!(GeneratedReviewIds, Generated);
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GeneratedTarget {
    #[serde(with = "wire")]
    target_id: ReviewTargetId,
    #[serde(with = "wire")]
    target_revision_id: ReviewTargetRevisionId,
}
impl wire::WireValue for (ReviewTargetId, ReviewTargetRevisionId) {
    fn encode<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        GeneratedTarget {
            target_id: self.0,
            target_revision_id: self.1,
        }
        .serialize(s)
    }
    fn decode<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = GeneratedTarget::deserialize(d)?;
        Ok((v.target_id, v.target_revision_id))
    }
}
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "PreparedUsageSelection",
    rename_all = "camelCase",
    deny_unknown_fields
)]
struct UsageSelection {
    #[serde(with = "wire")]
    id: ReviewUsageId,
    #[serde(with = "wire")]
    candidate: Option<PreparedUsageCandidate>,
}
remote_wire!(PreparedUsageSelection, UsageSelection);
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "PreparedUsageCandidate",
    rename_all = "camelCase",
    deny_unknown_fields
)]
struct UsageCandidate {
    #[serde(with = "wire")]
    canonical_digest: [u8; 32],
    #[serde(with = "wire")]
    source_digest: [u8; 32],
    #[serde(with = "wire")]
    source: RelativePath,
}
remote_wire!(PreparedUsageCandidate, UsageCandidate);

#[derive(Serialize, Deserialize)]
#[serde(
    remote = "TargetEdit",
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Edit {
    Add {
        #[serde(with = "wire")]
        asset_version_id: AssetVersionId,
        #[serde(with = "wire")]
        anchor: FeedbackAnchor,
    },
    Redraw {
        #[serde(with = "wire")]
        key: TargetVersionKey,
        #[serde(with = "wire")]
        asset_version_id: AssetVersionId,
        #[serde(with = "wire")]
        anchor: FeedbackAnchor,
    },
}
remote_wire!(TargetEdit, Edit, 10_000);
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "ReviewWorkspaceCommand",
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Command {
    SaveFeedback {
        #[serde(with = "wire")]
        feedback_id: Option<FeedbackId>,
        #[serde(with = "wire")]
        text: String,
        #[serde(with = "wire")]
        targets: Vec<TargetEdit>,
    },
    Withdraw {
        #[serde(with = "wire")]
        targets: Vec<TargetVersionKey>,
    },
    Archive(#[serde(with = "wire")] ArchiveSelection),
    Restore {
        #[serde(with = "wire")]
        archive_id: ReviewArchiveId,
        #[serde(with = "wire")]
        decisions: Vec<RestoreDecision>,
    },
    ContinueHistorical {
        #[serde(with = "wire")]
        history_ref: HistoryRef,
        #[serde(with = "wire")]
        bindings: Vec<SourceBindingDecision>,
    },
    ConfirmSource(#[serde(with = "wire")] SourceBindingDecision),
    ConfirmApplicability {
        #[serde(with = "wire")]
        key: TargetVersionKey,
        #[serde(with = "wire")]
        asset_version_id: AssetVersionId,
        #[serde(with = "wire")]
        anchor: FeedbackAnchor,
    },
    AdoptUsage {
        #[serde(with = "wire")]
        declaration_id: ReviewUsageId,
    },
    Migrate(#[serde(with = "wire")] MigrationPlan),
    ContinueLegacy {
        #[serde(with = "wire")]
        history_ref: HistoryRef,
        #[serde(with = "wire")]
        bindings: Vec<MigrationBinding>,
    },
}
remote_wire!(ReviewWorkspaceCommand, Command);
