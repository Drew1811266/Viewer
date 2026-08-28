//! Continuous-review wire contracts. Deliberately not dispatched through legacy Completed APIs.
mod archive_wire;
mod bounded;
mod catalog_wire;
mod history_result;
mod read_result;
mod records;
mod recovery_migration;
mod recovery_selection;
pub(in crate::review) mod recovery_wire;
#[cfg(test)]
mod tests;
mod validate;
mod wire;

use super::common::{
    MAX_REVIEW_DOCUMENT_BYTES, MAX_REVIEW_INDEX_BYTES, ReviewProtocolError, decode_document,
};
use bounded::encode_document;
pub(in crate::review) use bounded::encode_document as encode_bounded_json;
pub use read_result::*;
pub use records::*;

pub const REVIEW_PROTOCOL_V3: &str = "viewer.review/3";
pub const REVIEW_USAGE_PROTOCOL_V1: &str = "viewer.review.usage/1";

pub fn encode_state_v3(record: &ReviewStateRecord) -> Result<Vec<u8>, ReviewProtocolError> {
    validate::state(record)?;
    encode_document(&wire::State::from(record), MAX_REVIEW_DOCUMENT_BYTES)
}

pub fn decode_state_v3(bytes: &[u8]) -> Result<ReviewStateRecord, ReviewProtocolError> {
    let wire: wire::State = decode_document(bytes, MAX_REVIEW_DOCUMENT_BYTES, REVIEW_PROTOCOL_V3)?;
    let record = wire.into_record();
    validate::state(&record)?;
    Ok(record)
}

pub fn encode_index_v3(record: &ReviewIndexV3) -> Result<Vec<u8>, ReviewProtocolError> {
    validate::index(record)?;
    encode_document(&catalog_wire::Index::from(record), MAX_REVIEW_INDEX_BYTES)
}
pub fn decode_index_v3(bytes: &[u8]) -> Result<ReviewIndexV3, ReviewProtocolError> {
    let wire: catalog_wire::Index =
        decode_document(bytes, MAX_REVIEW_INDEX_BYTES, REVIEW_PROTOCOL_V3)?;
    let record = wire.into_record();
    validate::index(&record)?;
    Ok(record)
}
pub fn encode_archive_v3(record: &ReviewArchiveRecord) -> Result<Vec<u8>, ReviewProtocolError> {
    validate::archive(record)?;
    encode_document(
        &archive_wire::Archive::from(record),
        MAX_REVIEW_DOCUMENT_BYTES,
    )
}
pub fn decode_archive_v3(bytes: &[u8]) -> Result<ReviewArchiveRecord, ReviewProtocolError> {
    let wire: archive_wire::Archive =
        decode_document(bytes, MAX_REVIEW_DOCUMENT_BYTES, REVIEW_PROTOCOL_V3)?;
    let record = wire.into_record();
    validate::archive(&record)?;
    Ok(record)
}
pub fn encode_usage_v1(record: &ReviewUsageRecord) -> Result<Vec<u8>, ReviewProtocolError> {
    validate::usage(record)?;
    encode_document(
        &catalog_wire::Usage::from(record),
        MAX_REVIEW_DOCUMENT_BYTES,
    )
}
pub fn decode_usage_v1(bytes: &[u8]) -> Result<ReviewUsageRecord, ReviewProtocolError> {
    let wire: catalog_wire::Usage =
        decode_document(bytes, MAX_REVIEW_DOCUMENT_BYTES, REVIEW_USAGE_PROTOCOL_V1)?;
    let record = wire.into_record();
    validate::usage(&record)?;
    Ok(record)
}

pub fn encode_read_result_v3(record: &ReviewReadResult) -> Result<Vec<u8>, ReviewProtocolError> {
    record.validate()?;
    encode_document(record, MAX_REVIEW_DOCUMENT_BYTES)
}
pub fn decode_read_result_v3(bytes: &[u8]) -> Result<ReviewReadResult, ReviewProtocolError> {
    let record: ReviewReadResult =
        decode_document(bytes, MAX_REVIEW_DOCUMENT_BYTES, REVIEW_PROTOCOL_V3)?;
    record.validate()?;
    Ok(record)
}
