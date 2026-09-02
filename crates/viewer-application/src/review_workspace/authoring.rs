use super::*;
use viewer_domain::{review::ProductionScope, review::continuous::*, *};

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

#[derive(Clone, Debug, PartialEq)]
pub struct StoredAuthoringState {
    pub head: ReviewAuthoringHead,
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
