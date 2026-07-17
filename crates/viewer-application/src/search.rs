use std::sync::Arc;
use viewer_domain::{
    SessionId,
    search::{Generation, SearchPage, SearchQuery},
};

use crate::{SearchPort, scheduler::TaskCoordinator};

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SearchError {
    #[error("search request belongs to a different project session")]
    InvalidSession,
    #[error("search result belongs to a stale or cancelled generation")]
    Stale,
    #[error("search backend failed: {0}")]
    Backend(String),
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
