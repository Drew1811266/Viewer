use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use viewer_infrastructure::review::{
    ReviewProtocolError, decode_catalog, decode_completed, decode_draft,
    decode_production_manifest, encode_catalog, encode_completed, encode_draft,
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
