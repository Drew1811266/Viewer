use serde::{Deserialize, Serialize};

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
}
