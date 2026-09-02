#[path = "../examples/support/continuous_review_fixture.rs"]
mod continuous_review_fixture;

use async_trait::async_trait;
use continuous_review_fixture::*;
use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use viewer_application::{
    ClockPort, PreparedReviewAsset, ReviewArtifactError, ReviewAssetError, ReviewTaskCancellation,
    review_assets::{ContinuousReviewAssetPort, SourceRelocationDecision},
    review_evidence::{
        BoundReviewImage, ReviewEvidencePort, ReviewEvidenceRequest, ReviewEvidenceResult,
    },
    review_workspace::*,
};
use viewer_desktop::dto::review_workspace::ReviewAuthoringApplyResultDto;
use viewer_domain::{
    AssetVersionId, EntityId, FeedbackId, ReviewCommandId,
    review::{AssetVersion, FeedbackAnchor, NormalizedRect},
};
use viewer_infrastructure::{
    SystemClock,
    review::{ContinuousReviewCommandCodec, ProjectReviewRepositoryProvider},
};

#[derive(Default)]
struct ForegroundSpies {
    source_reads: AtomicUsize,
    evidence_renders: AtomicUsize,
    full_view_repository_opens: AtomicUsize,
}

struct PreparedAssets {
    value: PreparedReviewAsset,
    spies: Arc<ForegroundSpies>,
}

#[async_trait]
impl ContinuousReviewAssetPort for PreparedAssets {
    async fn prepare_additions(
        &self,
        entity_ids: &[EntityId],
        _: ReviewTaskCancellation,
    ) -> Result<Vec<PreparedReviewAsset>, ReviewAssetError> {
        if entity_ids == [self.value.entity_id] {
            Ok(vec![self.value.clone()])
        } else {
            Err(ReviewAssetError::NotFound)
        }
    }

    async fn reopen_exact(
        &self,
        _: &[AssetVersion],
        _: ReviewTaskCancellation,
    ) -> Result<Vec<PreparedReviewAsset>, ReviewAssetError> {
        self.spies.source_reads.fetch_add(1, Ordering::Relaxed);
        Err(ReviewAssetError::Unavailable)
    }

    async fn check_sources(
        &self,
        _: &[AssetVersion],
        _: ReviewTaskCancellation,
    ) -> Result<Vec<viewer_domain::review::continuous::SourceCheck>, ReviewAssetError> {
        self.spies.source_reads.fetch_add(1, Ordering::Relaxed);
        Err(ReviewAssetError::Unavailable)
    }

    async fn confirm_relocation(
        &self,
        _: &AssetVersion,
        _: SourceRelocationDecision,
        _: ReviewTaskCancellation,
    ) -> Result<(), ReviewAssetError> {
        self.spies.source_reads.fetch_add(1, Ordering::Relaxed);
        Err(ReviewAssetError::Unavailable)
    }
}

struct NoForegroundEvidence(Arc<ForegroundSpies>);

#[async_trait]
impl ReviewEvidencePort for NoForegroundEvidence {
    async fn capture_base(
        &self,
        _: PreparedReviewAsset,
        _: ReviewTaskCancellation,
    ) -> Result<BoundReviewImage, ReviewArtifactError> {
        self.0.evidence_renders.fetch_add(1, Ordering::Relaxed);
        Err(ReviewArtifactError::Unavailable)
    }

    async fn render(
        &self,
        _: ReviewEvidenceRequest,
    ) -> Result<ReviewEvidenceResult, ReviewArtifactError> {
        self.0.evidence_renders.fetch_add(1, Ordering::Relaxed);
        Err(ReviewArtifactError::Unavailable)
    }
}

struct ObservedProvider {
    inner: Arc<ProjectReviewRepositoryProvider>,
    spies: Arc<ForegroundSpies>,
}

impl ContinuousReviewRepositoryProviderPort for ObservedProvider {
    fn open_reader(&self) -> Result<Arc<dyn ContinuousReviewRepositoryPort>, ReviewCommitError> {
        self.spies
            .full_view_repository_opens
            .fetch_add(1, Ordering::Relaxed);
        self.inner.continuous_reader()
    }

    fn open_writer(&self) -> Result<Arc<dyn ContinuousReviewRepositoryPort>, ReviewCommitError> {
        self.spies
            .full_view_repository_opens
            .fetch_add(1, Ordering::Relaxed);
        self.inner.continuous_writer()
    }

    fn open_authoring_reader(
        &self,
    ) -> Result<Arc<dyn ContinuousReviewAuthoringRepositoryPort>, ReviewCommitError> {
        Ok(self.inner.authoring_reader()?)
    }

    fn open_authoring_writer(
        &self,
    ) -> Result<Arc<dyn ContinuousReviewAuthoringRepositoryPort>, ReviewCommitError> {
        Ok(self.inner.authoring_writer()?)
    }
}

struct SlowPublication;

#[async_trait]
impl ReviewPublicationPort for SlowPublication {
    async fn materialize(
        &self,
        _: StoredAuthoringState,
        _: ReviewTaskCancellation,
    ) -> Result<PreparedReviewPublication, ReviewWorkspaceError> {
        tokio::time::sleep(Duration::from_millis(250)).await;
        Err(ReviewCommitError::Io.into())
    }

    async fn publish(
        &self,
        _: PreparedReviewPublication,
    ) -> Result<ReviewPublicationReceipt, ReviewWorkspaceError> {
        Err(ReviewCommitError::Io.into())
    }

    async fn verify_publication(
        &self,
        _: &StoredAuthoringState,
    ) -> Result<Option<ReviewPublicationReceipt>, ReviewWorkspaceError> {
        Ok(None)
    }
}

struct Harness {
    _root: tempfile::TempDir,
    service: ContinuousReviewService,
    store: Arc<dyn ContinuousReviewAuthoringRepositoryPort>,
    asset: AssetVersionId,
    spies: Arc<ForegroundSpies>,
}

impl Harness {
    async fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/images/rotated-6.jpg");
        fs::copy(source, root.path().join("source.jpg")).unwrap();

        let real = build_portable_composition(root.path()).unwrap();
        let prepared = real
            .assets
            .prepare_additions(
                &[real.nodes[0].entity_id],
                ReviewTaskCancellation::default(),
            )
            .await
            .unwrap()
            .remove(0);
        let provider = real.provider.clone();
        let stream = real.stream_id;
        provider.bootstrap_authoring(stream).unwrap();
        let store: Arc<dyn ContinuousReviewAuthoringRepositoryPort> =
            provider.authoring_writer().unwrap();
        let spies = Arc::new(ForegroundSpies::default());
        let observed_provider: Arc<dyn ContinuousReviewRepositoryProviderPort> =
            Arc::new(ObservedProvider {
                inner: provider,
                spies: spies.clone(),
            });
        let assets: Arc<dyn ContinuousReviewAssetPort> = Arc::new(PreparedAssets {
            value: prepared.clone(),
            spies: spies.clone(),
        });
        let service = ContinuousReviewService::new(
            ReviewWorkspaceContext {
                project_id: real.project_id,
                stream_id: stream,
                production: None,
            },
            observed_provider,
            assets,
            Arc::new(NoForegroundEvidence(spies.clone())),
            Arc::new(ContinuousReviewCommandCodec),
            Arc::new(SystemClock),
        );
        service
            .prepare_assets(&[prepared.entity_id], ReviewTaskCancellation::default())
            .await
            .unwrap();
        Self {
            _root: root,
            service,
            store,
            asset: prepared.asset.id,
            spies,
        }
    }

    async fn save(
        &self,
        expected: Option<viewer_domain::ReviewSnapshotId>,
        feedback_id: Option<FeedbackId>,
        text: String,
        targets: Vec<TargetEdit>,
    ) -> (ReviewAuthoringApplyResult, Duration) {
        let started = Instant::now();
        let envelope = self
            .service
            .prepare(
                ReviewCommandId::new(),
                expected,
                ReviewWorkspaceCommand::SaveFeedback {
                    feedback_id,
                    text,
                    targets,
                },
            )
            .await
            .unwrap();
        let applied = self
            .service
            .apply_authoring_with_cancellation(envelope, ReviewTaskCancellation::default())
            .await
            .unwrap();
        serde_json::to_vec(&ReviewAuthoringApplyResultDto::from(applied.clone())).unwrap();
        (applied, started.elapsed())
    }

    fn assert_foreground_is_logical_only(&self) {
        assert_eq!(self.spies.source_reads.load(Ordering::Relaxed), 0);
        assert_eq!(self.spies.evidence_renders.load(Ordering::Relaxed), 0);
        assert_eq!(
            self.spies
                .full_view_repository_opens
                .load(Ordering::Relaxed),
            0
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "release-mode local filesystem performance gate"]
async fn normal_authoring_save_meets_latency_budgets() {
    let harness = Harness::new().await;
    let (created, asset_only) = harness
        .save(
            None,
            None,
            "asset-only".into(),
            vec![TargetEdit::Add {
                asset_version_id: harness.asset,
                anchor: FeedbackAnchor::Asset,
            }],
        )
        .await;
    let feedback = created.patch.upsert_feedback[0].id;
    let mut expected = Some(created.receipt.head.snapshot_id);

    let (first_region, uncached_region) = harness
        .save(
            expected,
            Some(feedback),
            "asset and first region".into(),
            vec![TargetEdit::Add {
                asset_version_id: harness.asset,
                anchor: FeedbackAnchor::ImageRect(NormalizedRect::new(0.1, 0.1, 0.2, 0.2).unwrap()),
            }],
        )
        .await;
    expected = Some(first_region.receipt.head.snapshot_id);
    let (additional_region, cached_additional_region) = harness
        .save(
            expected,
            Some(feedback),
            "asset and two regions".into(),
            vec![TargetEdit::Add {
                asset_version_id: harness.asset,
                anchor: FeedbackAnchor::ImageRect(
                    NormalizedRect::new(0.45, 0.45, 0.2, 0.2).unwrap(),
                ),
            }],
        )
        .await;
    expected = Some(additional_region.receipt.head.snapshot_id);
    let (edited, text_only) = harness
        .save(expected, Some(feedback), "text-only edit".into(), vec![])
        .await;
    expected = Some(edited.receipt.head.snapshot_id);

    let materializer = Arc::new(ReviewMaterializationService::new(
        harness.store.clone(),
        Arc::new(SlowPublication),
        Arc::new(SystemClock) as Arc<dyn ClockPort>,
    ));
    let slow_worker = {
        let materializer = materializer.clone();
        tokio::spawn(async move {
            materializer
                .run_one(ReviewTaskCancellation::default())
                .await
        })
    };
    tokio::task::yield_now().await;
    let mut samples = vec![
        asset_only,
        uncached_region,
        cached_additional_region,
        text_only,
    ];
    for index in 0..30 {
        let (saved, elapsed) = harness
            .save(
                expected,
                Some(feedback),
                format!("queued text edit {index}"),
                vec![],
            )
            .await;
        expected = Some(saved.receipt.head.snapshot_id);
        samples.push(elapsed);
    }
    assert!(matches!(
        slow_worker.await.unwrap().unwrap(),
        ReviewMaterializationOutcome::Retrying { .. }
    ));

    let evidence_dir = harness._root.path().join(".viewer/benchmark-evidence");
    fs::create_dir_all(&evidence_dir).unwrap();
    let payload = vec![7_u8; 64 * 1024];
    let mut evidence_p95 = Vec::new();
    for evidence_count in [1_usize, 10, 100, 1_000] {
        for index in 0..evidence_count {
            let path = evidence_dir.join(format!("{index:04}.bin"));
            if !path.exists() {
                fs::write(path, &payload).unwrap();
            }
        }
        let mut group = Vec::new();
        for index in 0..12 {
            let (saved, elapsed) = harness
                .save(
                    expected,
                    Some(feedback),
                    format!("evidence-{evidence_count}-edit-{index}"),
                    vec![],
                )
                .await;
            expected = Some(saved.receipt.head.snapshot_id);
            group.push(elapsed);
            samples.push(elapsed);
        }
        evidence_p95.push((evidence_count, percentile(&mut group, 95)));
    }

    for index in 0..918 {
        let (saved, elapsed) = harness
            .save(
                expected,
                Some(feedback),
                format!("latency sample {index}"),
                vec![],
            )
            .await;
        expected = Some(saved.receipt.head.snapshot_id);
        samples.push(elapsed);
    }
    assert_eq!(samples.len(), 1_000);
    let p50 = percentile(&mut samples.clone(), 50);
    let p95 = percentile(&mut samples.clone(), 95);
    let p99 = percentile(&mut samples, 99);
    eprintln!(
        "authoring p50={}us p95={}us p99={}us evidence_p95={evidence_p95:?}",
        p50.as_micros(),
        p95.as_micros(),
        p99.as_micros()
    );
    assert!(p50 <= Duration::from_millis(50));
    assert!(p95 <= Duration::from_millis(150));
    assert!(p99 <= Duration::from_millis(300));
    assert!(
        evidence_p95
            .iter()
            .all(|(_, elapsed)| *elapsed <= Duration::from_millis(150))
    );
    let fastest = evidence_p95.iter().map(|(_, value)| *value).min().unwrap();
    let slowest = evidence_p95.iter().map(|(_, value)| *value).max().unwrap();
    assert!(slowest <= fastest.saturating_mul(4) + Duration::from_millis(20));
    harness.assert_foreground_is_logical_only();
}

fn percentile(samples: &mut [Duration], percentile: usize) -> Duration {
    samples.sort_unstable();
    let rank = percentile.saturating_mul(samples.len()).div_ceil(100);
    samples[rank.saturating_sub(1).min(samples.len() - 1)]
}
