use super::*;
use std::{fs, os::unix::fs::MetadataExt, path::Path};
use viewer_application::{PreparedReviewAsset, ReviewTaskCancellation};
use viewer_domain::review::{
    AssetEvidence, AssetVersion, FeedbackAnchor, ImageStroke, NormalizedPoint, NormalizedRect,
    ReviewMedia, continuous::TargetVersionKey,
};
use viewer_domain::{
    AssetVersionId, EntityId, FeedbackId, RelativePath, ReviewTargetId, ReviewTargetRevisionId,
    ReviewTextRevisionId,
};

fn prepared(path: &Path, width: u32, height: u32) -> PreparedReviewAsset {
    let metadata = fs::metadata(path).unwrap();
    PreparedReviewAsset {
        entity_id: EntityId::from_u128(1),
        source_path: path.into(),
        failure: None,
        change_revision: 0,
        asset: AssetVersion {
            id: AssetVersionId::from_u128(2),
            source_entity_id: Some(EntityId::from_u128(1)),
            relative_path: RelativePath::parse(path.file_name().unwrap().to_str().unwrap())
                .unwrap(),
            evidence: AssetEvidence {
                size_bytes: metadata.len(),
                modified_ns: i128::from(metadata.mtime()) * 1_000_000_000
                    + i128::from(metadata.mtime_nsec()),
                blake3: Some(*blake3::hash(&fs::read(path).unwrap()).as_bytes()),
            },
            media: ReviewMedia::Image {
                width: Some(width),
                height: Some(height),
            },
            producer_asset_id: None,
            parent_asset_version_id: None,
        },
    }
}

fn annotations() -> Vec<NumberedTargetAnnotation> {
    let anchors = [
        FeedbackAnchor::ImageStroke(
            ImageStroke::new(vec![
                NormalizedPoint::new(0.1, 0.2).unwrap(),
                NormalizedPoint::new(0.2, 0.4).unwrap(),
            ])
            .unwrap(),
        ),
        FeedbackAnchor::ImageStroke(
            ImageStroke::new(vec![
                NormalizedPoint::new(0.3, 0.2).unwrap(),
                NormalizedPoint::new(0.4, 0.4).unwrap(),
            ])
            .unwrap(),
        ),
        FeedbackAnchor::ImageRect(NormalizedRect::new(0.5, 0.2, 0.1, 0.2).unwrap()),
        FeedbackAnchor::ImageRect(NormalizedRect::new(0.75, 0.2, 0.1, 0.2).unwrap()),
    ];
    anchors
        .into_iter()
        .enumerate()
        .map(|(i, anchor)| NumberedTargetAnnotation {
            ordinal: i as u32 + 1,
            key: TargetVersionKey {
                feedback_id: FeedbackId::from_u128(3),
                text_revision_id: ReviewTextRevisionId::from_u128(4),
                target_id: ReviewTargetId::from_u128(i as u128 + 10),
                target_revision_id: ReviewTargetRevisionId::from_u128(i as u128 + 20),
            },
            anchor,
        })
        .collect()
}

#[tokio::test]
async fn review_evidence_redraw_uses_captured_bytes_after_source_overwrite() {
    let root = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    let source = root.path().join("source.png");
    fs::copy(
        viewer_test_support::image_fixtures::image_fixture("alpha.png"),
        &source,
    )
    .unwrap();
    let renderer = MacReviewEvidenceRenderer::new(output.path()).unwrap();
    let base = renderer
        .capture_base(
            prepared(&source, 640, 480),
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap();
    let before_hash = base.blake3();
    fs::write(&source, b"replacement is deliberately not an image").unwrap();
    let rendered = renderer
        .render(ReviewEvidenceRequest {
            base,
            annotations: annotations(),
            cancellation: ReviewTaskCancellation::default(),
        })
        .await
        .unwrap();
    assert_eq!(rendered.base_ref.blake3, before_hash);
    assert_eq!(rendered.annotations.len(), 4);
    assert_eq!(rendered.files().len(), 2);
    assert_ne!(rendered.annotated_ref.as_ref().unwrap().blake3, before_hash);
    for file in rendered.files() {
        assert_eq!(
            *blake3::hash(&fs::read(&file.path).unwrap()).as_bytes(),
            file.reference.blake3
        );
    }
}

#[tokio::test]
async fn review_evidence_whole_image_and_leases_are_independent_of_cache_files() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.png");
    fs::copy(
        viewer_test_support::image_fixtures::image_fixture("alpha.png"),
        &source,
    )
    .unwrap();
    let output = tempfile::tempdir().unwrap();
    let renderer = MacReviewEvidenceRenderer::new(output.path()).unwrap();
    let base = renderer
        .capture_base(
            prepared(&source, 640, 480),
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap();
    assert_eq!(fs::read_dir(output.path()).unwrap().count(), 0);
    let rendered = renderer
        .render(ReviewEvidenceRequest {
            base: base.clone(),
            annotations: vec![],
            cancellation: ReviewTaskCancellation::default(),
        })
        .await
        .unwrap();
    assert!(rendered.annotated_ref.is_none());
    assert_eq!(rendered.files().len(), 1);
    let temporary = rendered.files()[0].path.clone();
    drop(rendered);
    assert!(!temporary.exists());
    assert_eq!(*blake3::hash(base.png()).as_bytes(), base.blake3());
    assert_eq!(fs::read_dir(output.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn review_evidence_exif_orientation_and_four_marker_colors_survive_redraw() {
    use crate::image::review_annotation::tests::{asymmetric_source, decoded_rgba};
    let root = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    let source = root.path().join("rotated.jpg");
    asymmetric_source(&source, 800, 600, 6);
    let renderer = MacReviewEvidenceRenderer::new(output.path()).unwrap();
    let base = renderer
        .capture_base(
            prepared(&source, 600, 800),
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        base.mapping(),
        ReviewPixelMapping {
            oriented_source_width: 600,
            oriented_source_height: 800,
            png_width: 600,
            png_height: 800
        }
    );
    let rendered = renderer
        .render(ReviewEvidenceRequest {
            base,
            annotations: annotations(),
            cancellation: ReviewTaskCancellation::default(),
        })
        .await
        .unwrap();
    let pixels = decoded_rgba(&rendered.files()[1].path, 600, 800);
    let pixel = |x: usize, y: usize| &pixels[(y * 600 + x) * 4..(y * 600 + x) * 4 + 3];
    assert!(pixel(580, 10)[0] > 240 && pixel(580, 10)[2] < 15);
    assert!(pixel(580, 780)[2] > 240 && pixel(580, 780)[0] < 15);
    for (outline_x, marker_x) in [(90, 60), (210, 180), (300, 300), (450, 450)] {
        for (channel, expected) in pixel(outline_x, 240).iter().zip([113_i16, 77, 0]) {
            assert!((i16::from(*channel) - expected).abs() < 8);
        }
        let mut whites = 0;
        for y in 153..167 {
            for x in marker_x - 7..marker_x + 7 {
                if pixel(x, y).iter().all(|v| *v > 245) {
                    whites += 1;
                }
            }
        }
        assert!(whites >= 3);
    }
}

#[tokio::test]
async fn review_evidence_capture_rejects_change_during_copy_or_decode() {
    for point in [
        Checkpoint::CaptureOpened,
        Checkpoint::CaptureCopied,
        Checkpoint::CaptureDecoded,
    ] {
        let root = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        let source = root.path().join("source.png");
        fs::copy(
            viewer_test_support::image_fixtures::image_fixture("alpha.png"),
            &source,
        )
        .unwrap();
        let asset = prepared(&source, 640, 480);
        let mut renderer = MacReviewEvidenceRenderer::new(output.path()).unwrap();
        renderer.hook = Arc::new(move |observed| {
            if observed == point {
                fs::write(&source, b"changed").unwrap();
            }
        });
        assert!(matches!(
            renderer
                .capture_base(asset, ReviewTaskCancellation::default())
                .await,
            Err(Error::SourceChanged)
        ));
        assert_eq!(fs::read_dir(output.path()).unwrap().count(), 0);
    }
}

#[tokio::test]
async fn review_evidence_cancellation_limits_and_invalid_mappings_do_not_leave_files() {
    let root = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    let source = root.path().join("source.png");
    fs::copy(
        viewer_test_support::image_fixtures::image_fixture("alpha.png"),
        &source,
    )
    .unwrap();
    let mut renderer = MacReviewEvidenceRenderer::new(output.path()).unwrap();
    let base = renderer
        .capture_base(
            prepared(&source, 640, 480),
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap();
    for point in [
        Checkpoint::RenderDecoded,
        Checkpoint::RenderDrawn,
        Checkpoint::RenderEncoded,
        Checkpoint::Return,
    ] {
        let cancellation = ReviewTaskCancellation::default();
        let observer = cancellation.clone();
        renderer.hook = Arc::new(move |observed| {
            if point == observed {
                observer.cancel();
            }
        });
        assert!(matches!(
            renderer
                .render(ReviewEvidenceRequest {
                    base: base.clone(),
                    annotations: annotations(),
                    cancellation
                })
                .await,
            Err(Error::Cancelled)
        ));
        assert_eq!(fs::read_dir(output.path()).unwrap().count(), 0);
    }
    renderer.hook = Arc::new(|_| {});
    renderer.limit = 1;
    assert!(matches!(
        renderer
            .capture_base(
                prepared(&source, 640, 480),
                ReviewTaskCancellation::default()
            )
            .await,
        Err(Error::LimitExceeded)
    ));
    assert!(matches!(
        renderer
            .render(ReviewEvidenceRequest {
                base: base.clone(),
                annotations: annotations(),
                cancellation: ReviewTaskCancellation::default()
            })
            .await,
        Err(Error::LimitExceeded)
    ));
    renderer.limit = MAX_REVIEW_ARTIFACT_BYTES;
    let mut duplicate = annotations();
    duplicate[1].key = duplicate[0].key;
    assert!(matches!(
        renderer
            .render(ReviewEvidenceRequest {
                base: base.clone(),
                annotations: duplicate,
                cancellation: ReviewTaskCancellation::default()
            })
            .await,
        Err(Error::InvalidRequest)
    ));
    let annotated = BoundReviewImage::from_verified_png(
        base.asset().clone(),
        base.reference().clone(),
        EvidenceRole::Annotated,
        base.png().to_vec(),
    )
    .unwrap();
    assert!(matches!(
        renderer
            .render(ReviewEvidenceRequest {
                base: annotated,
                annotations: annotations(),
                cancellation: ReviewTaskCancellation::default()
            })
            .await,
        Err(Error::InvalidRequest)
    ));
    assert_eq!(fs::read_dir(output.path()).unwrap().count(), 0);
}
