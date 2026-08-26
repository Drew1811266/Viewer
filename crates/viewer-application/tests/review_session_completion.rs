use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use viewer_application::{
    AddReviewFeedback, ClockPort, PreparedReviewAsset, ReviewAssetCatalogPort,
    ReviewAssetConflictKind, ReviewAssetError, ReviewAssetValidation, ReviewCatalog,
    ReviewCompletionProposalId, ReviewMutationGuard, ReviewProgressPort, ReviewRepositoryError,
    ReviewRepositoryInspection, ReviewRepositoryPort, ReviewRepositoryProviderPort, ReviewScope,
    ReviewScopeResolution, ReviewSessionPhase, ReviewSessionService, ReviewStreamHead,
    ReviewTaskCancellation, ReviewTaskProgress,
};
use viewer_domain::review::{
    AssetEvidence, AssetVersion, ReviewDraft, ReviewMedia, ReviewOutcomeKind, ReviewSnapshot,
    ReviewabilityFailure,
};
use viewer_domain::{EntityId, ProjectId, RelativePath, ReviewRoundId, ReviewStreamId};

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

struct AssetState {
    prepared: Vec<PreparedReviewAsset>,
    validation_sequences: Vec<Vec<ReviewAssetValidation>>,
    validation_calls: usize,
    block_validation: bool,
}

struct FakeAssetCatalog {
    state: Mutex<AssetState>,
    validation_started: tokio::sync::Notify,
    tracking_releases: AtomicUsize,
}

impl FakeAssetCatalog {
    fn set_validation_sequences(&self, sequences: Vec<Vec<ReviewAssetValidation>>) {
        let mut state = self.state.lock().unwrap();
        state.validation_sequences = sequences;
        state.validation_calls = 0;
    }

    fn prepared(&self) -> Vec<PreparedReviewAsset> {
        self.state.lock().unwrap().prepared.clone()
    }

    fn block_validation(&self) {
        self.state.lock().unwrap().block_validation = true;
    }
}

#[async_trait]
impl ReviewAssetCatalogPort for FakeAssetCatalog {
    async fn resolve_scope(
        &self,
        _scope: &ReviewScope,
    ) -> Result<ReviewScopeResolution, ReviewAssetError> {
        let prepared = self.state.lock().unwrap().prepared.clone();
        Ok(ReviewScopeResolution {
            candidate_entity_ids: prepared.iter().map(|asset| asset.entity_id).collect(),
            image_count: prepared.len() as u32,
            video_count: 0,
            excluded_count: 0,
        })
    }

    async fn prepare_assets(
        &self,
        _entity_ids: &[EntityId],
        cancellation: ReviewTaskCancellation,
        _progress: Arc<dyn ReviewProgressPort>,
    ) -> Result<Vec<PreparedReviewAsset>, ReviewAssetError> {
        if cancellation.is_cancelled() {
            Err(ReviewAssetError::Cancelled)
        } else {
            Ok(self.state.lock().unwrap().prepared.clone())
        }
    }

    async fn revalidate_assets(
        &self,
        assets: &[PreparedReviewAsset],
        cancellation: ReviewTaskCancellation,
        _progress: Arc<dyn ReviewProgressPort>,
    ) -> Result<Vec<ReviewAssetValidation>, ReviewAssetError> {
        if cancellation.is_cancelled() {
            return Err(ReviewAssetError::Cancelled);
        }
        let block = self.state.lock().unwrap().block_validation;
        if block {
            self.validation_started.notify_one();
            while !cancellation.is_cancelled() {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
            return Err(ReviewAssetError::Cancelled);
        }
        let mut state = self.state.lock().unwrap();
        let result = state
            .validation_sequences
            .get(state.validation_calls)
            .cloned()
            .unwrap_or_else(|| {
                assets
                    .iter()
                    .cloned()
                    .map(ReviewAssetValidation::Current)
                    .collect()
            });
        state.validation_calls += 1;
        Ok(result)
    }

    fn release_tracking(&self) {
        self.tracking_releases.fetch_add(1, Ordering::AcqRel);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublishBehavior {
    Success,
    FaultAfterRoundDurable,
    FaultBeforeIndexReplace,
    ErrorWithoutDurability,
    SuccessWithWrongHead,
}

struct RepositoryState {
    catalog: ReviewCatalog,
    draft: Option<ReviewDraft>,
    completed: HashMap<ReviewRoundId, ReviewSnapshot>,
    orphan: Option<ReviewSnapshot>,
    writer_held: bool,
    writer_opens: usize,
    save_attempts: usize,
    publish_attempts: usize,
    delete_attempts: usize,
    fail_save: bool,
    fail_delete: bool,
    publish_behavior: PublishBehavior,
}

struct FakeRepositories {
    state: Arc<Mutex<RepositoryState>>,
}

impl ReviewRepositoryProviderPort for FakeRepositories {
    fn inspect(&self) -> Result<ReviewRepositoryInspection, ReviewRepositoryError> {
        let state = self.state.lock().unwrap();
        Ok(ReviewRepositoryInspection {
            catalog: state.catalog.clone(),
            active_draft: state.draft.clone(),
        })
    }

    fn open_reader(&self) -> Result<Box<dyn ReviewRepositoryPort>, ReviewRepositoryError> {
        Ok(Box::new(FakeRepository {
            state: Arc::clone(&self.state),
            writer: false,
        }))
    }

    fn open_writer(&self) -> Result<Box<dyn ReviewRepositoryPort>, ReviewRepositoryError> {
        let mut state = self.state.lock().unwrap();
        if state.writer_held {
            return Err(ReviewRepositoryError::Busy);
        }
        state.writer_held = true;
        state.writer_opens += 1;
        if let Some(orphan) = state.orphan.take() {
            append_completed(&mut state, orphan);
        }
        drop(state);
        Ok(Box::new(FakeRepository {
            state: Arc::clone(&self.state),
            writer: true,
        }))
    }
}

struct FakeRepository {
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
        Ok(self.state.lock().unwrap().catalog.clone())
    }

    fn load_active_draft(&self) -> Result<Option<ReviewDraft>, ReviewRepositoryError> {
        Ok(self.state.lock().unwrap().draft.clone())
    }

    fn load_draft(
        &self,
        stream_id: ReviewStreamId,
        round_id: ReviewRoundId,
    ) -> Result<Option<ReviewDraft>, ReviewRepositoryError> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .draft
            .as_ref()
            .filter(|draft| {
                draft.review_stream_id == stream_id && draft.review_round_id == round_id
            })
            .cloned())
    }

    fn save_draft(&self, draft: &ReviewDraft) -> Result<(), ReviewRepositoryError> {
        let mut state = self.state.lock().unwrap();
        state.save_attempts += 1;
        if state.fail_save {
            return Err(ReviewRepositoryError::Unavailable);
        }
        state.draft = Some(draft.clone());
        Ok(())
    }

    fn delete_draft(
        &self,
        stream_id: ReviewStreamId,
        round_id: ReviewRoundId,
    ) -> Result<(), ReviewRepositoryError> {
        let mut state = self.state.lock().unwrap();
        state.delete_attempts += 1;
        if state.fail_delete {
            return Err(ReviewRepositoryError::Unavailable);
        }
        let matches = state.draft.as_ref().is_some_and(|draft| {
            draft.review_stream_id == stream_id && draft.review_round_id == round_id
        });
        if !matches {
            return Err(ReviewRepositoryError::NotFound);
        }
        state.draft = None;
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

    fn publish(&self, snapshot: &ReviewSnapshot) -> Result<(), ReviewRepositoryError> {
        let mut state = self.state.lock().unwrap();
        state.publish_attempts += 1;
        match state.publish_behavior {
            PublishBehavior::Success => {
                append_completed(&mut state, snapshot.clone());
                Ok(())
            }
            PublishBehavior::FaultAfterRoundDurable | PublishBehavior::FaultBeforeIndexReplace => {
                state.orphan = Some(snapshot.clone());
                Err(ReviewRepositoryError::Unavailable)
            }
            PublishBehavior::ErrorWithoutDurability => Err(ReviewRepositoryError::Unavailable),
            PublishBehavior::SuccessWithWrongHead => {
                state
                    .completed
                    .insert(snapshot.review_round_id, snapshot.clone());
                Ok(())
            }
        }
    }
}

fn append_completed(state: &mut RepositoryState, completed: ReviewSnapshot) {
    state
        .completed
        .insert(completed.review_round_id, completed.clone());
    let stream = state
        .catalog
        .streams
        .iter_mut()
        .find(|stream| stream.review_stream_id == completed.review_stream_id);
    if let Some(stream) = stream {
        stream.completed_round_ids.push(completed.review_round_id);
        stream.latest_completed_round_id = Some(completed.review_round_id);
    } else {
        state.catalog.streams.push(ReviewStreamHead {
            review_stream_id: completed.review_stream_id,
            production: completed.production.clone(),
            completed_round_ids: vec![completed.review_round_id],
            latest_completed_round_id: Some(completed.review_round_id),
        });
    }
    state.draft = None;
}

struct Fixture {
    service: Arc<ReviewSessionService>,
    assets: Arc<FakeAssetCatalog>,
    repositories: Arc<FakeRepositories>,
    clock: Arc<TestClock>,
    entity_ids: [EntityId; 3],
}

impl Fixture {
    async fn active(failure: Option<ReviewabilityFailure>) -> Self {
        let project_id = ProjectId::from_u128(1);
        let entity_ids = [
            EntityId::from_u128(1),
            EntityId::from_u128(2),
            EntityId::from_u128(3),
        ];
        let prepared = entity_ids
            .iter()
            .enumerate()
            .map(|(index, entity_id)| {
                prepared_asset(
                    *entity_id,
                    index + 1,
                    (index <= 1).then_some(failure).flatten(),
                )
            })
            .collect::<Vec<_>>();
        let assets = Arc::new(FakeAssetCatalog {
            state: Mutex::new(AssetState {
                prepared: prepared.clone(),
                validation_sequences: vec![],
                validation_calls: 0,
                block_validation: false,
            }),
            validation_started: tokio::sync::Notify::new(),
            tracking_releases: AtomicUsize::new(0),
        });
        let repositories = Arc::new(FakeRepositories {
            state: Arc::new(Mutex::new(RepositoryState {
                catalog: ReviewCatalog {
                    project_id,
                    streams: vec![],
                },
                draft: None,
                completed: HashMap::new(),
                orphan: None,
                writer_held: false,
                writer_opens: 0,
                save_attempts: 0,
                publish_attempts: 0,
                delete_attempts: 0,
                fail_save: false,
                fail_delete: false,
                publish_behavior: PublishBehavior::Success,
            })),
        });
        let clock = Arc::new(TestClock(AtomicI64::new(1_000)));
        let service = Arc::new(ReviewSessionService::new(
            project_id,
            assets.clone(),
            repositories.clone(),
            clock.clone(),
        ));
        let proposal = service
            .preview_start(ReviewScope::Selection {
                entity_ids: entity_ids.to_vec(),
            })
            .await
            .unwrap();
        service
            .start(proposal.id, Arc::new(NoProgress))
            .await
            .unwrap();
        Self {
            service,
            assets,
            repositories,
            clock,
            entity_ids,
        }
    }

    async fn snapshot(&self) -> viewer_application::ReviewSessionSnapshot {
        self.service.snapshot().await
    }

    fn progress(&self) -> Arc<dyn ReviewProgressPort> {
        Arc::new(NoProgress)
    }

    fn repository_state(&self) -> std::sync::MutexGuard<'_, RepositoryState> {
        self.repositories.state.lock().unwrap()
    }
}

fn prepared_asset(
    entity_id: EntityId,
    ordinal: usize,
    failure: Option<ReviewabilityFailure>,
) -> PreparedReviewAsset {
    PreparedReviewAsset {
        entity_id,
        asset: AssetVersion {
            id: viewer_domain::AssetVersionId::from_u128(100 + ordinal as u128),
            source_entity_id: Some(entity_id),
            relative_path: RelativePath::parse(&format!("asset-{ordinal}.png")).unwrap(),
            evidence: AssetEvidence {
                size_bytes: 10,
                modified_ns: 20,
                blake3: Some([ordinal as u8; 32]),
            },
            media: ReviewMedia::Image {
                width: Some(100),
                height: Some(50),
            },
            producer_asset_id: None,
            parent_asset_version_id: None,
        },
        failure,
        change_revision: 0,
    }
}

fn guard(snapshot: &viewer_application::ReviewSessionSnapshot) -> ReviewMutationGuard {
    ReviewMutationGuard {
        review_round_id: snapshot.review_round_id.unwrap(),
        expected_revision: snapshot.revision,
    }
}

fn conflict(
    prepared: &PreparedReviewAsset,
    kind: ReviewAssetConflictKind,
) -> ReviewAssetValidation {
    ReviewAssetValidation::Conflict {
        asset_version_id: prepared.asset.id,
        relative_path: prepared.asset.relative_path.clone(),
        kind,
    }
}

async fn summary(fixture: &Fixture) -> viewer_application::ReviewCompletionProposal {
    let snapshot = fixture.snapshot().await;
    fixture
        .service
        .completion_summary(guard(&snapshot))
        .await
        .unwrap()
}

#[tokio::test]
async fn feedback_wins_and_remaining_assets_pass_only_after_verified_completion() {
    let fixture = Fixture::active(Some(ReviewabilityFailure::Damaged)).await;
    let before = fixture.snapshot().await;
    assert_eq!(before.counts.pass, 0);
    let with_feedback = fixture
        .service
        .add_feedback(AddReviewFeedback {
            guard: guard(&before),
            text: "修正人物手部".to_owned(),
            target_entity_ids: vec![fixture.entity_ids[0]],
        })
        .await
        .unwrap();
    let proposal = fixture
        .service
        .completion_summary(guard(&with_feedback))
        .await
        .unwrap();
    assert_eq!(proposal.summary.revise, 1);
    assert_eq!(proposal.summary.unreviewable, 1);
    assert_eq!(proposal.summary.default_pass, 1);
    assert!(proposal.summary.can_complete);
    fixture.clock.0.store(2_000, Ordering::Release);

    let completed = fixture
        .service
        .complete(proposal.id, proposal.summary.guard(), fixture.progress())
        .await
        .unwrap();

    assert_eq!(completed.phase, ReviewSessionPhase::CompletedReadOnly);
    assert_eq!(completed.counts.revise, 1);
    assert_eq!(completed.counts.unreviewable, 1);
    assert_eq!(completed.counts.pass, 1);
    let state = fixture.repository_state();
    let persisted = state.completed.values().next().unwrap();
    assert_eq!(persisted.outcomes[0].kind, ReviewOutcomeKind::Revise);
    assert_eq!(persisted.outcomes[1].kind, ReviewOutcomeKind::Unreviewable);
    assert_eq!(persisted.outcomes[2].kind, ReviewOutcomeKind::Pass);
    assert!(!state.writer_held);
    assert!(state.draft.is_none());
}

#[tokio::test]
async fn a_round_without_feedback_or_failure_becomes_all_default_pass() {
    let fixture = Fixture::active(None).await;
    let proposal = summary(&fixture).await;
    assert_eq!(proposal.summary.default_pass, 3);
    fixture.clock.0.store(2_000, Ordering::Release);

    let completed = fixture
        .service
        .complete(proposal.id, proposal.summary.guard(), fixture.progress())
        .await
        .unwrap();

    assert_eq!(completed.counts.pass, 3);
}

#[tokio::test]
async fn summary_persists_a_changed_stable_failure_before_issuing_its_revision() {
    let fixture = Fixture::active(None).await;
    let before = fixture.snapshot().await;
    let attempts = fixture.repository_state().save_attempts;
    let mut changed = fixture.assets.prepared();
    changed[1].failure = Some(ReviewabilityFailure::Damaged);
    fixture.assets.set_validation_sequences(vec![
        changed
            .into_iter()
            .map(ReviewAssetValidation::Current)
            .collect(),
    ]);

    let proposal = fixture
        .service
        .completion_summary(guard(&before))
        .await
        .unwrap();

    assert_eq!(proposal.summary.revision, before.revision + 1);
    assert_eq!(proposal.summary.unreviewable, 1);
    let state = fixture.repository_state();
    assert_eq!(state.save_attempts, attempts + 1);
    assert_eq!(state.draft.as_ref().unwrap().unreviewable.len(), 1);
}

#[tokio::test]
async fn summary_save_failure_keeps_the_previous_draft_revision_and_issues_no_proposal() {
    let fixture = Fixture::active(None).await;
    let before = fixture.snapshot().await;
    let mut changed = fixture.assets.prepared();
    changed[1].failure = Some(ReviewabilityFailure::Damaged);
    fixture.assets.set_validation_sequences(vec![
        changed
            .into_iter()
            .map(ReviewAssetValidation::Current)
            .collect(),
    ]);
    fixture.repository_state().fail_save = true;

    let error = fixture
        .service
        .completion_summary(guard(&before))
        .await
        .unwrap_err();

    assert_eq!(error.code(), "review_save_failed");
    assert_eq!(fixture.snapshot().await, before);
    assert!(
        fixture
            .repository_state()
            .draft
            .as_ref()
            .unwrap()
            .unreviewable
            .is_empty()
    );
}

#[tokio::test]
async fn cancellation_during_completion_validation_returns_to_the_same_active_draft() {
    let fixture = Fixture::active(None).await;
    let before = fixture.snapshot().await;
    fixture.assets.block_validation();
    let service = fixture.service.clone();
    let completion_guard = guard(&before);
    let completion =
        tokio::spawn(async move { service.completion_summary(completion_guard).await });
    fixture.assets.validation_started.notified().await;

    assert!(fixture.service.cancel_task().await);
    assert_eq!(
        completion.await.unwrap().unwrap_err().code(),
        "review_task_cancelled"
    );
    let after = fixture.snapshot().await;
    assert_eq!(after.phase, ReviewSessionPhase::Active);
    assert_eq!(after.revision, before.revision);
    assert!(fixture.repository_state().writer_held);
}

#[tokio::test]
async fn every_conflict_kind_and_pending_media_block_completion() {
    for kind in [
        ReviewAssetConflictKind::Missing,
        ReviewAssetConflictKind::Moved,
        ReviewAssetConflictKind::Replaced,
        ReviewAssetConflictKind::SizeChanged,
        ReviewAssetConflictKind::ContentChanged,
        ReviewAssetConflictKind::MediaChanged,
    ] {
        let fixture = Fixture::active(None).await;
        let prepared = fixture.assets.prepared();
        fixture.assets.set_validation_sequences(vec![vec![
            conflict(&prepared[0], kind),
            ReviewAssetValidation::Current(prepared[1].clone()),
            ReviewAssetValidation::Current(prepared[2].clone()),
        ]]);
        let proposal = summary(&fixture).await;
        assert!(!proposal.summary.can_complete);
        assert_eq!(proposal.summary.conflicts[0].kind, kind);
        assert_eq!(
            fixture
                .service
                .complete(proposal.id, proposal.summary.guard(), fixture.progress(),)
                .await
                .unwrap_err()
                .code(),
            "review_completion_blocked"
        );
        assert_eq!(fixture.repository_state().publish_attempts, 0);
    }

    let fixture = Fixture::active(None).await;
    let prepared = fixture.assets.prepared();
    fixture.assets.set_validation_sequences(vec![vec![
        ReviewAssetValidation::Pending {
            asset_version_id: prepared[0].asset.id,
            relative_path: prepared[0].asset.relative_path.clone(),
        },
        ReviewAssetValidation::Current(prepared[1].clone()),
        ReviewAssetValidation::Current(prepared[2].clone()),
    ]]);
    let proposal = summary(&fixture).await;
    assert!(!proposal.summary.can_complete);
    assert_eq!(proposal.summary.pending.len(), 1);
}

#[tokio::test]
async fn changed_metadata_with_equal_digest_can_complete_but_changed_content_cannot() {
    let fixture = Fixture::active(None).await;
    let mut equal = fixture.assets.prepared();
    equal[0].asset.evidence.modified_ns += 1;
    equal[0].change_revision += 1;
    let current = equal
        .iter()
        .cloned()
        .map(ReviewAssetValidation::Current)
        .collect::<Vec<_>>();
    fixture
        .assets
        .set_validation_sequences(vec![current.clone(), current]);
    let proposal = summary(&fixture).await;
    assert!(proposal.summary.can_complete);
    fixture.clock.0.store(2_000, Ordering::Release);
    assert!(
        fixture
            .service
            .complete(proposal.id, proposal.summary.guard(), fixture.progress(),)
            .await
            .is_ok()
    );

    let fixture = Fixture::active(None).await;
    let prepared = fixture.assets.prepared();
    fixture.assets.set_validation_sequences(vec![vec![
        conflict(&prepared[0], ReviewAssetConflictKind::ContentChanged),
        ReviewAssetValidation::Current(prepared[1].clone()),
        ReviewAssetValidation::Current(prepared[2].clone()),
    ]]);
    assert!(!summary(&fixture).await.summary.can_complete);
}

#[tokio::test]
async fn a_change_after_confirmation_invalidates_the_proposal_without_publishing() {
    let fixture = Fixture::active(None).await;
    let prepared = fixture.assets.prepared();
    let current = prepared
        .iter()
        .cloned()
        .map(ReviewAssetValidation::Current)
        .collect::<Vec<_>>();
    fixture.assets.set_validation_sequences(vec![
        current,
        vec![
            conflict(&prepared[0], ReviewAssetConflictKind::ContentChanged),
            ReviewAssetValidation::Current(prepared[1].clone()),
            ReviewAssetValidation::Current(prepared[2].clone()),
        ],
    ]);
    let proposal = summary(&fixture).await;

    let error = fixture
        .service
        .complete(proposal.id, proposal.summary.guard(), fixture.progress())
        .await
        .unwrap_err();

    assert_eq!(error.code(), "review_completion_changed");
    assert_eq!(fixture.repository_state().publish_attempts, 0);
    assert_eq!(fixture.snapshot().await.phase, ReviewSessionPhase::Active);
    assert_eq!(
        fixture
            .service
            .complete(proposal.id, proposal.summary.guard(), fixture.progress(),)
            .await
            .unwrap_err()
            .code(),
        "review_completion_proposal_stale"
    );
}

#[tokio::test]
async fn feedback_mutation_invalidates_an_existing_completion_proposal() {
    let fixture = Fixture::active(None).await;
    let proposal = summary(&fixture).await;
    fixture
        .service
        .add_feedback(AddReviewFeedback {
            guard: proposal.summary.guard(),
            text: "需要调整".to_owned(),
            target_entity_ids: vec![fixture.entity_ids[0]],
        })
        .await
        .unwrap();

    assert_eq!(
        fixture
            .service
            .complete(proposal.id, proposal.summary.guard(), fixture.progress(),)
            .await
            .unwrap_err()
            .code(),
        "review_completion_proposal_stale"
    );
}

#[tokio::test]
async fn both_durable_pre_index_faults_reopen_recover_and_verify_exact_success() {
    for behavior in [
        PublishBehavior::FaultAfterRoundDurable,
        PublishBehavior::FaultBeforeIndexReplace,
    ] {
        let fixture = Fixture::active(None).await;
        fixture.repository_state().publish_behavior = behavior;
        let proposal = summary(&fixture).await;
        fixture.clock.0.store(2_000, Ordering::Release);

        let completed = fixture
            .service
            .complete(proposal.id, proposal.summary.guard(), fixture.progress())
            .await
            .unwrap();

        assert_eq!(completed.phase, ReviewSessionPhase::CompletedReadOnly);
        let state = fixture.repository_state();
        assert_eq!(state.writer_opens, 2);
        assert!(!state.writer_held);
        assert!(state.draft.is_none());
    }
}

#[tokio::test]
async fn uncertain_publication_without_exact_head_enters_recovery_not_false_success() {
    for behavior in [
        PublishBehavior::ErrorWithoutDurability,
        PublishBehavior::SuccessWithWrongHead,
    ] {
        let fixture = Fixture::active(None).await;
        fixture.repository_state().publish_behavior = behavior;
        let proposal = summary(&fixture).await;
        fixture.clock.0.store(2_000, Ordering::Release);

        let error = fixture
            .service
            .complete(proposal.id, proposal.summary.guard(), fixture.progress())
            .await
            .unwrap_err();

        assert_eq!(error.code(), "review_recovery_required");
        let snapshot = fixture.snapshot().await;
        assert_eq!(snapshot.phase, ReviewSessionPhase::RecoveryRequired);
        assert!(!fixture.repository_state().writer_held);
    }
}

#[tokio::test]
async fn abandon_clears_only_after_exact_delete_and_never_publishes() {
    let fixture = Fixture::active(None).await;
    let active = fixture.snapshot().await;
    fixture.repository_state().fail_delete = true;
    assert_eq!(
        fixture
            .service
            .abandon(guard(&active))
            .await
            .unwrap_err()
            .code(),
        "review_abandon_failed"
    );
    assert_eq!(fixture.snapshot().await, active);
    assert!(fixture.repository_state().writer_held);

    fixture.repository_state().fail_delete = false;
    let abandoned = fixture.service.abandon(guard(&active)).await.unwrap();
    assert_eq!(abandoned.phase, ReviewSessionPhase::Idle);
    let state = fixture.repository_state();
    assert!(state.draft.is_none());
    assert!(!state.writer_held);
    assert_eq!(state.publish_attempts, 0);
    assert_eq!(state.delete_attempts, 2);
}

#[test]
fn completion_proposal_id_is_an_opaque_service_token() {
    fn accepts_id(_id: ReviewCompletionProposalId) {}
    let _ = accepts_id;
}
