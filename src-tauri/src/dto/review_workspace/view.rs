use super::{remote_output, wire};
use serde::Serialize;
use viewer_application::review_workspace::*;
use viewer_domain::{
    review::{continuous::*, *},
    *,
};

#[derive(Serialize)]
#[serde(remote = "ReviewWorkspaceView", rename_all = "camelCase")]
struct View {
    #[serde(with = "wire")]
    stream_id: ReviewStreamId,
    #[serde(with = "wire")]
    history_selectors: Vec<viewer_application::review_evidence::HistorySelector>,
    #[serde(with = "wire")]
    current: Option<ReviewWorkspaceCurrent>,
    #[serde(with = "wire")]
    source_checks: Vec<SourceCheck>,
    #[serde(with = "wire")]
    projection: CurrentReviewProjection,
    #[serde(with = "wire")]
    recovery: Vec<RecoveryDraft>,
    #[serde(with = "wire")]
    migration: Option<MigrationInspection>,
    #[serde(with = "wire")]
    capabilities: ReviewWorkspaceCapabilities,
}
remote_output!(ReviewWorkspaceView, View);
#[derive(Serialize)]
#[serde(remote = "ReviewWorkspaceCapabilities", rename_all = "camelCase")]
struct Capabilities {
    continuous_editing: bool,
    usage_import: bool,
    migration: bool,
}
remote_output!(ReviewWorkspaceCapabilities, Capabilities);
#[derive(Serialize)]
#[serde(remote = "ReviewApplyResult", rename_all = "camelCase")]
struct Apply {
    #[serde(with = "wire")]
    receipt: ReviewCommitReceipt,
    #[serde(with = "wire")]
    view: ReviewWorkspaceView,
}
remote_output!(ReviewApplyResult, Apply);
#[derive(Serialize)]
#[serde(remote = "ReviewCommitReceipt", rename_all = "camelCase")]
struct Receipt {
    #[serde(with = "wire")]
    command_id: ReviewCommandId,
    #[serde(with = "wire")]
    payload_digest: [u8; 32],
    #[serde(with = "wire")]
    snapshot: SnapshotRef,
}
remote_output!(ReviewCommitReceipt, Receipt);

#[derive(Serialize)]
#[serde(remote = "ReviewAuthoringHead", rename_all = "camelCase")]
struct AuthoringHead {
    #[serde(with = "wire")]
    sequence: u64,
    #[serde(with = "wire")]
    snapshot_id: ReviewSnapshotId,
}
remote_output!(ReviewAuthoringHead, AuthoringHead);

#[derive(Serialize)]
#[serde(remote = "ReviewAuthoringReceipt", rename_all = "camelCase")]
struct AuthoringReceipt {
    #[serde(with = "wire")]
    command_id: ReviewCommandId,
    #[serde(with = "wire")]
    payload_digest: [u8; 32],
    #[serde(with = "wire")]
    head: ReviewAuthoringHead,
}
remote_output!(ReviewAuthoringReceipt, AuthoringReceipt);

#[derive(Serialize)]
#[serde(remote = "ReviewBarrierKind", rename_all = "snake_case")]
enum Barrier {
    None,
    Archive,
    Restore,
    Migration,
}
remote_output!(ReviewBarrierKind, Barrier);

#[derive(Serialize)]
#[serde(remote = "ArchiveCheckpoint", rename_all = "camelCase")]
struct Checkpoint {
    #[serde(with = "wire")]
    project_id: ProjectId,
    #[serde(with = "wire")]
    stream_id: ReviewStreamId,
    #[serde(with = "wire")]
    archive_id: ReviewArchiveId,
    #[serde(with = "wire")]
    created_at_ms: i64,
    #[serde(with = "wire")]
    before: SnapshotRef,
    #[serde(with = "wire")]
    groups: Vec<ArchiveGroup>,
    #[serde(with = "wire")]
    removed: Vec<TargetVersionKey>,
    #[serde(with = "wire")]
    retained: Vec<ArchiveRetention>,
}
remote_output!(ArchiveCheckpoint, Checkpoint);

#[derive(Serialize)]
#[serde(remote = "StoredAuthoringState", rename_all = "camelCase")]
struct AuthoringState {
    #[serde(with = "wire")]
    head: ReviewAuthoringHead,
    #[serde(with = "wire")]
    production: Option<ProductionScope>,
    #[serde(with = "wire")]
    state: ContinuousReviewState,
    #[serde(with = "wire")]
    command_id: ReviewCommandId,
    #[serde(with = "wire")]
    payload_digest: [u8; 32],
    #[serde(with = "wire")]
    generated: GeneratedReviewIds,
    #[serde(with = "wire")]
    changes: Vec<ReviewChange>,
    #[serde(with = "wire")]
    archives: Vec<ArchiveCheckpoint>,
    #[serde(with = "wire")]
    adopted_usage: Vec<ReviewUsageDeclaration>,
    #[serde(with = "wire")]
    barrier: ReviewBarrierKind,
}
remote_output!(StoredAuthoringState, AuthoringState);

#[derive(Serialize)]
#[serde(remote = "ReviewWorkspaceCurrent", rename_all = "camelCase")]
struct AuthoringCurrent {
    #[serde(with = "wire")]
    authoring: StoredAuthoringState,
    #[serde(with = "wire")]
    published_ref: Option<SnapshotRef>,
    #[serde(with = "wire")]
    evidence: Vec<ReviewEvidenceBinding>,
}
remote_output!(ReviewWorkspaceCurrent, AuthoringCurrent);

#[derive(Serialize)]
#[serde(remote = "ReviewWorkspacePatch", rename_all = "camelCase")]
struct Patch {
    #[serde(with = "wire", getter = "ReviewWorkspacePatch::project_id")]
    project_id: ProjectId,
    #[serde(with = "wire", getter = "ReviewWorkspacePatch::stream_id")]
    stream_id: ReviewStreamId,
    #[serde(with = "wire", getter = "ReviewWorkspacePatch::parent")]
    parent: Option<SnapshotRef>,
    #[serde(with = "wire")]
    basis_snapshot_id: Option<ReviewSnapshotId>,
    #[serde(with = "wire")]
    head: ReviewAuthoringHead,
    #[serde(with = "wire")]
    upsert_assets: Vec<AssetVersion>,
    #[serde(with = "wire")]
    remove_asset_version_ids: Vec<AssetVersionId>,
    #[serde(with = "wire")]
    upsert_feedback: Vec<VersionedFeedback>,
    #[serde(with = "wire")]
    remove_feedback_ids: Vec<FeedbackId>,
    #[serde(with = "wire")]
    projection: CurrentReviewProjection,
    #[serde(with = "wire")]
    history_selectors: Option<Vec<viewer_application::review_evidence::HistorySelector>>,
}
remote_output!(ReviewWorkspacePatch, Patch);

#[derive(Serialize)]
#[serde(remote = "ReviewAuthoringApplyResult", rename_all = "camelCase")]
struct AuthoringApply {
    #[serde(with = "wire")]
    receipt: ReviewAuthoringReceipt,
    #[serde(with = "wire")]
    patch: ReviewWorkspacePatch,
    #[serde(with = "wire")]
    publication: ReviewPublicationStatus,
}
remote_output!(ReviewAuthoringApplyResult, AuthoringApply);

#[derive(Serialize)]
#[serde(
    remote = "ReviewPublicationStatus",
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum PublicationStatus {
    Ready,
    Pending {
        pending_revisions: u32,
    },
    Blocked {
        #[serde(with = "wire")]
        code: ReviewMaterializationFailure,
    },
}
remote_output!(ReviewPublicationStatus, PublicationStatus);

#[derive(Serialize)]
#[serde(remote = "ReviewMaterializationFailure", rename_all = "snake_case")]
enum MaterializationFailure {
    SourceChanged,
    SourceMissing,
    SourceUnreadable,
    RenderFailed,
    Integrity,
    LimitExceeded,
    Io,
}
remote_output!(ReviewMaterializationFailure, MaterializationFailure);
#[derive(Serialize)]
#[serde(remote = "StoredContinuousSnapshot", rename_all = "camelCase")]
struct Stored {
    #[serde(with = "wire")]
    reference: SnapshotRef,
    #[serde(with = "wire")]
    production: Option<ProductionScope>,
    #[serde(with = "wire")]
    state: ContinuousReviewState,
    #[serde(with = "wire")]
    command_id: ReviewCommandId,
    #[serde(with = "wire")]
    payload_digest: [u8; 32],
    #[serde(with = "wire")]
    changes: Vec<ReviewChange>,
    #[serde(with = "wire")]
    evidence: Vec<ReviewEvidenceBinding>,
}
remote_output!(StoredContinuousSnapshot, Stored);

#[derive(Serialize)]
#[serde(remote = "ContinuousReviewState", rename_all = "camelCase")]
struct State {
    #[serde(with = "wire")]
    project_id: ProjectId,
    #[serde(with = "wire")]
    stream_id: ReviewStreamId,
    #[serde(with = "wire")]
    snapshot_id: ReviewSnapshotId,
    #[serde(with = "wire")]
    parent: Option<SnapshotRef>,
    #[serde(with = "wire")]
    assets: Vec<AssetVersion>,
    #[serde(with = "wire")]
    feedback: Vec<VersionedFeedback>,
}
remote_output!(ContinuousReviewState, State);
#[derive(Serialize)]
#[serde(remote = "AssetVersion", rename_all = "camelCase")]
struct Asset {
    #[serde(with = "wire")]
    id: AssetVersionId,
    #[serde(with = "wire")]
    source_entity_id: Option<EntityId>,
    #[serde(with = "wire")]
    relative_path: RelativePath,
    #[serde(with = "wire")]
    evidence: AssetEvidence,
    #[serde(with = "wire")]
    media: ReviewMedia,
    #[serde(with = "wire")]
    producer_asset_id: Option<ProductionId>,
    #[serde(with = "wire")]
    parent_asset_version_id: Option<AssetVersionId>,
}
remote_output!(AssetVersion, Asset);
#[derive(Serialize)]
#[serde(remote = "AssetEvidence", rename_all = "camelCase")]
struct SourceEvidence {
    #[serde(with = "wire")]
    size_bytes: u64,
    #[serde(with = "wire")]
    modified_ns: i128,
    #[serde(with = "wire")]
    blake3: Option<[u8; 32]>,
}
remote_output!(AssetEvidence, SourceEvidence);
#[derive(Serialize)]
#[serde(
    remote = "ReviewMedia",
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum Media {
    Image {
        #[serde(with = "wire")]
        width: Option<u32>,
        #[serde(with = "wire")]
        height: Option<u32>,
    },
    Video {
        #[serde(with = "wire")]
        duration_us: Option<u64>,
        #[serde(with = "wire")]
        display_width: Option<u32>,
        #[serde(with = "wire")]
        display_height: Option<u32>,
    },
}
remote_output!(ReviewMedia, Media);
#[derive(Serialize)]
#[serde(remote = "VersionedFeedback", rename_all = "camelCase")]
struct Feedback {
    #[serde(with = "wire")]
    id: FeedbackId,
    #[serde(with = "wire")]
    text_revision_id: ReviewTextRevisionId,
    #[serde(with = "wire")]
    text: String,
    #[serde(with = "wire")]
    created_at_ms: i64,
    #[serde(with = "wire")]
    targets: Vec<VersionedTarget>,
    #[serde(with = "wire")]
    history_ref: Option<HistoryRef>,
}
remote_output!(VersionedFeedback, Feedback);
#[derive(Serialize)]
#[serde(remote = "VersionedTarget", rename_all = "camelCase")]
struct Target {
    #[serde(with = "wire")]
    id: ReviewTargetId,
    #[serde(with = "wire")]
    revision_id: ReviewTargetRevisionId,
    #[serde(with = "wire")]
    asset_version_id: AssetVersionId,
    #[serde(with = "wire")]
    anchor: FeedbackAnchor,
    #[serde(with = "wire")]
    availability: ReviewAvailability,
}
remote_output!(VersionedTarget, Target);
#[derive(Serialize)]
#[serde(
    remote = "ReviewAvailability",
    tag = "kind",
    content = "reasons",
    rename_all = "snake_case"
)]
enum Availability {
    Ready,
    NeedsConfirmation(#[serde(with = "wire")] Vec<ReviewPendingReason>),
}
remote_output!(ReviewAvailability, Availability);
#[derive(Serialize)]
#[serde(remote = "ReviewPendingReason", rename_all = "snake_case")]
enum Pending {
    SourceChanged,
    SourceMissing,
    SourceUnreadable,
    SourceUnverified,
    LegacyUsageUnknown,
    LegacyEvidenceAbsent,
    ApplicabilityUnconfirmed,
}
remote_output!(ReviewPendingReason, Pending);
#[derive(Serialize)]
#[serde(remote = "SourceCheck", rename_all = "camelCase")]
struct Check {
    #[serde(with = "wire")]
    asset_version_id: AssetVersionId,
    #[serde(with = "wire")]
    checked_at_ms: i64,
    #[serde(with = "wire")]
    status: SourceCheckStatus,
}
remote_output!(SourceCheck, Check);
#[derive(Serialize)]
#[serde(remote = "SourceCheckStatus", rename_all = "snake_case")]
enum CheckStatus {
    Match,
    Changed,
    Missing,
    Unreadable,
    Unverified,
}
remote_output!(SourceCheckStatus, CheckStatus);
#[derive(Serialize)]
#[serde(remote = "CurrentReviewProjection", rename_all = "camelCase")]
struct Projection {
    #[serde(with = "wire")]
    actionable: Vec<ReviewTargetId>,
    #[serde(with = "wire")]
    needs_confirmation: Vec<ReviewTargetId>,
}
remote_output!(CurrentReviewProjection, Projection);
#[derive(Serialize)]
#[serde(remote = "ReviewChange", rename_all = "camelCase")]
struct Change {
    #[serde(with = "wire")]
    target_id: ReviewTargetId,
    #[serde(with = "wire")]
    before: Option<TargetVersionKey>,
    #[serde(with = "wire")]
    after: Option<TargetVersionKey>,
    #[serde(with = "wire")]
    kind: ReviewChangeKind,
    #[serde(with = "wire")]
    archive_id: Option<ReviewArchiveId>,
    #[serde(with = "wire")]
    historical_key: Option<TargetVersionKey>,
}
remote_output!(ReviewChange, Change);
#[derive(Serialize)]
#[serde(remote = "ReviewChangeKind", rename_all = "snake_case")]
enum ChangeKind {
    Added,
    Edited,
    Withdrawn,
    Archived,
    Restored,
    Rebound,
    AvailabilityChanged,
}
remote_output!(ReviewChangeKind, ChangeKind);
