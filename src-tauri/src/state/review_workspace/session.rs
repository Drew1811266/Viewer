use super::{
    check_cancelled,
    tasks::{ReviewWorkspaceTasks, TaskError},
};
use crate::dto::review_workspace::{
    ReviewWorkspaceErrorCode as Code, ReviewWorkspaceErrorDto as Error,
};
use std::{future::Future, path::PathBuf, sync::Arc};
use tokio::sync::{Mutex, OnceCell};
use viewer_application::{
    BrowseIndexPort, ClockPort, ImagePort, ProjectAccess, ReviewTaskCancellation,
    review_workspace::*,
};
use viewer_domain::{ProjectId, RelativePath, ReviewStreamId};
use viewer_infrastructure::{
    review::{
        ContinuousReviewCommandCodec, IndexedReviewAssetCatalog, ProjectReviewRepositoryProvider,
        ProjectUsageImporter, ReviewChangeLedger,
    },
    video_probe::VideoMetadataProbe,
};

#[cfg(test)]
mod tests;

#[derive(Clone)]
pub(in crate::state) struct ReviewWorkspaceConfig {
    pub root: PathBuf,
    pub project_id: ProjectId,
    pub access: ProjectAccess,
    pub index: Arc<dyn BrowseIndexPort>,
    pub image: Arc<dyn ImagePort>,
    pub video: Arc<dyn VideoMetadataProbe>,
    pub changes: ReviewChangeLedger,
    pub clock: Arc<dyn ClockPort>,
    pub staging: PathBuf,
}
pub(super) struct Initialized {
    pub service: ContinuousReviewService,
    pub provider: Arc<ProjectReviewRepositoryProvider>,
    pub context: ReviewWorkspaceContext,
}
pub(in crate::state) struct ReviewWorkspaceSession {
    pub tasks: Arc<ReviewWorkspaceTasks>,
    pub config: ReviewWorkspaceConfig,
    initialized: OnceCell<Arc<Initialized>>,
    gate: Mutex<()>,
}
impl ReviewWorkspaceSession {
    /// Pure session registration: legacy project open does not initialize or migrate v3 storage.
    pub fn new(config: ReviewWorkspaceConfig) -> Self {
        Self {
            tasks: Arc::default(),
            config,
            initialized: OnceCell::new(),
            gate: Mutex::new(()),
        }
    }
    pub(super) async fn run<
        T: Send + 'static,
        F: Future<Output = Result<T, Error>> + Send + 'static,
    >(
        self: &Arc<Self>,
        operation: impl FnOnce(Arc<Initialized>, ReviewTaskCancellation) -> F + Send + 'static,
    ) -> Result<T, Error> {
        let owner = self.clone();
        self.tasks
            .run(move |cancel| async move {
                let _serial = owner.gate.lock().await;
                check_cancelled(&cancel)?;
                let initialized = owner
                    .initialized
                    .get_or_try_init(|| async {
                        let config = owner.config.clone();
                        tokio::task::spawn_blocking(move || initialize(config))
                            .await
                            .map_err(|_| Error::new(Code::Internal))?
                    })
                    .await?
                    .clone();
                check_cancelled(&cancel)?;
                operation(initialized, cancel).await
            })
            .await
            .map_err(|e| {
                Error::new(match e {
                    TaskError::Closed => Code::StaleSession,
                    TaskError::Busy => Code::Busy,
                    TaskError::WorkerFailed => Code::Internal,
                })
            })?
    }
}

fn initialize(config: ReviewWorkspaceConfig) -> Result<Arc<Initialized>, Error> {
    let provider = Arc::new(ProjectReviewRepositoryProvider::new_with_access(
        &config.root,
        config.project_id,
        config.access,
    ));
    let stream = provider
        .manual_review_stream()
        .map_err(ReviewWorkspaceError::from)?
        .unwrap_or_else(ReviewStreamId::new);
    let context = ReviewWorkspaceContext {
        project_id: config.project_id,
        stream_id: stream,
        production: None,
    };
    let assets = Arc::new(
        IndexedReviewAssetCatalog::new(
            &config.root,
            config.index,
            config.image,
            config.video,
            config.changes,
        )
        .map_err(ReviewWorkspaceError::from)?,
    );
    let evidence = Arc::new(
        viewer_platform_macos::image::MacReviewEvidenceRenderer::new(&config.staging)
            .map_err(ReviewWorkspaceError::from)?,
    );
    let importer = Arc::new(DeferredUsageImporter {
        provider: provider.clone(),
        root: config.root,
        context: context.clone(),
    });
    let service = ContinuousReviewService::new(
        context.clone(),
        provider.clone(),
        assets,
        evidence,
        Arc::new(ContinuousReviewCommandCodec),
        config.clock,
    )
    .with_usage_importer(importer);
    Ok(Arc::new(Initialized {
        service,
        provider,
        context,
    }))
}

// Opening a legacy project must not require a v3 reader before explicit migration. This adapter
// opens the reader only for the user's later inspection, without holding a writer lease.
struct DeferredUsageImporter {
    provider: Arc<ProjectReviewRepositoryProvider>,
    root: PathBuf,
    context: ReviewWorkspaceContext,
}
impl UsageImportPort for DeferredUsageImporter {
    fn inspect(&self, source: &RelativePath) -> Result<UsageImportPreview, UsageImportError> {
        let reader = self
            .provider
            .continuous_reader()
            .map_err(|_| UsageImportError::UnknownBasis)?;
        ProjectUsageImporter::new(
            &self.root,
            self.context.project_id,
            self.context.stream_id,
            reader,
        )?
        .inspect(source)
    }
}
