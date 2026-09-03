//! Version 4 continuous-review transport over the shared strict wire model.
pub use super::v3::{
    ReviewArchiveRecord, ReviewIndexRecord, ReviewReadResult, ReviewStateRecord, ReviewStreamRecord,
};
use super::{ContinuousReviewProtocol, ReviewProtocolError};

pub type ReviewIndexV4 = ReviewIndexRecord;
pub type ReviewStreamV4 = ReviewStreamRecord;

pub const REVIEW_PROTOCOL_V4: &str = "viewer.review/4";

pub fn encode_state_v4(record: &ReviewStateRecord) -> Result<Vec<u8>, ReviewProtocolError> {
    super::v3::encode_state_for(ContinuousReviewProtocol::V4, record)
}

pub fn decode_state_v4(bytes: &[u8]) -> Result<ReviewStateRecord, ReviewProtocolError> {
    super::v3::decode_state_for(ContinuousReviewProtocol::V4, bytes)
}

pub fn encode_index_v4(record: &ReviewIndexV4) -> Result<Vec<u8>, ReviewProtocolError> {
    super::v3::encode_index_for(ContinuousReviewProtocol::V4, record)
}

pub fn decode_index_v4(bytes: &[u8]) -> Result<ReviewIndexV4, ReviewProtocolError> {
    super::v3::decode_index_for(ContinuousReviewProtocol::V4, bytes)
}

pub fn encode_archive_v4(record: &ReviewArchiveRecord) -> Result<Vec<u8>, ReviewProtocolError> {
    super::v3::encode_archive_for(ContinuousReviewProtocol::V4, record)
}

pub fn decode_archive_v4(bytes: &[u8]) -> Result<ReviewArchiveRecord, ReviewProtocolError> {
    super::v3::decode_archive_for(ContinuousReviewProtocol::V4, bytes)
}

pub fn encode_read_result_v4(record: &ReviewReadResult) -> Result<Vec<u8>, ReviewProtocolError> {
    super::v3::encode_read_result_for(ContinuousReviewProtocol::V4, record)
}

pub fn decode_read_result_v4(bytes: &[u8]) -> Result<ReviewReadResult, ReviewProtocolError> {
    super::v3::decode_read_result_for(ContinuousReviewProtocol::V4, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/review-protocol")
                .join(name),
        )
        .unwrap()
    }

    #[test]
    fn all_v4_wrappers_reject_v3_documents() {
        assert!(decode_state_v4(&fixture("review-state-v3.valid.json")).is_err());
        assert!(decode_index_v4(&fixture("review-index-v3.valid.json")).is_err());
        assert!(decode_archive_v4(&fixture("review-archive-v3.valid.json")).is_err());
        assert!(decode_read_result_v4(&fixture("review-read-result-v3.valid.json")).is_err());
    }

    #[test]
    fn all_v4_wrappers_round_trip_shared_legacy_records() {
        let state =
            super::super::v3::decode_state_v3(&fixture("review-state-v3.valid.json")).unwrap();
        let index =
            super::super::v3::decode_index_v3(&fixture("review-index-v3.valid.json")).unwrap();
        let archive =
            super::super::v3::decode_archive_v3(&fixture("review-archive-v3.valid.json")).unwrap();
        let read_result =
            super::super::v3::decode_read_result_v3(&fixture("review-read-result-v3.valid.json"))
                .unwrap();

        let state_bytes = encode_state_v4(&state).unwrap();
        let index_bytes = encode_index_v4(&index).unwrap();
        let archive_bytes = encode_archive_v4(&archive).unwrap();
        let read_result_bytes = encode_read_result_v4(&read_result).unwrap();

        for bytes in [
            &state_bytes,
            &index_bytes,
            &archive_bytes,
            &read_result_bytes,
        ] {
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(bytes).unwrap()["protocolVersion"],
                REVIEW_PROTOCOL_V4
            );
        }
        assert_eq!(decode_state_v4(&state_bytes).unwrap(), state);
        assert_eq!(decode_index_v4(&index_bytes).unwrap(), index);
        assert_eq!(decode_archive_v4(&archive_bytes).unwrap(), archive);
        let decoded_read_result = decode_read_result_v4(&read_result_bytes).unwrap();
        assert_eq!(
            encode_read_result_v4(&decoded_read_result).unwrap(),
            read_result_bytes
        );
    }
}
