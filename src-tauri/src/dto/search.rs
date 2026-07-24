use super::{ImageMetadataDto, MarkerDto};
use serde::{Deserialize, Serialize};
use viewer_application::metadata::IndexProgress;
use viewer_domain::{
    file::ReviewState,
    search::{MatchRange, SearchHit, SearchPage},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchRangeDto {
    pub start: u32,
    pub end: u32,
}

impl From<MatchRange> for MatchRangeDto {
    fn from(range: MatchRange) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHitDto {
    pub entity_id: String,
    pub relative_path: String,
    pub name: String,
    pub kind: viewer_domain::file::FileKind,
    pub size: u64,
    pub modified_ns: String,
    pub marker: MarkerDto,
    pub image_metadata: Option<ImageMetadataDto>,
    pub matched_field: viewer_domain::search::MatchedField,
    pub score: i64,
    pub group_relative_path: Option<String>,
    pub match_ranges: Vec<MatchRangeDto>,
}

impl From<SearchHit> for SearchHitDto {
    fn from(hit: SearchHit) -> Self {
        let name = hit
            .node
            .relative_path
            .as_str()
            .rsplit_once('/')
            .map_or(hit.node.relative_path.as_str(), |(_, name)| name)
            .to_owned();
        Self {
            entity_id: hit.node.entity_id.to_string(),
            relative_path: hit.node.relative_path.as_str().to_owned(),
            name,
            kind: hit.node.kind,
            size: hit.node.size,
            modified_ns: hit.node.modified_ns.to_string(),
            marker: hit.marker.into(),
            image_metadata: hit.image_metadata.map(Into::into),
            matched_field: hit.matched_field,
            score: hit.score,
            group_relative_path: hit.group_relative_path.map(|path| path.as_str().to_owned()),
            match_ranges: hit.match_ranges.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPageDto {
    pub revision: u64,
    pub total: u32,
    pub hits: Vec<SearchHitDto>,
    pub progress: SearchProgressDto,
}

impl SearchPageDto {
    pub fn from_page(revision: u64, page: SearchPage) -> Self {
        Self {
            revision,
            total: page.total,
            hits: page.hits.into_iter().map(Into::into).collect(),
            progress: page.progress.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchProgressDto {
    pub images_total: u64,
    pub images_ready: u64,
    pub images_failed: u64,
    pub text_total: u64,
    pub text_ready: u64,
    pub text_skipped: u64,
    pub text_failed: u64,
    pub complete: bool,
}

impl From<IndexProgress> for SearchProgressDto {
    fn from(progress: IndexProgress) -> Self {
        Self {
            images_total: progress.images_total,
            images_ready: progress.images_ready,
            images_failed: progress.images_failed,
            text_total: progress.text_total,
            text_ready: progress.text_ready,
            text_skipped: progress.text_skipped,
            text_failed: progress.text_failed,
            complete: progress.is_complete(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSnippetDto {
    pub revision: u64,
    pub entity_id: String,
    pub snippet: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchFiltersRequestDto {
    pub kinds: Vec<viewer_domain::file::FileKind>,
    pub review_states: Vec<ReviewState>,
    pub favorite_only: bool,
    pub unmarked_only: bool,
    pub orientations: Vec<viewer_domain::search::ImageOrientation>,
    pub width_min: Option<u32>,
    pub width_max: Option<u32>,
    pub height_min: Option<u32>,
    pub height_max: Option<u32>,
    pub size_min: Option<u64>,
    pub size_max: Option<u64>,
    pub modified_ns_min: Option<String>,
    pub modified_ns_max: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchSortRequestDto {
    pub key: viewer_domain::search::SearchSortKey,
    pub direction: viewer_domain::search::SortDirection,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchProjectRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub revision: u64,
    pub text: String,
    pub scope_folder_id: Option<String>,
    #[serde(default)]
    pub filters: SearchFiltersRequestDto,
    pub sort: SearchSortRequestDto,
    pub layout: viewer_domain::search::SearchLayout,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchTextSnippetRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub revision: u64,
    pub entity_id: String,
    pub query: String,
}
