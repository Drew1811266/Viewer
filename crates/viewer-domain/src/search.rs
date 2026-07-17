use crate::{
    EntityId, RelativePath,
    file::{FileKind, FileNode, ImageMetadata, Marker, ReviewState},
};
use serde::{Deserialize, Serialize};

pub const MAX_SEARCH_PAGE_SIZE: u32 = 200;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchScope {
    Project,
    Subtree(EntityId),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct NumericRange<T> {
    pub min: Option<T>,
    pub max: Option<T>,
}

impl<T: PartialOrd> NumericRange<T> {
    pub fn is_valid(&self) -> bool {
        !matches!((&self.min, &self.max), (Some(min), Some(max)) if min > max)
    }

    pub fn contains(&self, value: &T) -> bool {
        self.min.as_ref().is_none_or(|min| value >= min)
            && self.max.as_ref().is_none_or(|max| value <= max)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageOrientation {
    Landscape,
    Portrait,
    Square,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchFilters {
    pub kinds: Vec<FileKind>,
    pub review_states: Vec<ReviewState>,
    pub favorite_only: bool,
    pub unmarked_only: bool,
    pub orientations: Vec<ImageOrientation>,
    pub width: NumericRange<u32>,
    pub height: NumericRange<u32>,
    pub size: NumericRange<u64>,
    pub modified_ns: NumericRange<i128>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchSortKey {
    Relevance,
    NaturalName,
    ModifiedTime,
    Size,
    PixelDimensions,
    ReviewState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchSort {
    pub key: SearchSortKey,
    pub direction: SortDirection,
}

impl Default for SearchSort {
    fn default() -> Self {
        Self {
            key: SearchSortKey::NaturalName,
            direction: SortDirection::Ascending,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchLayout {
    #[default]
    Grouped,
    Flat,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: String,
    pub scope: SearchScope,
    pub filters: SearchFilters,
    pub sort: SearchSort,
    pub layout: SearchLayout,
    pub offset: u32,
    pub limit: u32,
}

impl SearchQuery {
    pub fn is_valid(&self) -> bool {
        self.limit > 0
            && self.limit <= MAX_SEARCH_PAGE_SIZE
            && self.filters.width.is_valid()
            && self.filters.height.is_valid()
            && self.filters.size.is_valid()
            && self.filters.modified_ns.is_valid()
            && self
                .filters
                .size
                .min
                .is_none_or(|value| i64::try_from(value).is_ok())
            && self
                .filters
                .size
                .max
                .is_none_or(|value| i64::try_from(value).is_ok())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchedField {
    ExactFilename,
    Filename,
    Path,
    Body,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MatchRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    pub node: FileNode,
    pub marker: Marker,
    pub image_metadata: Option<ImageMetadata>,
    pub matched_field: MatchedField,
    pub score: i64,
    pub group_relative_path: Option<RelativePath>,
    pub match_ranges: Vec<MatchRange>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct IndexProgress {
    pub images_total: u64,
    pub images_ready: u64,
    pub images_failed: u64,
    pub text_total: u64,
    pub text_ready: u64,
    pub text_skipped: u64,
    pub text_failed: u64,
}

impl IndexProgress {
    pub const fn completed_items(self) -> u64 {
        self.images_ready
            + self.images_failed
            + self.text_ready
            + self.text_skipped
            + self.text_failed
    }

    pub const fn total_items(self) -> u64 {
        self.images_total + self.text_total
    }

    pub const fn is_complete(self) -> bool {
        self.completed_items() == self.total_items()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchPage {
    pub total: u32,
    pub hits: Vec<SearchHit>,
    pub progress: IndexProgress,
}

#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct Generation(u64);

impl Generation {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}
