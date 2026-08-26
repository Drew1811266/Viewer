use async_trait::async_trait;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use viewer_application::{
    AddReviewFeedback, ClockPort, DeleteReviewFeedback, PersistedReviewDraft, PreparedReviewAsset,
    ReviewAssetCatalogPort, ReviewAssetError, ReviewAssetValidation, ReviewCatalog,
    ReviewMutationGuard, ReviewProgressPort, ReviewProtocolVersion, ReviewPublication,
    ReviewRepositoryError, ReviewRepositoryInspection, ReviewRepositoryPort,
    ReviewRepositoryProviderPort, ReviewScope, ReviewScopeResolution, ReviewSessionService,
    ReviewTaskCancellation, ReviewTaskProgress, UpdateReviewFeedback,
};
use viewer_domain::review::{
    AssetEvidence, AssetVersion, MAX_FEEDBACK_TEXT_BYTES, ReviewDraft, ReviewMedia, ReviewSnapshot,
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

struct FakeAssetCatalog {
    prepared: Vec<PreparedReviewAsset>,
}

#[async_trait]
impl ReviewAssetCatalogPort for FakeAssetCatalog {
    async fn resolve_scope(
        &self,
        _scope: &ReviewScope,
    ) -> Result<ReviewScopeResolution, ReviewAssetError> {
        Ok(ReviewScopeResolution {
            candidate_entity_ids: self
                .prepared
                .iter()
                .map(|prepared| prepared.entity_id)
                .collect(),
            image_count: 2,
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
            Ok(self.prepared.clone())
        }
    }

    async fn revalidate_assets(
        &self,
        assets: &[PreparedReviewAsset],
        cancellation: ReviewTaskCancellation,
        _progress: Arc<dyn ReviewProgressPort>,
    ) -> Result<Vec<ReviewAssetValidation>, ReviewAssetError> {
        if cancellation.is_cancelled() {
            Err(ReviewAssetError::Cancelled)
        } else {
            Ok(assets
                .iter()
                .cloned()
                .map(ReviewAssetValidation::Current)
                .collect())
        }
    }

    fn release_tracking(&self) {}
}

#[derive(Default)]
struct RepositoryState {
    draft: Option<ReviewDraft>,
    writer_held: bool,
    fail_save: bool,
    save_attempts: usize,
}

struct FakeRepositories {
    project_id: ProjectId,
    state: Arc<Mutex<RepositoryState>>,
}

impl ReviewRepositoryProviderPort for FakeRepositories {
    fn inspect(&self) -> Result<ReviewRepositoryInspection, ReviewRepositoryError> {
        Ok(ReviewRepositoryInspection {
            catalog: ReviewCatalog {
                project_id: self.project_id,
                streams: vec![],
            },
            active_draft: self.state.lock().unwrap().draft.clone().map(|draft| {
                PersistedReviewDraft {
                    protocol_version: ReviewProtocolVersion::V1,
                    draft,
                }
            }),
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
        if state.writer_held {
            return Err(ReviewRepositoryError::Busy);
        }
        state.writer_held = true;
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
        Ok(ReviewCatalog {
            project_id: self.project_id,
            streams: vec![],
        })
    }

    fn load_active_draft(&self) -> Result<Option<PersistedReviewDraft>, ReviewRepositoryError> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .draft
            .clone()
            .map(|draft| PersistedReviewDraft {
                protocol_version: ReviewProtocolVersion::V1,
                draft,
            }))
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
            .draft
            .as_ref()
            .filter(|draft| {
                draft.review_stream_id == stream_id && draft.review_round_id == round_id
            })
            .cloned()
            .map(|draft| PersistedReviewDraft {
                protocol_version: ReviewProtocolVersion::V1,
                draft,
            }))
    }

    fn save_draft(&self, draft: &PersistedReviewDraft) -> Result<(), ReviewRepositoryError> {
        let mut state = self.state.lock().unwrap();
        state.save_attempts += 1;
        if state.fail_save {
            return Err(ReviewRepositoryError::Unavailable);
        }
        state.draft = Some(draft.draft.clone());
        Ok(())
    }

    fn delete_draft(
        &self,
        _stream_id: ReviewStreamId,
        _round_id: ReviewRoundId,
    ) -> Result<(), ReviewRepositoryError> {
        self.state.lock().unwrap().draft = None;
        Ok(())
    }

    fn load_completed(
        &self,
        _stream_id: ReviewStreamId,
        _round_id: ReviewRoundId,
    ) -> Result<Option<ReviewSnapshot>, ReviewRepositoryError> {
        Ok(None)
    }

    fn publish(&self, _publication: &ReviewPublication) -> Result<(), ReviewRepositoryError> {
        Ok(())
    }
}

struct Fixture {
    service: Arc<ReviewSessionService>,
    repositories: Arc<FakeRepositories>,
    clock: Arc<TestClock>,
    entity_ids: [EntityId; 2],
}

impl Fixture {
    async fn active() -> Self {
        let project_id = ProjectId::from_u128(1);
        let entity_ids = [EntityId::from_u128(1), EntityId::from_u128(2)];
        let prepared = entity_ids
            .iter()
            .enumerate()
            .map(|(index, entity_id)| prepared_asset(*entity_id, index + 1))
            .collect();
        let catalog = Arc::new(FakeAssetCatalog { prepared });
        let repositories = Arc::new(FakeRepositories {
            project_id,
            state: Arc::new(Mutex::new(RepositoryState::default())),
        });
        let clock = Arc::new(TestClock(AtomicI64::new(1_000)));
        let service = Arc::new(ReviewSessionService::new(
            project_id,
            catalog,
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
            repositories,
            clock,
            entity_ids,
        }
    }

    async fn snapshot(&self) -> viewer_application::ReviewSessionSnapshot {
        self.service.inspect().await
    }

    fn saved_draft(&self) -> ReviewDraft {
        self.repositories
            .state
            .lock()
            .unwrap()
            .draft
            .clone()
            .unwrap()
    }

    fn save_attempts(&self) -> usize {
        self.repositories.state.lock().unwrap().save_attempts
    }
}

fn prepared_asset(entity_id: EntityId, ordinal: usize) -> PreparedReviewAsset {
    let path = RelativePath::parse(&format!("asset-{ordinal}.png")).unwrap();
    PreparedReviewAsset {
        entity_id,
        asset: AssetVersion {
            id: viewer_domain::AssetVersionId::from_u128(100 + ordinal as u128),
            source_entity_id: Some(entity_id),
            relative_path: path,
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
        failure: None,
        change_revision: 0,
        source_path: std::path::PathBuf::from(format!("/protected/asset-{ordinal}.png")),
    }
}

fn guard(snapshot: &viewer_application::ReviewSessionSnapshot) -> ReviewMutationGuard {
    ReviewMutationGuard {
        review_round_id: snapshot.review_round_id.unwrap(),
        expected_revision: snapshot.revision,
    }
}

fn add(
    guard: ReviewMutationGuard,
    text: &str,
    target_entity_ids: Vec<EntityId>,
) -> AddReviewFeedback {
    AddReviewFeedback {
        guard,
        text: text.to_owned(),
        target_entity_ids,
    }
}

#[tokio::test]
async fn add_feedback_preserves_natural_language_and_freezes_one_or_many_targets() {
    let fixture = Fixture::active().await;
    let before = fixture.snapshot().await;
    assert_eq!(before.counts.pass, 0);
    let text = "人物手部需要修正，整体光线保持不变。";

    let after = fixture
        .service
        .add_feedback(add(guard(&before), text, fixture.entity_ids.to_vec()))
        .await
        .unwrap();

    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(after.feedback.len(), 1);
    assert_eq!(after.feedback[0].text, text);
    assert_eq!(after.feedback[0].target_entity_ids, fixture.entity_ids);
    assert_eq!(after.counts.revise, 2);
    assert_eq!(after.counts.pass, 0);
    let saved = fixture.saved_draft();
    assert_eq!(saved.feedback[0].targets.len(), 2);
    assert_eq!(
        saved.feedback[0]
            .targets
            .iter()
            .map(|target| target.asset_version_id)
            .collect::<Vec<_>>(),
        saved
            .assets
            .iter()
            .map(|asset| asset.id)
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn multiple_feedback_items_can_target_the_same_fixed_asset() {
    let fixture = Fixture::active().await;
    let first = fixture.snapshot().await;
    let second = fixture
        .service
        .add_feedback(add(guard(&first), "修正手部", vec![fixture.entity_ids[0]]))
        .await
        .unwrap();
    let third = fixture
        .service
        .add_feedback(add(guard(&second), "降低高光", vec![fixture.entity_ids[0]]))
        .await
        .unwrap();

    assert_eq!(third.feedback.len(), 2);
    assert_eq!(third.members[0].feedback_items, 2);
    assert_eq!(third.counts.revise, 1);
}

#[tokio::test]
async fn update_keeps_feedback_identity_and_timestamp_but_replaces_text_and_targets() {
    let fixture = Fixture::active().await;
    let initial = fixture.snapshot().await;
    let added = fixture
        .service
        .add_feedback(add(guard(&initial), "旧意见", vec![fixture.entity_ids[0]]))
        .await
        .unwrap();
    let feedback_id = added.feedback[0].feedback_id;
    let created_at_ms = added.feedback[0].created_at_ms;
    fixture.clock.0.store(2_000, Ordering::Release);

    let updated = fixture
        .service
        .update_feedback(UpdateReviewFeedback {
            guard: guard(&added),
            feedback_id,
            text: "新意见".to_owned(),
            target_entity_ids: vec![fixture.entity_ids[1]],
        })
        .await
        .unwrap();

    assert_eq!(updated.feedback[0].feedback_id, feedback_id);
    assert_eq!(updated.feedback[0].created_at_ms, created_at_ms);
    assert_eq!(updated.feedback[0].text, "新意见");
    assert_eq!(
        updated.feedback[0].target_entity_ids,
        vec![fixture.entity_ids[1]]
    );
}

#[tokio::test]
async fn delete_requires_an_existing_feedback_and_increments_revision_once() {
    let fixture = Fixture::active().await;
    let initial = fixture.snapshot().await;
    let added = fixture
        .service
        .add_feedback(add(guard(&initial), "删除我", vec![fixture.entity_ids[0]]))
        .await
        .unwrap();
    let feedback_id = added.feedback[0].feedback_id;
    let deleted = fixture
        .service
        .delete_feedback(DeleteReviewFeedback {
            guard: guard(&added),
            feedback_id,
        })
        .await
        .unwrap();

    assert!(deleted.feedback.is_empty());
    assert_eq!(deleted.revision, added.revision + 1);
    let attempts = fixture.save_attempts();
    let error = fixture
        .service
        .delete_feedback(DeleteReviewFeedback {
            guard: guard(&deleted),
            feedback_id,
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), "review_feedback_not_found");
    assert_eq!(fixture.save_attempts(), attempts);
}

#[tokio::test]
async fn invalid_feedback_is_rejected_before_repository_io() {
    let fixture = Fixture::active().await;
    let snapshot = fixture.snapshot().await;
    let attempts = fixture.save_attempts();
    let invalid_commands = [
        add(guard(&snapshot), "  ", vec![fixture.entity_ids[0]]),
        add(guard(&snapshot), "修正", vec![]),
        add(
            guard(&snapshot),
            "修正",
            vec![fixture.entity_ids[0], fixture.entity_ids[0]],
        ),
        add(guard(&snapshot), "修正", vec![EntityId::from_u128(999)]),
        add(
            guard(&snapshot),
            &"x".repeat(MAX_FEEDBACK_TEXT_BYTES + 1),
            vec![fixture.entity_ids[0]],
        ),
    ];

    for command in invalid_commands {
        assert_eq!(
            fixture
                .service
                .add_feedback(command)
                .await
                .unwrap_err()
                .code(),
            "review_feedback_invalid"
        );
    }
    assert_eq!(fixture.save_attempts(), attempts);
    assert_eq!(fixture.snapshot().await, snapshot);
}

#[tokio::test]
async fn save_failure_preserves_the_in_memory_draft_and_revision() {
    let fixture = Fixture::active().await;
    let before = fixture.snapshot().await;
    fixture.repositories.state.lock().unwrap().fail_save = true;

    let error = fixture
        .service
        .add_feedback(add(guard(&before), "不能丢失", vec![fixture.entity_ids[0]]))
        .await
        .unwrap_err();

    assert_eq!(error.code(), "review_save_failed");
    assert_eq!(fixture.snapshot().await, before);
    assert!(fixture.saved_draft().feedback.is_empty());
}

#[tokio::test]
async fn concurrent_commands_with_one_revision_persist_exactly_one_change() {
    let fixture = Fixture::active().await;
    let before = fixture.snapshot().await;
    let attempts = fixture.save_attempts();
    let first =
        fixture
            .service
            .add_feedback(add(guard(&before), "第一条", vec![fixture.entity_ids[0]]));
    let second =
        fixture
            .service
            .add_feedback(add(guard(&before), "第二条", vec![fixture.entity_ids[1]]));

    let (first, second) = tokio::join!(first, second);

    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    let error = first.err().or_else(|| second.err()).unwrap();
    assert_eq!(error.code(), "review_revision_stale");
    assert_eq!(fixture.save_attempts(), attempts + 1);
    assert_eq!(fixture.snapshot().await.feedback.len(), 1);
}

#[tokio::test]
async fn stale_round_and_revision_are_rejected_without_saving() {
    let fixture = Fixture::active().await;
    let snapshot = fixture.snapshot().await;
    let attempts = fixture.save_attempts();
    let stale_round = ReviewMutationGuard {
        review_round_id: ReviewRoundId::from_u128(999),
        expected_revision: snapshot.revision,
    };
    assert_eq!(
        fixture
            .service
            .add_feedback(add(stale_round, "修正", vec![fixture.entity_ids[0]],))
            .await
            .unwrap_err()
            .code(),
        "review_round_stale"
    );
    let stale_revision = ReviewMutationGuard {
        review_round_id: snapshot.review_round_id.unwrap(),
        expected_revision: snapshot.revision + 1,
    };
    assert_eq!(
        fixture
            .service
            .add_feedback(add(stale_revision, "修正", vec![fixture.entity_ids[0]],))
            .await
            .unwrap_err()
            .code(),
        "review_revision_stale"
    );
    assert_eq!(fixture.save_attempts(), attempts);
}
