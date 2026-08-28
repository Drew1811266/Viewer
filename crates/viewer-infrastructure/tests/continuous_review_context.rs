#[path = "support/continuous_review.rs"]
mod support;
use support::*;
use viewer_domain::{
    ReviewStreamId,
    review::{ProductionId, ProductionScope},
};

#[test]
fn review_workspace_context_lookup_does_not_create_metadata() {
    let (root, provider) = setup();
    assert_eq!(provider.manual_review_stream().unwrap(), None);
    assert!(!root.path().join(".viewer").exists());
}

#[test]
fn review_workspace_manual_context_never_selects_or_retargets_production() {
    let (_root, provider) = setup();
    let mut production = request(3, None);
    production.production = Some(ProductionScope {
        task_id: ProductionId::parse("task-a").unwrap(),
        batch_id: ProductionId::parse("batch-a").unwrap(),
    });
    provider
        .continuous_writer()
        .unwrap()
        .commit(production)
        .unwrap();
    assert_eq!(provider.manual_review_stream().unwrap(), None);
    let mut manual = request(4, None);
    manual.next.state.stream_id = ReviewStreamId::from_u128(9);
    provider
        .continuous_writer()
        .unwrap()
        .commit(manual)
        .unwrap();
    assert_eq!(
        provider.manual_review_stream().unwrap(),
        Some(ReviewStreamId::from_u128(9))
    );
}

#[test]
fn review_workspace_context_lookup_rejects_future_protocol_and_wrong_project() {
    let (root, provider) = setup();
    provider
        .continuous_writer()
        .unwrap()
        .commit(request(3, None))
        .unwrap();
    let path = root.path().join(".viewer/reviews/index.json");
    let mut index: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    index["projectId"] = serde_json::json!(viewer_domain::ProjectId::from_u128(99).to_string());
    std::fs::write(&path, serde_json::to_vec(&index).unwrap()).unwrap();
    assert!(provider.manual_review_stream().is_err());
    index["protocolVersion"] = serde_json::json!("viewer.review/99");
    std::fs::write(&path, serde_json::to_vec(&index).unwrap()).unwrap();
    assert!(provider.manual_review_stream().is_err());
}
