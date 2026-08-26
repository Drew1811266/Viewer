use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use viewer_application::{
    AddReviewFeedback, DeleteReviewFeedback, ProjectAccess, ProjectProbeError, ProjectProbePort,
    ReplaceReviewFeedbackAnchor, ReviewFeedbackTargetInput, ReviewMutationGuard, ReviewProposalId,
    ReviewRepositoryProviderPort, ReviewScope, StartReviewWithFeedback, UpdateReviewFeedbackText,
};
use viewer_desktop::dto::{
    ReviewAddFeedbackRequestDto, ReviewAnchorDto, ReviewFeedbackTargetRequestDto,
    ReviewPreviewStartRequestDto, ReviewProgressDto, ReviewReplaceFeedbackAnchorRequestDto,
    ReviewScopeRequestDto, ReviewSessionPhaseDto, ReviewStartWithFeedbackRequestDto,
    ReviewUpdateFeedbackTextRequestDto,
};
use viewer_desktop::state::{DesktopEventSink, DesktopRuntime};
use viewer_domain::review::{FeedbackAnchor, NormalizedRect};
use viewer_domain::search::Generation;
use viewer_domain::{EntityId, FeedbackId, ProjectId, ReviewRoundId, SessionId};
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
        targets: vec![
            ReviewFeedbackTargetRequestDto {
                entity_id: EntityId::from_u128(2).to_string(),
                anchor: ReviewAnchorDto::Asset,
            },
            ReviewFeedbackTargetRequestDto {
                entity_id: EntityId::from_u128(2).to_string(),
                anchor: ReviewAnchorDto::Asset,
            },
        ],
    };
    assert_eq!(
        duplicate.clone().try_into_parts().unwrap_err().code,
        "review_invalid_data"
    );
    let over_limit = ReviewAddFeedbackRequestDto {
        targets: vec![ReviewFeedbackTargetRequestDto {
            entity_id: EntityId::from_u128(2).to_string(),
            anchor: ReviewAnchorDto::Asset,
        }],
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
        targets: (1..=viewer_domain::review::MAX_TARGETS_PER_FEEDBACK + 1)
            .map(|value| ReviewFeedbackTargetRequestDto {
                entity_id: EntityId::from_u128(value as u128).to_string(),
                anchor: ReviewAnchorDto::Asset,
            })
            .collect(),
    };
    assert_eq!(
        too_many_targets.try_into_parts().unwrap_err().code,
        "review_invalid_data"
    );
}

#[test]
fn anchored_review_dtos_are_strict_bounded_and_path_free() {
    let session_id = SessionId::from_u128(1).to_string();
    let round_id = ReviewRoundId::from_u128(4).to_string();
    let entity_id = EntityId::from_u128(2).to_string();
    let stroke = serde_json::json!({
        "sessionId": session_id,
        "generation": 7,
        "reviewRoundId": round_id,
        "expectedRevision": 3,
        "text": "重画袖口边缘",
        "targets": [{
            "entityId": entity_id,
            "anchor": {
                "kind": "image_stroke",
                "points": [{"x": 0.2, "y": 0.3}, {"x": 0.7, "y": 0.6}]
            }
        }]
    });
    let dto: ReviewAddFeedbackRequestDto = serde_json::from_value(stroke.clone()).unwrap();
    assert!(matches!(
        dto.try_into_parts().unwrap().1.targets[0].anchor,
        FeedbackAnchor::ImageStroke(_)
    ));

    let start: ReviewStartWithFeedbackRequestDto = serde_json::from_value(serde_json::json!({
        "sessionId": SessionId::from_u128(1).to_string(),
        "generation": 7,
        "proposalId": 11,
        "text": "修正衣领边缘",
        "targets": [{
            "entityId": EntityId::from_u128(2).to_string(),
            "anchor": {"kind": "image_rect", "x": 0.2, "y": 0.1, "width": 0.3, "height": 0.2}
        }]
    }))
    .unwrap();
    assert!(matches!(
        start.try_into_parts().unwrap().1.targets[0].anchor,
        FeedbackAnchor::ImageRect(_)
    ));

    let text_only: ReviewUpdateFeedbackTextRequestDto = serde_json::from_value(serde_json::json!({
        "sessionId": SessionId::from_u128(1).to_string(),
        "generation": 7,
        "reviewRoundId": ReviewRoundId::from_u128(4).to_string(),
        "expectedRevision": 3,
        "feedbackId": viewer_domain::FeedbackId::from_u128(5).to_string(),
        "text": "只改文字"
    }))
    .unwrap();
    assert_eq!(text_only.try_into_parts().unwrap().1.text, "只改文字");

    let replace: ReviewReplaceFeedbackAnchorRequestDto =
        serde_json::from_value(serde_json::json!({
            "sessionId": SessionId::from_u128(1).to_string(),
            "generation": 7,
            "reviewRoundId": ReviewRoundId::from_u128(4).to_string(),
            "expectedRevision": 3,
            "feedbackId": viewer_domain::FeedbackId::from_u128(5).to_string(),
            "target": {
                "entityId": EntityId::from_u128(2).to_string(),
                "anchor": {"kind": "image_rect", "x": 0.4, "y": 0.3, "width": 0.2, "height": 0.2}
            }
        }))
        .unwrap();
    assert!(matches!(
        replace.try_into_parts().unwrap().1.target.anchor,
        FeedbackAnchor::ImageRect(_)
    ));
    let replace_asset: ReviewReplaceFeedbackAnchorRequestDto =
        serde_json::from_value(serde_json::json!({
            "sessionId": SessionId::from_u128(1).to_string(),
            "generation": 7,
            "reviewRoundId": ReviewRoundId::from_u128(4).to_string(),
            "expectedRevision": 3,
            "feedbackId": viewer_domain::FeedbackId::from_u128(5).to_string(),
            "target": {
                "entityId": EntityId::from_u128(2).to_string(),
                "anchor": {"kind": "asset"}
            }
        }))
        .unwrap();
    assert_eq!(
        replace_asset.try_into_parts().unwrap_err().code,
        "review_invalid_data"
    );

    let mut invalid = Vec::new();
    invalid.push(serde_json::json!({
        "sessionId": SessionId::from_u128(1).to_string(), "generation": 7,
        "reviewRoundId": ReviewRoundId::from_u128(4).to_string(), "expectedRevision": 3,
        "text": " ", "targets": [{"entityId": EntityId::from_u128(2).to_string(), "anchor": {"kind": "asset"}}]
    }));
    invalid.push(serde_json::json!({
        "sessionId": SessionId::from_u128(1).to_string(), "generation": 7,
        "reviewRoundId": ReviewRoundId::from_u128(4).to_string(), "expectedRevision": 3,
        "text": "修正", "targets": []
    }));
    invalid.push(serde_json::json!({
        "sessionId": SessionId::from_u128(1).to_string(), "generation": 7,
        "reviewRoundId": ReviewRoundId::from_u128(4).to_string(), "expectedRevision": 3,
        "text": "修正", "targets": [
            {"entityId": EntityId::from_u128(2).to_string(), "anchor": {"kind": "asset"}},
            {"entityId": EntityId::from_u128(2).to_string(), "anchor": {"kind": "image_rect", "x": 0.1, "y": 0.1, "width": 0.2, "height": 0.2}}
        ]
    }));
    invalid.push(serde_json::json!({
        "sessionId": SessionId::from_u128(1).to_string(), "generation": 7,
        "reviewRoundId": ReviewRoundId::from_u128(4).to_string(), "expectedRevision": 3,
        "text": "修正", "targets": [{"entityId": EntityId::from_u128(2).to_string(), "anchor": {"kind": "image_rect", "x": 0.9, "y": 0.1, "width": 0.2, "height": 0.2}}]
    }));
    invalid.push(serde_json::json!({
        "sessionId": SessionId::from_u128(1).to_string(), "generation": 7,
        "reviewRoundId": ReviewRoundId::from_u128(4).to_string(), "expectedRevision": 3,
        "text": "修正", "targets": [{"entityId": EntityId::from_u128(2).to_string(), "anchor": {"kind": "image_stroke", "points": (0..2049).map(|index| serde_json::json!({"x": index as f64 / 2048.0, "y": if index % 2 == 0 {0.2} else {0.3}})).collect::<Vec<_>>()}}]
    }));
    for value in invalid {
        let dto: ReviewAddFeedbackRequestDto = serde_json::from_value(value).unwrap();
        assert_eq!(
            dto.try_into_parts().unwrap_err().code,
            "review_invalid_data"
        );
    }

    for forbidden in [
        "path",
        "sourcePath",
        "artifactPath",
        "digest",
        "assetVersion",
    ] {
        let mut injected = stroke.clone();
        injected.as_object_mut().unwrap().insert(
            forbidden.to_owned(),
            serde_json::Value::String("private".to_owned()),
        );
        assert!(serde_json::from_value::<ReviewAddFeedbackRequestDto>(injected).is_err());
    }
    let unknown_kind = serde_json::json!({
        "sessionId": SessionId::from_u128(1).to_string(), "generation": 7,
        "reviewRoundId": ReviewRoundId::from_u128(4).to_string(), "expectedRevision": 3,
        "text": "修正", "targets": [{"entityId": EntityId::from_u128(2).to_string(), "anchor": {"kind": "polygon"}}]
    });
    assert!(serde_json::from_value::<ReviewAddFeedbackRequestDto>(unknown_kind).is_err());
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
    let serialized = serde_json::to_string(&active).unwrap();
    assert!(!serialized.contains(project.path().to_str().unwrap()));
    assert!(!serialized.contains("sourcePath"));
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
                targets: vec![ReviewFeedbackTargetInput {
                    entity_id,
                    anchor: FeedbackAnchor::Asset,
                }],
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
async fn anchored_commands_enforce_generation_and_round_trip_target_rich_snapshots() {
    let cache = tempfile::tempdir().unwrap();
    let project = create_project_with_image();
    let runtime = DesktopRuntime::new_with_dependencies(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        Arc::new(viewer_infrastructure::scan::walker::ProjectWalker),
        Arc::new(RecordingEvents::default()),
    );
    let opened = runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let session_id = SessionId::from_str(&opened.session_id).unwrap();
    let generation = Generation::new(opened.generation);
    let viewer_desktop::dto::FolderWorkspaceDto::Content { images, .. } =
        runtime.query_folder(None).await.unwrap()
    else {
        panic!("root should contain the image")
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
    let start = StartReviewWithFeedback {
        proposal_id: ReviewProposalId::from_raw(proposal.proposal_id).unwrap(),
        text: "修正衣领边缘".to_owned(),
        targets: vec![ReviewFeedbackTargetInput {
            entity_id,
            anchor: FeedbackAnchor::ImageRect(NormalizedRect::new(0.2, 0.1, 0.3, 0.2).unwrap()),
        }],
    };
    assert_eq!(
        runtime
            .review_start_with_feedback(
                session_id,
                Generation::new(generation.get() + 1),
                start.clone(),
            )
            .await
            .unwrap_err()
            .code,
        "review_stale_session"
    );
    let active = runtime
        .review_start_with_feedback(session_id, generation, start)
        .await
        .unwrap();
    let serialized = serde_json::to_string(&active).unwrap();
    assert!(!serialized.contains(project.path().to_str().unwrap()));
    assert!(!serialized.contains("sourcePath"));
    assert!(serialized.contains("\"kind\":\"image_rect\""));
    assert!(matches!(
        active.feedback[0].targets[0].anchor,
        ReviewAnchorDto::ImageRect { .. }
    ));
    let feedback_id = FeedbackId::from_str(&active.feedback[0].feedback_id).unwrap();
    let round_id = ReviewRoundId::from_str(active.review_round_id.as_ref().unwrap()).unwrap();
    let updated = runtime
        .review_update_feedback_text(
            session_id,
            generation,
            UpdateReviewFeedbackText {
                guard: ReviewMutationGuard {
                    review_round_id: round_id,
                    expected_revision: active.revision,
                },
                feedback_id,
                text: "只改意见文字".to_owned(),
            },
        )
        .await
        .unwrap();
    let replaced = runtime
        .review_replace_feedback_anchor(
            session_id,
            generation,
            ReplaceReviewFeedbackAnchor {
                guard: ReviewMutationGuard {
                    review_round_id: round_id,
                    expected_revision: updated.revision,
                },
                feedback_id,
                target: ReviewFeedbackTargetInput {
                    entity_id,
                    anchor: FeedbackAnchor::ImageRect(
                        NormalizedRect::new(0.5, 0.4, 0.2, 0.2).unwrap(),
                    ),
                },
            },
        )
        .await
        .unwrap();
    let deleted = runtime
        .review_delete_feedback(
            session_id,
            generation,
            DeleteReviewFeedback {
                guard: ReviewMutationGuard {
                    review_round_id: round_id,
                    expected_revision: replaced.revision,
                },
                feedback_id,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        deleted.restorable_feedback_id,
        Some(feedback_id.to_string())
    );
    let restored = runtime
        .review_restore_deleted_feedback(
            session_id,
            generation,
            ReviewMutationGuard {
                review_round_id: round_id,
                expected_revision: deleted.revision,
            },
            feedback_id,
        )
        .await
        .unwrap();
    assert_eq!(restored.feedback[0].text, "只改意见文字");
    assert!(matches!(
        restored.feedback[0].targets[0].anchor,
        ReviewAnchorDto::ImageRect { x, .. } if x == 0.5
    ));
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
