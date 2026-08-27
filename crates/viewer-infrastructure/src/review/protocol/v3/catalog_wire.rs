use super::wire::{self, Protocol};
use super::{ReviewIndexV3, ReviewStreamV3, ReviewUsageRecord, UsageOutputRecord};
use serde::{Deserialize, Serialize};
use viewer_domain::review::continuous::{SnapshotRef, TargetVersionKey};
use viewer_domain::{ProjectId, ReviewStreamId, ReviewUsageId};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Index {
    protocol_version: Protocol,
    kind: IndexKind,
    #[serde(deserialize_with = "wire::canonical_id")]
    project_id: ProjectId,
    streams: Vec<ReviewStreamV3>,
}
#[derive(Serialize, Deserialize)]
enum IndexKind {
    #[serde(rename = "index")]
    Index,
}
impl From<&ReviewIndexV3> for Index {
    fn from(value: &ReviewIndexV3) -> Self {
        Self {
            protocol_version: Protocol::V3,
            kind: IndexKind::Index,
            project_id: value.project_id,
            streams: value.streams.clone(),
        }
    }
}
impl Index {
    pub fn into_record(self) -> ReviewIndexV3 {
        ReviewIndexV3 {
            project_id: self.project_id,
            streams: self.streams,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Usage {
    protocol_version: UsageVersion,
    #[serde(deserialize_with = "wire::canonical_id")]
    declaration_id: ReviewUsageId,
    #[serde(deserialize_with = "wire::canonical_id")]
    project_id: ProjectId,
    #[serde(deserialize_with = "wire::canonical_id")]
    review_stream_id: ReviewStreamId,
    #[serde(with = "wire::snapshot")]
    basis: SnapshotRef,
    #[serde(with = "wire::keys")]
    targets: Vec<TargetVersionKey>,
    outputs: Vec<UsageOutputRecord>,
}
#[derive(Serialize, Deserialize)]
enum UsageVersion {
    #[serde(rename = "viewer.review.usage/1")]
    V1,
}
impl From<&ReviewUsageRecord> for Usage {
    fn from(v: &ReviewUsageRecord) -> Self {
        Self {
            protocol_version: UsageVersion::V1,
            declaration_id: v.declaration_id,
            project_id: v.project_id,
            review_stream_id: v.review_stream_id,
            basis: v.basis,
            targets: v.targets.clone(),
            outputs: v.outputs.clone(),
        }
    }
}
impl Usage {
    pub fn into_record(self) -> ReviewUsageRecord {
        ReviewUsageRecord {
            declaration_id: self.declaration_id,
            project_id: self.project_id,
            review_stream_id: self.review_stream_id,
            basis: self.basis,
            targets: self.targets,
            outputs: self.outputs,
        }
    }
}
