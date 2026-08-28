use super::*;
use crate::{
    PersistedReviewDraft, ReviewArtifactError, ReviewAssetError, ReviewProtocolVersion,
    review_evidence::HistorySelector,
};
use viewer_domain::{
    review::{AssetVersion, FeedbackAnchor, ProductionScope, ReviewSnapshot, continuous::*},
    *,
};

/// A service is permanently scoped to one project and stream; viewing never changes its owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewWorkspaceContext {
    pub project_id: ProjectId,
    pub stream_id: ReviewStreamId,
    pub production: Option<ProductionScope>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TargetEdit {
    Add {
        asset_version_id: AssetVersionId,
        anchor: FeedbackAnchor,
    },
    Redraw {
        key: TargetVersionKey,
        asset_version_id: AssetVersionId,
        anchor: FeedbackAnchor,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ReviewWorkspaceCommand {
    SaveFeedback {
        feedback_id: Option<FeedbackId>,
        text: String,
        targets: Vec<TargetEdit>,
    },
    Withdraw {
        targets: Vec<TargetVersionKey>,
    },
    Archive(ArchiveSelection),
    Restore {
        archive_id: ReviewArchiveId,
        decisions: Vec<RestoreDecision>,
    },
    ContinueHistorical {
        history_ref: HistoryRef,
        bindings: Vec<SourceBindingDecision>,
    },
    ConfirmSource(SourceBindingDecision),
    AdoptUsage {
        declaration_id: ReviewUsageId,
    },
    Migrate(MigrationPlan),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedReviewIds {
    pub snapshot_id: ReviewSnapshotId,
    pub feedback_id: FeedbackId,
    pub text_revision_id: ReviewTextRevisionId,
    pub archive_id: ReviewArchiveId,
    pub targets: Vec<(ReviewTargetId, ReviewTargetRevisionId)>,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewCommandEnvelope {
    pub context: ReviewWorkspaceContext,
    pub command_id: ReviewCommandId,
    pub expected_snapshot_id: Option<ReviewSnapshotId>,
    pub payload_digest: [u8; 32],
    pub generated: GeneratedReviewIds,
    pub command: ReviewWorkspaceCommand,
}

pub trait ReviewCommandCodecPort: Send + Sync {
    /// Canonical, bounded encoding of all fields except payload_digest, followed by BLAKE3.
    fn digest(&self, envelope: &ReviewCommandEnvelope) -> Result<[u8; 32], ReviewWorkspaceError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ReviewWorkspaceError {
    #[error(transparent)]
    Domain(#[from] ContinuousReviewError),
    #[error(transparent)]
    Repository(#[from] ReviewCommitError),
    #[error(transparent)]
    Asset(#[from] ReviewAssetError),
    #[error(transparent)]
    Evidence(#[from] ReviewArtifactError),
    #[error(transparent)]
    Usage(#[from] UsageImportError),
    #[error("review task was cancelled; input is not published")]
    Cancelled,
    #[error("review command has no changes")]
    NoChanges,
    #[error("this review capability has not been provided")]
    CapabilityUnavailable,
    #[error("review command belongs to another project or stream")]
    WrongContext,
    #[error("review asset must be prepared for preview before saving")]
    PreviewRequired,
    #[error("review committed, but refreshing the current view failed; retain the receipt")]
    CommittedViewUnavailable(ReviewCommitReceipt),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewWorkspaceCapabilities {
    pub continuous_editing: bool,
    pub usage_import: bool,
    pub migration: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewWorkspaceView {
    pub stream_id: ReviewStreamId,
    pub current: Option<StoredContinuousSnapshot>,
    pub source_checks: Vec<SourceCheck>,
    pub projection: CurrentReviewProjection,
    pub recovery: Vec<RecoveryDraft>,
    pub migration: Option<MigrationInspection>,
    pub capabilities: ReviewWorkspaceCapabilities,
}
#[derive(Clone, Debug, PartialEq)]
pub enum ReviewWorkspacePreview {
    Archive(ArchivePlan),
    Restore(RestorePlan),
}
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewApplyResult {
    pub receipt: ReviewCommitReceipt,
    pub view: ReviewWorkspaceView,
}

/// Explicit historical entries preserve separate basis snapshots and never expose actionable data.
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryEntry {
    pub snapshot: SnapshotRef,
    pub feedback: Vec<VersionedFeedback>,
    pub assets: Vec<AssetVersion>,
    pub evidence: Vec<ReviewEvidenceBinding>,
    pub selected: Vec<TargetVersionKey>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryView {
    pub selector: HistorySelector,
    pub entries: Vec<HistoryEntry>,
    pub legacy: Option<ReviewSnapshot>,
    pub limitations: Vec<ReviewHistoryLimitation>,
    pub restore_actions: Vec<TargetVersionKey>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewHistoryLimitation {
    BackgroundOnly,
    LegacyEvidenceAbsent,
    ExternalCopiesCannotBeRevoked,
    UsageUnconfirmed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageImportPreview {
    pub declaration: ReviewUsageDeclaration,
    pub canonical_digest: [u8; 32],
    pub source: RelativePath,
    pub source_digest: [u8; 32],
    pub outputs: Vec<UsageOutputCheck>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageOutputCheck {
    pub output: UsageOutput,
    pub status: UsageOutputStatus,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsageOutputStatus {
    VerifiedCandidate,
    UnknownPreviousAsset,
    Missing,
    Changed,
    Unsafe,
    Unreadable,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum UsageImportError {
    #[error("invalid usage declaration")]
    InvalidDeclaration,
    #[error("usage declaration belongs to another context")]
    WrongContext,
    #[error("usage basis cannot be verified")]
    UnknownBasis,
    #[error("usage target scope is invalid")]
    InvalidScope,
    #[error("usage declaration identity conflicts")]
    Conflict,
    #[error("unsafe usage source path")]
    UnsafePath,
    #[error("usage resource limit exceeded")]
    LimitExceeded,
    #[error("usage source changed since inspection")]
    SourceChanged,
}
pub trait UsageImportPort: Send + Sync {
    fn inspect(&self, source: &RelativePath) -> Result<UsageImportPreview, UsageImportError>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct MigrationInspection {
    pub legacy_protocol: ReviewProtocolVersion,
    pub index_digest: [u8; 32],
    pub active_draft: Option<PersistedReviewDraft>,
    pub completed_candidates: Vec<ReviewSnapshot>,
    pub limitations: Vec<ReviewHistoryLimitation>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum MigrationChoice {
    ContinueSelected {
        legacy_targets: Vec<LegacyTargetRef>,
        bindings: Vec<MigrationBinding>,
    },
    KeepHistoryOnly,
}
#[derive(Clone, Debug, PartialEq)]
pub struct MigrationBinding {
    pub legacy_target: LegacyTargetRef,
    pub new_asset_version_id: AssetVersionId,
    pub anchor: FeedbackAnchor,
    pub position_confirmed: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct MigrationPlan {
    pub inspection_digest: [u8; 32],
    pub choice: MigrationChoice,
}
