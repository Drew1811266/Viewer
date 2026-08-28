use super::{remote_output, wire};
use serde::Serialize;
use viewer_application::review_workspace::*;
use viewer_domain::{review::continuous::*, *};

#[derive(Serialize)]
#[serde(remote = "RecoveryDraft", rename_all = "camelCase")]
struct Draft {
    #[serde(with = "wire")]
    stream_id: ReviewStreamId,
    #[serde(with = "wire")]
    command_id: ReviewCommandId,
    #[serde(with = "wire")]
    expected_snapshot_id: Option<ReviewSnapshotId>,
    #[serde(with = "wire")]
    payload_digest: [u8; 32],
    #[serde(with = "wire")]
    editor_input: RecoveryEditorInput,
    #[serde(with = "wire")]
    failure: ReviewRecoveryFailure,
}
remote_output!(RecoveryDraft, Draft);
#[derive(Serialize)]
#[serde(remote = "RecoveryEditorInput", rename_all = "camelCase")]
struct Input {
    #[serde(with = "wire")]
    migration: Option<MigrationPlan>,
    #[serde(with = "wire")]
    selections: Vec<RecoveryTargetSelection>,
    #[serde(with = "wire")]
    text: String,
    #[serde(with = "wire")]
    feedback_id: Option<FeedbackId>,
    #[serde(with = "wire")]
    targets: Vec<VersionedTarget>,
    #[serde(with = "wire")]
    history_ref: Option<HistoryRef>,
}
remote_output!(RecoveryEditorInput, Input);
#[derive(Serialize)]
#[serde(remote = "RecoveryTargetSelection", rename_all = "camelCase")]
struct Selection {
    #[serde(with = "wire")]
    origin: RecoveryTargetOrigin,
    #[serde(with = "wire")]
    feedback_id: FeedbackId,
    #[serde(with = "wire")]
    text_revision_id: ReviewTextRevisionId,
    #[serde(with = "wire")]
    target_id: ReviewTargetId,
    #[serde(with = "wire")]
    target_revision_id: ReviewTargetRevisionId,
    #[serde(with = "wire")]
    asset_version_id: AssetVersionId,
    #[serde(with = "wire")]
    confirmation: RecoveryTargetConfirmation,
}
remote_output!(RecoveryTargetSelection, Selection);
#[derive(Serialize)]
#[serde(
    remote = "RecoveryTargetOrigin",
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum Origin {
    Current {
        #[serde(with = "wire")]
        key: TargetVersionKey,
    },
    Snapshot {
        #[serde(with = "wire")]
        key: TargetVersionKey,
    },
    Legacy {
        #[serde(with = "wire")]
        round_id: ReviewRoundId,
        #[serde(with = "wire")]
        feedback_id: FeedbackId,
        target_index: u32,
    },
    Archive {
        #[serde(with = "wire")]
        archive_id: ReviewArchiveId,
        #[serde(with = "wire")]
        key: TargetVersionKey,
    },
}
remote_output!(RecoveryTargetOrigin, Origin);
#[derive(Serialize)]
#[serde(
    remote = "RecoveryTargetConfirmation",
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum Confirmation {
    Unconfirmed {},
    UserConfirmed {},
    ProducerVerified {
        #[serde(with = "wire")]
        usage_id: ReviewUsageId,
    },
}
remote_output!(RecoveryTargetConfirmation, Confirmation);
#[derive(Serialize)]
#[serde(remote = "ReviewRecoveryFailure", rename_all = "snake_case")]
enum Failure {
    RenderFailed,
    SourceChanged,
    WriteFailed,
    CommitUnknown,
    Cancelled,
    StaleSnapshot,
}
remote_output!(ReviewRecoveryFailure, Failure);
