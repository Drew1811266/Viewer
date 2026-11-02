//! Version 5 continuous-review transport over the shared strict wire model.
pub use super::v3::{
    ReviewArchiveRecord, ReviewIndexRecord, ReviewReadResult, ReviewStateRecord, ReviewStreamRecord,
};
use super::{ContinuousReviewProtocol, ReviewProtocolError};

pub type ReviewIndexV5 = ReviewIndexRecord;
pub type ReviewStreamV5 = ReviewStreamRecord;

pub const REVIEW_PROTOCOL_V5: &str = "viewer.review/5";

pub fn encode_state_v5(record: &ReviewStateRecord) -> Result<Vec<u8>, ReviewProtocolError> {
    super::v3::encode_state_for(ContinuousReviewProtocol::V5, record)
}

pub fn decode_state_v5(bytes: &[u8]) -> Result<ReviewStateRecord, ReviewProtocolError> {
    super::v3::decode_state_for(ContinuousReviewProtocol::V5, bytes)
}

pub(in crate::review) fn encode_authoring_state(
    record: &ReviewStateRecord,
) -> Result<Vec<u8>, ReviewProtocolError> {
    super::v3::encode_authoring_state_for(ContinuousReviewProtocol::V5, record)
}

pub(in crate::review) fn decode_authoring_state(
    bytes: &[u8],
) -> Result<ReviewStateRecord, ReviewProtocolError> {
    super::v3::decode_authoring_state_for(ContinuousReviewProtocol::V5, bytes)
}

pub fn encode_index_v5(record: &ReviewIndexV5) -> Result<Vec<u8>, ReviewProtocolError> {
    super::v3::encode_index_for(ContinuousReviewProtocol::V5, record)
}

pub fn decode_index_v5(bytes: &[u8]) -> Result<ReviewIndexV5, ReviewProtocolError> {
    super::v3::decode_index_for(ContinuousReviewProtocol::V5, bytes)
}

pub fn encode_archive_v5(record: &ReviewArchiveRecord) -> Result<Vec<u8>, ReviewProtocolError> {
    super::v3::encode_archive_for(ContinuousReviewProtocol::V5, record)
}

pub fn decode_archive_v5(bytes: &[u8]) -> Result<ReviewArchiveRecord, ReviewProtocolError> {
    super::v3::decode_archive_for(ContinuousReviewProtocol::V5, bytes)
}

pub fn encode_read_result_v5(record: &ReviewReadResult) -> Result<Vec<u8>, ReviewProtocolError> {
    super::v3::encode_read_result_for(ContinuousReviewProtocol::V5, record)
}

pub fn decode_read_result_v5(bytes: &[u8]) -> Result<ReviewReadResult, ReviewProtocolError> {
    super::v3::decode_read_result_for(ContinuousReviewProtocol::V5, bytes)
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
    fn all_v5_wrappers_reject_v3_and_v4_documents() {
        assert!(decode_state_v5(&fixture("review-state-v3.valid.json")).is_err());
        assert!(decode_index_v5(&fixture("review-index-v3.valid.json")).is_err());
        assert!(decode_archive_v5(&fixture("review-archive-v3.valid.json")).is_err());
        assert!(decode_read_result_v5(&fixture("review-read-result-v3.valid.json")).is_err());

        let state =
            super::super::v3::decode_state_v3(&fixture("review-state-v3.valid.json")).unwrap();
        let index =
            super::super::v3::decode_index_v3(&fixture("review-index-v3.valid.json")).unwrap();
        let archive =
            super::super::v3::decode_archive_v3(&fixture("review-archive-v3.valid.json")).unwrap();
        let read_result =
            super::super::v3::decode_read_result_v3(&fixture("review-read-result-v3.valid.json"))
                .unwrap();
        assert!(decode_state_v5(&super::super::v4::encode_state_v4(&state).unwrap()).is_err());
        assert!(decode_index_v5(&super::super::v4::encode_index_v4(&index).unwrap()).is_err());
        assert!(
            decode_archive_v5(&super::super::v4::encode_archive_v4(&archive).unwrap()).is_err()
        );
        assert!(
            decode_read_result_v5(&super::super::v4::encode_read_result_v4(&read_result).unwrap())
                .is_err()
        );
    }

    #[test]
    fn style_survives_state_round_trip() {
        let bytes = fixture("review-state-v5.valid.json");
        let state = decode_state_v5(&bytes).unwrap();
        let encoded = encode_state_v5(&state).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(value["protocolVersion"], REVIEW_PROTOCOL_V5);
        assert_eq!(
            value["feedback"][0]["targets"][1]["style"]["tool"],
            "rectangle"
        );
        assert_eq!(value["feedback"][0]["targets"][1]["style"]["color"], "blue");
        assert_eq!(
            value["feedback"][0]["targets"][1]["style"]["width"],
            "thick"
        );
    }
    #[test]
    fn all_v5_wrappers_round_trip_shared_legacy_records() {
        let state =
            super::super::v3::decode_state_v3(&fixture("review-state-v3.valid.json")).unwrap();
        let index =
            super::super::v3::decode_index_v3(&fixture("review-index-v3.valid.json")).unwrap();
        let archive =
            super::super::v3::decode_archive_v3(&fixture("review-archive-v3.valid.json")).unwrap();
        let read_result =
            super::super::v3::decode_read_result_v3(&fixture("review-read-result-v3.valid.json"))
                .unwrap();

        let state_bytes = encode_state_v5(&state).unwrap();
        let index_bytes = encode_index_v5(&index).unwrap();
        let archive_bytes = encode_archive_v5(&archive).unwrap();
        let read_result_bytes = encode_read_result_v5(&read_result).unwrap();

        for bytes in [
            &state_bytes,
            &index_bytes,
            &archive_bytes,
            &read_result_bytes,
        ] {
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(bytes).unwrap()["protocolVersion"],
                REVIEW_PROTOCOL_V5
            );
        }
        assert_eq!(decode_state_v5(&state_bytes).unwrap(), state);
        assert_eq!(decode_index_v5(&index_bytes).unwrap(), index);
        assert_eq!(decode_archive_v5(&archive_bytes).unwrap(), archive);
        let decoded_read_result = decode_read_result_v5(&read_result_bytes).unwrap();
        assert_eq!(
            encode_read_result_v5(&decoded_read_result).unwrap(),
            read_result_bytes
        );
    }
}
