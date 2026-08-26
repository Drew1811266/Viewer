use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use viewer_application::{
    AddReviewFeedback, ProjectAccess, ProjectProbeError, ProjectProbePort, ReviewMutationGuard,
    ReviewProposalId, ReviewRepositoryProviderPort, ReviewScope,
};
use viewer_desktop::dto::{
    ReviewAddFeedbackRequestDto, ReviewPreviewStartRequestDto, ReviewProgressDto,
    ReviewScopeRequestDto, ReviewSessionPhaseDto,
};
use viewer_desktop::state::{DesktopEventSink, DesktopRuntime};
use viewer_domain::search::Generation;
use viewer_domain::{EntityId, ProjectId, ReviewRoundId, SessionId};
use viewer_infrastructure::review::ProjectReviewRepositoryProvider;

struct FixedProbe(ProjectAccess);

impl ProjectProbePort for FixedProbe {
    fn probe(&self, _root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
        Ok(self.0)
    }
}

#[derive(Default)]
struct RecordingEvents(Mutex<Vec<ReviewProgressDto>>);

impl DesktopEventSink for RecordingEvents {
    fn emit_scan(&self, _event: viewer_desktop::dto::ScanEventDto) {}

    fn emit_review(&self, event: ReviewProgressDto) {
        self.0.lock().unwrap().push(event);
    }
}

fn create_project_with_image() -> tempfile::TempDir {
    let project = tempfile::tempdir().unwrap();
    fs::copy(
        viewer_test_support::image_fixtures::image_fixture("alpha.png"),
        project.path().join("alpha.png"),
    )
    .unwrap();
    project
}

#[test]
fn review_request_dtos_are_exact_path_free_and_bounded_before_domain_use() {
    let request: ReviewPreviewStartRequestDto = serde_json::from_value(serde_json::json!({
        "sessionId": "00000000-0000-0000-0000-000000000001",
        "generation": 3,
        "scope": {
            "kind": "selection",
            "entityIds": ["00000000-0000-0000-0000-000000000002"]
        }
    }))
    .unwrap();
    let (context, scope) = request.try_into_parts().unwrap();
    assert_eq!(context.session_id, SessionId::from_u128(1));
    assert_eq!(context.generation, Generation::new(3));
    assert_eq!(
        scope,
        ReviewScope::Selection {
            entity_ids: vec![EntityId::from_u128(2)]
        }
    );

    let snake_case = serde_json::from_value::<ReviewPreviewStartRequestDto>(serde_json::json!({
        "session_id": "00000000-0000-0000-0000-000000000001",
        "generation": 3,
        "scope": { "kind": "selection", "entityIds": [] }
    }));
    assert!(snake_case.is_err());
    let path_injection = serde_json::from_value::<ReviewScopeRequestDto>(serde_json::json!({
        "kind": "selection",
        "entityIds": [],
        "path": "/Users/private/secret.png"
    }));
    assert!(path_injection.is_err());
    let invalid_session = ReviewPreviewStartRequestDto {
        session_id: "not-a-uuid".to_owned(),
        generation: 3,
        scope: ReviewScopeRequestDto::Selection {
            entity_ids: vec![EntityId::from_u128(2).to_string()],
        },
    };
    assert_eq!(
        invalid_session.try_into_parts().unwrap_err().code,
        "review_invalid_data"
    );

    let duplicate = ReviewAddFeedbackRequestDto {
        session_id: SessionId::from_u128(1).to_string(),
        generation: 3,
        review_round_id: ReviewRoundId::from_u128(4).to_string(),
        expected_revision: 1,
        text: "自然语言意见".to_owned(),
        target_entity_ids: vec![
            EntityId::from_u128(2).to_string(),
            EntityId::from_u128(2).to_string(),
        ],
    };
    assert_eq!(
        duplicate.clone().try_into_parts().unwrap_err().code,
        "review_invalid_data"
    );
    let over_limit = ReviewAddFeedbackRequestDto {
        target_entity_ids: vec![EntityId::from_u128(2).to_string()],
        text: "x".repeat(viewer_domain::review::MAX_FEEDBACK_TEXT_BYTES + 1),
        ..duplicate
    };
    assert_eq!(
        over_limit.try_into_parts().unwrap_err().code,
        "review_invalid_data"
    );
    let too_many_targets = ReviewAddFeedbackRequestDto {
        session_id: SessionId::from_u128(1).to_string(),
        generation: 3,
        review_round_id: ReviewRoundId::from_u128(4).to_string(),
        expected_revision: 1,
        text: "自然语言意见".to_owned(),
        target_entity_ids: (1..=viewer_domain::review::MAX_TARGETS_PER_FEEDBACK + 1)
            .map(|value| EntityId::from_u128(value as u128).to_string())
            .collect(),
    };
    assert_eq!(
        too_many_targets.try_into_parts().unwrap_err().code,
        "review_invalid_data"
    );
}

#[tokio::test]
async fn desktop_review_session_uses_current_session_emits_path_free_progress_and_releases_writer()
{
    let cache = tempfile::tempdir().unwrap();
    let project = create_project_with_image();
    let events = Arc::new(RecordingEvents::default());
    let event_port: Arc<dyn DesktopEventSink> = events.clone();
    let runtime = DesktopRuntime::new_with_dependencies(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        Arc::new(viewer_infrastructure::scan::walker::ProjectWalker),
        event_port,
    );
    let opened = runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let session_id = SessionId::from_str(&opened.session_id).unwrap();
    let generation = Generation::new(opened.generation);
    let workspace = runtime.query_folder(None).await.unwrap();
    let viewer_desktop::dto::FolderWorkspaceDto::Content { images, .. } = workspace else {
        panic!("root should contain the image")
    };
    let entity_id = EntityId::from_str(&images[0].entity_id).unwrap();

    assert_eq!(
        runtime
            .review_status(SessionId::new(), generation)
            .await
            .unwrap_err()
            .code,
        "review_stale_session"
    );
    assert_eq!(
        runtime
            .review_status(session_id, Generation::new(opened.generation + 1))
            .await
            .unwrap_err()
            .code,
        "review_stale_session"
    );
    let idle = runtime.review_status(session_id, generation).await.unwrap();
    assert_eq!(idle.phase, ReviewSessionPhaseDto::Idle);
    let proposal = runtime
        .review_preview_start(
            session_id,
            generation,
            ReviewScope::Selection {
                entity_ids: vec![entity_id],
            },
        )
        .await
        .unwrap();
    assert_eq!(proposal.resolution.image_count, 1);
    let active = runtime
        .review_start(
            session_id,
            generation,
            ReviewProposalId::from_raw(proposal.proposal_id).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(active.phase, ReviewSessionPhaseDto::Active);
    assert_eq!(active.counts.pass, 0);
    let progress = events.0.lock().unwrap().clone();
    assert!(!progress.is_empty());
    assert!(progress.iter().all(|event| {
        event.session_id == opened.session_id
            && event.generation == opened.generation
            && !serde_json::to_string(event)
                .unwrap()
                .contains(project.path().to_str().unwrap())
    }));

    let round_id = ReviewRoundId::from_str(active.review_round_id.as_ref().unwrap()).unwrap();
    let error = runtime
        .review_add_feedback(
            session_id,
            generation,
            AddReviewFeedback {
                guard: ReviewMutationGuard {
                    review_round_id: round_id,
                    expected_revision: active.revision + 1,
                },
                text: "需要调整".to_owned(),
                target_entity_ids: vec![entity_id],
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "review_stale_revision");

    runtime.close_project().await.unwrap();
    let provider = ProjectReviewRepositoryProvider::new(
        project.path(),
        ProjectId::from_str(&opened.project_id).unwrap(),
    );
    assert!(provider.open_writer().is_ok());
}

#[tokio::test]
async fn a_busy_review_writer_never_blocks_normal_project_browsing() {
    let cache = tempfile::tempdir().unwrap();
    let project = create_project_with_image();
    let runtime = DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
    );
    let opened = runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let session_id = SessionId::from_str(&opened.session_id).unwrap();
    let generation = Generation::new(opened.generation);
    let viewer_desktop::dto::FolderWorkspaceDto::Content { images, .. } =
        runtime.query_folder(None).await.unwrap()
    else {
        panic!("root should remain browsable")
    };
    let entity_id = EntityId::from_str(&images[0].entity_id).unwrap();
    let provider = ProjectReviewRepositoryProvider::new(
        project.path(),
        ProjectId::from_str(&opened.project_id).unwrap(),
    );
    let external_writer = provider.open_writer().unwrap();
    let proposal = runtime
        .review_preview_start(
            session_id,
            generation,
            ReviewScope::Selection {
                entity_ids: vec![entity_id],
            },
        )
        .await
        .unwrap();

    assert_eq!(
        runtime
            .review_start(
                session_id,
                generation,
                ReviewProposalId::from_raw(proposal.proposal_id).unwrap(),
            )
            .await
            .unwrap_err()
            .code,
        "review_busy"
    );
    assert!(matches!(
        runtime.query_folder(None).await.unwrap(),
        viewer_desktop::dto::FolderWorkspaceDto::Content { .. }
    ));
    drop(external_writer);
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn read_only_projects_keep_browsing_available_and_never_create_review_storage() {
    let cache = tempfile::tempdir().unwrap();
    let project = create_project_with_image();
    let runtime = DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadOnly)),
    );
    let opened = runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let session_id = SessionId::from_str(&opened.session_id).unwrap();
    let generation = Generation::new(opened.generation);
    let viewer_desktop::dto::FolderWorkspaceDto::Content { images, .. } =
        runtime.query_folder(None).await.unwrap()
    else {
        panic!("read-only root should remain browsable")
    };
    let entity_id = EntityId::from_str(&images[0].entity_id).unwrap();
    let proposal = runtime
        .review_preview_start(
            session_id,
            generation,
            ReviewScope::Selection {
                entity_ids: vec![entity_id],
            },
        )
        .await
        .unwrap();

    assert_eq!(
        runtime
            .review_start(
                session_id,
                generation,
                ReviewProposalId::from_raw(proposal.proposal_id).unwrap(),
            )
            .await
            .unwrap_err()
            .code,
        "review_read_only"
    );
    assert!(matches!(
        runtime.query_folder(None).await.unwrap(),
        viewer_desktop::dto::FolderWorkspaceDto::Content { .. }
    ));
    assert!(!project.path().join(".viewer/reviews").exists());
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn malformed_review_records_surface_recovery_without_blocking_project_open_or_browse() {
    let cache = tempfile::tempdir().unwrap();
    let project = create_project_with_image();
    let drafts = project.path().join(".viewer/reviews/drafts");
    fs::create_dir_all(&drafts).unwrap();
    fs::write(
        drafts.join("00000000-0000-0000-0000-000000000001.json"),
        b"{}",
    )
    .unwrap();
    let runtime = DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
    );

    let opened = runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let status = runtime
        .review_status(
            SessionId::from_str(&opened.session_id).unwrap(),
            Generation::new(opened.generation),
        )
        .await
        .unwrap();

    assert_eq!(status.phase, ReviewSessionPhaseDto::RecoveryRequired);
    assert!(matches!(
        runtime.query_folder(None).await.unwrap(),
        viewer_desktop::dto::FolderWorkspaceDto::Content { .. }
    ));
    runtime.close_project().await.unwrap();
}
