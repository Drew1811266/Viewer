use super::{remote_output, wire};
use serde::Serialize;
use viewer_application::{PersistedReviewDraft, ReviewProtocolVersion, review_workspace::*};
use viewer_domain::{review::*, *};

#[derive(Serialize)]
#[serde(remote = "ReviewProtocolVersion")]
enum Protocol {
    #[serde(rename = "viewer.review/1")]
    V1,
    #[serde(rename = "viewer.review/2")]
    V2,
}
remote_output!(ReviewProtocolVersion, Protocol);
#[derive(Serialize)]
#[serde(remote = "LegacyReviewReference", rename_all = "camelCase")]
struct Reference {
    #[serde(with = "wire")]
    stream_id: ReviewStreamId,
    #[serde(with = "wire")]
    round_id: ReviewRoundId,
    #[serde(with = "wire")]
    protocol: ReviewProtocolVersion,
    is_draft: bool,
    #[serde(with = "wire")]
    blake3: [u8; 32],
}
remote_output!(LegacyReviewReference, Reference);
#[derive(Serialize)]
#[serde(remote = "LegacyReviewRecord", rename_all = "camelCase")]
struct Record {
    #[serde(with = "wire")]
    reference: LegacyReviewReference,
    #[serde(with = "wire")]
    contents: LegacyReviewContents,
}
remote_output!(LegacyReviewRecord, Record);
#[derive(Serialize)]
#[serde(
    remote = "LegacyReviewContents",
    tag = "kind",
    content = "record",
    rename_all = "snake_case"
)]
enum Contents {
    Draft(#[serde(with = "wire")] ReviewDraft),
    Completed(#[serde(with = "wire")] ReviewSnapshot),
}
remote_output!(LegacyReviewContents, Contents);
#[derive(Serialize)]
#[serde(remote = "PersistedReviewDraft", rename_all = "camelCase")]
struct Persisted {
    #[serde(with = "wire")]
    protocol_version: ReviewProtocolVersion,
    #[serde(with = "wire")]
    draft: ReviewDraft,
}
remote_output!(PersistedReviewDraft, Persisted);
#[derive(Serialize)]
#[serde(remote = "ReviewDraft", rename_all = "camelCase")]
struct Draft {
    #[serde(with = "wire")]
    project_id: ProjectId,
    #[serde(with = "wire")]
    review_stream_id: ReviewStreamId,
    #[serde(with = "wire")]
    review_round_id: ReviewRoundId,
    #[serde(with = "wire")]
    production: Option<ProductionScope>,
    #[serde(with = "wire")]
    previous_completed_round_id: Option<ReviewRoundId>,
    #[serde(with = "wire")]
    created_at_ms: i64,
    #[serde(with = "wire")]
    assets: Vec<AssetVersion>,
    #[serde(with = "wire")]
    feedback: Vec<Feedback>,
    #[serde(with = "wire")]
    unreviewable: Vec<UnreviewableAsset>,
}
remote_output!(ReviewDraft, Draft);
#[derive(Serialize)]
#[serde(remote = "ReviewSnapshot", rename_all = "camelCase")]
struct Snapshot {
    #[serde(with = "wire")]
    project_id: ProjectId,
    #[serde(with = "wire")]
    review_stream_id: ReviewStreamId,
    #[serde(with = "wire")]
    review_round_id: ReviewRoundId,
    #[serde(with = "wire")]
    production: Option<ProductionScope>,
    #[serde(with = "wire")]
    previous_completed_round_id: Option<ReviewRoundId>,
    #[serde(with = "wire")]
    created_at_ms: i64,
    #[serde(with = "wire")]
    completed_at_ms: i64,
    #[serde(with = "wire")]
    assets: Vec<AssetVersion>,
    #[serde(with = "wire")]
    feedback: Vec<Feedback>,
    #[serde(with = "wire")]
    outcomes: Vec<ReviewOutcome>,
}
remote_output!(ReviewSnapshot, Snapshot);
#[derive(Serialize)]
#[serde(remote = "Feedback", rename_all = "camelCase")]
struct FeedbackValue {
    #[serde(with = "wire")]
    id: FeedbackId,
    #[serde(with = "wire")]
    text: String,
    #[serde(with = "wire")]
    created_at_ms: i64,
    #[serde(with = "wire")]
    targets: Vec<FeedbackTarget>,
}
remote_output!(Feedback, FeedbackValue);
#[derive(Serialize)]
#[serde(remote = "FeedbackTarget", rename_all = "camelCase")]
struct Target {
    #[serde(with = "wire")]
    asset_version_id: AssetVersionId,
    #[serde(with = "wire")]
    anchor: FeedbackAnchor,
}
remote_output!(FeedbackTarget, Target);
#[derive(Serialize)]
#[serde(remote = "UnreviewableAsset", rename_all = "camelCase")]
struct Unreviewable {
    #[serde(with = "wire")]
    asset_version_id: AssetVersionId,
    #[serde(with = "wire")]
    failure: ReviewabilityFailure,
}
remote_output!(UnreviewableAsset, Unreviewable);
#[derive(Serialize)]
#[serde(remote = "ReviewabilityFailure", rename_all = "snake_case")]
enum Failure {
    Unsupported,
    Damaged,
    Unreadable,
    PermissionDenied,
    Missing,
    DecodeFailed,
}
remote_output!(ReviewabilityFailure, Failure);
#[derive(Serialize)]
#[serde(remote = "ReviewOutcome", rename_all = "camelCase")]
struct Outcome {
    #[serde(with = "wire")]
    asset_version_id: AssetVersionId,
    #[serde(with = "wire")]
    kind: ReviewOutcomeKind,
    #[serde(with = "wire")]
    feedback_ids: Vec<FeedbackId>,
    #[serde(with = "wire")]
    failure: Option<ReviewabilityFailure>,
}
remote_output!(ReviewOutcome, Outcome);
#[derive(Serialize)]
#[serde(remote = "ReviewOutcomeKind", rename_all = "snake_case")]
enum OutcomeKind {
    Pass,
    Revise,
    Unreviewable,
}
remote_output!(ReviewOutcomeKind, OutcomeKind);
