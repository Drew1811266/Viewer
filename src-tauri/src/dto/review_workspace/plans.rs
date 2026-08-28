use super::{remote_output, wire};
use serde::Serialize;
use std::sync::Arc;
use viewer_domain::{
    review::{AssetVersion, continuous::*},
    *,
};

#[derive(Serialize)]
#[serde(remote = "ArchivePlan", rename_all = "camelCase")]
struct Archive {
    #[serde(with = "wire")]
    expected_snapshot_id: ReviewSnapshotId,
    #[serde(with = "wire")]
    groups: Vec<ArchiveGroup>,
    #[serde(with = "wire")]
    removed: Vec<TargetVersionKey>,
    #[serde(with = "wire")]
    retained: Vec<ArchiveRetention>,
    #[serde(with = "wire")]
    already_covered: Vec<TargetVersionKey>,
}
remote_output!(ArchivePlan, Archive);
#[derive(Serialize)]
#[serde(remote = "ArchiveRetention", rename_all = "camelCase")]
struct Retention {
    #[serde(with = "wire")]
    basis: TargetVersionKey,
    #[serde(with = "wire")]
    current: Option<TargetVersionKey>,
    #[serde(with = "wire")]
    disposition: ArchiveDisposition,
}
remote_output!(ArchiveRetention, Retention);
#[derive(Serialize)]
#[serde(remote = "ArchiveDisposition", rename_all = "snake_case")]
enum Disposition {
    RemoveCurrent,
    RetainLaterEdit,
    AlreadyAbsent,
}
remote_output!(ArchiveDisposition, Disposition);
#[derive(Serialize)]
#[serde(remote = "RestorePlan", rename_all = "camelCase")]
struct Restore {
    #[serde(with = "wire")]
    expected_snapshot_id: ReviewSnapshotId,
    #[serde(with = "wire")]
    restored: Vec<RestoredFeedback>,
    #[serde(with = "wire")]
    conflicts: Vec<TargetVersionKey>,
    #[serde(with = "wire")]
    coverage_reversals: Vec<ArchiveCoverage>,
    #[serde(with = "wire")]
    requires_source_check: Vec<ReviewTargetId>,
}
remote_output!(RestorePlan, Restore);
#[derive(Serialize)]
#[serde(remote = "RestoredFeedback", rename_all = "camelCase")]
struct Restored {
    #[serde(with = "wire")]
    historical_key: TargetVersionKey,
    #[serde(with = "wire")]
    feedback: Arc<RestoreFeedbackContent>,
    #[serde(with = "wire")]
    target: VersionedTarget,
    #[serde(with = "wire")]
    asset: Arc<AssetVersion>,
    continued_as_new: bool,
}
remote_output!(RestoredFeedback, Restored);
#[derive(Serialize)]
#[serde(remote = "RestoreFeedbackContent", rename_all = "camelCase")]
struct Content {
    #[serde(with = "wire")]
    id: FeedbackId,
    #[serde(with = "wire")]
    text_revision_id: ReviewTextRevisionId,
    #[serde(with = "wire")]
    text: Arc<str>,
    #[serde(with = "wire")]
    created_at_ms: i64,
    #[serde(with = "wire")]
    history_ref: Option<HistoryRef>,
}
remote_output!(RestoreFeedbackContent, Content);
#[derive(Serialize)]
#[serde(remote = "ArchiveCoverage", rename_all = "camelCase")]
struct Coverage {
    #[serde(with = "wire")]
    archive_id: ReviewArchiveId,
    #[serde(with = "wire")]
    key: TargetVersionKey,
    active: bool,
}
remote_output!(ArchiveCoverage, Coverage);
