use serde::{Deserialize, Serialize};
use viewer_domain::review::FeedbackAnchor;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ContinuousReviewProtocol {
    #[serde(rename = "viewer.review/3")]
    V3,
    #[serde(rename = "viewer.review/4")]
    V4,
}

impl ContinuousReviewProtocol {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V3 => "viewer.review/3",
            Self::V4 => "viewer.review/4",
        }
    }

    pub(crate) fn supports_anchor(self, anchor: &FeedbackAnchor) -> bool {
        self == Self::V4
            || !matches!(
                anchor,
                FeedbackAnchor::ImagePoint(_)
                    | FeedbackAnchor::ImageArrow(_)
                    | FeedbackAnchor::ImageEllipse(_)
            )
    }
}
