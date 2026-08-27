use serde::{Deserialize, Serialize};
use viewer_domain::review::continuous::{
    ArchiveCheckpoint, ContinuousReviewState, ReviewChange, SnapshotRef, TargetVersionKey,
};
use viewer_domain::{
    AssetVersionId, ProjectId, RelativePath, ReviewArchiveId, ReviewCommandId, ReviewRoundId,
    ReviewSnapshotId, ReviewStreamId, ReviewUsageId,
};

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewStateRecord {
    pub state: ContinuousReviewState,
    pub command_id: ReviewCommandId,
    pub payload_digest: [u8; 32],
    pub changes: Vec<ReviewChange>,
    pub evidence: Vec<EvidenceBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceRef {
    #[serde(with = "super::wire::digest")]
    pub blake3: [u8; 32],
    pub size_bytes: u64,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceAnnotation {
    pub ordinal: u32,
    #[serde(with = "super::wire::key")]
    pub key: TargetVersionKey,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum EvidenceCapability {
    Image {
        base: EvidenceRef,
        #[serde(deserialize_with = "super::wire::required_option")]
        annotated: Option<EvidenceRef>,
        annotations: Vec<EvidenceAnnotation>,
    },
    LegacyAbsent {},
    NotImage {},
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceBinding {
    #[serde(deserialize_with = "super::wire::canonical_id")]
    pub asset_version_id: AssetVersionId,
    pub capability: EvidenceCapability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewIndexV3 {
    pub project_id: ProjectId,
    pub streams: Vec<ReviewStreamV3>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewStreamV3 {
    #[serde(deserialize_with = "super::wire::canonical_id")]
    pub review_stream_id: ReviewStreamId,
    #[serde(deserialize_with = "super::wire::required_option")]
    pub task_id: Option<String>,
    #[serde(deserialize_with = "super::wire::required_option")]
    pub batch_id: Option<String>,
    #[serde(with = "super::wire::optional_snapshot")]
    pub current_ref: Option<SnapshotRef>,
    pub archive_refs: Vec<ArchiveRecordRef>,
    pub legacy_refs: Vec<LegacyRecordRef>,
    pub usage_refs: Vec<UsageRecordRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArchiveRecordRef {
    #[serde(deserialize_with = "super::wire::canonical_id")]
    pub archive_id: ReviewArchiveId,
    pub location: String,
    #[serde(with = "super::wire::digest")]
    pub blake3: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegacyRecordRef {
    #[serde(deserialize_with = "super::wire::canonical_id")]
    pub round_id: ReviewRoundId,
    pub protocol_version: String,
    pub location: String,
    #[serde(with = "super::wire::digest")]
    pub blake3: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageRecordRef {
    #[serde(deserialize_with = "super::wire::canonical_id")]
    pub declaration_id: ReviewUsageId,
    pub location: String,
    #[serde(with = "super::wire::digest")]
    pub blake3: [u8; 32],
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewArchiveRecord {
    pub checkpoint: ArchiveCheckpoint,
    pub result_snapshot_id: ReviewSnapshotId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewUsageRecord {
    pub declaration_id: ReviewUsageId,
    pub project_id: ProjectId,
    pub review_stream_id: ReviewStreamId,
    pub basis: SnapshotRef,
    pub targets: Vec<TargetVersionKey>,
    pub outputs: Vec<UsageOutputRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageOutputRecord {
    pub relative_path: RelativePath,
    #[serde(with = "super::wire::digest")]
    pub blake3: [u8; 32],
    #[serde(deserialize_with = "super::wire::canonical_id")]
    pub previous_asset_version_id: AssetVersionId,
}
