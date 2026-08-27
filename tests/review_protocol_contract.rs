use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use viewer_domain::EntityId;
use viewer_domain::review::{
    FeedbackAnchor, ImageStroke, NormalizedPoint, ReviewMedia, ReviewabilityFailure,
};
use viewer_infrastructure::review::{
    ReviewProtocolError, decode_catalog, decode_catalog_versioned, decode_completed,
    decode_completed_versioned, decode_draft, decode_production_manifest, encode_catalog,
    encode_catalog_v2, encode_completed, encode_completed_v2, encode_draft,
};

fn fixture(name: &str) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("tests/fixtures/review-protocol")
            .join(name),
    )
    .unwrap()
}

fn assert_json_eq(actual: Vec<u8>, expected: &[u8]) {
    assert_eq!(
        serde_json::from_slice::<Value>(&actual).unwrap(),
        serde_json::from_slice::<Value>(expected).unwrap()
    );
    assert_eq!(actual.last(), Some(&b'\n'));
}

fn mutate_document(name: &str, mutation: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut document = serde_json::from_slice::<Value>(&fixture(name)).unwrap();
    mutation(&mut document);
    serde_json::to_vec(&document).unwrap()
}

#[test]
fn canonical_v1_documents_decode_and_reencode_without_semantic_drift() {
    let production = fixture("viewer-production-v1.valid.json");
    let draft = fixture("review-draft-v1.valid.json");
    let completed = fixture("review-round-v1.valid.json");
    let index = fixture("review-index-v1.valid.json");

    assert_json_eq(
        encode_draft(&decode_draft(&draft).unwrap()).unwrap(),
        &draft,
    );
    assert_json_eq(
        encode_completed(&decode_completed(&completed).unwrap()).unwrap(),
        &completed,
    );
    assert_json_eq(
        encode_catalog(&decode_catalog(&index).unwrap()).unwrap(),
        &index,
    );
    assert_eq!(
        decode_production_manifest(&production)
            .unwrap()
            .assets
            .len(),
        2
    );
}

#[test]
fn v1_round_fixture_round_trips_byte_for_byte() {
    let completed = fixture("review-round-v1.valid.json");
    assert_eq!(
        encode_completed(&decode_completed(&completed).unwrap()).unwrap(),
        completed,
    );
}

#[test]
fn v1_image_region_maps_to_internal_image_rect() {
    let draft = decode_draft(&fixture("review-draft-v1.valid.json")).unwrap();

    assert!(
        draft
            .feedback
            .iter()
            .flat_map(|feedback| &feedback.targets)
            .any(|target| matches!(target.anchor, FeedbackAnchor::ImageRect(_)))
    );
    assert!(
        String::from_utf8(encode_draft(&draft).unwrap())
            .unwrap()
            .contains("\"imageRegion\"")
    );
}

#[test]
fn v1_encoder_rejects_image_strokes_without_panicking() {
    let mut draft = decode_draft(&fixture("review-draft-v1.valid.json")).unwrap();
    draft.feedback[0].targets[0].anchor = FeedbackAnchor::ImageStroke(
        ImageStroke::new(vec![
            NormalizedPoint::new(0.1, 0.2).unwrap(),
            NormalizedPoint::new(0.7, 0.8).unwrap(),
        ])
        .unwrap(),
    );

    assert_eq!(encode_draft(&draft), Err(ReviewProtocolError::InvalidData));
}

#[test]
fn canonical_v2_round_decodes_but_cannot_be_reencoded_without_its_artifacts() {
    let completed = fixture("review-round-v2.valid.json");
    let decoded = decode_completed_versioned(&completed).unwrap();

    assert_eq!(
        encode_completed_v2(&decoded.value),
        Err(ReviewProtocolError::InvalidData)
    );
}

#[test]
fn v2_round_rejects_missing_noncanonical_and_oversized_artifact_records() {
    for mutation in [
        "missing",
        "nested",
        "pixels",
        "empty_mapping",
        "swapped_mapping",
    ] {
        let bytes = mutate_document("review-round-v2.valid.json", |document| match mutation {
            "missing" => document["artifacts"] = json!([]),
            "nested" => {
                document["artifacts"][0]["relativePath"] =
                    json!("artifacts/nested/00000000-0000-4000-8000-000000000301-annotation.png")
            }
            "pixels" => document["artifacts"][0]["width"] = json!(u32::MAX),
            "empty_mapping" => document["artifacts"][0]["annotations"] = json!([]),
            "swapped_mapping" => {
                document["artifacts"][0]["annotations"][0]["ordinal"] = json!(2);
                document["artifacts"][0]["annotations"][1]["ordinal"] = json!(1);
            }
            _ => unreachable!(),
        });
        assert!(
            decode_completed_versioned(&bytes).is_err(),
            "accepted {mutation}"
        );
    }
}

#[test]
fn canonical_v2_index_decodes_and_reencodes_byte_for_byte() {
    let index = fixture("review-index-v2.valid.json");
    let decoded = decode_catalog_versioned(&index).unwrap();

    assert_eq!(encode_catalog_v2(&decoded.value).unwrap(), index);
}

#[test]
fn protocol_rejects_unknown_versions_fields_absolute_paths_and_oversize_values() {
    let unsupported = mutate_document("review-round-v1.valid.json", |document| {
        document["protocolVersion"] = json!("viewer.review/2");
    });
    assert_eq!(
        decode_completed(&unsupported),
        Err(ReviewProtocolError::UnsupportedVersion)
    );

    let unknown = mutate_document("review-round-v1.valid.json", |document| {
        document["unexpected"] = json!(true);
    });
    assert_eq!(
        decode_completed(&unknown),
        Err(ReviewProtocolError::InvalidData)
    );

    let absolute = mutate_document("review-round-v1.valid.json", |document| {
        document["assets"][0]["relativePath"] = json!("/Users/private/item.png");
    });
    assert_eq!(
        decode_completed(&absolute),
        Err(ReviewProtocolError::InvalidData)
    );

    assert_eq!(
        decode_completed(&vec![b' '; 64 * 1024 * 1024 + 1]),
        Err(ReviewProtocolError::LimitExceeded)
    );
}

#[test]
fn catalog_rejects_global_or_internally_inconsistent_heads() {
    let global = mutate_document("review-index-v1.valid.json", |document| {
        document["latestCompletedRoundId"] = json!("00000000-0000-4000-8000-000000000202");
    });
    assert_eq!(
        decode_catalog(&global),
        Err(ReviewProtocolError::InvalidData)
    );

    let wrong_head = mutate_document("review-index-v1.valid.json", |document| {
        document["streams"][0]["latestCompletedRoundId"] =
            json!("00000000-0000-4000-8000-000000000999");
    });
    assert_eq!(
        decode_catalog(&wrong_head),
        Err(ReviewProtocolError::InvalidData)
    );

    let duplicate_scope = mutate_document("review-index-v1.valid.json", |document| {
        document["streams"][1]["taskId"] = json!("task-a");
        document["streams"][1]["batchId"] = json!("batch-a");
    });
    assert_eq!(
        decode_catalog(&duplicate_scope),
        Err(ReviewProtocolError::InvalidData)
    );
}

#[test]
fn draft_and_completed_status_payloads_are_not_interchangeable() {
    let draft_with_outcomes = mutate_document("review-draft-v1.valid.json", |document| {
        document["outcomes"] = json!([]);
    });
    assert_eq!(
        decode_draft(&draft_with_outcomes),
        Err(ReviewProtocolError::InvalidData)
    );

    let completed_as_draft = mutate_document("review-round-v1.valid.json", |document| {
        document["status"] = json!("draft");
    });
    assert_eq!(
        decode_completed(&completed_as_draft),
        Err(ReviewProtocolError::InvalidData)
    );
}

#[test]
fn review_v1_accepts_legacy_assets_without_local_source_identity() {
    let bytes = mutate_document("review-draft-v1.valid.json", |document| {
        document["assets"][0]
            .as_object_mut()
            .unwrap()
            .remove("sourceEntityId");
    });
    let draft = decode_draft(&bytes).unwrap();

    assert_eq!(draft.assets[0].source_entity_id, None);
}

#[test]
fn review_v1_round_trips_optional_local_source_identity() {
    let entity_id = EntityId::from_u128(0x51);
    let mut draft = decode_draft(&fixture("review-draft-v1.valid.json")).unwrap();
    draft.assets[0].source_entity_id = Some(entity_id);
    let mut completed = decode_completed(&fixture("review-round-v1.valid.json")).unwrap();
    completed.assets[0].source_entity_id = Some(entity_id);

    let decoded_draft = decode_draft(&encode_draft(&draft).unwrap()).unwrap();
    let decoded_completed = decode_completed(&encode_completed(&completed).unwrap()).unwrap();

    assert_eq!(decoded_draft.assets[0].source_entity_id, Some(entity_id));
    assert_eq!(
        decoded_completed.assets[0].source_entity_id,
        Some(entity_id)
    );
}

#[test]
fn review_v1_represents_confirmed_unreviewable_image_without_fake_dimensions() {
    let bytes = mutate_document("review-draft-v1.valid.json", |document| {
        document["assets"][2]["media"] = json!({ "kind": "image" });
        document["unreviewable"] = json!([{
            "assetVersionId": "00000000-0000-4000-8000-000000000303",
            "failure": "decodeFailed"
        }]);
    });

    let draft = decode_draft(&bytes).unwrap();

    assert_eq!(
        draft.assets[2].media,
        ReviewMedia::Image {
            width: None,
            height: None,
        }
    );
    assert_eq!(
        draft.unreviewable[0].failure,
        ReviewabilityFailure::DecodeFailed
    );
    let encoded: Value = serde_json::from_slice(&encode_draft(&draft).unwrap()).unwrap();
    assert_eq!(encoded["assets"][2]["media"], json!({ "kind": "image" }));
}

#[test]
fn review_v1_rejects_partial_or_zero_image_dimensions() {
    for media in [
        json!({ "kind": "image", "width": 100 }),
        json!({ "kind": "image", "height": 100 }),
        json!({ "kind": "image", "width": 0, "height": 100 }),
    ] {
        let bytes = mutate_document("review-draft-v1.valid.json", |document| {
            document["assets"][0]["media"] = media;
        });
        assert_eq!(decode_draft(&bytes), Err(ReviewProtocolError::InvalidData));
    }
}
