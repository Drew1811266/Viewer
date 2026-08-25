mod asset;
mod feedback;

pub use asset::{
    AssetEvidence, AssetVersion, ProductionId, ProductionScope, ReviewAssetKind, ReviewMedia,
};
pub use feedback::{Feedback, FeedbackAnchor, FeedbackTarget, NormalizedRect};

use thiserror::Error;

pub const MAX_ASSETS_PER_ROUND: usize = 50_000;
pub const MAX_FEEDBACK_ITEMS_PER_ROUND: usize = 10_000;
pub const MAX_TARGETS_PER_FEEDBACK: usize = 10_000;
pub const MAX_FEEDBACK_TEXT_BYTES: usize = 65_536;

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
