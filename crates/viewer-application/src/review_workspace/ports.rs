use std::{path::PathBuf, sync::Arc};
use viewer_domain::review::{ProductionScope, continuous::*};
use viewer_domain::{
    AssetVersionId, ProjectId, RelativePath, ReviewArchiveId, ReviewCommandId, ReviewStreamId,
    ReviewUsageId,
};

#[derive(Clone, Debug, PartialEq)]
pub struct RecoveryDraft {
    pub stream_id: ReviewStreamId,
    pub command_id: ReviewCommandId,
    pub expected_snapshot_id: Option<viewer_domain::ReviewSnapshotId>,
    pub payload_digest: [u8; 32],
    pub editor_input: RecoveryEditorInput,
    pub failure: ReviewRecoveryFailure,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecoveryEditorInput {
    pub migration: Option<super::MigrationPlan>,
    pub selections: Vec<RecoveryTargetSelection>,
    pub text: String,
    pub feedback_id: Option<viewer_domain::FeedbackId>,
    pub targets: Vec<VersionedTarget>,
    pub history_ref: Option<HistoryRef>,
}

/// Unpublished editor choices. These are recovery input, not executable feedback or an
/// authorization to replay an old command against a changed current snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryTargetSelection {
    pub origin: RecoveryTargetOrigin,
    pub feedback_id: viewer_domain::FeedbackId,
    pub text_revision_id: viewer_domain::ReviewTextRevisionId,
    pub target_id: viewer_domain::ReviewTargetId,
    pub target_revision_id: viewer_domain::ReviewTargetRevisionId,
    pub asset_version_id: AssetVersionId,
    pub confirmation: RecoveryTargetConfirmation,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryTargetOrigin {
    Current {
        key: TargetVersionKey,
    },
    Snapshot {
        key: TargetVersionKey,
    },
    Legacy {
        round_id: viewer_domain::ReviewRoundId,
        feedback_id: viewer_domain::FeedbackId,
        target_index: u32,
    },
    Archive {
        archive_id: ReviewArchiveId,
        key: TargetVersionKey,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryTargetConfirmation {
    Unconfirmed,
    UserConfirmed,
    ProducerVerified { usage_id: ReviewUsageId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewRecoveryFailure {
    RenderFailed,
    SourceChanged,
    WriteFailed,
    CommitUnknown,
    Cancelled,
    StaleSnapshot,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedContinuousSnapshot {
    pub state: ContinuousReviewState,
    pub command_id: ReviewCommandId,
    pub payload_digest: [u8; 32],
    pub changes: Vec<ReviewChange>,
    pub evidence: Vec<ReviewEvidenceBinding>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredContinuousSnapshot {
    pub reference: SnapshotRef,
    pub production: Option<ProductionScope>,
    pub state: ContinuousReviewState,
    pub command_id: ReviewCommandId,
    pub payload_digest: [u8; 32],
    pub changes: Vec<ReviewChange>,
    pub evidence: Vec<ReviewEvidenceBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceRef {
    pub blake3: [u8; 32],
    pub size_bytes: u64,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceAnnotation {
    pub ordinal: u32,
    pub key: TargetVersionKey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvidenceCapability {
    Image {
        base: EvidenceRef,
        annotated: Option<EvidenceRef>,
        annotations: Vec<EvidenceAnnotation>,
    },
    LegacyAbsent,
    NotImage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewEvidenceBinding {
    pub asset_version_id: AssetVersionId,
    pub capability: EvidenceCapability,
}

/// An application-owned temporary artifact, never an IPC-supplied source path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedEvidenceFile {
    pub path: PathBuf,
    pub reference: EvidenceRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewUsageDeclaration {
    pub id: ReviewUsageId,
    pub project_id: ProjectId,
    pub stream_id: ReviewStreamId,
    pub basis: SnapshotRef,
    pub targets: Vec<TargetVersionKey>,
    pub outputs: Vec<UsageOutput>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageOutput {
    pub relative_path: RelativePath,
    pub blake3: [u8; 32],
    pub previous_asset_version_id: AssetVersionId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewCommitRequest {
    pub expected: Option<SnapshotRef>,
    /// Existing streams must keep their original scope. None is the unique manual stream.
    pub production: Option<ProductionScope>,
    pub next: PreparedContinuousSnapshot,
    pub archives: Vec<ArchiveCheckpoint>,
    pub adopted_usage: Vec<ReviewUsageDeclaration>,
    pub staged_evidence: Vec<PreparedEvidenceFile>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewCommitReceipt {
    pub command_id: ReviewCommandId,
    pub payload_digest: [u8; 32],
    pub snapshot: SnapshotRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandLookup {
    Found(ReviewCommitReceipt),
    Absent,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ReviewCommitError {
    #[error("legacy review data requires explicit migration")]
    MigrationRequired,
    #[error("unsupported review protocol version")]
    UnsupportedProtocol,
    #[error("review snapshot changed")]
    StaleSnapshot,
    #[error("command identity has a different payload")]
    CommandConflict,
    #[error("repository is read-only")]
    ReadOnly,
    #[error("review writer lease is busy")]
    LeaseBusy,
    #[error("review repository integrity check failed")]
    Integrity,
    #[error("review resource limit exceeded")]
    LimitExceeded,
    #[error("committed command history is unavailable")]
    LookupUnavailable,
    #[error("this historical record did not declare the requested image evidence")]
    EvidenceAbsent,
    #[error("archive contains multiple image revisions; select an exact snapshot")]
    AmbiguousEvidence,
    #[error("review repository IO failed")]
    Io,
    #[error("commit outcome is unknown; look up the original command before retrying")]
    OutcomeUnknown,
}

pub trait ContinuousReviewRepositoryPort: Send + Sync {
    fn load_unresolved_recovery(
        &self,
        stream: ReviewStreamId,
    ) -> Result<Vec<RecoveryDraft>, ReviewCommitError> {
        self.load_recovery()?
            .into_iter()
            .filter(|d| d.stream_id == stream)
            .filter_map(|d| match self.resolve_recovery(d.command_id) {
                Ok(CommandLookup::Found(_)) => None,
                Ok(_) => Some(Ok(d)),
                Err(e) => Some(Err(e)),
            })
            .collect()
    }
    fn load_legacy(
        &self,
        _stream: ReviewStreamId,
        _round: viewer_domain::ReviewRoundId,
    ) -> Result<super::LegacyReviewRecord, ReviewCommitError> {
        Err(ReviewCommitError::EvidenceAbsent)
    }
    fn load_usage(
        &self,
        stream_id: ReviewStreamId,
        id: ReviewUsageId,
    ) -> Result<Option<ReviewUsageDeclaration>, ReviewCommitError>;
    /// Coverage at an exact committed head, not at a guessed latest state.
    fn load_coverage(
        &self,
        stream_id: ReviewStreamId,
        head: SnapshotRef,
        keys: &[TargetVersionKey],
    ) -> Result<Vec<ArchiveCoverage>, ReviewCommitError>;
    fn load_evidence(
        &self,
        stream_id: ReviewStreamId,
        selector: &crate::review_evidence::HistorySelector,
        asset_version_id: AssetVersionId,
        role: crate::review_evidence::EvidenceRole,
    ) -> Result<crate::review_evidence::BoundReviewImage, ReviewCommitError>;
    fn save_recovery(&self, draft: &RecoveryDraft) -> Result<(), ReviewCommitError>;
    fn load_recovery(&self) -> Result<Vec<RecoveryDraft>, ReviewCommitError>;
    fn resolve_recovery(
        &self,
        command_id: ReviewCommandId,
    ) -> Result<CommandLookup, ReviewCommitError>;
    fn load_current_ref(
        &self,
        stream_id: ReviewStreamId,
    ) -> Result<Option<SnapshotRef>, ReviewCommitError>;
    fn load_current(
        &self,
        stream_id: ReviewStreamId,
    ) -> Result<Option<StoredContinuousSnapshot>, ReviewCommitError>;
    fn load_snapshot(
        &self,
        stream_id: ReviewStreamId,
        reference: &SnapshotRef,
    ) -> Result<StoredContinuousSnapshot, ReviewCommitError>;
    fn load_archive(
        &self,
        stream_id: ReviewStreamId,
        archive_id: ReviewArchiveId,
    ) -> Result<ArchiveCheckpoint, ReviewCommitError>;
    fn find_command(
        &self,
        stream_id: ReviewStreamId,
        command_id: ReviewCommandId,
    ) -> Result<CommandLookup, ReviewCommitError>;
    fn commit(
        &self,
        request: ReviewCommitRequest,
    ) -> Result<ReviewCommitReceipt, ReviewCommitError>;
}

pub trait ContinuousReviewRepositoryProviderPort: Send + Sync {
    fn save_migration_recovery(&self, _draft: &RecoveryDraft) -> Result<(), ReviewCommitError> {
        Err(ReviewCommitError::MigrationRequired)
    }
    fn load_migration_recovery(&self) -> Result<Vec<RecoveryDraft>, ReviewCommitError> {
        Ok(vec![])
    }
    fn inspect_migration(&self) -> Result<Option<super::MigrationInspection>, ReviewCommitError> {
        Ok(None)
    }
    fn migrate(
        &self,
        _request: super::MigrationCommitRequest,
    ) -> Result<ReviewCommitReceipt, ReviewCommitError> {
        Err(ReviewCommitError::MigrationRequired)
    }
    fn open_reader(&self) -> Result<Arc<dyn ContinuousReviewRepositoryPort>, ReviewCommitError>;
    fn open_writer(&self) -> Result<Arc<dyn ContinuousReviewRepositoryPort>, ReviewCommitError>;
}
