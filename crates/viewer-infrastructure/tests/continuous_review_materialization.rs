use async_trait::async_trait;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tempfile::TempDir;
use viewer_application::{
    ClockPort, PreparedReviewAsset, ProjectAccess, ReviewArtifactError, ReviewAssetError,
    ReviewTaskCancellation,
    review_assets::{ContinuousReviewAssetPort, SourceRelocationDecision},
    review_evidence::{
        BoundReviewImage, ReviewEvidencePort, ReviewEvidenceRequest, ReviewEvidenceResult,
    },
    review_workspace::*,
};
use viewer_domain::{
    FeedbackId, ProjectId, ReviewArchiveId, ReviewCommandId, ReviewSnapshotId, ReviewStreamId,
    ReviewTextRevisionId,
    review::{AssetVersion, continuous::ContinuousReviewState},
};
use viewer_infrastructure::{
    portable::PortableProjectMetadata,
    review::{ProjectReviewRepositoryProvider, ReviewCommitFaultInjector, ReviewCommitFaultPoint},
};

struct Clock(i64);
impl ClockPort for Clock {
    fn unix_millis(&self) -> i64 {
        self.0
    }
}

struct Assets;
#[async_trait]
impl ContinuousReviewAssetPort for Assets {
    async fn prepare_additions(
        &self,
        _: &[viewer_domain::EntityId],
        _: ReviewTaskCancellation,
    ) -> Result<Vec<PreparedReviewAsset>, ReviewAssetError> {
        panic!("materializer never prepares new identities")
    }
    async fn reopen_exact(
        &self,
        assets: &[AssetVersion],
        _: ReviewTaskCancellation,
    ) -> Result<Vec<PreparedReviewAsset>, ReviewAssetError> {
        assert!(assets.is_empty());
        Ok(vec![])
    }
    async fn check_sources(
        &self,
        _: &[AssetVersion],
        _: ReviewTaskCancellation,
    ) -> Result<Vec<viewer_domain::review::continuous::SourceCheck>, ReviewAssetError> {
        panic!("materializer uses reopen_exact")
    }
    async fn confirm_relocation(
        &self,
        _: &AssetVersion,
        _: SourceRelocationDecision,
        _: ReviewTaskCancellation,
    ) -> Result<(), ReviewAssetError> {
        panic!("materializer never confirms relocation")
    }
}

struct Evidence;
#[async_trait]
impl ReviewEvidencePort for Evidence {
    async fn capture_base(
        &self,
        _: PreparedReviewAsset,
        _: ReviewTaskCancellation,
    ) -> Result<BoundReviewImage, ReviewArtifactError> {
        panic!("empty target has no evidence")
    }
    async fn render(
        &self,
        _: ReviewEvidenceRequest,
    ) -> Result<ReviewEvidenceResult, ReviewArtifactError> {
        panic!("empty target has no evidence")
    }
}

struct FailOnce {
    point: ReviewCommitFaultPoint,
    pending: AtomicBool,
}
impl FailOnce {
    fn at(point: ReviewCommitFaultPoint) -> Arc<Self> {
        Arc::new(Self {
            point,
            pending: AtomicBool::new(true),
        })
    }
}
impl ReviewCommitFaultInjector for FailOnce {
    fn check(&self, point: ReviewCommitFaultPoint) -> Result<(), ReviewCommitError> {
        if point == self.point && self.pending.swap(false, Ordering::AcqRel) {
            Err(ReviewCommitError::Io)
        } else {
            Ok(())
        }
    }
}

struct FaultProvider {
    inner: Arc<ProjectReviewRepositoryProvider>,
    faults: Arc<dyn ReviewCommitFaultInjector>,
}

struct FailPublishedHeadOnce {
    inner: Arc<viewer_infrastructure::review::SqliteContinuousReviewAuthoringStore>,
    pending: AtomicBool,
}
impl ReviewMaterializationQueuePort for FailPublishedHeadOnce {
    fn next(&self, now_ms: i64) -> Result<Option<ClaimedReviewMaterialization>, ReviewCommitError> {
        self.inner.next(now_ms)
    }
    fn retry(
        &self,
        claim: &ClaimedReviewMaterialization,
        code: ReviewMaterializationFailure,
        next_attempt_at_ms: i64,
    ) -> Result<(), ReviewCommitError> {
        self.inner.retry(claim, code, next_attempt_at_ms)
    }
    fn block(
        &self,
        claim: &ClaimedReviewMaterialization,
        code: ReviewMaterializationFailure,
    ) -> Result<(), ReviewCommitError> {
        self.inner.block(claim, code)
    }
    fn mark_published(
        &self,
        claim: &ClaimedReviewMaterialization,
        receipt: ReviewPublicationReceipt,
    ) -> Result<(), ReviewCommitError> {
        if self.pending.swap(false, Ordering::AcqRel) {
            return Err(ReviewCommitError::Io);
        }
        self.inner.mark_published(claim, receipt)
    }
    fn status(&self, stream: ReviewStreamId) -> Result<ReviewPublicationStatus, ReviewCommitError> {
        self.inner.status(stream)
    }
    fn requeue_expired(&self, now_ms: i64) -> Result<u32, ReviewCommitError> {
        self.inner.requeue_expired(now_ms)
    }
}
impl ContinuousReviewRepositoryProviderPort for FaultProvider {
    fn open_reader(&self) -> Result<Arc<dyn ContinuousReviewRepositoryPort>, ReviewCommitError> {
        self.inner.continuous_reader()
    }
    fn open_writer(&self) -> Result<Arc<dyn ContinuousReviewRepositoryPort>, ReviewCommitError> {
        self.inner
            .continuous_writer_with_faults(self.faults.clone())
    }
}

struct Fixture {
    _root: TempDir,
    stream: ReviewStreamId,
    provider: Arc<ProjectReviewRepositoryProvider>,
    authoring: Arc<viewer_infrastructure::review::SqliteContinuousReviewAuthoringStore>,
    target: StoredAuthoringState,
}
impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().unwrap();
        let metadata = PortableProjectMetadata::open(root.path(), ProjectAccess::ReadWrite, 1)
            .expect("create portable metadata");
        let project_id = metadata.project_id();
        drop(metadata);
        let stream = ReviewStreamId::from_u128(2);
        let provider = Arc::new(ProjectReviewRepositoryProvider::new(
            root.path(),
            project_id,
        ));
        provider.bootstrap_authoring(stream).unwrap();
        let authoring = provider.authoring_writer().unwrap();
        let target = logical_target(project_id, stream);
        let command = target.command_id;
        let digest = target.payload_digest;
        let request = ReviewAuthoringCommitRequest {
            expected_snapshot_id: None,
            next: target.clone(),
        };
        authoring
            .commit(stream, command, digest, &mut |_| Ok(request.clone()))
            .unwrap();
        Self {
            _root: root,
            stream,
            provider,
            authoring,
            target,
        }
    }

    fn service(
        &self,
        provider: Arc<dyn ContinuousReviewRepositoryProviderPort>,
        now_ms: i64,
    ) -> ReviewMaterializationService {
        ReviewMaterializationService::new(
            self.authoring.clone(),
            Arc::new(ContinuousReviewPublication::new(
                self.stream,
                provider,
                Arc::new(Assets),
                Arc::new(Evidence),
            )),
            Arc::new(Clock(now_ms)),
        )
    }
}

fn logical_target(project_id: ProjectId, stream: ReviewStreamId) -> StoredAuthoringState {
    let snapshot_id = ReviewSnapshotId::from_u128(10);
    StoredAuthoringState {
        head: ReviewAuthoringHead {
            sequence: 1,
            snapshot_id,
        },
        production: None,
        state: ContinuousReviewState::empty(project_id, stream, snapshot_id),
        command_id: ReviewCommandId::from_u128(11),
        payload_digest: [12; 32],
        generated: GeneratedReviewIds {
            snapshot_id,
            feedback_id: FeedbackId::from_u128(13),
            text_revision_id: ReviewTextRevisionId::from_u128(14),
            archive_id: ReviewArchiveId::from_u128(15),
            targets: vec![],
            migration: vec![],
            created_at_ms: 1_000,
        },
        changes: vec![],
        archives: vec![],
        adopted_usage: vec![],
        barrier: ReviewBarrierKind::None,
    }
}

fn commit_second_target(fixture: &Fixture) -> StoredAuthoringState {
    let mut second = fixture.target.clone();
    second.head = ReviewAuthoringHead {
        sequence: 2,
        snapshot_id: ReviewSnapshotId::from_u128(20),
    };
    second.state.snapshot_id = second.head.snapshot_id;
    second.state.parent = Some(viewer_domain::review::continuous::SnapshotRef {
        snapshot_id: fixture.target.head.snapshot_id,
        blake3: fixture.target.payload_digest,
    });
    second.command_id = ReviewCommandId::from_u128(21);
    second.payload_digest = [22; 32];
    second.generated.snapshot_id = second.head.snapshot_id;
    second.generated.created_at_ms = 1_001;
    fixture
        .authoring
        .commit(
            fixture.stream,
            second.command_id,
            second.payload_digest,
            &mut |_| {
                Ok(ReviewAuthoringCommitRequest {
                    expected_snapshot_id: Some(fixture.target.head.snapshot_id),
                    next: second.clone(),
                })
            },
        )
        .unwrap();
    second
}

#[tokio::test]
async fn real_v3_publication_advances_the_database_head_only_after_verification() {
    let fixture = Fixture::new();
    assert_ne!(
        fixture
            .authoring
            .load_heads(fixture.stream)
            .unwrap()
            .authoring,
        fixture
            .authoring
            .load_heads(fixture.stream)
            .unwrap()
            .published
    );

    let outcome = fixture
        .service(fixture.provider.clone(), 2_000)
        .run_one(ReviewTaskCancellation::default())
        .await
        .unwrap();

    assert!(matches!(
        outcome,
        ReviewMaterializationOutcome::Published { .. }
    ));
    let heads = fixture.authoring.load_heads(fixture.stream).unwrap();
    assert_eq!(heads.authoring, heads.published);
    let v3 = fixture
        .provider
        .continuous_reader()
        .unwrap()
        .load_current(fixture.stream)
        .unwrap()
        .unwrap();
    assert_eq!(v3.state, fixture.target.state);
    assert_eq!(v3.command_id, fixture.target.command_id);
    assert_eq!(v3.payload_digest, fixture.target.payload_digest);
}

#[tokio::test]
async fn restart_after_immutable_state_write_retries_without_advancing_the_head_early() {
    let fixture = Fixture::new();
    let faulted: Arc<dyn ContinuousReviewRepositoryProviderPort> = Arc::new(FaultProvider {
        inner: fixture.provider.clone(),
        faults: FailOnce::at(ReviewCommitFaultPoint::AfterState),
    });

    let first = fixture
        .service(faulted, 2_000)
        .run_one(ReviewTaskCancellation::default())
        .await
        .unwrap();
    assert!(matches!(
        first,
        ReviewMaterializationOutcome::Retrying { .. }
    ));
    let heads = fixture.authoring.load_heads(fixture.stream).unwrap();
    assert_ne!(heads.authoring, heads.published);

    let second = fixture
        .service(fixture.provider.clone(), 2_100)
        .run_one(ReviewTaskCancellation::default())
        .await
        .unwrap();
    assert!(matches!(
        second,
        ReviewMaterializationOutcome::Published { .. }
    ));
    let heads = fixture.authoring.load_heads(fixture.stream).unwrap();
    assert_eq!(heads.authoring, heads.published);
}

#[tokio::test]
async fn consecutive_pending_revisions_bind_to_the_actual_published_parent() {
    let fixture = Fixture::new();
    let second_target = commit_second_target(&fixture);

    assert!(matches!(
        fixture
            .service(fixture.provider.clone(), 2_000)
            .run_one(ReviewTaskCancellation::default())
            .await
            .unwrap(),
        ReviewMaterializationOutcome::Published { .. }
    ));
    assert!(matches!(
        fixture
            .service(fixture.provider.clone(), 2_001)
            .run_one(ReviewTaskCancellation::default())
            .await
            .unwrap(),
        ReviewMaterializationOutcome::Published { .. }
    ));

    let current = fixture
        .provider
        .continuous_reader()
        .unwrap()
        .load_current(fixture.stream)
        .unwrap()
        .unwrap();
    assert_eq!(current.state.snapshot_id, second_target.head.snapshot_id);
    assert_eq!(
        current.state.parent.unwrap().snapshot_id,
        fixture.target.head.snapshot_id
    );
    let heads = fixture.authoring.load_heads(fixture.stream).unwrap();
    assert_eq!(heads.authoring, heads.published);
}

#[tokio::test]
async fn outcome_unknown_after_index_publish_is_verified_and_finalized() {
    let fixture = Fixture::new();
    let faulted: Arc<dyn ContinuousReviewRepositoryProviderPort> = Arc::new(FaultProvider {
        inner: fixture.provider.clone(),
        faults: FailOnce::at(ReviewCommitFaultPoint::AfterIndex),
    });

    let outcome = fixture
        .service(faulted, 2_000)
        .run_one(ReviewTaskCancellation::default())
        .await
        .unwrap();

    assert!(matches!(
        outcome,
        ReviewMaterializationOutcome::Published { .. }
    ));
    let heads = fixture.authoring.load_heads(fixture.stream).unwrap();
    assert_eq!(heads.authoring, heads.published);
}

#[tokio::test]
async fn verification_rejects_same_snapshot_identity_with_different_logical_payload() {
    let fixture = Fixture::new();
    fixture
        .service(fixture.provider.clone(), 2_000)
        .run_one(ReviewTaskCancellation::default())
        .await
        .unwrap();
    let publication = ContinuousReviewPublication::new(
        fixture.stream,
        fixture.provider.clone(),
        Arc::new(Assets),
        Arc::new(Evidence),
    );
    let mut wrong = fixture.target.clone();
    wrong.payload_digest = [99; 32];

    assert_eq!(
        publication.verify_publication(&wrong).await.unwrap_err(),
        ReviewWorkspaceError::Repository(ReviewCommitError::Integrity)
    );
}

#[tokio::test]
async fn restart_before_published_head_reconciles_without_rewriting_v3() {
    let fixture = Fixture::new();
    let failing_queue: Arc<dyn ReviewMaterializationQueuePort> = Arc::new(FailPublishedHeadOnce {
        inner: fixture.authoring.clone(),
        pending: AtomicBool::new(true),
    });
    let first = ReviewMaterializationService::new(
        failing_queue,
        Arc::new(ContinuousReviewPublication::new(
            fixture.stream,
            fixture.provider.clone(),
            Arc::new(Assets),
            Arc::new(Evidence),
        )),
        Arc::new(Clock(2_000)),
    );

    assert_eq!(
        first
            .run_one(ReviewTaskCancellation::default())
            .await
            .unwrap_err(),
        ReviewWorkspaceError::Repository(ReviewCommitError::Io)
    );
    assert_ne!(
        fixture
            .authoring
            .load_heads(fixture.stream)
            .unwrap()
            .authoring,
        fixture
            .authoring
            .load_heads(fixture.stream)
            .unwrap()
            .published
    );
    let state_count = std::fs::read_dir(fixture._root.path().join(".viewer/reviews/states"))
        .unwrap()
        .count();

    let second = fixture
        .service(fixture.provider.clone(), 32_000)
        .run_one(ReviewTaskCancellation::default())
        .await
        .unwrap();
    assert!(matches!(
        second,
        ReviewMaterializationOutcome::Published { .. }
    ));
    assert_eq!(
        std::fs::read_dir(fixture._root.path().join(".viewer/reviews/states"))
            .unwrap()
            .count(),
        state_count
    );
    let heads = fixture.authoring.load_heads(fixture.stream).unwrap();
    assert_eq!(heads.authoring, heads.published);
}

#[test]
fn a_retry_delay_on_the_oldest_revision_prevents_newer_revision_from_skipping_it() {
    let fixture = Fixture::new();
    commit_second_target(&fixture);

    let oldest = fixture.authoring.next(2_000).unwrap().unwrap();
    assert_eq!(oldest.target.head.sequence, 1);
    fixture
        .authoring
        .retry(&oldest, ReviewMaterializationFailure::Io, 10_000)
        .unwrap();

    assert_eq!(fixture.authoring.next(2_000).unwrap(), None);
}
