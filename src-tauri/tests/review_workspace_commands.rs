use std::{fs, path::Path, sync::Arc, time::Duration};
use viewer_application::{
    ProjectAccess, ProjectProbeError, ProjectProbePort, review_evidence::*, review_workspace::*,
};
use viewer_desktop::{
    dto::{FolderWorkspaceDto, review_workspace::*},
    state::DesktopRuntime,
};
use viewer_domain::{
    review::{FeedbackAnchor, continuous::*},
    search::Generation,
    *,
};

struct Probe(ProjectAccess);
impl ProjectProbePort for Probe {
    fn probe(&self, _: &Path) -> Result<ProjectAccess, ProjectProbeError> {
        Ok(self.0)
    }
}
fn project() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::copy(
        viewer_test_support::image_fixtures::image_fixture("alpha.png"),
        root.path().join("image.png"),
    )
    .unwrap();
    root
}

fn legacy_project() -> tempfile::TempDir {
    let root = project();
    let fixture = |name: &str| {
        fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../tests/fixtures/review-protocol")
                .join(name),
        )
        .unwrap()
    };
    let round_bytes = fixture("review-round-v1.valid.json");
    let round = viewer_infrastructure::review::decode_completed(&round_bytes).unwrap();
    let mut index: serde_json::Value =
        serde_json::from_slice(&fixture("review-index-v1.valid.json")).unwrap();
    index["streams"].as_array_mut().unwrap().remove(0);
    let viewer = root.path().join(".viewer");
    let reviews = viewer.join("reviews");
    fs::create_dir_all(reviews.join("rounds")).unwrap();
    fs::write(
        viewer.join("project.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "projectId": round.project_id.to_string(),
            "createdAtMs": 0
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(
        reviews.join("index.json"),
        serde_json::to_vec(&index).unwrap(),
    )
    .unwrap();
    fs::write(
        reviews.join(format!("rounds/{}.json", round.review_round_id)),
        round_bytes,
    )
    .unwrap();
    root
}
async fn open(runtime: &DesktopRuntime, path: &Path) -> (SessionId, Generation, EntityId) {
    let opened = runtime.open_project(path).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let FolderWorkspaceDto::Content { images, .. } = runtime.query_folder(None).await.unwrap()
    else {
        panic!("image missing")
    };
    (
        opened.session_id.parse().unwrap(),
        Generation::new(opened.generation),
        images[0].entity_id.parse().unwrap(),
    )
}
fn save(asset: AssetVersionId) -> ReviewWorkspaceCommand {
    ReviewWorkspaceCommand::SaveFeedback {
        feedback_id: None,
        text: "袖口收紧，保留材质".into(),
        targets: vec![TargetEdit::Add {
            asset_version_id: asset,
            anchor: FeedbackAnchor::Asset,
        }],
    }
}

async fn wait_for_publication(
    runtime: &DesktopRuntime,
    session: SessionId,
    generation: Generation,
) -> ReviewPublicationStatus {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let status = runtime
                .get_review_publication_status(session, generation)
                .await
                .unwrap();
            if !matches!(status, ReviewPublicationStatus::Pending { .. }) {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("review publication reaches a terminal state")
}

#[tokio::test]
async fn review_workspace_prebind_save_history_and_restart_retry_use_one_committed_version() {
    let root = project();
    let cache = tempfile::tempdir().unwrap();
    let runtime = DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(Probe(ProjectAccess::ReadWrite)),
    );
    let (session, generation, entity) = open(&runtime, root.path()).await;
    let empty = runtime
        .get_review_workspace(session, generation)
        .await
        .unwrap();
    assert!(empty.current.is_none());
    assert!(!root.path().join(".viewer/reviews/index.json").exists());
    let previews = runtime
        .prepare_review_assets(session, generation, vec![entity])
        .await
        .unwrap();
    let preview = &previews[0];
    assert_eq!(
        preview.preview.as_ref().unwrap().asset_version_id,
        preview.asset.0.id
    );
    assert!(
        preview
            .preview
            .as_ref()
            .unwrap()
            .url
            .starts_with(&format!("viewer-image://localhost/{session}/"))
    );
    let envelope = runtime
        .prepare_review_command(
            session,
            generation,
            ReviewCommandId::new(),
            None,
            save(preview.asset.0.id),
        )
        .await
        .unwrap();
    let serialized = serde_json::to_vec(&PreparedReviewCommandDto::from(envelope.clone())).unwrap();
    let mut tampered = envelope.clone();
    tampered.generated.created_at_ms += 1;
    assert_eq!(
        runtime
            .apply_review_command(session, generation, tampered)
            .await
            .unwrap_err()
            .code,
        ReviewWorkspaceErrorCode::CommandConflict
    );
    let mut other_stream = envelope.clone();
    other_stream.context.stream_id = ReviewStreamId::new();
    assert_eq!(
        runtime
            .apply_review_command(session, generation, other_stream)
            .await
            .unwrap_err()
            .code,
        ReviewWorkspaceErrorCode::WrongContext
    );
    let result = runtime
        .apply_review_command(session, generation, envelope.clone())
        .await
        .unwrap();
    assert!(matches!(
        result.publication,
        ReviewPublicationStatus::Pending { .. }
    ));
    assert_eq!(
        wait_for_publication(&runtime, session, generation).await,
        ReviewPublicationStatus::Ready
    );
    let saved_view = runtime
        .get_review_workspace(session, generation)
        .await
        .unwrap();
    assert_eq!(
        saved_view.current.as_ref().unwrap().authoring.state.assets[0],
        preview.asset.0
    );
    assert_eq!(saved_view.projection.actionable.len(), 1);
    let published = saved_view.current.as_ref().unwrap().published_ref.unwrap();
    let image = runtime
        .get_review_evidence(
            session,
            generation,
            HistorySelector::Snapshot(published),
            preview.asset.0.id,
            EvidenceRole::Base,
        )
        .await
        .unwrap();
    assert_eq!((image.width, image.height), (640, 480));
    let history = runtime
        .get_review_history(session, generation, HistorySelector::Snapshot(published))
        .await
        .unwrap();
    assert_eq!(history.entries[0].feedback[0].text, "袖口收紧，保留材质");
    assert_eq!(
        runtime
            .get_review_workspace(session, Generation::new(generation.get() + 1))
            .await
            .unwrap_err()
            .code,
        ReviewWorkspaceErrorCode::StaleSession
    );
    runtime.close_project().await.unwrap();
    assert!(
        runtime
            .get_review_workspace(session, generation)
            .await
            .is_err()
    );
    let (reopened, new_generation, _) = open(&runtime, root.path()).await;
    let retry: PreparedReviewCommandDto = serde_json::from_slice(&serialized).unwrap();
    let retried = runtime
        .apply_review_command(reopened, new_generation, retry.0)
        .await
        .unwrap();
    assert_eq!(retried.receipt, result.receipt);
    let retried_view = runtime
        .get_review_workspace(reopened, new_generation)
        .await
        .unwrap();
    assert_eq!(
        retried_view.current.unwrap().authoring.state.feedback.len(),
        1
    );
    assert_eq!(
        fs::read_dir(root.path().join(".viewer/reviews/states"))
            .unwrap()
            .count(),
        1
    );
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn review_workspace_unprepared_and_changed_source_never_silently_rebind() {
    let root = project();
    let cache = tempfile::tempdir().unwrap();
    let runtime = DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(Probe(ProjectAccess::ReadWrite)),
    );
    let (session, generation, entity) = open(&runtime, root.path()).await;
    let unprepared = runtime
        .prepare_review_command(
            session,
            generation,
            ReviewCommandId::new(),
            None,
            save(AssetVersionId::new()),
        )
        .await
        .unwrap();
    assert_eq!(
        runtime
            .apply_review_command(session, generation, unprepared)
            .await
            .unwrap_err()
            .code,
        ReviewWorkspaceErrorCode::PreviewRequired
    );
    let previews = runtime
        .prepare_review_assets(session, generation, vec![entity])
        .await
        .unwrap();
    let envelope = runtime
        .prepare_review_command(
            session,
            generation,
            ReviewCommandId::new(),
            None,
            save(previews[0].asset.0.id),
        )
        .await
        .unwrap();
    fs::copy(
        viewer_test_support::image_fixtures::image_fixture("srgb.jpg"),
        root.path().join("image.png"),
    )
    .unwrap();
    let saved = runtime
        .apply_review_command(session, generation, envelope)
        .await
        .unwrap();
    assert_eq!(saved.patch.upsert_feedback.len(), 1);
    assert_eq!(
        wait_for_publication(&runtime, session, generation).await,
        ReviewPublicationStatus::Blocked {
            code: ReviewMaterializationFailure::SourceChanged,
        }
    );
    let failed = runtime
        .get_review_workspace(session, generation)
        .await
        .unwrap();
    assert_eq!(
        failed.current.as_ref().unwrap().authoring.state.feedback[0].text,
        "袖口收紧，保留材质"
    );
    assert!(failed.recovery.is_empty());
    runtime.close_project().await.unwrap();
    let (reopened, new_generation, _) = open(&runtime, root.path()).await;
    let resumed = runtime
        .get_review_workspace(reopened, new_generation)
        .await
        .unwrap();
    assert_eq!(
        resumed.stream_id, failed.stream_id,
        "a failed first save has no committed index but still owns recovery input"
    );
    assert_eq!(resumed.recovery, failed.recovery);
    assert_eq!(
        resumed.current.unwrap().authoring.state.feedback[0].text,
        "袖口收紧，保留材质"
    );
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn review_workspace_read_only_browsing_does_not_enable_or_create_review_writes() {
    let root = project();
    let cache = tempfile::tempdir().unwrap();
    let runtime = DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(Probe(ProjectAccess::ReadOnly)),
    );
    let (session, generation, _) = open(&runtime, root.path()).await;
    let view = runtime
        .get_review_workspace(session, generation)
        .await
        .unwrap();
    assert!(!view.capabilities.continuous_editing);
    assert_eq!(
        runtime
            .prepare_review_command(
                session,
                generation,
                ReviewCommandId::new(),
                None,
                save(AssetVersionId::new())
            )
            .await
            .unwrap_err()
            .code,
        ReviewWorkspaceErrorCode::ReadOnly
    );
    runtime.close_project().await.unwrap();
    assert!(!root.path().join(".viewer").exists());
}

#[tokio::test]
async fn review_workspace_legacy_migration_activates_authoring_without_sync_save_fallback() {
    let root = legacy_project();
    let cache = tempfile::tempdir().unwrap();
    let runtime = DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(Probe(ProjectAccess::ReadWrite)),
    );
    let (session, generation, _) = open(&runtime, root.path()).await;
    let inspection = runtime
        .inspect_review_migration(session, generation)
        .await
        .unwrap()
        .expect("legacy data requires explicit migration");
    let command = runtime
        .prepare_review_command(
            session,
            generation,
            ReviewCommandId::new(),
            None,
            ReviewWorkspaceCommand::Migrate(MigrationPlan {
                inspection_digest: inspection.inspection_digest,
                choice: MigrationChoice::KeepHistoryOnly,
            }),
        )
        .await
        .unwrap();
    let migrated = runtime
        .apply_review_command(session, generation, command)
        .await
        .unwrap();
    assert_eq!(migrated.publication, ReviewPublicationStatus::Ready);
    assert!(migrated.patch.history_selectors.is_some());
    let view = runtime
        .get_review_workspace(session, generation)
        .await
        .unwrap();
    assert!(view.migration.is_none());
    let current = view
        .current
        .expect("migration creates the v3/authoring head");
    assert_eq!(current.authoring.head, migrated.receipt.head);
    assert!(current.authoring.state.feedback.is_empty());
    let migrated_index: serde_json::Value =
        serde_json::from_slice(&fs::read(root.path().join(".viewer/reviews/index.json")).unwrap())
            .unwrap();
    assert_eq!(migrated_index["protocolVersion"], "viewer.review/3");
    assert!(migrated_index["legacyIndex"].is_object());
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn review_workspace_archive_and_restore_previews_are_read_only_and_keep_history_separate() {
    let root = project();
    let cache = tempfile::tempdir().unwrap();
    let runtime = DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(Probe(ProjectAccess::ReadWrite)),
    );
    let (session, generation, entity) = open(&runtime, root.path()).await;
    assert!(
        runtime
            .inspect_review_migration(session, generation)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        runtime
            .cancel_review_workspace_task(session, generation)
            .await
            .unwrap(),
        0
    );
    let previews = runtime
        .prepare_review_assets(session, generation, vec![entity])
        .await
        .unwrap();
    let command = runtime
        .prepare_review_command(
            session,
            generation,
            ReviewCommandId::new(),
            None,
            save(previews[0].asset.0.id),
        )
        .await
        .unwrap();
    let saved = runtime
        .apply_review_command(session, generation, command)
        .await
        .unwrap();
    assert_eq!(
        wait_for_publication(&runtime, session, generation).await,
        ReviewPublicationStatus::Ready
    );
    let saved_view = runtime
        .get_review_workspace(session, generation)
        .await
        .unwrap();
    let state = &saved_view.current.as_ref().unwrap().authoring.state;
    let key = state.target_key(state.feedback[0].targets[0].id).unwrap();
    let index = root.path().join(".viewer/reviews/index.json");
    let before = fs::read(&index).unwrap();
    let selection = ArchiveSelection {
        expected_snapshot_id: saved.receipt.head.snapshot_id,
        groups: vec![ArchiveGroup {
            basis: ArchiveBasis::Unknown,
            targets: vec![key],
        }],
    };
    let plan = runtime
        .preview_review_archive(session, generation, selection.clone())
        .await
        .unwrap();
    assert_eq!(plan.removed, vec![key]);
    assert_eq!(fs::read(&index).unwrap(), before);
    let command = runtime
        .prepare_review_command(
            session,
            generation,
            ReviewCommandId::new(),
            Some(saved.receipt.head.snapshot_id),
            ReviewWorkspaceCommand::Archive(selection),
        )
        .await
        .unwrap();
    let archive = command.generated.archive_id;
    let archived = runtime
        .apply_review_command(session, generation, command)
        .await
        .unwrap();
    let archived_view = runtime
        .get_review_workspace(session, generation)
        .await
        .unwrap();
    assert!(
        archived_view
            .current
            .as_ref()
            .unwrap()
            .authoring
            .state
            .feedback
            .is_empty()
    );
    assert!(archived_view.projection.actionable.is_empty());
    assert_eq!(
        wait_for_publication(&runtime, session, generation).await,
        ReviewPublicationStatus::Ready
    );
    let history = runtime
        .get_review_history(session, generation, HistorySelector::Archive(archive))
        .await
        .unwrap();
    assert_eq!(history.restore_actions, vec![key]);
    assert_eq!(history.entries[0].feedback[0].text, "袖口收紧，保留材质");
    let before = fs::read(&index).unwrap();
    let decisions = vec![RestoreDecision {
        historical_key: key,
        choice: RestoreChoice::UseHistorical,
    }];
    let plan = runtime
        .preview_review_restore(session, generation, archive, decisions.clone())
        .await
        .unwrap();
    assert_eq!(plan.restored.len(), 1);
    assert_eq!(plan.restored[0].historical_key, key);
    assert_eq!(fs::read(&index).unwrap(), before);
    let command = runtime
        .prepare_review_command(
            session,
            generation,
            ReviewCommandId::new(),
            Some(archived.receipt.head.snapshot_id),
            ReviewWorkspaceCommand::Restore {
                archive_id: archive,
                decisions,
            },
        )
        .await
        .unwrap();
    runtime
        .apply_review_command(session, generation, command)
        .await
        .unwrap();
    assert_eq!(
        wait_for_publication(&runtime, session, generation).await,
        ReviewPublicationStatus::Ready
    );
    let restored_view = runtime
        .get_review_workspace(session, generation)
        .await
        .unwrap();
    assert_eq!(restored_view.projection.actionable.len(), 1);
    assert_eq!(
        restored_view
            .current
            .as_ref()
            .unwrap()
            .authoring
            .state
            .target_key(key.target_id),
        Some(key)
    );
    assert_eq!(
        runtime
            .inspect_review_usage(session, generation, EntityId::new())
            .await
            .unwrap_err()
            .code,
        ReviewWorkspaceErrorCode::AssetUnavailable
    );
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn review_workspace_explicit_usage_selection_is_project_relative_and_excludes_viewer_data() {
    let root = project();
    let cache = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let runtime = DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(Probe(ProjectAccess::ReadWrite)),
    );
    let (session, generation, _) = open(&runtime, root.path()).await;
    let declaration = root.path().join("handoff/review-usage.json");
    fs::create_dir_all(declaration.parent().unwrap()).unwrap();
    fs::write(&declaration, b"{}").unwrap();
    let manifest = root.path().join("viewer-production.json");
    fs::write(&manifest, b"production-manifest-must-not-change").unwrap();
    let downloads = outside.path().join("Downloads/review-usage.json");
    fs::create_dir_all(downloads.parent().unwrap()).unwrap();
    fs::write(&downloads, b"{}").unwrap();

    assert_eq!(
        runtime
            .inspect_review_usage_selection(session, generation, declaration.clone())
            .await
            .unwrap_err()
            .code,
        ReviewWorkspaceErrorCode::UsageInvalid,
        "the selected in-project file reaches the existing declaration inspector"
    );
    assert_eq!(
        runtime
            .inspect_review_usage_selection(session, generation, downloads.clone())
            .await
            .unwrap_err()
            .code,
        ReviewWorkspaceErrorCode::WrongContext,
        "Viewer never scans or accepts a Downloads declaration outside the project"
    );
    assert_eq!(
        runtime
            .inspect_review_usage_selection(
                session,
                generation,
                declaration.parent().unwrap().to_path_buf(),
            )
            .await
            .unwrap_err()
            .code,
        ReviewWorkspaceErrorCode::UnsafeSource,
        "an explicit picker selection must be a regular file"
    );
    #[cfg(unix)]
    {
        let escaped = root.path().join("handoff/escaped-review-usage.json");
        std::os::unix::fs::symlink(&downloads, &escaped).unwrap();
        assert_eq!(
            runtime
                .inspect_review_usage_selection(session, generation, escaped)
                .await
                .unwrap_err()
                .code,
            ReviewWorkspaceErrorCode::WrongContext,
            "a selected symlink cannot escape the project root"
        );
    }
    let reserved = root.path().join(".viewer/review-usage.json");
    fs::write(&reserved, b"{}").unwrap();
    assert_eq!(
        runtime
            .inspect_review_usage_selection(session, generation, reserved)
            .await
            .unwrap_err()
            .code,
        ReviewWorkspaceErrorCode::UnsafeSource
    );
    assert_eq!(
        fs::read(&manifest).unwrap(),
        b"production-manifest-must-not-change"
    );
    runtime.close_project().await.unwrap();
}
