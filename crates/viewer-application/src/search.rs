use std::cmp::Ordering;
use std::sync::Arc;
use viewer_domain::{
    SessionId,
    search::{Generation, SearchPage, SearchQuery},
};

use crate::{SearchPort, SearchSnippetPort, scheduler::TaskCoordinator};

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SearchError {
    #[error("search request belongs to a different project session")]
    InvalidSession,
    #[error("search result belongs to a stale or cancelled generation")]
    Stale,
    #[error("search backend failed: {0}")]
    Backend(String),
    #[error("search request is invalid")]
    InvalidQuery,
}

pub fn natural_cmp(left: &str, right: &str) -> Ordering {
    let left_tokens = natural_tokens(left);
    let right_tokens = natural_tokens(right);
    for (left, right) in left_tokens.iter().zip(&right_tokens) {
        let ordering = match (left, right) {
            (NaturalToken::Number(left), NaturalToken::Number(right)) => {
                compare_numbers(left, right)
            }
            (NaturalToken::Text(left), NaturalToken::Text(right)) => left.cmp(right),
            (NaturalToken::Number(left), NaturalToken::Text(right))
            | (NaturalToken::Text(left), NaturalToken::Number(right)) => left.cmp(right),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left_tokens
        .len()
        .cmp(&right_tokens.len())
        .then_with(|| left.cmp(right))
}

#[derive(Debug)]
enum NaturalToken {
    Number(String),
    Text(String),
}

fn natural_tokens(value: &str) -> Vec<NaturalToken> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut numeric = None;
    for character in value.chars() {
        let next_numeric = character.is_ascii_digit();
        if numeric.is_some_and(|numeric| numeric != next_numeric) {
            tokens.push(if numeric == Some(true) {
                NaturalToken::Number(std::mem::take(&mut current))
            } else {
                NaturalToken::Text(std::mem::take(&mut current).to_lowercase())
            });
        }
        numeric = Some(next_numeric);
        current.push(character);
    }
    if !current.is_empty() {
        tokens.push(if numeric == Some(true) {
            NaturalToken::Number(current)
        } else {
            NaturalToken::Text(current.to_lowercase())
        });
    }
    tokens
}

fn compare_numbers(left: &str, right: &str) -> Ordering {
    let left_value = left.trim_start_matches('0');
    let right_value = right.trim_start_matches('0');
    let left_value = if left_value.is_empty() {
        "0"
    } else {
        left_value
    };
    let right_value = if right_value.is_empty() {
        "0"
    } else {
        right_value
    };
    left_value
        .len()
        .cmp(&right_value.len())
        .then_with(|| left_value.cmp(right_value))
        .then_with(|| left.len().cmp(&right.len()))
}

pub struct CoordinatedSearch {
    inner: Arc<dyn SearchPort>,
    coordinator: Arc<TaskCoordinator>,
}

impl CoordinatedSearch {
    pub fn new(inner: Arc<dyn SearchPort>, coordinator: Arc<TaskCoordinator>) -> Self {
        Self { inner, coordinator }
    }
}

#[async_trait::async_trait]
impl SearchPort for CoordinatedSearch {
    async fn search(
        &self,
        session_id: SessionId,
        generation: Generation,
        query: SearchQuery,
    ) -> Result<SearchPage, SearchError> {
        if !self.coordinator.is_publishable(session_id, generation) {
            return Err(SearchError::Stale);
        }
        let result = self.inner.search(session_id, generation, query).await;
        if !self.coordinator.is_publishable(session_id, generation) {
            return Err(SearchError::Stale);
        }
        result
    }
}

pub struct CoordinatedSnippet {
    inner: Arc<dyn SearchSnippetPort>,
    coordinator: Arc<TaskCoordinator>,
}

impl CoordinatedSnippet {
    pub fn new(inner: Arc<dyn SearchSnippetPort>, coordinator: Arc<TaskCoordinator>) -> Self {
        Self { inner, coordinator }
    }
}

#[async_trait::async_trait]
impl SearchSnippetPort for CoordinatedSnippet {
    async fn text_snippet(
        &self,
        session_id: SessionId,
        generation: Generation,
        entity_id: viewer_domain::EntityId,
        query: String,
    ) -> Result<Option<String>, SearchError> {
        if !self.coordinator.is_publishable(session_id, generation) {
            return Err(SearchError::Stale);
        }
        let result = self
            .inner
            .text_snippet(session_id, generation, entity_id, query)
            .await;
        if !self.coordinator.is_publishable(session_id, generation) {
            return Err(SearchError::Stale);
        }
        result
    }
}
