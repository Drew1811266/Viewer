use super::wire;
use serde::{Deserialize, Serialize};
use viewer_application::{review_evidence::*, review_workspace::*};
use viewer_domain::{review::continuous::*, *};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareReviewAssetsRequestDto {
    #[serde(with = "wire")]
    pub session_id: SessionId,
    #[serde(with = "wire")]
    pub generation: u64,
    #[serde(with = "wire")]
    pub entity_ids: Vec<EntityId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewWorkspaceSessionRequestDto {
    #[serde(with = "wire")]
    pub session_id: SessionId,
    #[serde(with = "wire")]
    pub generation: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareReviewCommandRequestDto {
    #[serde(with = "wire")]
    pub session_id: SessionId,
    #[serde(with = "wire")]
    pub generation: u64,
    #[serde(with = "wire")]
    pub command_id: ReviewCommandId,
    #[serde(with = "wire")]
    pub expected_snapshot_id: Option<ReviewSnapshotId>,
    #[serde(with = "wire")]
    pub command: ReviewWorkspaceCommand,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyReviewCommandRequestDto {
    #[serde(with = "wire")]
    pub session_id: SessionId,
    #[serde(with = "wire")]
    pub generation: u64,
    #[serde(with = "wire")]
    pub envelope: ReviewCommandEnvelope,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewReviewArchiveRequestDto {
    #[serde(with = "wire")]
    pub session_id: SessionId,
    #[serde(with = "wire")]
    pub generation: u64,
    #[serde(with = "wire")]
    pub selection: ArchiveSelection,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewReviewRestoreRequestDto {
    #[serde(with = "wire")]
    pub session_id: SessionId,
    #[serde(with = "wire")]
    pub generation: u64,
    #[serde(with = "wire")]
    pub archive_id: ReviewArchiveId,
    #[serde(with = "wire")]
    pub decisions: Vec<RestoreDecision>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewHistoryRequestDto {
    #[serde(with = "wire")]
    pub session_id: SessionId,
    #[serde(with = "wire")]
    pub generation: u64,
    #[serde(with = "wire")]
    pub selector: HistorySelector,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectReviewUsageRequestDto {
    #[serde(with = "wire")]
    pub session_id: SessionId,
    #[serde(with = "wire")]
    pub generation: u64,
    #[serde(with = "wire")]
    pub entity_id: EntityId,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewEvidenceRequestDto {
    #[serde(with = "wire")]
    pub session_id: SessionId,
    #[serde(with = "wire")]
    pub generation: u64,
    #[serde(with = "wire")]
    pub selector: HistorySelector,
    #[serde(with = "wire")]
    pub asset_version_id: AssetVersionId,
    #[serde(with = "wire")]
    pub role: EvidenceRole,
}
