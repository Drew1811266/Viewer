use super::*;
use crate::{dto::FolderWorkspaceDto, state::DesktopRuntime};
use std::{fs, path::Path, sync::Mutex as StdMutex, time::Duration};
use tokio::sync::{Notify, oneshot};
use viewer_application::{
    PreparedReviewAsset, ProjectProbeError, ProjectProbePort, ReviewArtifactError,
    review_evidence::*,
};
use viewer_domain::{
    EntityId, ReviewCommandId, ReviewStreamId, SessionId, review::FeedbackAnchor,
    search::Generation,
};
use viewer_platform_macos::image::MacReviewEvidenceRenderer;

struct Probe;
impl ProjectProbePort for Probe {
    fn probe(&self, _: &Path) -> Result<ProjectAccess, ProjectProbeError> {
        Ok(ProjectAccess::ReadWrite)
    }
}

#[tokio::test]
async fn review_workspace_cancelled_queued_retry_still_reports_the_committed_receipt() {
    let root = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    fs::copy(
        viewer_test_support::image_fixtures::image_fixture("alpha.png"),
        root.path().join("image.png"),
    )
    .unwrap();
    let runtime = Arc::new(DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(Probe),
    ));
    let opened = runtime.open_project(root.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let id: SessionId = opened.session_id.parse().unwrap();
    let generation = Generation::new(opened.generation);
    let FolderWorkspaceDto::Content { images, .. } = runtime.query_folder(None).await.unwrap()
    else {
        panic!("image missing")
    };
    let entity: EntityId = images[0].entity_id.parse().unwrap();
    let asset = runtime
        .prepare_review_assets(id, generation, vec![entity])
        .await
        .unwrap()[0]
        .asset
        .0
        .id;
    let envelope = runtime
        .prepare_review_command(
            id,
            generation,
            ReviewCommandId::new(),
            None,
            ReviewWorkspaceCommand::SaveFeedback {
                feedback_id: None,
                text: "保留原文".into(),
                targets: vec![TargetEdit::Add {
                    asset_version_id: asset,
                    anchor: FeedbackAnchor::Asset,
                }],
            },
        )
        .await
        .unwrap();
    let saved = runtime
        .apply_review_command(id, generation, envelope.clone())
        .await
        .unwrap();
    let session = runtime
        .session
        .lock()
        .await
        .as_ref()
        .unwrap()
        .review_workspace
        .clone();
    let gate = session.gate.lock().await;
    let owner = runtime.clone();
    let retry =
        tokio::spawn(async move { owner.apply_review_command(id, generation, envelope).await });
    tokio::time::timeout(Duration::from_secs(10), async {
        while session.tasks.cancel() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    drop(gate);
    let error = retry.await.unwrap().unwrap_err();
    assert_eq!(error.code, Code::CommittedViewUnavailable);
    assert_eq!(error.committed_authoring_receipt.unwrap().0, saved.receipt);
    runtime.close_project().await.unwrap();
}
struct GatedRenderer {
    real: MacReviewEvidenceRenderer,
    entered: StdMutex<Option<oneshot::Sender<ReviewTaskCancellation>>>,
    release: Arc<Notify>,
}
#[async_trait::async_trait]
impl ReviewEvidencePort for GatedRenderer {
    async fn capture_base(
        &self,
        asset: PreparedReviewAsset,
        cancel: ReviewTaskCancellation,
    ) -> Result<BoundReviewImage, ReviewArtifactError> {
        self.real.capture_base(asset, cancel).await
    }
    async fn render(
        &self,
        request: ReviewEvidenceRequest,
    ) -> Result<ReviewEvidenceResult, ReviewArtifactError> {
        if let Some(sender) = self.entered.lock().unwrap().take() {
            sender.send(request.cancellation.clone()).unwrap();
        }
        self.release.notified().await;
        self.real.render(request).await
    }
}

#[tokio::test]
async fn review_workspace_project_close_revokes_previews_and_waits_for_renderer_before_cleanup() {
    let root = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    fs::copy(
        viewer_test_support::image_fixtures::image_fixture("alpha.png"),
        root.path().join("image.png"),
    )
    .unwrap();
    let runtime = Arc::new(DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(Probe),
    ));
    let opened = runtime.open_project(root.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let id: SessionId = opened.session_id.parse().unwrap();
    let generation = Generation::new(opened.generation);
    let FolderWorkspaceDto::Content { images, .. } = runtime.query_folder(None).await.unwrap()
    else {
        panic!("missing fixture")
    };
    let entity: EntityId = images[0].entity_id.parse().unwrap();
    let session = runtime
        .session
        .lock()
        .await
        .as_ref()
        .unwrap()
        .review_workspace
        .clone();
    let config = session.config.clone();
    let staging = config.staging.clone();
    let context = ReviewWorkspaceContext {
        project_id: config.project_id,
        stream_id: ReviewStreamId::new(),
        production: None,
    };
    let provider = Arc::new(ProjectReviewRepositoryProvider::new(
        &config.root,
        config.project_id,
    ));
    provider.bootstrap_authoring(context.stream_id).unwrap();
    let authoring = provider.authoring_writer().unwrap();
    let assets = Arc::new(
        IndexedReviewAssetCatalog::new(
            &config.root,
            config.index,
            config.image,
            config.video,
            config.changes,
        )
        .unwrap(),
    );
    let (entered, started) = oneshot::channel();
    let release = Arc::new(Notify::new());
    let renderer = Arc::new(GatedRenderer {
        real: MacReviewEvidenceRenderer::new(&config.staging).unwrap(),
        entered: StdMutex::new(Some(entered)),
        release: release.clone(),
    });
    let service = ContinuousReviewService::new(
        context.clone(),
        provider.clone(),
        assets.clone(),
        renderer.clone(),
        Arc::new(ContinuousReviewCommandCodec),
        config.clock.clone(),
    );
    let materializer = Arc::new(ReviewMaterializationService::new(
        authoring.clone(),
        Arc::new(ContinuousReviewPublication::new_with_cache(
            context.stream_id,
            provider.clone(),
            assets,
            renderer,
            authoring,
            CURRENT_REVIEW_EVIDENCE_ACTION_POLICY,
        )),
        config.clock,
    ));
    assert!(
        session
            .initialized
            .set(Arc::new(Initialized {
                service,
                provider,
                context,
                materializer: Some(materializer),
            }))
            .is_ok()
    );
    let previews = runtime
        .prepare_review_assets(id, generation, vec![entity])
        .await
        .unwrap();
    let envelope = runtime
        .prepare_review_command(
            id,
            generation,
            ReviewCommandId::new(),
            None,
            ReviewWorkspaceCommand::SaveFeedback {
                feedback_id: None,
                text: "keep this unpublished input".into(),
                targets: vec![TargetEdit::Add {
                    asset_version_id: previews[0].asset.0.id,
                    anchor: FeedbackAnchor::Asset,
                }],
            },
        )
        .await
        .unwrap();
    let owner = runtime.clone();
    let saving =
        tokio::spawn(async move { owner.apply_review_command(id, generation, envelope).await });
    let cancellation = tokio::time::timeout(Duration::from_secs(10), started)
        .await
        .unwrap()
        .unwrap();
    let owner = runtime.clone();
    let closing = tokio::spawn(async move { owner.close_project().await });
    tokio::time::timeout(Duration::from_secs(10), async {
        while !cancellation.is_cancelled() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(runtime.active_image_session.get(), None);
    assert!(!closing.is_finished());
    assert!(
        staging.exists(),
        "renderer resources must still exist until it exits"
    );
    assert!(
        runtime
            .prepare_review_assets(id, generation, vec![entity])
            .await
            .is_err()
    );
    release.notify_one();
    assert!(saving.await.unwrap().is_ok());
    closing.await.unwrap().unwrap();
    assert!(!staging.exists());
    assert!(!root.path().join(".viewer/reviews/index.json").exists());
    let resolver = crate::image_protocol::ImageProtocolResolver::new(
        runtime.active_image_session.clone(),
        runtime.image_registry.clone(),
    );
    let url = &previews[0].preview.as_ref().unwrap().url;
    assert_eq!(
        resolver.resolve(url.strip_prefix("viewer-image://localhost").unwrap()),
        Err(crate::image_protocol::ProtocolError::Forbidden)
    );
}
