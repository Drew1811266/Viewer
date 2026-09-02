use rusqlite::params;
use viewer_application::{
    ProjectAccess,
    review_workspace::{
        CachedReviewEvidenceAction, EvidenceCapability, ReviewEvidenceActionCachePort,
        ReviewEvidenceActionKey,
    },
};
use viewer_infrastructure::{
    portable::PortableProjectMetadata, review::ProjectReviewRepositoryProvider,
};

#[path = "support/continuous_review.rs"]
mod support;

fn fixture() -> (
    tempfile::TempDir,
    ProjectReviewRepositoryProvider,
    viewer_application::review_workspace::EvidenceRef,
) {
    let root = tempfile::tempdir().unwrap();
    let metadata = PortableProjectMetadata::open(root.path(), ProjectAccess::ReadWrite, 1).unwrap();
    let project_id = metadata.project_id();
    drop(metadata);
    let provider = ProjectReviewRepositoryProvider::new(root.path(), project_id);
    let mut request = support::image_request(&root);
    request.next.state.project_id = project_id;
    let reference = match &request.next.evidence[0].capability {
        EvidenceCapability::Image { base, .. } => base.clone(),
        _ => unreachable!(),
    };
    provider
        .continuous_writer()
        .unwrap()
        .commit(request)
        .unwrap();
    (root, provider, reference)
}

#[test]
fn verified_action_cache_round_trips_exact_evidence() {
    let (_root, provider, reference) = fixture();
    let cache = provider.authoring_writer().unwrap();
    let action = CachedReviewEvidenceAction {
        action_key: ReviewEvidenceActionKey([7; 32]),
        base: reference,
        annotated: None,
        renderer_version: 1,
        output_policy_version: 1,
    };

    cache.store_verified(&action).unwrap();

    assert_eq!(
        cache.load_verified(action.action_key).unwrap(),
        Some(action)
    );
}

#[test]
fn keyed_lookup_does_not_scan_other_rows_and_corrupt_object_evicts_only_its_row() {
    let (root, provider, reference) = fixture();
    let cache = provider.authoring_writer().unwrap();
    let mut encoded = Vec::new();
    encoded.extend_from_slice(&reference.blake3);
    encoded.extend_from_slice(&reference.size_bytes.to_be_bytes());
    encoded.extend_from_slice(&reference.width.to_be_bytes());
    encoded.extend_from_slice(&reference.height.to_be_bytes());
    let database = root.path().join(".viewer/metadata.sqlite");
    let mut connection = rusqlite::Connection::open(&database).unwrap();
    let transaction = connection.transaction().unwrap();
    for index in 0..1_000_u32 {
        let mut key = [0_u8; 32];
        key[..4].copy_from_slice(&index.to_be_bytes());
        transaction
            .execute(
                "INSERT INTO review_evidence_action_cache(
                    action_key, base_evidence, annotated_evidence, renderer_version,
                    output_policy_version, updated_at_ms
                 ) VALUES (?1, ?2, NULL, '1', '1', 1)",
                params![key.as_slice(), encoded.as_slice()],
            )
            .unwrap();
    }
    transaction.commit().unwrap();
    drop(connection);
    let mut selected = [0_u8; 32];
    selected[..4].copy_from_slice(&731_u32.to_be_bytes());

    assert!(
        cache
            .load_verified(ReviewEvidenceActionKey(selected))
            .unwrap()
            .is_some()
    );
    let object = root.path().join(format!(
        ".viewer/reviews/evidence/{}.png",
        blake3::Hash::from_bytes(reference.blake3).to_hex()
    ));
    std::fs::write(object, b"corrupt").unwrap();

    assert_eq!(
        cache
            .load_verified(ReviewEvidenceActionKey(selected))
            .unwrap(),
        None
    );
    let connection = rusqlite::Connection::open(database).unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM review_evidence_action_cache",
                [],
                |row| { row.get::<_, u32>(0) }
            )
            .unwrap(),
        999
    );
}
