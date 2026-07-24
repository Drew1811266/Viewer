use serde::{Deserialize, Serialize};
use viewer_application::metadata::MarkerChange;
use viewer_domain::file::{Marker, ReviewState};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkerDto {
    pub review_state: Option<ReviewState>,
    pub favorite: bool,
}

impl From<Marker> for MarkerDto {
    fn from(marker: Marker) -> Self {
        Self {
            review_state: marker.review_state,
            favorite: marker.favorite,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkerChangeDto {
    pub entity_id: String,
    pub relative_path: String,
    pub kind: viewer_domain::file::FileKind,
    pub marker: MarkerDto,
}

impl From<MarkerChange> for MarkerChangeDto {
    fn from(change: MarkerChange) -> Self {
        Self {
            entity_id: change.target.entity_id.to_string(),
            relative_path: change.target.relative_path.as_str().to_owned(),
            kind: change.target.kind,
            marker: change.marker.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkerBatchResultDto {
    pub changes: Vec<MarkerChangeDto>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SetReviewStateRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub entity_ids: Vec<String>,
    pub review_state: Option<ReviewState>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ToggleFavoriteRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub entity_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SelectionInfoRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub entity_ids: Vec<String>,
}

impl From<Vec<MarkerChange>> for MarkerBatchResultDto {
    fn from(changes: Vec<MarkerChange>) -> Self {
        Self {
            changes: changes.into_iter().map(Into::into).collect(),
        }
    }
}
