use crate::error::{CommandError, ErrorCategory};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::str::FromStr;
use viewer_application::{
    AddReviewFeedback, DeleteReviewFeedback, ReplaceReviewFeedbackAnchor, ReviewAssetConflictKind,
    ReviewCompletionProposal, ReviewCompletionProposalId, ReviewConflictSnapshot,
    ReviewFeedbackSnapshot, ReviewFeedbackTargetInput, ReviewFeedbackTargetSnapshot,
    ReviewMemberSnapshot, ReviewMutationGuard, ReviewProposalId, ReviewScope, ReviewScopeProposal,
    ReviewSessionCounts, ReviewSessionPhase, ReviewSessionSnapshot, ReviewUnreviewableSnapshot,
    ReviewUserError, StartReviewWithFeedback, UpdateReviewFeedback, UpdateReviewFeedbackText,
};
use viewer_domain::review::{
    FeedbackAnchor, ImageStroke, MAX_ASSETS_PER_ROUND, MAX_FEEDBACK_TEXT_BYTES,
    MAX_TARGETS_PER_FEEDBACK, NormalizedArrow, NormalizedPoint, NormalizedRect, ReviewAssetKind,
    ReviewabilityFailure,
};
use viewer_domain::search::Generation;
use viewer_domain::{EntityId, FeedbackId, ReviewRoundId, SessionId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewRequestContext {
    pub session_id: SessionId,
    pub generation: Generation,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionGenerationRequestDto {
    pub session_id: String,
    pub generation: u64,
}

impl SessionGenerationRequestDto {
    pub fn try_into_context(self) -> Result<ReviewRequestContext, CommandError> {
        request_context(&self.session_id, self.generation)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ReviewScopeRequestDto {
    Selection {
        entity_ids: Vec<String>,
    },
    Folder {
        folder_id: Option<String>,
        include_descendants: bool,
    },
}

impl ReviewScopeRequestDto {
    pub fn try_into_scope(self) -> Result<ReviewScope, CommandError> {
        match self {
            Self::Selection { entity_ids } => Ok(ReviewScope::Selection {
                entity_ids: parse_entity_ids(&entity_ids, MAX_ASSETS_PER_ROUND)?,
            }),
            Self::Folder {
                folder_id,
                include_descendants,
            } => Ok(ReviewScope::Folder {
                folder_id: folder_id.as_deref().map(parse_entity_id).transpose()?,
                include_descendants,
            }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewPreviewStartRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub scope: ReviewScopeRequestDto,
}

impl ReviewPreviewStartRequestDto {
    pub fn try_into_parts(self) -> Result<(ReviewRequestContext, ReviewScope), CommandError> {
        Ok((
            request_context(&self.session_id, self.generation)?,
            self.scope.try_into_scope()?,
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewStartRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub proposal_id: u64,
}

impl ReviewStartRequestDto {
    pub fn try_into_parts(self) -> Result<(ReviewRequestContext, ReviewProposalId), CommandError> {
        Ok((
            request_context(&self.session_id, self.generation)?,
            ReviewProposalId::from_raw(self.proposal_id).ok_or_else(review_invalid_data)?,
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewGuardRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub review_round_id: String,
    pub expected_revision: u64,
}

impl ReviewGuardRequestDto {
    pub fn try_into_parts(
        self,
    ) -> Result<(ReviewRequestContext, ReviewMutationGuard), CommandError> {
        guard_parts(
            &self.session_id,
            self.generation,
            &self.review_round_id,
            self.expected_revision,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ReviewAnchorDto {
    Asset,
    ImagePoint {
        x: f64,
        y: f64,
    },
    ImageArrow {
        tail: ReviewPointDto,
        head: ReviewPointDto,
    },
    ImageRect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    ImageEllipse {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    ImageStroke {
        points: Vec<ReviewPointDto>,
    },
    VideoPoint {
        position_us: u64,
    },
    VideoRange {
        start_us: u64,
        end_us: u64,
    },
}

impl ReviewAnchorDto {
    fn try_into_anchor(self) -> Result<FeedbackAnchor, CommandError> {
        match self {
            Self::Asset => Ok(FeedbackAnchor::Asset),
            Self::ImagePoint { x, y } => NormalizedPoint::new(x, y)
                .map(FeedbackAnchor::ImagePoint)
                .map_err(|_| review_invalid_data()),
            Self::ImageArrow { tail, head } => {
                let tail =
                    NormalizedPoint::new(tail.x, tail.y).map_err(|_| review_invalid_data())?;
                let head =
                    NormalizedPoint::new(head.x, head.y).map_err(|_| review_invalid_data())?;
                NormalizedArrow::new(tail, head)
                    .map(FeedbackAnchor::ImageArrow)
                    .map_err(|_| review_invalid_data())
            }
            Self::ImageRect {
                x,
                y,
                width,
                height,
            } => NormalizedRect::new(x, y, width, height)
                .map(FeedbackAnchor::ImageRect)
                .map_err(|_| review_invalid_data()),
            Self::ImageEllipse {
                x,
                y,
                width,
                height,
            } => NormalizedRect::new(x, y, width, height)
                .map(FeedbackAnchor::ImageEllipse)
                .map_err(|_| review_invalid_data()),
            Self::ImageStroke { points } => points
                .into_iter()
                .map(|point| {
                    NormalizedPoint::new(point.x, point.y).map_err(|_| review_invalid_data())
                })
                .collect::<Result<Vec<_>, _>>()
                .and_then(|points| {
                    ImageStroke::new(points)
                        .map(FeedbackAnchor::ImageStroke)
                        .map_err(|_| review_invalid_data())
                }),
            Self::VideoPoint { position_us } => Ok(FeedbackAnchor::VideoPoint { position_us }),
            Self::VideoRange { start_us, end_us } if start_us < end_us => {
                Ok(FeedbackAnchor::VideoRange { start_us, end_us })
            }
            Self::VideoRange { .. } => Err(review_invalid_data()),
        }
    }
}

impl From<FeedbackAnchor> for ReviewAnchorDto {
    fn from(anchor: FeedbackAnchor) -> Self {
        match anchor {
            FeedbackAnchor::Asset => Self::Asset,
            FeedbackAnchor::ImagePoint(point) => Self::ImagePoint {
                x: point.x(),
                y: point.y(),
            },
            FeedbackAnchor::ImageArrow(arrow) => Self::ImageArrow {
                tail: ReviewPointDto {
                    x: arrow.tail().x(),
                    y: arrow.tail().y(),
                },
                head: ReviewPointDto {
                    x: arrow.head().x(),
                    y: arrow.head().y(),
                },
            },
            FeedbackAnchor::ImageRect(rect) => Self::ImageRect {
                x: rect.x(),
                y: rect.y(),
                width: rect.width(),
                height: rect.height(),
            },
            FeedbackAnchor::ImageEllipse(rect) => Self::ImageEllipse {
                x: rect.x(),
                y: rect.y(),
                width: rect.width(),
                height: rect.height(),
            },
            FeedbackAnchor::ImageStroke(stroke) => Self::ImageStroke {
                points: stroke
                    .points()
                    .iter()
                    .map(|point| ReviewPointDto {
                        x: point.x(),
                        y: point.y(),
                    })
                    .collect(),
            },
            FeedbackAnchor::VideoPoint { position_us } => Self::VideoPoint { position_us },
            FeedbackAnchor::VideoRange { start_us, end_us } => {
                Self::VideoRange { start_us, end_us }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewPointDto {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewFeedbackTargetRequestDto {
    pub entity_id: String,
    pub anchor: ReviewAnchorDto,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewStartWithFeedbackRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub proposal_id: u64,
    pub text: String,
    pub targets: Vec<ReviewFeedbackTargetRequestDto>,
}

impl ReviewStartWithFeedbackRequestDto {
    pub fn try_into_parts(
        self,
    ) -> Result<(ReviewRequestContext, StartReviewWithFeedback), CommandError> {
        validate_feedback_text(&self.text)?;
        Ok((
            request_context(&self.session_id, self.generation)?,
            StartReviewWithFeedback {
                proposal_id: ReviewProposalId::from_raw(self.proposal_id)
                    .ok_or_else(review_invalid_data)?,
                text: self.text,
                targets: parse_feedback_targets(self.targets)?,
            },
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewAddFeedbackRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub review_round_id: String,
    pub expected_revision: u64,
    pub text: String,
    pub targets: Vec<ReviewFeedbackTargetRequestDto>,
}

impl ReviewAddFeedbackRequestDto {
    pub fn try_into_parts(self) -> Result<(ReviewRequestContext, AddReviewFeedback), CommandError> {
        let (context, guard) = guard_parts(
            &self.session_id,
            self.generation,
            &self.review_round_id,
            self.expected_revision,
        )?;
        validate_feedback_text(&self.text)?;
        Ok((
            context,
            AddReviewFeedback {
                guard,
                text: self.text,
                targets: parse_feedback_targets(self.targets)?,
            },
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewUpdateFeedbackRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub review_round_id: String,
    pub expected_revision: u64,
    pub feedback_id: String,
    pub text: String,
    pub targets: Vec<ReviewFeedbackTargetRequestDto>,
}

impl ReviewUpdateFeedbackRequestDto {
    pub fn try_into_parts(
        self,
    ) -> Result<(ReviewRequestContext, UpdateReviewFeedback), CommandError> {
        let (context, guard) = guard_parts(
            &self.session_id,
            self.generation,
            &self.review_round_id,
            self.expected_revision,
        )?;
        validate_feedback_text(&self.text)?;
        Ok((
            context,
            UpdateReviewFeedback {
                guard,
                feedback_id: FeedbackId::from_str(&self.feedback_id)
                    .map_err(|_| review_invalid_data())?,
                text: self.text,
                targets: parse_feedback_targets(self.targets)?,
            },
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewUpdateFeedbackTextRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub review_round_id: String,
    pub expected_revision: u64,
    pub feedback_id: String,
    pub text: String,
}

impl ReviewUpdateFeedbackTextRequestDto {
    pub fn try_into_parts(
        self,
    ) -> Result<(ReviewRequestContext, UpdateReviewFeedbackText), CommandError> {
        let (context, guard) = guard_parts(
            &self.session_id,
            self.generation,
            &self.review_round_id,
            self.expected_revision,
        )?;
        validate_feedback_text(&self.text)?;
        Ok((
            context,
            UpdateReviewFeedbackText {
                guard,
                feedback_id: FeedbackId::from_str(&self.feedback_id)
                    .map_err(|_| review_invalid_data())?,
                text: self.text,
            },
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewReplaceFeedbackAnchorRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub review_round_id: String,
    pub expected_revision: u64,
    pub feedback_id: String,
    pub target: ReviewFeedbackTargetRequestDto,
}

impl ReviewReplaceFeedbackAnchorRequestDto {
    pub fn try_into_parts(
        self,
    ) -> Result<(ReviewRequestContext, ReplaceReviewFeedbackAnchor), CommandError> {
        let (context, guard) = guard_parts(
            &self.session_id,
            self.generation,
            &self.review_round_id,
            self.expected_revision,
        )?;
        let mut targets = parse_feedback_targets(vec![self.target])?;
        if !matches!(
            targets.first().map(|target| &target.anchor),
            Some(
                FeedbackAnchor::ImagePoint(_)
                    | FeedbackAnchor::ImageArrow(_)
                    | FeedbackAnchor::ImageStroke(_)
                    | FeedbackAnchor::ImageRect(_)
                    | FeedbackAnchor::ImageEllipse(_)
            )
        ) {
            return Err(review_invalid_data());
        }
        Ok((
            context,
            ReplaceReviewFeedbackAnchor {
                guard,
                feedback_id: FeedbackId::from_str(&self.feedback_id)
                    .map_err(|_| review_invalid_data())?,
                target: targets.pop().ok_or_else(review_invalid_data)?,
            },
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewDeleteFeedbackRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub review_round_id: String,
    pub expected_revision: u64,
    pub feedback_id: String,
}

impl ReviewDeleteFeedbackRequestDto {
    pub fn try_into_parts(
        self,
    ) -> Result<(ReviewRequestContext, DeleteReviewFeedback), CommandError> {
        let (context, guard) = guard_parts(
            &self.session_id,
            self.generation,
            &self.review_round_id,
            self.expected_revision,
        )?;
        Ok((
            context,
            DeleteReviewFeedback {
                guard,
                feedback_id: FeedbackId::from_str(&self.feedback_id)
                    .map_err(|_| review_invalid_data())?,
            },
        ))
    }
}

pub type ReviewRestoreDeletedFeedbackRequestDto = ReviewDeleteFeedbackRequestDto;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewCompleteRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub review_round_id: String,
    pub expected_revision: u64,
    pub proposal_id: u64,
}

impl ReviewCompleteRequestDto {
    pub fn try_into_parts(
        self,
    ) -> Result<
        (
            ReviewRequestContext,
            ReviewCompletionProposalId,
            ReviewMutationGuard,
        ),
        CommandError,
    > {
        let (context, guard) = guard_parts(
            &self.session_id,
            self.generation,
            &self.review_round_id,
            self.expected_revision,
        )?;
        Ok((
            context,
            ReviewCompletionProposalId::from_raw(self.proposal_id)
                .ok_or_else(review_invalid_data)?,
            guard,
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewTaskKindDto {
    Start,
    Resume,
    CompletionSummary,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewSessionPhaseDto {
    Idle,
    Preparing,
    Active,
    Completing,
    CompletedReadOnly,
    WriteUnavailable,
    RecoveryRequired,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewProgressDto {
    pub session_id: String,
    pub generation: u64,
    pub task_kind: ReviewTaskKindDto,
    pub completed: u32,
    pub total: u32,
    pub cancellable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewScopeResolutionDto {
    pub candidate_count: u32,
    pub image_count: u32,
    pub video_count: u32,
    pub excluded_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewScopeProposalDto {
    pub proposal_id: u64,
    pub resolution: ReviewScopeResolutionDto,
}

impl From<ReviewScopeProposal> for ReviewScopeProposalDto {
    fn from(proposal: ReviewScopeProposal) -> Self {
        Self {
            proposal_id: proposal.id.get(),
            resolution: ReviewScopeResolutionDto {
                candidate_count: proposal.resolution.candidate_entity_ids.len() as u32,
                image_count: proposal.resolution.image_count,
                video_count: proposal.resolution.video_count,
                excluded_count: proposal.resolution.excluded_count,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewAssetKindDto {
    Image,
    Video,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewMemberSnapshotDto {
    pub asset_version_id: String,
    pub entity_id: Option<String>,
    pub relative_path: String,
    pub display_name: String,
    pub kind: ReviewAssetKindDto,
    pub feedback_items: u32,
}

impl From<ReviewMemberSnapshot> for ReviewMemberSnapshotDto {
    fn from(member: ReviewMemberSnapshot) -> Self {
        Self {
            asset_version_id: member.asset_version_id.to_string(),
            entity_id: member.entity_id.map(|id| id.to_string()),
            relative_path: member.relative_path.as_str().to_owned(),
            display_name: member.display_name,
            kind: match member.kind {
                ReviewAssetKind::Image => ReviewAssetKindDto::Image,
                ReviewAssetKind::Video => ReviewAssetKindDto::Video,
            },
            feedback_items: member.feedback_items,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFeedbackTargetSnapshotDto {
    pub asset_version_id: String,
    pub entity_id: Option<String>,
    pub anchor: ReviewAnchorDto,
}

impl From<ReviewFeedbackTargetSnapshot> for ReviewFeedbackTargetSnapshotDto {
    fn from(target: ReviewFeedbackTargetSnapshot) -> Self {
        Self {
            asset_version_id: target.asset_version_id.to_string(),
            entity_id: target.entity_id.map(|id| id.to_string()),
            anchor: target.anchor.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFeedbackSnapshotDto {
    pub feedback_id: String,
    pub text: String,
    pub created_at_ms: i64,
    pub target_entity_ids: Vec<String>,
    pub targets: Vec<ReviewFeedbackTargetSnapshotDto>,
    pub target_count: u32,
}

impl From<ReviewFeedbackSnapshot> for ReviewFeedbackSnapshotDto {
    fn from(feedback: ReviewFeedbackSnapshot) -> Self {
        Self {
            feedback_id: feedback.feedback_id.to_string(),
            text: feedback.text,
            created_at_ms: feedback.created_at_ms,
            target_entity_ids: feedback
                .target_entity_ids
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
            targets: feedback.targets.into_iter().map(Into::into).collect(),
            target_count: feedback.target_count,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewabilityFailureDto {
    Unsupported,
    Damaged,
    Unreadable,
    PermissionDenied,
    Missing,
    DecodeFailed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewUnreviewableSnapshotDto {
    pub asset_version_id: String,
    pub relative_path: String,
    pub failure: ReviewabilityFailureDto,
}

impl From<ReviewUnreviewableSnapshot> for ReviewUnreviewableSnapshotDto {
    fn from(item: ReviewUnreviewableSnapshot) -> Self {
        Self {
            asset_version_id: item.asset_version_id.to_string(),
            relative_path: item.relative_path.as_str().to_owned(),
            failure: item.failure.into(),
        }
    }
}

impl From<ReviewabilityFailure> for ReviewabilityFailureDto {
    fn from(failure: ReviewabilityFailure) -> Self {
        match failure {
            ReviewabilityFailure::Unsupported => Self::Unsupported,
            ReviewabilityFailure::Damaged => Self::Damaged,
            ReviewabilityFailure::Unreadable => Self::Unreadable,
            ReviewabilityFailure::PermissionDenied => Self::PermissionDenied,
            ReviewabilityFailure::Missing => Self::Missing,
            ReviewabilityFailure::DecodeFailed => Self::DecodeFailed,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewConflictKindDto {
    Missing,
    Moved,
    Replaced,
    SizeChanged,
    ContentChanged,
    MediaChanged,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewConflictSnapshotDto {
    pub asset_version_id: String,
    pub relative_path: String,
    pub kind: ReviewConflictKindDto,
}

impl From<ReviewConflictSnapshot> for ReviewConflictSnapshotDto {
    fn from(conflict: ReviewConflictSnapshot) -> Self {
        Self {
            asset_version_id: conflict.asset_version_id.to_string(),
            relative_path: conflict.relative_path.as_str().to_owned(),
            kind: match conflict.kind {
                ReviewAssetConflictKind::Missing => ReviewConflictKindDto::Missing,
                ReviewAssetConflictKind::Moved => ReviewConflictKindDto::Moved,
                ReviewAssetConflictKind::Replaced => ReviewConflictKindDto::Replaced,
                ReviewAssetConflictKind::SizeChanged => ReviewConflictKindDto::SizeChanged,
                ReviewAssetConflictKind::ContentChanged => ReviewConflictKindDto::ContentChanged,
                ReviewAssetConflictKind::MediaChanged => ReviewConflictKindDto::MediaChanged,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSessionCountsDto {
    pub total: u32,
    pub feedback_items: u32,
    pub revise: u32,
    pub unreviewable: u32,
    pub pass: u32,
}

impl From<ReviewSessionCounts> for ReviewSessionCountsDto {
    fn from(counts: ReviewSessionCounts) -> Self {
        Self {
            total: counts.total,
            feedback_items: counts.feedback_items,
            revise: counts.revise,
            unreviewable: counts.unreviewable,
            pass: counts.pass,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewUserErrorDto {
    pub code: String,
    pub retryable: bool,
    pub affected_paths: Vec<String>,
}

impl From<ReviewUserError> for ReviewUserErrorDto {
    fn from(error: ReviewUserError) -> Self {
        Self {
            code: error.code.to_owned(),
            retryable: error.retryable,
            affected_paths: error
                .affected_paths
                .into_iter()
                .map(|path| path.as_str().to_owned())
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewResumeSnapshotDto {
    pub review_stream_id: String,
    pub review_round_id: String,
    pub created_at_ms: i64,
    pub total: u32,
    pub feedback_items: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSessionSnapshotDto {
    pub phase: ReviewSessionPhaseDto,
    pub resume: Option<ReviewResumeSnapshotDto>,
    pub review_stream_id: Option<String>,
    pub review_round_id: Option<String>,
    pub revision: u64,
    pub members: Vec<ReviewMemberSnapshotDto>,
    pub feedback: Vec<ReviewFeedbackSnapshotDto>,
    pub restorable_feedback_id: Option<String>,
    pub unreviewable: Vec<ReviewUnreviewableSnapshotDto>,
    pub conflicts: Vec<ReviewConflictSnapshotDto>,
    pub counts: ReviewSessionCountsDto,
    pub error: Option<ReviewUserErrorDto>,
}

impl From<ReviewSessionSnapshot> for ReviewSessionSnapshotDto {
    fn from(snapshot: ReviewSessionSnapshot) -> Self {
        Self {
            phase: match snapshot.phase {
                ReviewSessionPhase::Idle => ReviewSessionPhaseDto::Idle,
                ReviewSessionPhase::Preparing => ReviewSessionPhaseDto::Preparing,
                ReviewSessionPhase::Active => ReviewSessionPhaseDto::Active,
                ReviewSessionPhase::Completing => ReviewSessionPhaseDto::Completing,
                ReviewSessionPhase::CompletedReadOnly => ReviewSessionPhaseDto::CompletedReadOnly,
                ReviewSessionPhase::WriteUnavailable => ReviewSessionPhaseDto::WriteUnavailable,
                ReviewSessionPhase::RecoveryRequired => ReviewSessionPhaseDto::RecoveryRequired,
            },
            resume: snapshot.resume.map(|resume| ReviewResumeSnapshotDto {
                review_stream_id: resume.review_stream_id.to_string(),
                review_round_id: resume.review_round_id.to_string(),
                created_at_ms: resume.created_at_ms,
                total: resume.total,
                feedback_items: resume.feedback_items,
            }),
            review_stream_id: snapshot.review_stream_id.map(|id| id.to_string()),
            review_round_id: snapshot.review_round_id.map(|id| id.to_string()),
            revision: snapshot.revision,
            members: snapshot.members.into_iter().map(Into::into).collect(),
            feedback: snapshot.feedback.into_iter().map(Into::into).collect(),
            restorable_feedback_id: snapshot.restorable_feedback_id.map(|id| id.to_string()),
            unreviewable: snapshot.unreviewable.into_iter().map(Into::into).collect(),
            conflicts: snapshot.conflicts.into_iter().map(Into::into).collect(),
            counts: snapshot.counts.into(),
            error: snapshot.error.map(Into::into),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFeedbackSummaryDto {
    pub feedback_id: String,
    pub text: String,
    pub target_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewCompletionSummaryDto {
    pub review_round_id: String,
    pub revision: u64,
    pub total: u32,
    pub revise: u32,
    pub unreviewable: u32,
    pub default_pass: u32,
    pub feedback: Vec<ReviewFeedbackSummaryDto>,
    pub conflicts: Vec<ReviewConflictSnapshotDto>,
    pub pending: Vec<String>,
    pub can_complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewCompletionProposalDto {
    pub proposal_id: u64,
    pub summary: ReviewCompletionSummaryDto,
}

impl From<ReviewCompletionProposal> for ReviewCompletionProposalDto {
    fn from(proposal: ReviewCompletionProposal) -> Self {
        Self {
            proposal_id: proposal.id.get(),
            summary: ReviewCompletionSummaryDto {
                review_round_id: proposal.summary.review_round_id.to_string(),
                revision: proposal.summary.revision,
                total: proposal.summary.total,
                revise: proposal.summary.revise,
                unreviewable: proposal.summary.unreviewable,
                default_pass: proposal.summary.default_pass,
                feedback: proposal
                    .summary
                    .feedback
                    .into_iter()
                    .map(|feedback| ReviewFeedbackSummaryDto {
                        feedback_id: feedback.feedback_id.to_string(),
                        text: feedback.text,
                        target_count: feedback.target_count,
                    })
                    .collect(),
                conflicts: proposal
                    .summary
                    .conflicts
                    .into_iter()
                    .map(Into::into)
                    .collect(),
                pending: proposal
                    .summary
                    .pending
                    .into_iter()
                    .map(|path| path.as_str().to_owned())
                    .collect(),
                can_complete: proposal.summary.can_complete,
            },
        }
    }
}

fn request_context(
    session_id: &str,
    generation: u64,
) -> Result<ReviewRequestContext, CommandError> {
    Ok(ReviewRequestContext {
        session_id: SessionId::from_str(session_id).map_err(|_| review_invalid_data())?,
        generation: Generation::new(generation),
    })
}

fn guard_parts(
    session_id: &str,
    generation: u64,
    review_round_id: &str,
    expected_revision: u64,
) -> Result<(ReviewRequestContext, ReviewMutationGuard), CommandError> {
    Ok((
        request_context(session_id, generation)?,
        ReviewMutationGuard {
            review_round_id: ReviewRoundId::from_str(review_round_id)
                .map_err(|_| review_invalid_data())?,
            expected_revision,
        },
    ))
}

fn parse_entity_id(value: &str) -> Result<EntityId, CommandError> {
    EntityId::from_str(value).map_err(|_| review_invalid_data())
}

fn parse_entity_ids(values: &[String], maximum: usize) -> Result<Vec<EntityId>, CommandError> {
    if values.is_empty() || values.len() > maximum {
        return Err(review_invalid_data());
    }
    let mut seen = HashSet::with_capacity(values.len());
    values
        .iter()
        .map(|value| {
            let entity_id = parse_entity_id(value)?;
            if !seen.insert(entity_id) {
                return Err(review_invalid_data());
            }
            Ok(entity_id)
        })
        .collect()
}

fn parse_feedback_targets(
    targets: Vec<ReviewFeedbackTargetRequestDto>,
) -> Result<Vec<ReviewFeedbackTargetInput>, CommandError> {
    if targets.is_empty() || targets.len() > MAX_TARGETS_PER_FEEDBACK {
        return Err(review_invalid_data());
    }
    let mut seen = HashSet::with_capacity(targets.len());
    targets
        .into_iter()
        .map(|target| {
            let entity_id = parse_entity_id(&target.entity_id)?;
            if !seen.insert(entity_id) {
                return Err(review_invalid_data());
            }
            Ok(ReviewFeedbackTargetInput {
                entity_id,
                anchor: target.anchor.try_into_anchor()?,
            })
        })
        .collect()
}

fn validate_feedback_text(text: &str) -> Result<(), CommandError> {
    if text.trim().is_empty() || text.len() > MAX_FEEDBACK_TEXT_BYTES {
        Err(review_invalid_data())
    } else {
        Ok(())
    }
}

fn review_invalid_data() -> CommandError {
    CommandError::new(
        "review_invalid_data",
        ErrorCategory::Validation,
        "评审请求无效，请刷新后重试。",
        false,
    )
}
