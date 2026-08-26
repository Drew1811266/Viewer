mod common;
#[cfg(test)]
mod tests;
pub mod v1;

pub const PRODUCTION_PROTOCOL_V1: &str = "viewer.production/1";
pub const REVIEW_PROTOCOL_V1: &str = "viewer.review/1";

pub use common::{MAX_REVIEW_DOCUMENT_BYTES, MAX_REVIEW_INDEX_BYTES, ReviewProtocolError};
pub use v1::{
    decode_catalog, decode_completed, decode_draft, decode_production_manifest, encode_catalog,
    encode_completed, encode_draft,
};

pub fn detect_review_protocol(bytes: &[u8]) -> Result<&'static str, ReviewProtocolError> {
    common::detect_protocol(bytes, MAX_REVIEW_DOCUMENT_BYTES, &[REVIEW_PROTOCOL_V1])
}
