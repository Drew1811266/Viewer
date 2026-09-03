use super::*;
use std::fs;
use viewer_application::{review_evidence::*, review_workspace::*};
use viewer_domain::{
    review::{AssetEvidence, AssetVersion, ReviewMedia, continuous::*},
    *,
};
use viewer_infrastructure::{
    image_cache::register_review_png, review::ProjectReviewRepositoryProvider,
};

fn committed_image() -> (tempfile::TempDir, BoundReviewImage, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let png = fs::read(viewer_test_support::image_fixtures::image_fixture(
        "alpha.png",
    ))
    .unwrap();
    // Fixed fixture digest; the real repository independently hashes the bytes on write/read.
    let hex = "c7656fdc8b6441fb9693ef94efdec4e6a226ba74167195ff299d0423509a7dc8";
    let digest = std::array::from_fn(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap());
    let path = root.path().join("source.png");
    fs::write(&path, &png).unwrap();
    let asset = AssetVersion {
        id: AssetVersionId::from_u128(10),
        source_entity_id: Some(EntityId::from_u128(11)),
        relative_path: RelativePath::parse("source.png").unwrap(),
        evidence: AssetEvidence {
            size_bytes: png.len() as u64,
            modified_ns: 1,
            blake3: Some(digest),
        },
        media: ReviewMedia::Image {
            width: Some(640),
            height: Some(480),
        },
        producer_asset_id: None,
        parent_asset_version_id: None,
    };
    let reference = EvidenceRef {
        blake3: digest,
        size_bytes: png.len() as u64,
        width: 640,
        height: 480,
    };
    let stream = ReviewStreamId::from_u128(2);
    let mut state = ContinuousReviewState::empty(
        ProjectId::from_u128(1),
        stream,
        ReviewSnapshotId::from_u128(3),
    );
    state.assets.push(asset.clone());
    let provider = ProjectReviewRepositoryProvider::new(root.path(), state.project_id);
    let receipt = provider
        .continuous_writer()
        .unwrap()
        .commit(ReviewCommitRequest {
            expected: None,
            production: None,
            next: PreparedContinuousSnapshot {
                publication_protocol: ReviewPublicationProtocol::V3,
                state,
                command_id: ReviewCommandId::from_u128(4),
                payload_digest: [4; 32],
                changes: vec![],
                evidence: vec![ReviewEvidenceBinding {
                    asset_version_id: asset.id,
                    capability: EvidenceCapability::Image {
                        base: reference.clone(),
                        annotated: None,
                        annotations: vec![],
                    },
                }],
            },
            archives: vec![],
            adopted_usage: vec![],
            staged_evidence: vec![PreparedEvidenceFile { path, reference }],
        })
        .unwrap();
    let image = provider
        .continuous_reader()
        .unwrap()
        .load_evidence(
            stream,
            &HistorySelector::Snapshot(receipt.snapshot),
            asset.id,
            EvidenceRole::Base,
        )
        .unwrap();
    let evidence_path = root
        .path()
        .join(format!(".viewer/reviews/evidence/{hex}.png"));
    (root, image, evidence_path)
}

#[test]
fn review_workspace_evidence_tokens_are_session_bound_and_revoke_on_close() {
    let (_root, verified, _) = committed_image();
    let active = ActiveImageSession::default();
    let session = SessionId::from_u128(1);
    let other = SessionId::from_u128(2);
    active.set(Some(session));
    let registry = Arc::new(ImageArtifactRegistry::default());
    let token =
        register_review_png(&registry, session, EntityId::from_u128(11), &verified).unwrap();
    let resolver = ImageProtocolResolver::new(active.clone(), registry.clone());
    let path = format!("/{session}/{}", token.as_str());
    assert!(resolver.resolve(&path).is_ok());
    active.set(Some(other));
    assert_eq!(resolver.resolve(&path), Err(ProtocolError::Forbidden));
    assert_eq!(
        resolver.resolve(&format!("/{other}/{}", token.as_str())),
        Err(ProtocolError::Forbidden)
    );
    active.set(None);
    assert_eq!(resolver.resolve(&path), Err(ProtocolError::Forbidden));
    registry.remove_session(session);
    active.set(Some(session));
    assert_eq!(resolver.resolve(&path), Err(ProtocolError::NotFound));
}

#[test]
fn review_workspace_serves_verified_bytes_even_if_evidence_path_is_replaced() {
    let (_root, verified, evidence_path) = committed_image();
    let active = ActiveImageSession::default();
    let session = SessionId::from_u128(1);
    active.set(Some(session));
    let registry = Arc::new(ImageArtifactRegistry::default());
    let token =
        register_review_png(&registry, session, EntityId::from_u128(11), &verified).unwrap();
    fs::rename(&evidence_path, evidence_path.with_extension("original")).unwrap();
    fs::write(evidence_path, b"replacement must not be served").unwrap();
    let resolver = ImageProtocolResolver::new(active, registry);
    let request = Request::builder()
        .uri(format!(
            "viewer-image://localhost/{session}/{}",
            token.as_str()
        ))
        .body(vec![])
        .unwrap();
    let response = response_for_request(&resolver, &request);
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.body(), verified.png());
}

#[test]
fn review_workspace_image_cache_reuses_tokens_and_bounds_retained_previews() {
    let (_root, verified, _) = committed_image();
    let registry = ImageArtifactRegistry::default();
    let session = SessionId::from_u128(1);
    let entity = EntityId::from_u128(11);
    let first = register_review_png(&registry, session, entity, &verified).unwrap();
    assert_eq!(
        register_review_png(&registry, session, entity, &verified).unwrap(),
        first
    );
    for i in 1..=128 {
        let mut asset = verified.asset().clone();
        asset.id = AssetVersionId::from_u128(100 + i);
        let image = BoundReviewImage::from_verified_png(
            asset,
            verified.reference().clone(),
            EvidenceRole::Base,
            verified.png().to_vec(),
        )
        .unwrap();
        let token = register_review_png(&registry, session, entity, &image).unwrap();
        assert!(registry.resolve(session, &token).is_some());
    }
    assert!(
        registry.resolve(session, &first).is_none(),
        "only expendable preview tokens expire; repository evidence is untouched"
    );
    assert_eq!(registry.remove_session(session), 128);
}
