use std::collections::BTreeMap;
use thiserror::Error;
use viewer_domain::review::{
    ProductionId, ProductionScope, ReviewAssetKind, ReviewDraft, ReviewSnapshot,
};
use viewer_domain::{ProjectId, RelativePath, ReviewRoundId, ReviewStreamId};

pub const MAX_REVIEW_STREAMS: usize = 10_000;
pub const MAX_COMPLETED_ROUNDS_PER_STREAM: usize = 10_000;
pub const MAX_PRODUCTION_CONTEXT_ENTRIES: usize = 64;
pub const MAX_PRODUCTION_CONTEXT_KEY_BYTES: usize = 128;
pub const MAX_PRODUCTION_CONTEXT_VALUE_BYTES: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewProtocolVersion {
    V1,
    V2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecodedReview<T> {
    pub version: ReviewProtocolVersion,
    pub value: T,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionAsset {
    pub producer_asset_id: ProductionId,
    pub relative_path: RelativePath,
    pub kind: ReviewAssetKind,
    pub parent_producer_asset_id: Option<ProductionId>,
    pub generation: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionManifest {
    pub production: ProductionScope,
    pub context: BTreeMap<String, String>,
    pub assets: Vec<ProductionAsset>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewStreamHead {
    pub review_stream_id: ReviewStreamId,
    pub production: Option<ProductionScope>,
    pub completed_round_ids: Vec<ReviewRoundId>,
    pub latest_completed_round_id: Option<ReviewRoundId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewCatalog {
    pub project_id: ProjectId,
    pub streams: Vec<ReviewStreamHead>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReviewStreamLocator {
    Id(ReviewStreamId),
    Production(ProductionScope),
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ReviewCatalogError {
    #[error("review stream was not found")]
    NotFound,
    #[error("review stream selection is ambiguous")]
    Ambiguous,
}

impl ReviewCatalog {
    pub fn resolve_stream(
        &self,
        locator: Option<&ReviewStreamLocator>,
    ) -> Result<&ReviewStreamHead, ReviewCatalogError> {
        let mut matches = self.streams.iter().filter(|stream| match locator {
            None => true,
            Some(ReviewStreamLocator::Id(id)) => stream.review_stream_id == *id,
            Some(ReviewStreamLocator::Production(production)) => {
                stream.production.as_ref() == Some(production)
            }
        });
        let first = matches.next().ok_or(ReviewCatalogError::NotFound)?;
        if matches.next().is_some() {
            Err(ReviewCatalogError::Ambiguous)
        } else {
            Ok(first)
        }
    }
}

pub trait ReviewRepositoryPort: Send + Sync {
    fn load_catalog(&self) -> Result<ReviewCatalog, ReviewRepositoryError>;
    fn load_active_draft(&self) -> Result<Option<ReviewDraft>, ReviewRepositoryError>;
    fn load_draft(
        &self,
        stream_id: ReviewStreamId,
        round_id: ReviewRoundId,
    ) -> Result<Option<ReviewDraft>, ReviewRepositoryError>;
    fn save_draft(&self, draft: &ReviewDraft) -> Result<(), ReviewRepositoryError>;
    fn delete_draft(
        &self,
        stream_id: ReviewStreamId,
        round_id: ReviewRoundId,
    ) -> Result<(), ReviewRepositoryError>;
    fn load_completed(
        &self,
        stream_id: ReviewStreamId,
        round_id: ReviewRoundId,
    ) -> Result<Option<ReviewSnapshot>, ReviewRepositoryError>;
    fn publish(&self, snapshot: &ReviewSnapshot) -> Result<(), ReviewRepositoryError>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewRepositoryInspection {
    pub catalog: ReviewCatalog,
    pub active_draft: Option<ReviewDraft>,
}

pub trait ReviewRepositoryProviderPort: Send + Sync {
    fn inspect(&self) -> Result<ReviewRepositoryInspection, ReviewRepositoryError>;
    fn open_reader(&self) -> Result<Box<dyn ReviewRepositoryPort>, ReviewRepositoryError>;
    fn open_writer(&self) -> Result<Box<dyn ReviewRepositoryPort>, ReviewRepositoryError>;
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ReviewRepositoryError {
    #[error("review repository is read-only")]
    ReadOnly,
    #[error("review repository already has a writer")]
    Busy,
    #[error("review record was not found")]
    NotFound,
    #[error("review record conflicts with published history")]
    Conflict,
    #[error("review repository requires explicit recovery")]
    RecoveryRequired,
    #[error("review protocol version is unsupported")]
    UnsupportedVersion,
    #[error("review repository data is invalid")]
    InvalidData,
    #[error("review repository limit was exceeded")]
    LimitExceeded,
    #[error("review repository is unavailable")]
    Unavailable,
}
