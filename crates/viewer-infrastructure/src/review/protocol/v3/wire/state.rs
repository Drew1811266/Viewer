use super::*;
use crate::review::protocol::ContinuousReviewProtocol;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::review::protocol::v3) struct State {
    protocol_version: ContinuousReviewProtocol,
    kind: StateKind,
    #[serde(deserialize_with = "canonical_id")]
    project_id: ProjectId,
    #[serde(deserialize_with = "canonical_id")]
    review_stream_id: ReviewStreamId,
    #[serde(deserialize_with = "canonical_id")]
    snapshot_id: ReviewSnapshotId,
    #[serde(with = "optional_snapshot")]
    parent: Option<SnapshotRef>,
    #[serde(deserialize_with = "canonical_id")]
    command_id: ReviewCommandId,
    #[serde(with = "digest")]
    payload_digest: [u8; 32],
    #[serde(with = "assets")]
    assets: Vec<AssetVersion>,
    #[serde(with = "feedback")]
    feedback: Vec<VersionedFeedback>,
    #[serde(with = "changes")]
    changes: Vec<ReviewChange>,
    evidence: Vec<EvidenceBinding>,
}

#[derive(Serialize, Deserialize)]
enum StateKind {
    #[serde(rename = "state")]
    State,
}

impl State {
    pub fn from_record(v: &ReviewStateRecord, protocol: ContinuousReviewProtocol) -> Self {
        Self {
            protocol_version: protocol,
            kind: StateKind::State,
            project_id: v.state.project_id,
            review_stream_id: v.state.stream_id,
            snapshot_id: v.state.snapshot_id,
            parent: v.state.parent,
            command_id: v.command_id,
            payload_digest: v.payload_digest,
            assets: v.state.assets.clone(),
            feedback: v.state.feedback.clone(),
            changes: v.changes.clone(),
            evidence: v.evidence.clone(),
        }
    }

    pub fn into_record(self) -> ReviewStateRecord {
        ReviewStateRecord {
            state: ContinuousReviewState {
                project_id: self.project_id,
                stream_id: self.review_stream_id,
                snapshot_id: self.snapshot_id,
                parent: self.parent,
                assets: self.assets,
                feedback: self.feedback,
            },
            command_id: self.command_id,
            payload_digest: self.payload_digest,
            changes: self.changes,
            evidence: self.evidence,
        }
    }
}
