use super::*;
use crate::image::{MacImagePort, review_annotation::tests::asymmetric_source};
use viewer_application::{ImagePort, review_assets::ContinuousReviewAssetPort};
use viewer_domain::{
    file::{FileKind, FileNode, ImageMetadata},
    search::Generation,
};
use viewer_infrastructure::{
    review::{IndexedReviewAssetCatalog, ReviewChangeLedger},
    search::index::SessionIndex,
    video_probe::UnavailableVideoProbe,
};

#[tokio::test]
async fn review_evidence_catalog_to_native_capture_preserves_exif_upright_dimensions() {
    let root = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    let index = Arc::new(SessionIndex::open(output.path().join("index.sqlite")).unwrap());
    let image = Arc::new(MacImagePort::new(output.path().join("image-cache")).unwrap());
    let catalog = IndexedReviewAssetCatalog::new(
        root.path(),
        index.clone(),
        image.clone(),
        Arc::new(UnavailableVideoProbe),
        ReviewChangeLedger::default(),
    )
    .unwrap();
    let renderer = MacReviewEvidenceRenderer::new(output.path()).unwrap();
    for orientation in 1_u8..=8 {
        let name = format!("orientation-{orientation}.jpg");
        let source = root.path().join(&name);
        asymmetric_source(&source, 800, 600, i64::from(orientation));
        let original = fs::read(&source).unwrap();
        let raw = image.probe(&source).await.unwrap();
        assert_eq!(
            (raw.width, raw.height, raw.orientation),
            (800, 600, orientation)
        );
        let metadata = fs::metadata(&source).unwrap();
        let node = FileNode {
            entity_id: EntityId::from_u128(
                (u128::from(metadata.dev()) << 64) | u128::from(metadata.ino()),
            ),
            relative_path: RelativePath::parse(&name).unwrap(),
            kind: FileKind::Jpeg,
            size: metadata.len(),
            modified_ns: i128::from(metadata.mtime()) * 1_000_000_000
                + i128::from(metadata.mtime_nsec()),
        };
        index
            .upsert_batch(std::slice::from_ref(&node), Generation::new(1))
            .unwrap();
        let (width, height) = if orientation >= 5 {
            (600, 800)
        } else {
            (800, 600)
        };
        // Match the real scanner's Ready metadata: it is already EXIF-upright.
        index
            .replace_image_metadata(
                node.entity_id,
                &node.relative_path,
                Ok(ImageMetadata { width, height }),
            )
            .unwrap();
        let prepared = catalog
            .prepare_additions(&[node.entity_id], ReviewTaskCancellation::default())
            .await
            .unwrap()
            .remove(0);
        let asset = prepared.asset.clone();
        let base = renderer
            .capture_base(prepared, ReviewTaskCancellation::default())
            .await
            .unwrap_or_else(|error| panic!("orientation {orientation}: {error:?}"));
        assert_eq!(base.asset(), &asset);
        assert_eq!(
            base.mapping(),
            ReviewPixelMapping {
                oriented_source_width: width,
                oriented_source_height: height,
                png_width: width,
                png_height: height,
            }
        );
        assert_eq!(fs::read(source).unwrap(), original);
    }
}
