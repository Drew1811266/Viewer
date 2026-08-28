use super::{remote_output, wire};
use serde::Serialize;
use viewer_application::{
    PersistedReviewDraft, ReviewProtocolVersion, review_evidence::HistorySelector,
    review_workspace::*,
};
use viewer_domain::{
    review::{AssetVersion, ReviewSnapshot, continuous::*},
    *,
};

#[derive(Serialize)]
#[serde(remote = "HistoryView", rename_all = "camelCase")]
struct History {
    #[serde(with = "wire")]
    selector: HistorySelector,
    #[serde(with = "wire")]
    entries: Vec<HistoryEntry>,
    #[serde(with = "wire")]
    legacy: Option<LegacyReviewRecord>,
    #[serde(with = "wire")]
    limitations: Vec<ReviewHistoryLimitation>,
    #[serde(with = "wire")]
    restore_actions: Vec<TargetVersionKey>,
}
remote_output!(HistoryView, History);
#[derive(Serialize)]
#[serde(remote = "HistoryEntry", rename_all = "camelCase")]
struct Entry {
    #[serde(with = "wire")]
    snapshot: SnapshotRef,
    #[serde(with = "wire")]
    feedback: Vec<VersionedFeedback>,
    #[serde(with = "wire")]
    assets: Vec<AssetVersion>,
    #[serde(with = "wire")]
    evidence: Vec<ReviewEvidenceBinding>,
    #[serde(with = "wire")]
    selected: Vec<TargetVersionKey>,
}
remote_output!(HistoryEntry, Entry);
#[derive(Serialize)]
#[serde(remote = "ReviewHistoryLimitation", rename_all = "snake_case")]
enum Limitation {
    BackgroundOnly,
    LegacyEvidenceAbsent,
    ExternalCopiesCannotBeRevoked,
    UsageUnconfirmed,
}
remote_output!(ReviewHistoryLimitation, Limitation);
#[derive(Serialize)]
#[serde(remote = "MigrationInspection", rename_all = "camelCase")]
struct Inspection {
    #[serde(with = "wire")]
    legacy_protocol: ReviewProtocolVersion,
    #[serde(with = "wire")]
    index_digest: [u8; 32],
    #[serde(with = "wire")]
    inspection_digest: [u8; 32],
    #[serde(with = "wire")]
    legacy_records: Vec<LegacyReviewReference>,
    #[serde(with = "wire")]
    active_draft: Option<PersistedReviewDraft>,
    #[serde(with = "wire")]
    completed_candidates: Vec<ReviewSnapshot>,
    #[serde(with = "wire")]
    limitations: Vec<ReviewHistoryLimitation>,
}
remote_output!(MigrationInspection, Inspection);
#[derive(Serialize)]
#[serde(remote = "UsageImportPreview", rename_all = "camelCase")]
struct UsagePreview {
    #[serde(with = "wire")]
    declaration: ReviewUsageDeclaration,
    #[serde(with = "wire")]
    canonical_digest: [u8; 32],
    #[serde(with = "wire")]
    source: RelativePath,
    #[serde(with = "wire")]
    source_digest: [u8; 32],
    #[serde(with = "wire")]
    outputs: Vec<UsageOutputCheck>,
}
remote_output!(UsageImportPreview, UsagePreview);
#[derive(Serialize)]
#[serde(remote = "ReviewUsageDeclaration", rename_all = "camelCase")]
struct Usage {
    #[serde(with = "wire")]
    id: ReviewUsageId,
    #[serde(with = "wire")]
    project_id: ProjectId,
    #[serde(with = "wire")]
    stream_id: ReviewStreamId,
    #[serde(with = "wire")]
    basis: SnapshotRef,
    #[serde(with = "wire")]
    targets: Vec<TargetVersionKey>,
    #[serde(with = "wire")]
    outputs: Vec<UsageOutput>,
}
remote_output!(ReviewUsageDeclaration, Usage);
#[derive(Serialize)]
#[serde(remote = "UsageOutput", rename_all = "camelCase")]
struct Output {
    #[serde(with = "wire")]
    relative_path: RelativePath,
    #[serde(with = "wire")]
    blake3: [u8; 32],
    #[serde(with = "wire")]
    previous_asset_version_id: AssetVersionId,
}
remote_output!(UsageOutput, Output);
#[derive(Serialize)]
#[serde(remote = "UsageOutputCheck", rename_all = "camelCase")]
struct OutputCheck {
    #[serde(with = "wire")]
    output: UsageOutput,
    #[serde(with = "wire")]
    status: UsageOutputStatus,
}
remote_output!(UsageOutputCheck, OutputCheck);
#[derive(Serialize)]
#[serde(remote = "UsageOutputStatus", rename_all = "snake_case")]
enum OutputStatus {
    VerifiedCandidate,
    UnknownPreviousAsset,
    Missing,
    Changed,
    Unsafe,
    Unreadable,
}
remote_output!(UsageOutputStatus, OutputStatus);
