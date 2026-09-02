#![cfg(target_os = "macos")]

use std::{fs, os::unix::fs::MetadataExt, path::Path};
use viewer_application::{
    PreparedReviewAsset, ReviewTaskCancellation,
    review_evidence::{EvidenceRole, ReviewEvidencePort},
};
use viewer_domain::{
    AssetVersionId, EntityId, RelativePath,
    review::{AssetEvidence, AssetVersion, ReviewMedia},
};
use viewer_platform_macos::image::MacReviewEvidenceRenderer;

fn prepared(path: &Path) -> PreparedReviewAsset {
    let metadata = fs::metadata(path).unwrap();
    let entity_id =
        EntityId::from_u128((u128::from(metadata.dev()) << 64) | u128::from(metadata.ino()));
    PreparedReviewAsset {
        entity_id,
        source_path: path.into(),
        failure: None,
        change_revision: 0,
        asset: AssetVersion {
            id: AssetVersionId::from_u128(1),
            source_entity_id: Some(entity_id),
            relative_path: RelativePath::parse("source.png").unwrap(),
            evidence: AssetEvidence {
                size_bytes: metadata.len(),
                modified_ns: i128::from(metadata.mtime()) * 1_000_000_000
                    + i128::from(metadata.mtime_nsec()),
                blake3: Some(*blake3::hash(&fs::read(path).unwrap()).as_bytes()),
            },
            media: ReviewMedia::Image {
                width: Some(640),
                height: Some(480),
            },
            producer_asset_id: None,
            parent_asset_version_id: None,
        },
    }
}

#[tokio::test]
async fn prewarmed_exact_base_is_reused_without_reopening_the_source() {
    let root = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let source = root.path().join("source.png");
    fs::copy(
        viewer_test_support::image_fixtures::image_fixture("alpha.png"),
        &source,
    )
    .unwrap();
    let asset = prepared(&source);
    let renderer = MacReviewEvidenceRenderer::new(scratch.path()).unwrap();

    renderer
        .prewarm_base_evidence(asset.clone(), ReviewTaskCancellation::default())
        .await
        .unwrap();
    fs::remove_file(&source).unwrap();

    let cached = renderer
        .capture_prewarmed_base(asset.clone(), ReviewTaskCancellation::default())
        .await
        .unwrap();
    assert_eq!(cached.asset(), &asset.asset);
    assert_eq!(cached.role(), EvidenceRole::Base);
    assert_eq!(*blake3::hash(cached.png()).as_bytes(), cached.blake3());
}
