use super::*;
use crate::ReviewTaskCancellation;
use async_trait::async_trait;
use viewer_domain::{ReviewStreamId, review::continuous::SnapshotRef};

#[derive(Clone, Debug, PartialEq)]
pub struct ClaimedReviewMaterialization {
    pub stream_id: ReviewStreamId,
    pub target: StoredAuthoringState,
    pub lease_epoch: u64,
    pub attempt_count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedReviewPublication {
    pub target: ReviewAuthoringHead,
    pub request: ReviewCommitRequest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewPublicationReceipt {
    pub target: ReviewAuthoringHead,
    pub snapshot: SnapshotRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewMaterializationOutcome {
    Idle,
    Published {
        receipt: ReviewPublicationReceipt,
    },
    Retrying {
        target: ReviewAuthoringHead,
        attempt_count: u32,
    },
    Blocked {
        target: ReviewAuthoringHead,
        code: ReviewMaterializationFailure,
    },
}

pub trait ReviewMaterializationQueuePort: Send + Sync {
    fn next(&self, now_ms: i64) -> Result<Option<ClaimedReviewMaterialization>, ReviewCommitError>;
    fn retry(
        &self,
        claim: &ClaimedReviewMaterialization,
        code: ReviewMaterializationFailure,
        next_attempt_at_ms: i64,
    ) -> Result<(), ReviewCommitError>;
    fn block(
        &self,
        claim: &ClaimedReviewMaterialization,
        code: ReviewMaterializationFailure,
    ) -> Result<(), ReviewCommitError>;
    fn mark_published(
        &self,
        claim: &ClaimedReviewMaterialization,
        receipt: ReviewPublicationReceipt,
    ) -> Result<(), ReviewCommitError>;
    fn status(&self, stream: ReviewStreamId) -> Result<ReviewPublicationStatus, ReviewCommitError>;
    fn requeue_expired(&self, now_ms: i64) -> Result<u32, ReviewCommitError>;
}

#[async_trait]
pub trait ReviewPublicationPort: Send + Sync {
    async fn materialize(
        &self,
        target: StoredAuthoringState,
        cancellation: ReviewTaskCancellation,
    ) -> Result<PreparedReviewPublication, ReviewWorkspaceError>;
    async fn publish(
        &self,
        prepared: PreparedReviewPublication,
    ) -> Result<ReviewPublicationReceipt, ReviewWorkspaceError>;
    async fn verify_publication(
        &self,
        target: ReviewAuthoringHead,
    ) -> Result<Option<ReviewPublicationReceipt>, ReviewWorkspaceError>;
}
