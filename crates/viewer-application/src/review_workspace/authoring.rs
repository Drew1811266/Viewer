use super::*;
use serde::{Deserialize, Serialize};
use viewer_domain::{
    review::continuous::*,
    review::{FeedbackAnchor, ProductionScope},
    *,
};

pub type ReviewAuthoringSequence = u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewAuthoringHead {
    pub sequence: ReviewAuthoringSequence,
    pub snapshot_id: ReviewSnapshotId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewHeads {
    pub authoring: Option<ReviewAuthoringHead>,
    pub published: Option<ReviewAuthoringHead>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewBarrierKind {
    None,
    Archive,
    Restore,
    Migration,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewPublicationProtocol {
    #[default]
    V3,
    V4,
}

impl ReviewPublicationProtocol {
    pub fn promote_for(self, state: &ContinuousReviewState) -> Self {
        let requires_v4 = state
            .feedback
            .iter()
            .flat_map(|item| &item.targets)
            .any(|target| {
                matches!(
                    &target.anchor,
                    FeedbackAnchor::ImagePoint(_)
                        | FeedbackAnchor::ImageArrow(_)
                        | FeedbackAnchor::ImageEllipse(_)
                )
            });
        if self == Self::V4 || requires_v4 {
            Self::V4
        } else {
            Self::V3
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredAuthoringState {
    pub head: ReviewAuthoringHead,
    pub publication_protocol: ReviewPublicationProtocol,
    pub production: Option<ProductionScope>,
    pub state: ContinuousReviewState,
    pub command_id: ReviewCommandId,
    pub payload_digest: [u8; 32],
    pub generated: GeneratedReviewIds,
    pub changes: Vec<ReviewChange>,
    pub archives: Vec<ArchiveCheckpoint>,
    pub adopted_usage: Vec<ReviewUsageDeclaration>,
    pub barrier: ReviewBarrierKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewAuthoringReceipt {
    pub command_id: ReviewCommandId,
    pub payload_digest: [u8; 32],
    pub head: ReviewAuthoringHead,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewMaterializationFailure {
    SourceChanged,
    SourceMissing,
    SourceUnreadable,
    RenderFailed,
    Integrity,
    LimitExceeded,
    Io,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewPublicationStatus {
    Ready,
    Pending { pending_revisions: u32 },
    Blocked { code: ReviewMaterializationFailure },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewWorkspaceCurrent {
    pub authoring: StoredAuthoringState,
    pub published_ref: Option<SnapshotRef>,
    pub evidence: Vec<ReviewEvidenceBinding>,
}

impl ReviewWorkspaceCurrent {
    pub(crate) fn from_published(value: StoredContinuousSnapshot) -> Self {
        let StoredContinuousSnapshot {
            reference,
            publication_protocol,
            production,
            state,
            command_id,
            payload_digest,
            changes,
            evidence,
        } = value;
        let snapshot_id = state.snapshot_id;
        Self {
            authoring: StoredAuthoringState {
                head: ReviewAuthoringHead {
                    sequence: 1,
                    snapshot_id,
                },
                publication_protocol,
                production,
                state,
                command_id,
                payload_digest,
                generated: GeneratedReviewIds {
                    snapshot_id,
                    feedback_id: FeedbackId::from_u128(0),
                    text_revision_id: ReviewTextRevisionId::from_u128(0),
                    archive_id: ReviewArchiveId::from_u128(0),
                    targets: vec![],
                    migration: vec![],
                    created_at_ms: 0,
                },
                changes,
                archives: vec![],
                adopted_usage: vec![],
                barrier: ReviewBarrierKind::None,
            },
            published_ref: Some(reference),
            evidence,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthoringCommandLookup {
    Found(ReviewAuthoringReceipt),
    Absent,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewAuthoringCommitRequest {
    pub expected_snapshot_id: Option<ReviewSnapshotId>,
    pub next: StoredAuthoringState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewAuthoringApplyResult {
    pub receipt: ReviewAuthoringReceipt,
    pub patch: ReviewWorkspacePatch,
    pub publication: ReviewPublicationStatus,
}

pub trait ContinuousReviewAuthoringStorePort: Send + Sync {
    fn load_heads(&self, stream: ReviewStreamId) -> Result<ReviewHeads, ReviewCommitError>;
    fn load_current(
        &self,
        stream: ReviewStreamId,
    ) -> Result<Option<StoredAuthoringState>, ReviewCommitError>;
    /// Loads one exact immutable logical revision. This is required to reconstruct the
    /// same UI patch for an idempotent command retry without rerunning the command.
    fn load_snapshot(
        &self,
        stream: ReviewStreamId,
        sequence: ReviewAuthoringSequence,
    ) -> Result<StoredAuthoringState, ReviewCommitError>;
    fn find_command(
        &self,
        stream: ReviewStreamId,
        command: ReviewCommandId,
    ) -> Result<AuthoringCommandLookup, ReviewCommitError>;
    fn commit(
        &self,
        stream: ReviewStreamId,
        command: ReviewCommandId,
        payload_digest: [u8; 32],
        prepare: &mut dyn FnMut(
            Option<&StoredAuthoringState>,
        )
            -> Result<ReviewAuthoringCommitRequest, ReviewWorkspaceError>,
    ) -> Result<ReviewAuthoringReceipt, ReviewWorkspaceError>;
}

/// One authoring connection owns both the logical store and its transactional outbox view.
/// Keeping these capabilities together prevents the foreground service from observing queue
/// status through a different database connection or persistence implementation.
pub trait ContinuousReviewAuthoringRepositoryPort:
    ContinuousReviewAuthoringStorePort + ReviewMaterializationQueuePort
{
}

impl<T> ContinuousReviewAuthoringRepositoryPort for T where
    T: ContinuousReviewAuthoringStorePort + ReviewMaterializationQueuePort
{
}
