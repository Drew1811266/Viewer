use std::{fs, path::Path, sync::Arc};
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
    assert_eq!(
        result.view.current.as_ref().unwrap().state.assets[0],
        preview.asset.0
    );
    assert_eq!(result.view.projection.actionable.len(), 1);
    let image = runtime
        .get_review_evidence(
            session,
            generation,
            HistorySelector::Snapshot(result.receipt.snapshot),
            preview.asset.0.id,
            EvidenceRole::Base,
        )
        .await
        .unwrap();
    assert_eq!((image.width, image.height), (640, 480));
    let history = runtime
        .get_review_history(
            session,
            generation,
            HistorySelector::Snapshot(result.receipt.snapshot),
        )
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
    assert_eq!(retried.view.current.unwrap().state.feedback.len(), 1);
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
    assert_eq!(
        runtime
            .apply_review_command(session, generation, envelope)
            .await
            .unwrap_err()
            .code,
        ReviewWorkspaceErrorCode::SourceChanged
    );
    assert!(
        runtime
            .get_review_workspace(session, generation)
            .await
            .unwrap()
            .current
            .is_none()
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
    let state = &saved.view.current.as_ref().unwrap().state;
    let key = state.target_key(state.feedback[0].targets[0].id).unwrap();
    let index = root.path().join(".viewer/reviews/index.json");
    let before = fs::read(&index).unwrap();
    let selection = ArchiveSelection {
        expected_snapshot_id: saved.receipt.snapshot.snapshot_id,
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
            Some(saved.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::Archive(selection),
        )
        .await
        .unwrap();
    let archive = command.generated.archive_id;
    let archived = runtime
        .apply_review_command(session, generation, command)
        .await
        .unwrap();
    assert!(
        archived
            .view
            .current
            .as_ref()
            .unwrap()
            .state
            .feedback
            .is_empty()
    );
    assert!(archived.view.projection.actionable.is_empty());
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
            Some(archived.receipt.snapshot.snapshot_id),
            ReviewWorkspaceCommand::Restore {
                archive_id: archive,
                decisions,
            },
        )
        .await
        .unwrap();
    let restored = runtime
        .apply_review_command(session, generation, command)
        .await
        .unwrap();
    assert_eq!(restored.view.projection.actionable.len(), 1);
    assert_eq!(
        restored
            .view
            .current
            .as_ref()
            .unwrap()
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
