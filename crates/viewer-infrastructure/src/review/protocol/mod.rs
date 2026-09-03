mod common;
mod continuous_version;
#[cfg(test)]
mod tests;
pub mod v1;
pub mod v2;
pub mod v3;
pub mod v4;

pub const PRODUCTION_PROTOCOL_V1: &str = "viewer.production/1";
pub const REVIEW_PROTOCOL_V1: &str = "viewer.review/1";
pub const REVIEW_PROTOCOL_V2: &str = "viewer.review/2";

pub use common::{MAX_REVIEW_DOCUMENT_BYTES, MAX_REVIEW_INDEX_BYTES, ReviewProtocolError};
pub use continuous_version::ContinuousReviewProtocol;
pub use v1::{
    decode_catalog, decode_completed, decode_draft, decode_production_manifest, encode_catalog,
    encode_completed, encode_draft,
};
use viewer_application::{DecodedReview, ReviewCatalog, ReviewProtocolVersion};
use viewer_domain::review::{ReviewDraft, ReviewSnapshot};

pub fn detect_review_protocol(bytes: &[u8]) -> Result<&'static str, ReviewProtocolError> {
    common::detect_protocol(
        bytes,
        MAX_REVIEW_DOCUMENT_BYTES,
        &[REVIEW_PROTOCOL_V1, REVIEW_PROTOCOL_V2],
    )
}

pub fn detect_continuous_review_protocol(
    bytes: &[u8],
    max_bytes: u64,
) -> Result<ContinuousReviewProtocol, ReviewProtocolError> {
    match common::detect_protocol(
        bytes,
        max_bytes,
        &[v3::REVIEW_PROTOCOL_V3, v4::REVIEW_PROTOCOL_V4],
    )? {
        v3::REVIEW_PROTOCOL_V3 => Ok(ContinuousReviewProtocol::V3),
        v4::REVIEW_PROTOCOL_V4 => Ok(ContinuousReviewProtocol::V4),
        _ => Err(ReviewProtocolError::UnsupportedVersion),
    }
}

pub fn decode_draft_versioned(
    bytes: &[u8],
) -> Result<DecodedReview<ReviewDraft>, ReviewProtocolError> {
    match detect_review_protocol(bytes)? {
        REVIEW_PROTOCOL_V1 => Ok(DecodedReview {
            version: ReviewProtocolVersion::V1,
            value: v1::decode_draft(bytes)?,
        }),
        REVIEW_PROTOCOL_V2 => Ok(DecodedReview {
            version: ReviewProtocolVersion::V2,
            value: v2::decode_draft(bytes)?,
        }),
        _ => Err(ReviewProtocolError::UnsupportedVersion),
    }
}

pub fn decode_catalog_versioned(
    bytes: &[u8],
) -> Result<DecodedReview<ReviewCatalog>, ReviewProtocolError> {
    match detect_review_protocol(bytes)? {
        REVIEW_PROTOCOL_V1 => Ok(DecodedReview {
            version: ReviewProtocolVersion::V1,
            value: v1::decode_catalog(bytes)?,
        }),
        REVIEW_PROTOCOL_V2 => Ok(DecodedReview {
            version: ReviewProtocolVersion::V2,
            value: v2::decode_catalog(bytes)?,
        }),
        _ => Err(ReviewProtocolError::UnsupportedVersion),
    }
}

pub fn decode_completed_versioned(
    bytes: &[u8],
) -> Result<DecodedReview<ReviewSnapshot>, ReviewProtocolError> {
    match detect_review_protocol(bytes)? {
        REVIEW_PROTOCOL_V1 => Ok(DecodedReview {
            version: ReviewProtocolVersion::V1,
            value: v1::decode_completed(bytes)?,
        }),
        REVIEW_PROTOCOL_V2 => Ok(DecodedReview {
            version: ReviewProtocolVersion::V2,
            value: v2::decode_completed(bytes)?,
        }),
        _ => Err(ReviewProtocolError::UnsupportedVersion),
    }
}

pub fn encode_draft_v2(draft: &ReviewDraft) -> Result<Vec<u8>, ReviewProtocolError> {
    v2::encode_draft(draft)
}

pub fn encode_catalog_v2(catalog: &ReviewCatalog) -> Result<Vec<u8>, ReviewProtocolError> {
    v2::encode_catalog(catalog)
}

pub fn encode_completed_v2(snapshot: &ReviewSnapshot) -> Result<Vec<u8>, ReviewProtocolError> {
    v2::encode_completed(snapshot)
}
