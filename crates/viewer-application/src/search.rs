#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SearchError {
    #[error("search request belongs to a different project session")]
    InvalidSession,
    #[error("search backend failed: {0}")]
    Backend(String),
}
