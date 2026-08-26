use super::{REVIEW_PROTOCOL_V1, ReviewProtocolError, detect_review_protocol, v1};
use viewer_domain::review::FeedbackAnchor;

#[test]
fn v1_round_fixture_round_trips_byte_for_byte() {
    let fixture =
        include_bytes!("../../../../../tests/fixtures/review-protocol/review-round-v1.valid.json");
    let decoded = v1::decode_completed(fixture).unwrap();

    assert_eq!(v1::encode_completed(&decoded).unwrap(), fixture);
}

#[test]
fn v1_image_region_maps_to_internal_image_rect() {
    let fixture =
        include_bytes!("../../../../../tests/fixtures/review-protocol/review-draft-v1.valid.json");
    let decoded = v1::decode_draft(fixture).unwrap();

    assert!(
        decoded
            .feedback
            .iter()
            .flat_map(|feedback| &feedback.targets)
            .any(|target| matches!(target.anchor, FeedbackAnchor::ImageRect(_)))
    );
    assert!(
        String::from_utf8(v1::encode_draft(&decoded).unwrap())
            .unwrap()
            .contains("\"imageRegion\"")
    );
}

#[test]
fn v1_protocol_detection_is_strict() {
    assert_eq!(
        detect_review_protocol(br#"{"protocolVersion":"viewer.review/1"}"#),
        Ok(REVIEW_PROTOCOL_V1),
    );
    assert_eq!(
        detect_review_protocol(br#"{"protocolVersion":"viewer.review/99"}"#),
        Err(ReviewProtocolError::UnsupportedVersion),
    );
}
