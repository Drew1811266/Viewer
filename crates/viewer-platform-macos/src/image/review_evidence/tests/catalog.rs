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

#[tokio::test]
async fn review_workspace_native_save_archive_restore_and_preview_version_guard() {
    use viewer_application::{ClockPort, review_workspace::*};
    use viewer_domain::{ProjectId, ReviewCommandId, ReviewStreamId, review::continuous::*};
    use viewer_infrastructure::review::{
        ContinuousReviewCommandCodec, ProjectReviewRepositoryProvider,
    };
    struct Clock;
    impl ClockPort for Clock {
        fn unix_millis(&self) -> i64 {
            100
        }
    }
    let root = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    let source = root.path().join("source.jpg");
    asymmetric_source(&source, 800, 600, 6);
    let original = fs::read(&source).unwrap();
    let index = Arc::new(SessionIndex::open(output.path().join("index.sqlite")).unwrap());
    let image = Arc::new(MacImagePort::new(output.path().join("image-cache")).unwrap());
    let metadata = fs::metadata(&source).unwrap();
    let node = FileNode {
        entity_id: EntityId::from_u128(
            (u128::from(metadata.dev()) << 64) | u128::from(metadata.ino()),
        ),
        relative_path: RelativePath::parse("source.jpg").unwrap(),
        kind: FileKind::Jpeg,
        size: metadata.len(),
        modified_ns: i128::from(metadata.mtime()) * 1_000_000_000
            + i128::from(metadata.mtime_nsec()),
    };
    index
        .upsert_batch(std::slice::from_ref(&node), Generation::new(1))
        .unwrap();
    let catalog = Arc::new(
        IndexedReviewAssetCatalog::new(
            root.path(),
            index,
            image,
            Arc::new(UnavailableVideoProbe),
            ReviewChangeLedger::default(),
        )
        .unwrap(),
    );
    let project = ProjectId::from_u128(1);
    let stream = ReviewStreamId::from_u128(2);
    let provider = Arc::new(ProjectReviewRepositoryProvider::new(root.path(), project));
    let service = ContinuousReviewService::new(
        ReviewWorkspaceContext {
            project_id: project,
            stream_id: stream,
            production: None,
        },
        provider.clone(),
        catalog,
        Arc::new(MacReviewEvidenceRenderer::new(output.path()).unwrap()),
        Arc::new(ContinuousReviewCommandCodec),
        Arc::new(Clock),
    );
    let asset = service
        .prepare_assets(&[node.entity_id], ReviewTaskCancellation::default())
        .await
        .unwrap()
        .remove(0);
    let command = ReviewWorkspaceCommand::SaveFeedback {
        feedback_id: None,
        text: "四处标记，共同原文".into(),
        targets: annotations()
            .into_iter()
            .map(|a| TargetEdit::Add {
                asset_version_id: asset.id,
                anchor: a.anchor,
            })
            .collect(),
    };
    let e = service
        .prepare(ReviewCommandId::new(), None, command)
        .await
        .unwrap();
    let first = service.apply(e).await.unwrap();
    let state = &first.view.current.as_ref().unwrap().state;
    assert_eq!(first.view.projection.actionable.len(), 4);
    let reader = provider.continuous_reader().unwrap();
    let base = reader
        .load_evidence(
            stream,
            &HistorySelector::Snapshot(first.receipt.snapshot),
            asset.id,
            EvidenceRole::Base,
        )
        .unwrap();
    assert_eq!(
        (base.mapping().png_width, base.mapping().png_height),
        (600, 800)
    );
    let image = reader
        .load_evidence(
            stream,
            &HistorySelector::Snapshot(first.receipt.snapshot),
            asset.id,
            EvidenceRole::Annotated,
        )
        .unwrap();
    assert_ne!(base.blake3(), image.blake3());
    let selected = state.target_key(state.feedback[0].targets[0].id).unwrap();
    let archive = service
        .prepare(
            ReviewCommandId::new(),
            Some(first.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::Archive(ArchiveSelection {
                expected_snapshot_id: first.receipt.snapshot.snapshot_id,
                groups: vec![ArchiveGroup {
                    basis: ArchiveBasis::Unknown,
                    targets: vec![selected],
                }],
            }),
        )
        .await
        .unwrap();
    let archive_id = archive.generated.archive_id;
    let after = service.apply(archive).await.unwrap();
    assert_eq!(after.view.projection.actionable.len(), 3);
    let restore = service
        .prepare(
            ReviewCommandId::new(),
            Some(after.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::Restore {
                archive_id,
                decisions: vec![RestoreDecision {
                    historical_key: selected,
                    choice: RestoreChoice::UseHistorical,
                }],
            },
        )
        .await
        .unwrap();
    let restored = service.apply(restore).await.unwrap();
    assert_eq!(restored.view.projection.actionable.len(), 4);
    assert_eq!(fs::read(&source).unwrap(), original);
    // The cached preview version must not be replaced by a freshly prepared save-time version.
    let pending = service
        .prepare(
            ReviewCommandId::new(),
            Some(restored.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::SaveFeedback {
                feedback_id: Some(selected.feedback_id),
                text: "改文字，仍针对旧版".into(),
                targets: vec![],
            },
        )
        .await
        .unwrap();
    fs::write(&source, b"external replacement after preview").unwrap();
    let changed = service.apply(pending).await.unwrap();
    assert!(changed.view.projection.actionable.is_empty());
    assert_eq!(changed.view.projection.needs_confirmation.len(), 4);
    assert_eq!(
        reader
            .load_evidence(
                stream,
                &HistorySelector::Snapshot(first.receipt.snapshot),
                asset.id,
                EvidenceRole::Base
            )
            .unwrap()
            .blake3(),
        base.blake3()
    );
}
