use crate::{
    EntityId,
    file::{FileKind, FileNode, ReviewState},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchScope {
    Project,
    Subtree(EntityId),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: String,
    pub scope: SearchScope,
    pub kinds: Vec<FileKind>,
    pub review_states: Vec<ReviewState>,
    pub favorite_only: bool,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchedField {
    ExactFilename,
    Filename,
    Path,
    Body,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    pub node: FileNode,
    pub matched_field: MatchedField,
    pub score: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchPage {
    pub total: u32,
    pub hits: Vec<SearchHit>,
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
