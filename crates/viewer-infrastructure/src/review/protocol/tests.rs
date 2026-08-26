use super::{
    REVIEW_PROTOCOL_V1, ReviewProtocolError, decode_completed_versioned, decode_draft_versioned,
    detect_review_protocol, encode_completed_v2, encode_draft_v2, v1, v2,
};
use serde_json::{Value, json};
use viewer_application::ReviewProtocolVersion;
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

#[test]
fn v2_round_preserves_rect_stroke_and_reserved_video_anchors() {
    let bytes =
        include_bytes!("../../../../../tests/fixtures/review-protocol/review-round-v2.valid.json");
    let decoded = decode_completed_versioned(bytes).unwrap();

    assert_eq!(decoded.version, ReviewProtocolVersion::V2);
    assert!(
        decoded
            .value
            .feedback
            .iter()
            .flat_map(|feedback| &feedback.targets)
            .any(|target| matches!(target.anchor, FeedbackAnchor::ImageStroke(_)))
    );
    assert_eq!(encode_completed_v2(&decoded.value).unwrap(), bytes);
}

#[test]
fn v2_draft_preserves_image_strokes_byte_for_byte() {
    let bytes =
        include_bytes!("../../../../../tests/fixtures/review-protocol/review-draft-v2.valid.json");
    let decoded = decode_draft_versioned(bytes).unwrap();

    assert_eq!(decoded.version, ReviewProtocolVersion::V2);
    assert!(matches!(
        decoded.value.feedback[0].targets[0].anchor,
        FeedbackAnchor::ImageStroke(_)
    ));
    assert_eq!(encode_draft_v2(&decoded.value).unwrap(), bytes);
}

#[test]
fn protocol_dispatch_rejects_unknown_versions_instead_of_guessing() {
    let bytes = br#"{"protocolVersion":"viewer.review/99"}"#;

    assert_eq!(
        decode_completed_versioned(bytes),
        Err(ReviewProtocolError::UnsupportedVersion),
    );
}

fn v2_round_value() -> Value {
    serde_json::from_slice(include_bytes!(
        "../../../../../tests/fixtures/review-protocol/review-round-v2.valid.json"
    ))
    .unwrap()
}

fn decode_v2_value(
    value: &Value,
) -> Result<viewer_domain::review::ReviewSnapshot, ReviewProtocolError> {
    v2::decode_completed(&serde_json::to_vec(value).unwrap())
}

fn image_artifact() -> Value {
    json!({
        "assetVersionId": "00000000-0000-4000-8000-000000000301",
        "relativePath": "artifacts/00000000-0000-4000-8000-000000000301-annotation.png",
        "blake3": "0000000000000000000000000000000000000000000000000000000000000000",
        "mediaType": "image/png",
        "width": 1024,
        "height": 1024,
        "annotations": [
            { "ordinal": 1, "feedbackId": "00000000-0000-4000-8000-000000000401" },
            { "ordinal": 2, "feedbackId": "00000000-0000-4000-8000-000000000402" }
        ]
    })
}

#[test]
fn v2_rejects_strokes_above_the_domain_point_limit() {
    let mut value = v2_round_value();
    value["feedback"][1]["targets"][0]["anchor"]["points"] =
        Value::Array(vec![json!({ "x": 0.2, "y": 0.3 }); 2_049]);

    assert_eq!(
        decode_v2_value(&value),
        Err(ReviewProtocolError::LimitExceeded),
    );
}

#[test]
fn v2_rejects_non_json_numbers() {
    let bytes = String::from_utf8(
        include_bytes!("../../../../../tests/fixtures/review-protocol/review-round-v2.valid.json")
            .to_vec(),
    )
    .unwrap()
    .replacen("\"x\": 0.2", "\"x\": NaN", 1);

    assert_eq!(
        v2::decode_completed(bytes.as_bytes()),
        Err(ReviewProtocolError::InvalidData),
    );
}

#[test]
fn v2_rejects_escaping_artifact_locations() {
    let mut value = v2_round_value();
    let mut artifact = image_artifact();
    artifact["relativePath"] = json!("artifacts/../outside.png");
    value["artifacts"] = json!([artifact]);

    assert_eq!(
        decode_v2_value(&value),
        Err(ReviewProtocolError::InvalidData),
    );
}

#[test]
fn v2_rejects_incomplete_artifact_annotation_mappings() {
    let mut value = v2_round_value();
    let mut artifact = image_artifact();
    artifact["annotations"].as_array_mut().unwrap().pop();
    value["artifacts"] = json!([artifact]);

    assert_eq!(
        decode_v2_value(&value),
        Err(ReviewProtocolError::InvalidData),
    );
}

#[test]
fn v2_rejects_noncanonical_artifact_digests() {
    let mut value = v2_round_value();
    let mut artifact = image_artifact();
    artifact["blake3"] = json!("G000000000000000000000000000000000000000000000000000000000000000");
    value["artifacts"] = json!([artifact]);

    assert_eq!(
        decode_v2_value(&value),
        Err(ReviewProtocolError::InvalidData),
    );
}

#[test]
fn v2_accepts_complete_artifact_annotation_mappings() {
    let mut value = v2_round_value();
    value["artifacts"] = json!([image_artifact()]);

    assert!(decode_v2_value(&value).is_ok());
}
