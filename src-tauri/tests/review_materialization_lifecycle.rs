use std::{fs, path::Path, sync::Arc, time::Duration};
use viewer_application::{
    ProjectAccess, ProjectProbeError, ProjectProbePort,
    review_workspace::{
        ReviewMaterializationFailure, ReviewPublicationStatus, ReviewWorkspaceCommand, TargetEdit,
    },
};
use viewer_desktop::{
    dto::{FolderWorkspaceDto, review_workspace::ReviewAuthoringApplyResultDto},
    state::DesktopRuntime,
};
use viewer_domain::{
    AssetVersionId, EntityId, ReviewCommandId, SessionId,
    review::{FeedbackAnchor, NormalizedArrow, NormalizedPoint},
    search::Generation,
};

struct Probe;

impl ProjectProbePort for Probe {
    fn probe(&self, _: &Path) -> Result<ProjectAccess, ProjectProbeError> {
        Ok(ProjectAccess::ReadWrite)
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

fn save(asset_version_id: AssetVersionId) -> ReviewWorkspaceCommand {
    ReviewWorkspaceCommand::SaveFeedback {
        feedback_id: None,
        text: "袖口收紧，保留材质".into(),
        targets: vec![TargetEdit::Add {
            asset_version_id,
            anchor: FeedbackAnchor::Asset,
        }],
    }
}

fn save_arrow(asset_version_id: AssetVersionId) -> ReviewWorkspaceCommand {
    ReviewWorkspaceCommand::SaveFeedback {
        feedback_id: None,
        text: "沿箭头方向调整".into(),
        targets: vec![TargetEdit::Add {
            asset_version_id,
            anchor: FeedbackAnchor::ImageArrow(
                NormalizedArrow::new(
                    NormalizedPoint::new(0.2, 0.3).unwrap(),
                    NormalizedPoint::new(0.7, 0.6).unwrap(),
                )
                .unwrap(),
            ),
        }],
    }
}

async fn wait_for_terminal_status(
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
    .expect("publication reaches a terminal state")
}

#[tokio::test]
async fn authoring_apply_returns_pending_patch_and_status_converges_without_a_view_reload() {
    let root = project();
    let cache = tempfile::tempdir().unwrap();
    let runtime = DesktopRuntime::new(cache.path().to_path_buf(), Arc::new(Probe));
    let (session, generation, entity) = open(&runtime, root.path()).await;
    let initial = runtime
        .get_review_workspace(session, generation)
        .await
        .unwrap();
    assert!(initial.current.is_none());
    let asset = runtime
        .prepare_review_assets(session, generation, vec![entity])
        .await
        .unwrap()[0]
        .asset
        .0
        .id;
    let envelope = runtime
        .prepare_review_command(
            session,
            generation,
            ReviewCommandId::new(),
            None,
            save_arrow(asset),
        )
        .await
        .unwrap();

    let reply = runtime
        .apply_review_command(session, generation, envelope)
        .await
        .unwrap();

    assert_eq!(reply.patch.basis_snapshot_id, None);
    assert_eq!(reply.patch.upsert_feedback.len(), 1);
    assert!(matches!(
        reply.patch.upsert_feedback[0].targets[0].anchor,
        FeedbackAnchor::ImageArrow(_)
    ));
    assert!(matches!(
        reply.publication,
        ReviewPublicationStatus::Pending {
            pending_revisions: 1
        }
    ));
    let json = serde_json::to_value(ReviewAuthoringApplyResultDto::from(reply)).unwrap();
    assert_eq!(json["publication"]["kind"], "pending");
    assert!(json.get("view").is_none());
    assert_eq!(
        wait_for_terminal_status(&runtime, session, generation).await,
        ReviewPublicationStatus::Ready
    );
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn source_replacement_blocks_publication_but_keeps_the_saved_authoring_marker() {
    let root = project();
    let cache = tempfile::tempdir().unwrap();
    let runtime = DesktopRuntime::new(cache.path().to_path_buf(), Arc::new(Probe));
    let (session, generation, entity) = open(&runtime, root.path()).await;
    let asset = runtime
        .prepare_review_assets(session, generation, vec![entity])
        .await
        .unwrap()[0]
        .asset
        .0
        .id;
    let envelope = runtime
        .prepare_review_command(
            session,
            generation,
            ReviewCommandId::new(),
            None,
            save(asset),
        )
        .await
        .unwrap();
    fs::copy(
        viewer_test_support::image_fixtures::image_fixture("srgb.jpg"),
        root.path().join("image.png"),
    )
    .unwrap();

    let reply = runtime
        .apply_review_command(session, generation, envelope)
        .await
        .unwrap();
    assert_eq!(reply.patch.upsert_feedback.len(), 1);
    assert_eq!(
        wait_for_terminal_status(&runtime, session, generation).await,
        ReviewPublicationStatus::Blocked {
            code: ReviewMaterializationFailure::SourceChanged,
        }
    );
    let view = runtime
        .get_review_workspace(session, generation)
        .await
        .unwrap();
    assert_eq!(view.current.unwrap().authoring.state.feedback.len(), 1);
    runtime.close_project().await.unwrap();

    let (reopened, new_generation, _) = open(&runtime, root.path()).await;
    assert_eq!(
        runtime
            .get_review_publication_status(reopened, new_generation)
            .await
            .unwrap(),
        ReviewPublicationStatus::Blocked {
            code: ReviewMaterializationFailure::SourceChanged,
        }
    );
    assert_eq!(
        runtime
            .get_review_workspace(reopened, new_generation)
            .await
            .unwrap()
            .current
            .unwrap()
            .authoring
            .state
            .feedback
            .len(),
        1
    );
    runtime.close_project().await.unwrap();
}
