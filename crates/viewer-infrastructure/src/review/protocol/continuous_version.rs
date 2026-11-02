use serde::{Deserialize, Serialize};
use viewer_domain::review::FeedbackAnchor;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ContinuousReviewProtocol {
    #[serde(rename = "viewer.review/3")]
    V3,
    #[serde(rename = "viewer.review/4")]
    V4,
    #[serde(rename = "viewer.review/5")]
    V5,
}

impl ContinuousReviewProtocol {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V3 => "viewer.review/3",
            Self::V4 => "viewer.review/4",
            Self::V5 => "viewer.review/5",
        }
    }

    pub(crate) fn supports_anchor(self, anchor: &FeedbackAnchor) -> bool {
        match self {
            Self::V3 => !matches!(
                anchor,
                FeedbackAnchor::ImagePoint(_)
                    | FeedbackAnchor::ImageArrow(_)
                    | FeedbackAnchor::ImageEllipse(_)
                    | FeedbackAnchor::ImageText(_)
                    | FeedbackAnchor::ImageLine(_)
                    | FeedbackAnchor::ImagePolygon(_)
            ),
            // v4 is the 0.2.5-compatible extension: point/arrow/ellipse only.
            Self::V4 => !matches!(
                anchor,
                FeedbackAnchor::ImageText(_)
                    | FeedbackAnchor::ImageLine(_)
                    | FeedbackAnchor::ImagePolygon(_)
            ),
            Self::V5 => true,
        }
    }
}
