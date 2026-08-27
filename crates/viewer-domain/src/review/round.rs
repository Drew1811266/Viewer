use super::{
    AssetVersion, Feedback, FeedbackAnchor, MAX_ASSETS_PER_ROUND, MAX_FEEDBACK_ITEMS_PER_ROUND,
    MAX_IMAGE_STROKE_POINTS_PER_ROUND, ProductionScope, ReviewMedia,
};
use crate::{AssetVersionId, FeedbackId, ProjectId, ReviewRoundId, ReviewStreamId};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewabilityFailure {
    Unsupported,
    Damaged,
    Unreadable,
    PermissionDenied,
    Missing,
    DecodeFailed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnreviewableAsset {
    pub asset_version_id: AssetVersionId,
    pub failure: ReviewabilityFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewOutcomeKind {
    Pass,
    Revise,
    Unreviewable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewOutcome {
    pub asset_version_id: AssetVersionId,
    pub kind: ReviewOutcomeKind,
    pub feedback_ids: Vec<FeedbackId>,
    pub failure: Option<ReviewabilityFailure>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewDraft {
    pub project_id: ProjectId,
    pub review_stream_id: ReviewStreamId,
    pub review_round_id: ReviewRoundId,
    pub production: Option<ProductionScope>,
    pub previous_completed_round_id: Option<ReviewRoundId>,
    pub created_at_ms: i64,
    pub assets: Vec<AssetVersion>,
    pub feedback: Vec<Feedback>,
    pub unreviewable: Vec<UnreviewableAsset>,
}

impl ReviewDraft {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        project_id: ProjectId,
        review_stream_id: ReviewStreamId,
        review_round_id: ReviewRoundId,
        production: Option<ProductionScope>,
        previous_completed_round_id: Option<ReviewRoundId>,
        created_at_ms: i64,
        assets: Vec<AssetVersion>,
    ) -> Result<Self, ReviewRoundError> {
        if created_at_ms < 0 {
            return Err(ReviewRoundError::InvalidTimestamp);
        }
        if assets.is_empty() {
            return Err(ReviewRoundError::EmptyAssets);
        }
        if assets.len() > MAX_ASSETS_PER_ROUND {
            return Err(ReviewRoundError::LimitExceeded);
        }
        let mut ids = HashSet::with_capacity(assets.len());
        let mut paths = HashSet::with_capacity(assets.len());
        for asset in &assets {
            if !ids.insert(asset.id) {
                return Err(ReviewRoundError::DuplicateAssetId);
            }
            if !paths.insert(asset.relative_path.clone()) {
                return Err(ReviewRoundError::DuplicateAssetPath);
            }
            if !asset_is_valid(asset) {
                return Err(ReviewRoundError::InvalidAsset);
            }
        }

        Ok(Self {
            project_id,
            review_stream_id,
            review_round_id,
            production,
            previous_completed_round_id,
            created_at_ms,
            assets,
            feedback: Vec::new(),
            unreviewable: Vec::new(),
        })
    }

    pub fn upsert_feedback(&mut self, feedback: Feedback) -> Result<(), ReviewRoundError> {
        if feedback.created_at_ms < self.created_at_ms {
            return Err(ReviewRoundError::InvalidTimestamp);
        }
        for target in &feedback.targets {
            let asset = self
                .assets
                .iter()
                .find(|asset| asset.id == target.asset_version_id)
                .ok_or(ReviewRoundError::UnknownAsset)?;
            validate_anchor(asset, &target.anchor)?;
        }

        let retained_stroke_points = self
            .feedback
            .iter()
            .filter(|candidate| candidate.id != feedback.id)
            .flat_map(|candidate| &candidate.targets)
            .map(stroke_point_count)
            .sum::<usize>();
        let candidate_stroke_points = feedback
            .targets
            .iter()
            .map(stroke_point_count)
            .sum::<usize>();
        if retained_stroke_points.saturating_add(candidate_stroke_points)
            > MAX_IMAGE_STROKE_POINTS_PER_ROUND
        {
            return Err(ReviewRoundError::LimitExceeded);
        }

        if let Some(index) = self
            .feedback
            .iter()
            .position(|candidate| candidate.id == feedback.id)
        {
            self.feedback[index] = feedback;
            return Ok(());
        }
        if self.feedback.len() >= MAX_FEEDBACK_ITEMS_PER_ROUND {
            return Err(ReviewRoundError::LimitExceeded);
        }
        self.feedback.push(feedback);
        Ok(())
    }

    pub fn remove_feedback(&mut self, feedback_id: FeedbackId) -> bool {
        let Some(index) = self
            .feedback
            .iter()
            .position(|feedback| feedback.id == feedback_id)
        else {
            return false;
        };
        self.feedback.remove(index);
        true
    }

    pub fn mark_unreviewable(
        &mut self,
        asset_version_id: AssetVersionId,
        failure: ReviewabilityFailure,
    ) -> Result<(), ReviewRoundError> {
        if !self.assets.iter().any(|asset| asset.id == asset_version_id) {
            return Err(ReviewRoundError::UnknownAsset);
        }
        if let Some(existing) = self
            .unreviewable
            .iter_mut()
            .find(|item| item.asset_version_id == asset_version_id)
        {
            existing.failure = failure;
        } else {
            self.unreviewable.push(UnreviewableAsset {
                asset_version_id,
                failure,
            });
        }
        Ok(())
    }

    pub fn complete(self, completed_at_ms: i64) -> Result<ReviewSnapshot, ReviewRoundError> {
        if completed_at_ms < self.created_at_ms
            || self
                .feedback
                .iter()
                .any(|feedback| feedback.created_at_ms > completed_at_ms)
        {
            return Err(ReviewRoundError::InvalidTimestamp);
        }
        if self.assets.iter().any(|asset| {
            matches!(
                asset.media,
                ReviewMedia::Image {
                    width: None,
                    height: None,
                }
            ) && !self
                .unreviewable
                .iter()
                .any(|item| item.asset_version_id == asset.id)
        }) {
            return Err(ReviewRoundError::InvalidAsset);
        }

        let outcomes = self
            .assets
            .iter()
            .map(|asset| {
                let feedback_ids = self
                    .feedback
                    .iter()
                    .filter(|feedback| {
                        feedback
                            .targets
                            .iter()
                            .any(|target| target.asset_version_id == asset.id)
                    })
                    .map(|feedback| feedback.id)
                    .collect::<Vec<_>>();
                if !feedback_ids.is_empty() {
                    return ReviewOutcome {
                        asset_version_id: asset.id,
                        kind: ReviewOutcomeKind::Revise,
                        feedback_ids,
                        failure: None,
                    };
                }
                if let Some(unreviewable) = self
                    .unreviewable
                    .iter()
                    .find(|item| item.asset_version_id == asset.id)
                {
                    return ReviewOutcome {
                        asset_version_id: asset.id,
                        kind: ReviewOutcomeKind::Unreviewable,
                        feedback_ids,
                        failure: Some(unreviewable.failure),
                    };
                }
                ReviewOutcome {
                    asset_version_id: asset.id,
                    kind: ReviewOutcomeKind::Pass,
                    feedback_ids,
                    failure: None,
                }
            })
            .collect();

        Ok(ReviewSnapshot {
            project_id: self.project_id,
            review_stream_id: self.review_stream_id,
            review_round_id: self.review_round_id,
            production: self.production,
            previous_completed_round_id: self.previous_completed_round_id,
            created_at_ms: self.created_at_ms,
            completed_at_ms,
            assets: self.assets,
            feedback: self.feedback,
            outcomes,
        })
    }
}

pub(super) fn asset_is_valid(asset: &AssetVersion) -> bool {
    if asset.parent_asset_version_id == Some(asset.id) {
        return false;
    }
    match asset.media {
        ReviewMedia::Image { width, height } => match (width, height) {
            (None, None) => true,
            (Some(width), Some(height)) => width > 0 && height > 0,
            _ => false,
        },
        ReviewMedia::Video {
            duration_us,
            display_width,
            display_height,
        } => {
            duration_us != Some(0)
                && match (display_width, display_height) {
                    (None, None) => true,
                    (Some(width), Some(height)) => width > 0 && height > 0,
                    _ => false,
                }
        }
    }
}

pub(super) fn validate_anchor(
    asset: &AssetVersion,
    anchor: &FeedbackAnchor,
) -> Result<(), ReviewRoundError> {
    match (&asset.media, anchor) {
        (_, FeedbackAnchor::Asset) => Ok(()),
        (
            ReviewMedia::Image {
                width: Some(width),
                height: Some(height),
            },
            FeedbackAnchor::ImageRect(_) | FeedbackAnchor::ImageStroke(_),
        ) if *width > 0 && *height > 0 => Ok(()),
        (
            ReviewMedia::Image { .. },
            FeedbackAnchor::ImageRect(_) | FeedbackAnchor::ImageStroke(_),
        ) => Err(ReviewRoundError::AnchorUnavailable),
        (ReviewMedia::Video { duration_us, .. }, FeedbackAnchor::VideoPoint { position_us }) => {
            if duration_us.is_some_and(|duration| *position_us > duration) {
                Err(ReviewRoundError::AnchorOutOfBounds)
            } else {
                Ok(())
            }
        }
        (ReviewMedia::Video { duration_us, .. }, FeedbackAnchor::VideoRange { end_us, .. }) => {
            if duration_us.is_some_and(|duration| *end_us > duration) {
                Err(ReviewRoundError::AnchorOutOfBounds)
            } else {
                Ok(())
            }
        }
        _ => Err(ReviewRoundError::AnchorMediaMismatch),
    }
}

fn stroke_point_count(target: &super::FeedbackTarget) -> usize {
    match &target.anchor {
        FeedbackAnchor::ImageStroke(stroke) => stroke.points().len(),
        _ => 0,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewSnapshot {
    pub project_id: ProjectId,
    pub review_stream_id: ReviewStreamId,
    pub review_round_id: ReviewRoundId,
    pub production: Option<ProductionScope>,
    pub previous_completed_round_id: Option<ReviewRoundId>,
    pub created_at_ms: i64,
    pub completed_at_ms: i64,
    pub assets: Vec<AssetVersion>,
    pub feedback: Vec<Feedback>,
    pub outcomes: Vec<ReviewOutcome>,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ReviewRoundError {
    #[error("a review round must contain at least one asset")]
    EmptyAssets,
    #[error("a review round exceeds a protocol limit")]
    LimitExceeded,
    #[error("review timestamp is invalid")]
    InvalidTimestamp,
    #[error("review round contains a duplicate asset id")]
    DuplicateAssetId,
    #[error("review round contains a duplicate asset path")]
    DuplicateAssetPath,
    #[error("review round contains an invalid asset")]
    InvalidAsset,
    #[error("feedback targets an asset outside the review round")]
    UnknownAsset,
    #[error("feedback anchor does not match the target media")]
    AnchorMediaMismatch,
    #[error("feedback anchor exceeds the target media bounds")]
    AnchorOutOfBounds,
    #[error("feedback anchor requires media bounds that are unavailable")]
    AnchorUnavailable,
}
