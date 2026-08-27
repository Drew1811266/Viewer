mod asset;
pub mod continuous;
mod feedback;
mod round;

pub use asset::{
    AssetEvidence, AssetVersion, ProductionId, ProductionScope, ReviewAssetKind, ReviewMedia,
};
pub use feedback::{
    Feedback, FeedbackAnchor, FeedbackTarget, ImageStroke, NormalizedPoint, NormalizedRect,
};
pub use round::{
    ReviewDraft, ReviewOutcome, ReviewOutcomeKind, ReviewRoundError, ReviewSnapshot,
    ReviewabilityFailure, UnreviewableAsset,
};

use thiserror::Error;

pub const MAX_ASSETS_PER_ROUND: usize = 50_000;
pub const MAX_FEEDBACK_ITEMS_PER_ROUND: usize = 10_000;
pub const MAX_TARGETS_PER_FEEDBACK: usize = 10_000;
pub const MAX_FEEDBACK_TEXT_BYTES: usize = 65_536;
pub const MAX_IMAGE_STROKE_POINTS: usize = 2_048;
pub const MAX_IMAGE_STROKE_POINTS_PER_ROUND: usize = 200_000;

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ReviewValueError {
    #[error("value must not be empty")]
    Empty,
    #[error("value has an invalid format")]
    InvalidFormat,
    #[error("numeric value is invalid")]
    InvalidNumber,
    #[error("value exceeds a protocol limit")]
    LimitExceeded,
    #[error("feedback contains a duplicate target")]
    DuplicateTarget,
}
