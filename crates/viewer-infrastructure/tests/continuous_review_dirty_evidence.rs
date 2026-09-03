use rusqlite::params;
use viewer_application::{
    ProjectAccess,
    review_evidence::{EvidenceRole, HistorySelector},
    review_workspace::{
        CURRENT_REVIEW_EVIDENCE_ACTION_POLICY, CachedReviewEvidenceAction,
        ContinuousReviewAuthoringStorePort, EvidenceCapability, ReviewEvidenceActionCachePort,
        ReviewEvidenceActionKey, ReviewEvidenceActionPolicy, dirty_evidence_assets,
        evidence_action_key,
    },
};
use viewer_domain::{
    AssetVersionId, RelativePath, ReviewStreamId,
    review::{FeedbackAnchor, NormalizedArrow, NormalizedPoint, NormalizedRect},
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
fn previous_renderer_cache_is_not_reused_and_historical_evidence_remains_readable() {
    let (_root, provider, reference) = fixture();
    let stream = viewer_domain::ReviewStreamId::from_u128(2);
    let reader = provider.continuous_reader().unwrap();
    let historical_snapshot = reader.load_current(stream).unwrap().unwrap();
    let asset_id = historical_snapshot.state.assets[0].id;
    let previous_policy = ReviewEvidenceActionPolicy {
        renderer_version: 1,
        output_policy_version: 1,
    };
    let previous_key =
        evidence_action_key(&historical_snapshot.state, asset_id, previous_policy).unwrap();
    let current_key = evidence_action_key(
        &historical_snapshot.state,
        asset_id,
        CURRENT_REVIEW_EVIDENCE_ACTION_POLICY,
    )
    .unwrap();
    assert_ne!(previous_key, current_key);

    let cache = provider.authoring_writer().unwrap();
    cache
        .store_verified(&CachedReviewEvidenceAction {
            action_key: previous_key,
            base: reference.clone(),
            annotated: None,
            renderer_version: previous_policy.renderer_version,
            output_policy_version: previous_policy.output_policy_version,
        })
        .unwrap();

    assert!(cache.load_verified(current_key).unwrap().is_none());
    assert!(cache.load_verified(previous_key).unwrap().is_some());
    let historical = reader
        .load_evidence(
            stream,
            &HistorySelector::Snapshot(historical_snapshot.reference),
            asset_id,
            EvidenceRole::Base,
        )
        .unwrap();
    assert_eq!(historical.blake3(), reference.blake3);
    assert_eq!(*blake3::hash(historical.png()).as_bytes(), reference.blake3);
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

#[test]
fn extended_geometry_changes_dirty_only_its_own_image_evidence() {
    let (_root, provider, _) = fixture();
    let stream = ReviewStreamId::from_u128(2);
    provider.bootstrap_authoring(stream).unwrap();
    let store = provider.authoring_reader().unwrap();
    let mut point = store.load_current(stream).unwrap().unwrap();
    let affected = point.state.assets[0].id;
    let mut unaffected_asset = point.state.assets[0].clone();
    unaffected_asset.id = AssetVersionId::from_u128(999);
    unaffected_asset.relative_path = RelativePath::parse("unaffected.png").unwrap();
    point.state.assets.push(unaffected_asset);
    point.state.feedback[0].targets[0].anchor =
        FeedbackAnchor::ImagePoint(NormalizedPoint::new(0.2, 0.3).unwrap());

    let mut arrow = point.clone();
    arrow.state.feedback[0].targets[0].anchor = FeedbackAnchor::ImageArrow(
        NormalizedArrow::new(
            NormalizedPoint::new(0.2, 0.3).unwrap(),
            NormalizedPoint::new(0.7, 0.6).unwrap(),
        )
        .unwrap(),
    );
    let mut ellipse = arrow.clone();
    ellipse.state.feedback[0].targets[0].anchor =
        FeedbackAnchor::ImageEllipse(NormalizedRect::new(0.1, 0.2, 0.3, 0.4).unwrap());

    assert_eq!(
        dirty_evidence_assets(Some(&point), &arrow, CURRENT_REVIEW_EVIDENCE_ACTION_POLICY,)
            .unwrap(),
        vec![affected]
    );
    assert_eq!(
        dirty_evidence_assets(
            Some(&arrow),
            &ellipse,
            CURRENT_REVIEW_EVIDENCE_ACTION_POLICY,
        )
        .unwrap(),
        vec![affected]
    );
}
