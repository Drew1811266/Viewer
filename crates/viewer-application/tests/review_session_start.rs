use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use viewer_application::{
    ClockPort, PersistedReviewDraft, PreparedReviewAsset, ReviewAssetCatalogPort, ReviewAssetError,
    ReviewAssetValidation, ReviewCatalog, ReviewProgressPort, ReviewProtocolVersion,
    ReviewPublication, ReviewRecordLocation, ReviewRepositoryError, ReviewRepositoryInspection,
    ReviewRepositoryPort, ReviewRepositoryProviderPort, ReviewRoundRecord, ReviewScope,
    ReviewScopeResolution, ReviewSessionPhase, ReviewSessionService, ReviewStreamHead,
    ReviewTaskCancellation, ReviewTaskProgress,
};
use viewer_domain::review::{
    AssetEvidence, AssetVersion, ProductionId, ProductionScope, ReviewDraft, ReviewMedia,
    ReviewSnapshot, ReviewabilityFailure,
};
use viewer_domain::{
    AssetVersionId, EntityId, ProjectId, RelativePath, ReviewRoundId, ReviewStreamId,
};

#[derive(Default)]
struct NoProgress;

impl ReviewProgressPort for NoProgress {
    fn report(&self, _progress: ReviewTaskProgress) {}
}

struct TestClock(AtomicI64);

impl ClockPort for TestClock {
    fn unix_millis(&self) -> i64 {
        self.0.load(Ordering::Acquire)
    }
}

struct AssetCatalogState {
    resolution: ReviewScopeResolution,
    prepared: Vec<PreparedReviewAsset>,
    validations: Vec<ReviewAssetValidation>,
    block_prepare: bool,
}

struct FakeAssetCatalog {
    state: Mutex<AssetCatalogState>,
    prepare_started: tokio::sync::Notify,
    tracking_releases: AtomicUsize,
}

impl FakeAssetCatalog {
    fn new(prepared: Vec<PreparedReviewAsset>) -> Self {
        let resolution = resolution_for(&prepared);
        let validations = prepared
            .iter()
            .cloned()
            .map(ReviewAssetValidation::Current)
            .collect();
        Self {
            state: Mutex::new(AssetCatalogState {
                resolution,
                prepared,
                validations,
                block_prepare: false,
            }),
            prepare_started: tokio::sync::Notify::new(),
            tracking_releases: AtomicUsize::new(0),
        }
    }

    fn set_resolution(&self, resolution: ReviewScopeResolution) {
        self.state.lock().unwrap().resolution = resolution;
    }

    fn set_validations(&self, validations: Vec<ReviewAssetValidation>) {
        self.state.lock().unwrap().validations = validations;
    }

    fn block_prepare(&self) {
        self.state.lock().unwrap().block_prepare = true;
    }
}

#[async_trait]
impl ReviewAssetCatalogPort for FakeAssetCatalog {
    async fn resolve_scope(
        &self,
        _scope: &ReviewScope,
    ) -> Result<ReviewScopeResolution, ReviewAssetError> {
        Ok(self.state.lock().unwrap().resolution.clone())
    }

    async fn prepare_assets(
        &self,
        entity_ids: &[EntityId],
        cancellation: ReviewTaskCancellation,
        _progress: Arc<dyn ReviewProgressPort>,
    ) -> Result<Vec<PreparedReviewAsset>, ReviewAssetError> {
        let (block, prepared) = {
            let state = self.state.lock().unwrap();
            (state.block_prepare, state.prepared.clone())
        };
        let expected = prepared
            .iter()
            .map(|prepared| prepared.entity_id)
            .collect::<Vec<_>>();
        if entity_ids != expected {
            return Err(ReviewAssetError::InvalidScope);
        }
        if block {
            self.prepare_started.notify_one();
            while !cancellation.is_cancelled() {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        }
        if cancellation.is_cancelled() {
            Err(ReviewAssetError::Cancelled)
        } else {
            Ok(prepared)
        }
    }

    async fn revalidate_assets(
        &self,
        _assets: &[PreparedReviewAsset],
        cancellation: ReviewTaskCancellation,
        _progress: Arc<dyn ReviewProgressPort>,
    ) -> Result<Vec<ReviewAssetValidation>, ReviewAssetError> {
        if cancellation.is_cancelled() {
            Err(ReviewAssetError::Cancelled)
        } else {
            Ok(self.state.lock().unwrap().validations.clone())
        }
    }

    fn release_tracking(&self) {
        self.tracking_releases.fetch_add(1, Ordering::AcqRel);
    }
}

#[derive(Default)]
struct RepositoryState {
    catalog: Option<ReviewCatalog>,
    drafts: Vec<ReviewDraft>,
    completed: HashMap<ReviewRoundId, ReviewSnapshot>,
    writer_held: bool,
    busy: bool,
    read_only: bool,
    fail_save: bool,
    draft_on_next_writer_open: Option<ReviewDraft>,
    save_count: usize,
    last_saved_protocol_version: Option<ReviewProtocolVersion>,
}

struct FakeRepositories {
    project_id: ProjectId,
    state: Arc<Mutex<RepositoryState>>,
}

impl FakeRepositories {
    fn new(project_id: ProjectId) -> Self {
        Self {
            project_id,
            state: Arc::new(Mutex::new(RepositoryState {
                catalog: Some(ReviewCatalog {
                    project_id,
                    streams: vec![],
                }),
                ..RepositoryState::default()
            })),
        }
    }

    fn set_drafts(&self, drafts: Vec<ReviewDraft>) {
        self.state.lock().unwrap().drafts = drafts;
    }

    fn insert_draft_after_inspection(&self, draft: ReviewDraft) {
        self.state.lock().unwrap().draft_on_next_writer_open = Some(draft);
    }

    fn set_catalog(&self, catalog: ReviewCatalog) {
        self.state.lock().unwrap().catalog = Some(catalog);
    }

    fn add_completed(&self, completed: ReviewSnapshot) {
        self.state
            .lock()
            .unwrap()
            .completed
            .insert(completed.review_round_id, completed);
    }

    fn writer_is_held(&self) -> bool {
        self.state.lock().unwrap().writer_held
    }

    fn saved_draft(&self) -> Option<ReviewDraft> {
        self.state.lock().unwrap().drafts.first().cloned()
    }
}

impl ReviewRepositoryProviderPort for FakeRepositories {
    fn inspect(&self) -> Result<ReviewRepositoryInspection, ReviewRepositoryError> {
        let state = self.state.lock().unwrap();
        if state.drafts.len() > 1 {
            return Err(ReviewRepositoryError::RecoveryRequired);
        }
        Ok(ReviewRepositoryInspection {
            catalog: state.catalog.clone().unwrap_or(ReviewCatalog {
                project_id: self.project_id,
                streams: vec![],
            }),
            active_draft: state.drafts.first().cloned().map(versioned_draft),
        })
    }

    fn open_reader(&self) -> Result<Box<dyn ReviewRepositoryPort>, ReviewRepositoryError> {
        Ok(Box::new(FakeRepository {
            project_id: self.project_id,
            state: Arc::clone(&self.state),
            writer: false,
        }))
    }

    fn open_writer(&self) -> Result<Box<dyn ReviewRepositoryPort>, ReviewRepositoryError> {
        let mut state = self.state.lock().unwrap();
        if state.busy || state.writer_held {
            return Err(ReviewRepositoryError::Busy);
        }
        if state.read_only {
            return Err(ReviewRepositoryError::ReadOnly);
        }
        state.writer_held = true;
        if let Some(draft) = state.draft_on_next_writer_open.take() {
            state.drafts.push(draft);
        }
        drop(state);
        Ok(Box::new(FakeRepository {
            project_id: self.project_id,
            state: Arc::clone(&self.state),
            writer: true,
        }))
    }
}

struct FakeRepository {
    project_id: ProjectId,
    state: Arc<Mutex<RepositoryState>>,
    writer: bool,
}

impl Drop for FakeRepository {
    fn drop(&mut self) {
        if self.writer {
            self.state.lock().unwrap().writer_held = false;
        }
    }
}

impl ReviewRepositoryPort for FakeRepository {
    fn load_catalog(&self) -> Result<ReviewCatalog, ReviewRepositoryError> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .catalog
            .clone()
            .unwrap_or(ReviewCatalog {
                project_id: self.project_id,
                streams: vec![],
            }))
    }

    fn load_active_draft(&self) -> Result<Option<PersistedReviewDraft>, ReviewRepositoryError> {
        let state = self.state.lock().unwrap();
        if state.drafts.len() > 1 {
            Err(ReviewRepositoryError::RecoveryRequired)
        } else {
            Ok(state.drafts.first().cloned().map(versioned_draft))
        }
    }

    fn load_draft(
        &self,
        stream_id: ReviewStreamId,
        round_id: ReviewRoundId,
    ) -> Result<Option<PersistedReviewDraft>, ReviewRepositoryError> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .drafts
            .iter()
            .find(|draft| draft.review_stream_id == stream_id && draft.review_round_id == round_id)
            .cloned()
            .map(versioned_draft))
    }

    fn save_draft(&self, draft: &PersistedReviewDraft) -> Result<(), ReviewRepositoryError> {
        let mut state = self.state.lock().unwrap();
        state.save_count += 1;
        state.last_saved_protocol_version = Some(draft.protocol_version);
        if state.fail_save {
            return Err(ReviewRepositoryError::Unavailable);
        }
        if let Some(existing) = state
            .drafts
            .iter_mut()
            .find(|existing| existing.review_round_id == draft.draft.review_round_id)
        {
            *existing = draft.draft.clone();
        } else {
            state.drafts.push(draft.draft.clone());
        }
        Ok(())
    }

    fn delete_draft(
        &self,
        stream_id: ReviewStreamId,
        round_id: ReviewRoundId,
    ) -> Result<(), ReviewRepositoryError> {
        let mut state = self.state.lock().unwrap();
        let Some(index) = state.drafts.iter().position(|draft| {
            draft.review_stream_id == stream_id && draft.review_round_id == round_id
        }) else {
            return Err(ReviewRepositoryError::NotFound);
        };
        state.drafts.remove(index);
        Ok(())
    }

    fn load_completed(
        &self,
        stream_id: ReviewStreamId,
        round_id: ReviewRoundId,
    ) -> Result<Option<ReviewSnapshot>, ReviewRepositoryError> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .completed
            .get(&round_id)
            .filter(|completed| completed.review_stream_id == stream_id)
            .cloned())
    }

    fn publish(&self, _publication: &ReviewPublication) -> Result<(), ReviewRepositoryError> {
        Ok(())
    }
}

struct Fixture {
    project_id: ProjectId,
    assets: Arc<FakeAssetCatalog>,
    repositories: Arc<FakeRepositories>,
    service: Arc<ReviewSessionService>,
}

impl Fixture {
    fn new() -> Self {
        let project_id = ProjectId::from_u128(1);
        let prepared = vec![
            prepared_asset(1, "a.png", None),
            prepared_asset(2, "b.mp4", None),
        ];
        let assets = Arc::new(FakeAssetCatalog::new(prepared));
        let repositories = Arc::new(FakeRepositories::new(project_id));
        let service = Arc::new(ReviewSessionService::new(
            project_id,
            assets.clone(),
            repositories.clone(),
            Arc::new(TestClock(AtomicI64::new(1_000))),
        ));
        Self {
            project_id,
            assets,
            repositories,
            service,
        }
    }

    fn progress(&self) -> Arc<dyn ReviewProgressPort> {
        Arc::new(NoProgress)
    }
}

fn prepared_asset(
    id: u128,
    path: &str,
    failure: Option<ReviewabilityFailure>,
) -> PreparedReviewAsset {
    let entity_id = EntityId::from_u128(id);
    let is_video = path.ends_with(".mp4");
    PreparedReviewAsset {
        entity_id,
        asset: AssetVersion {
            id: AssetVersionId::from_u128(id + 100),
            source_entity_id: Some(entity_id),
            relative_path: RelativePath::parse(path).unwrap(),
            evidence: AssetEvidence {
                size_bytes: 10,
                modified_ns: 20,
                blake3: Some([id as u8; 32]),
            },
            media: if is_video {
                ReviewMedia::Video {
                    duration_us: Some(1_000),
                    display_width: Some(100),
                    display_height: Some(50),
                }
            } else {
                ReviewMedia::Image {
                    width: Some(100),
                    height: Some(50),
                }
            },
            producer_asset_id: None,
            parent_asset_version_id: None,
        },
        failure,
        change_revision: 0,
    }
}

fn resolution_for(prepared: &[PreparedReviewAsset]) -> ReviewScopeResolution {
    ReviewScopeResolution {
        candidate_entity_ids: prepared.iter().map(|asset| asset.entity_id).collect(),
        image_count: prepared
            .iter()
            .filter(|asset| matches!(asset.asset.media, ReviewMedia::Image { .. }))
            .count() as u32,
        video_count: prepared
            .iter()
            .filter(|asset| matches!(asset.asset.media, ReviewMedia::Video { .. }))
            .count() as u32,
        excluded_count: 0,
    }
}

fn selection() -> ReviewScope {
    ReviewScope::Selection {
        entity_ids: vec![EntityId::from_u128(1), EntityId::from_u128(2)],
    }
}

fn draft(project_id: ProjectId, stream: u128, round: u128) -> ReviewDraft {
    ReviewDraft::new(
        project_id,
        ReviewStreamId::from_u128(stream),
        ReviewRoundId::from_u128(round),
        None,
        None,
        900,
        vec![prepared_asset(1, "a.png", None).asset],
    )
    .unwrap()
}

fn versioned_draft(draft: ReviewDraft) -> PersistedReviewDraft {
    PersistedReviewDraft {
        protocol_version: ReviewProtocolVersion::V1,
        draft,
    }
}

fn round_record(round_id: ReviewRoundId) -> ReviewRoundRecord {
    ReviewRoundRecord {
        review_round_id: round_id,
        protocol_version: ReviewProtocolVersion::V1,
        location: ReviewRecordLocation::new(format!("rounds/{round_id}.json")).unwrap(),
        blake3: [0; 32],
    }
}

fn production_scope() -> ProductionScope {
    ProductionScope {
        task_id: ProductionId::parse("task-1").unwrap(),
        batch_id: ProductionId::parse("batch-1").unwrap(),
    }
}

#[tokio::test]
async fn inspection_projects_zero_one_many_and_future_production_drafts_without_writing() {
    let fixture = Fixture::new();
    assert_eq!(
        fixture.service.inspect().await.phase,
        ReviewSessionPhase::Idle
    );
    assert!(!fixture.repositories.writer_is_held());

    let resumable = draft(fixture.project_id, 10, 11);
    fixture.repositories.set_drafts(vec![resumable.clone()]);
    let snapshot = fixture.service.inspect().await;
    assert_eq!(snapshot.phase, ReviewSessionPhase::Idle);
    assert_eq!(
        snapshot.resume.unwrap().review_round_id,
        resumable.review_round_id
    );

    fixture
        .repositories
        .set_drafts(vec![resumable.clone(), draft(fixture.project_id, 10, 12)]);
    assert_eq!(
        fixture.service.inspect().await.phase,
        ReviewSessionPhase::RecoveryRequired
    );

    let mut future = resumable;
    future.production = Some(production_scope());
    fixture.repositories.set_drafts(vec![future]);
    assert_eq!(
        fixture.service.inspect().await.phase,
        ReviewSessionPhase::RecoveryRequired
    );
}

#[tokio::test]
async fn inspection_selects_one_exact_manual_head_and_ignores_production_streams() {
    let fixture = Fixture::new();
    let completed = draft(fixture.project_id, 10, 11).complete(1_000).unwrap();
    fixture.repositories.add_completed(completed.clone());
    fixture.repositories.set_catalog(ReviewCatalog {
        project_id: fixture.project_id,
        streams: vec![
            ReviewStreamHead {
                review_stream_id: ReviewStreamId::from_u128(99),
                production: Some(production_scope()),
                completed_rounds: vec![],
                latest_completed_round_id: None,
            },
            ReviewStreamHead {
                review_stream_id: completed.review_stream_id,
                production: None,
                completed_rounds: vec![round_record(completed.review_round_id)],
                latest_completed_round_id: Some(completed.review_round_id),
            },
        ],
    });

    let snapshot = fixture.service.inspect().await;
    assert_eq!(snapshot.phase, ReviewSessionPhase::CompletedReadOnly);
    assert_eq!(snapshot.review_round_id, Some(completed.review_round_id));
    assert_eq!(snapshot.members.len(), 1);
    assert!(!fixture.repositories.writer_is_held());

    let mut catalog = fixture
        .repositories
        .state
        .lock()
        .unwrap()
        .catalog
        .clone()
        .unwrap();
    catalog.streams.push(ReviewStreamHead {
        review_stream_id: ReviewStreamId::from_u128(12),
        production: None,
        completed_rounds: vec![],
        latest_completed_round_id: None,
    });
    fixture.repositories.set_catalog(catalog);
    assert_eq!(
        fixture.service.inspect().await.phase,
        ReviewSessionPhase::RecoveryRequired
    );
}

#[tokio::test]
async fn preview_is_read_only_rejects_empty_scope_and_invalidates_older_proposals() {
    let fixture = Fixture::new();
    let first = fixture.service.preview_start(selection()).await.unwrap();
    let second = fixture.service.preview_start(selection()).await.unwrap();
    assert_ne!(first.id, second.id);
    assert!(!fixture.repositories.writer_is_held());
    assert_eq!(
        fixture
            .service
            .start(first.id, fixture.progress())
            .await
            .unwrap_err()
            .code(),
        "review_proposal_stale"
    );

    fixture.assets.set_resolution(ReviewScopeResolution {
        candidate_entity_ids: vec![],
        image_count: 0,
        video_count: 0,
        excluded_count: 2,
    });
    assert_eq!(
        fixture
            .service
            .preview_start(selection())
            .await
            .unwrap_err()
            .code(),
        "review_scope_empty"
    );
}

#[tokio::test]
async fn start_creates_one_manual_draft_marks_failures_and_holds_writer_until_shutdown() {
    let fixture = Fixture::new();
    let prepared = vec![
        prepared_asset(1, "a.png", Some(ReviewabilityFailure::Damaged)),
        prepared_asset(2, "b.mp4", None),
    ];
    {
        let mut assets = fixture.assets.state.lock().unwrap();
        assets.resolution = resolution_for(&prepared);
        assets.prepared = prepared;
    }
    let proposal = fixture.service.preview_start(selection()).await.unwrap();

    let snapshot = fixture
        .service
        .start(proposal.id, fixture.progress())
        .await
        .unwrap();

    assert_eq!(snapshot.phase, ReviewSessionPhase::Active);
    assert_eq!(snapshot.revision, 1);
    assert_eq!(snapshot.unreviewable.len(), 1);
    assert!(fixture.repositories.writer_is_held());
    let saved = fixture.repositories.saved_draft().unwrap();
    assert_eq!(saved.production, None);
    assert_eq!(saved.previous_completed_round_id, None);
    assert_eq!(saved.assets.len(), 2);
    assert_eq!(
        fixture
            .repositories
            .state
            .lock()
            .unwrap()
            .last_saved_protocol_version,
        Some(ReviewProtocolVersion::V2)
    );

    fixture.service.shutdown().await;
    assert!(!fixture.repositories.writer_is_held());
}

#[tokio::test]
async fn later_manual_round_reuses_stream_and_exact_head_while_production_is_isolated() {
    let fixture = Fixture::new();
    let head = draft(fixture.project_id, 10, 11).complete(1_000).unwrap();
    fixture.repositories.add_completed(head.clone());
    fixture.repositories.set_catalog(ReviewCatalog {
        project_id: fixture.project_id,
        streams: vec![
            ReviewStreamHead {
                review_stream_id: ReviewStreamId::from_u128(99),
                production: Some(production_scope()),
                completed_rounds: vec![],
                latest_completed_round_id: None,
            },
            ReviewStreamHead {
                review_stream_id: head.review_stream_id,
                production: None,
                completed_rounds: vec![round_record(head.review_round_id)],
                latest_completed_round_id: Some(head.review_round_id),
            },
        ],
    });
    fixture.service.inspect().await;
    let proposal = fixture.service.preview_start(selection()).await.unwrap();

    fixture
        .service
        .start(proposal.id, fixture.progress())
        .await
        .unwrap();

    let saved = fixture.repositories.saved_draft().unwrap();
    assert_eq!(saved.review_stream_id, head.review_stream_id);
    assert_eq!(
        saved.previous_completed_round_id,
        Some(head.review_round_id)
    );
}

#[tokio::test]
async fn start_rechecks_draft_and_scope_after_acquiring_the_writer() {
    let fixture = Fixture::new();
    let proposal = fixture.service.preview_start(selection()).await.unwrap();
    fixture
        .repositories
        .insert_draft_after_inspection(draft(fixture.project_id, 10, 11));
    assert_eq!(
        fixture
            .service
            .start(proposal.id, fixture.progress())
            .await
            .unwrap_err()
            .code(),
        "review_draft_already_active"
    );
    assert!(!fixture.repositories.writer_is_held());

    fixture.repositories.set_drafts(vec![]);
    let proposal = fixture.service.preview_start(selection()).await.unwrap();
    fixture.assets.set_resolution(ReviewScopeResolution {
        candidate_entity_ids: vec![EntityId::from_u128(1)],
        image_count: 1,
        video_count: 0,
        excluded_count: 1,
    });
    assert_eq!(
        fixture
            .service
            .start(proposal.id, fixture.progress())
            .await
            .unwrap_err()
            .code(),
        "review_scope_changed"
    );
    assert!(!fixture.repositories.writer_is_held());
}

#[tokio::test]
async fn start_rejects_asset_adapter_output_that_does_not_match_the_confirmed_members() {
    let fixture = Fixture::new();
    let proposal = fixture.service.preview_start(selection()).await.unwrap();
    fixture.assets.state.lock().unwrap().prepared[0]
        .asset
        .source_entity_id = Some(EntityId::from_u128(999));

    let error = fixture
        .service
        .start(proposal.id, fixture.progress())
        .await
        .unwrap_err();

    assert_eq!(error.code(), "review_asset_unavailable");
    assert!(fixture.repositories.saved_draft().is_none());
    assert!(!fixture.repositories.writer_is_held());
}

#[tokio::test]
async fn busy_read_only_and_save_failure_never_leak_the_writer_or_tracking() {
    for configure in ["busy", "readonly", "save"] {
        let fixture = Fixture::new();
        {
            let mut state = fixture.repositories.state.lock().unwrap();
            state.busy = configure == "busy";
            state.read_only = configure == "readonly";
            state.fail_save = configure == "save";
        }
        let proposal = fixture.service.preview_start(selection()).await.unwrap();
        assert!(
            fixture
                .service
                .start(proposal.id, fixture.progress())
                .await
                .is_err()
        );
        assert!(!fixture.repositories.writer_is_held());
        assert!(fixture.assets.tracking_releases.load(Ordering::Acquire) > 0);
    }
}

#[tokio::test]
async fn read_only_resume_keeps_the_discovered_draft_available_for_a_later_retry() {
    let fixture = Fixture::new();
    let saved = draft(fixture.project_id, 10, 11);
    fixture.repositories.set_drafts(vec![saved.clone()]);
    let discovered = fixture.service.inspect().await;
    assert_eq!(
        discovered
            .resume
            .as_ref()
            .map(|resume| resume.review_round_id),
        Some(saved.review_round_id)
    );
    fixture.repositories.state.lock().unwrap().read_only = true;

    let error = fixture
        .service
        .resume(fixture.progress())
        .await
        .unwrap_err();

    assert_eq!(error.code(), "review_project_read_only");
    assert!(!fixture.repositories.writer_is_held());
    let rediscovered = fixture.service.inspect().await;
    assert_eq!(rediscovered.phase, ReviewSessionPhase::Idle);
    assert_eq!(
        rediscovered
            .resume
            .as_ref()
            .map(|resume| resume.review_round_id),
        Some(saved.review_round_id)
    );
}

#[tokio::test]
async fn start_rejects_ambiguous_manual_streams_rechecked_under_the_writer() {
    let fixture = Fixture::new();
    let proposal = fixture.service.preview_start(selection()).await.unwrap();
    fixture.repositories.set_catalog(ReviewCatalog {
        project_id: fixture.project_id,
        streams: vec![
            ReviewStreamHead {
                review_stream_id: ReviewStreamId::from_u128(10),
                production: None,
                completed_rounds: vec![],
                latest_completed_round_id: None,
            },
            ReviewStreamHead {
                review_stream_id: ReviewStreamId::from_u128(20),
                production: None,
                completed_rounds: vec![],
                latest_completed_round_id: None,
            },
        ],
    });

    let error = fixture
        .service
        .start(proposal.id, fixture.progress())
        .await
        .unwrap_err();

    assert_eq!(error.code(), "review_recovery_required");
    assert!(!fixture.repositories.writer_is_held());
}

#[tokio::test]
async fn cancellation_before_save_restores_idle_and_shutdown_releases_active_writer() {
    let fixture = Fixture::new();
    fixture.assets.block_prepare();
    let proposal = fixture.service.preview_start(selection()).await.unwrap();
    let service = Arc::clone(&fixture.service);
    let progress = fixture.progress();
    let start = tokio::spawn(async move { service.start(proposal.id, progress).await });
    fixture.assets.prepare_started.notified().await;

    assert!(fixture.service.cancel_task().await);
    assert_eq!(
        start.await.unwrap().unwrap_err().code(),
        "review_task_cancelled"
    );
    assert_eq!(
        fixture.service.inspect().await.phase,
        ReviewSessionPhase::Idle
    );
    assert!(fixture.repositories.saved_draft().is_none());
    assert!(!fixture.repositories.writer_is_held());
}

#[tokio::test]
async fn shutdown_cancels_and_joins_blocking_preparation_before_releasing_resources() {
    let fixture = Fixture::new();
    fixture.assets.block_prepare();
    let proposal = fixture.service.preview_start(selection()).await.unwrap();
    let service = Arc::clone(&fixture.service);
    let progress = fixture.progress();
    let start = tokio::spawn(async move { service.start(proposal.id, progress).await });
    fixture.assets.prepare_started.notified().await;

    fixture.service.shutdown().await;

    assert_eq!(
        start.await.unwrap().unwrap_err().code(),
        "review_task_cancelled"
    );
    assert!(fixture.repositories.saved_draft().is_none());
    assert!(!fixture.repositories.writer_is_held());
    assert!(fixture.assets.tracking_releases.load(Ordering::Acquire) > 0);
}

#[tokio::test]
async fn proposal_ids_remain_monotonic_across_shutdown_within_one_service() {
    let fixture = Fixture::new();
    let first = fixture.service.preview_start(selection()).await.unwrap();
    fixture
        .service
        .start(first.id, fixture.progress())
        .await
        .unwrap();
    fixture.service.shutdown().await;
    fixture.repositories.set_drafts(vec![]);
    fixture.service.inspect().await;

    let second = fixture.service.preview_start(selection()).await.unwrap();

    assert_ne!(first.id, second.id);
}

#[tokio::test]
async fn resume_revalidates_exact_members_and_keeps_hash_conflicts_visible() {
    let fixture = Fixture::new();
    let saved = draft(fixture.project_id, 10, 11);
    fixture.repositories.set_drafts(vec![saved.clone()]);
    fixture.service.inspect().await;
    let prepared = PreparedReviewAsset {
        entity_id: saved.assets[0].source_entity_id.unwrap(),
        asset: saved.assets[0].clone(),
        failure: None,
        change_revision: 0,
    };
    fixture
        .assets
        .set_validations(vec![ReviewAssetValidation::Conflict {
            asset_version_id: prepared.asset.id,
            relative_path: prepared.asset.relative_path.clone(),
            kind: viewer_application::ReviewAssetConflictKind::ContentChanged,
        }]);

    let snapshot = fixture.service.resume(fixture.progress()).await.unwrap();

    assert_eq!(snapshot.phase, ReviewSessionPhase::Active);
    assert_eq!(snapshot.conflicts.len(), 1);
    assert_eq!(snapshot.feedback.len(), saved.feedback.len());
    assert!(fixture.repositories.writer_is_held());
    fixture.service.shutdown().await;
}

#[tokio::test]
async fn resume_persists_changed_stable_failure_facts_before_entering_active() {
    let fixture = Fixture::new();
    let saved = draft(fixture.project_id, 10, 11);
    fixture.repositories.set_drafts(vec![saved.clone()]);
    fixture.service.inspect().await;
    let mut current = PreparedReviewAsset {
        entity_id: saved.assets[0].source_entity_id.unwrap(),
        asset: saved.assets[0].clone(),
        failure: Some(ReviewabilityFailure::Damaged),
        change_revision: 0,
    };
    current.asset.evidence.modified_ns += 1;
    fixture
        .assets
        .set_validations(vec![ReviewAssetValidation::Current(current)]);

    let resumed = fixture.service.resume(fixture.progress()).await.unwrap();

    assert_eq!(resumed.phase, ReviewSessionPhase::Active);
    assert_eq!(resumed.unreviewable.len(), 1);
    assert_eq!(
        fixture
            .repositories
            .saved_draft()
            .unwrap()
            .unreviewable
            .len(),
        1
    );
    fixture.service.shutdown().await;
}

#[tokio::test]
async fn resume_reloads_the_exact_draft_and_manual_head_under_the_writer() {
    let fixture = Fixture::new();
    let discovered = draft(fixture.project_id, 10, 11);
    fixture.repositories.set_drafts(vec![discovered]);
    fixture.service.inspect().await;
    fixture
        .repositories
        .set_drafts(vec![draft(fixture.project_id, 10, 12)]);

    let error = fixture
        .service
        .resume(fixture.progress())
        .await
        .unwrap_err();

    assert_eq!(error.code(), "review_recovery_required");
    assert!(!fixture.repositories.writer_is_held());
}

#[tokio::test]
async fn resume_rejects_revalidation_output_for_a_different_fixed_member() {
    let fixture = Fixture::new();
    let saved = draft(fixture.project_id, 10, 11);
    fixture.repositories.set_drafts(vec![saved.clone()]);
    fixture.service.inspect().await;
    let mut wrong = PreparedReviewAsset {
        entity_id: saved.assets[0].source_entity_id.unwrap(),
        asset: saved.assets[0].clone(),
        failure: None,
        change_revision: 0,
    };
    wrong.asset.id = AssetVersionId::from_u128(999);
    fixture
        .assets
        .set_validations(vec![ReviewAssetValidation::Current(wrong)]);

    let error = fixture
        .service
        .resume(fixture.progress())
        .await
        .unwrap_err();

    assert_eq!(error.code(), "review_asset_unavailable");
    assert!(!fixture.repositories.writer_is_held());
}
