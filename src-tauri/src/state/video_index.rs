use super::{
    entity_id_for_metadata, is_stale_derived_write_error, modified_ns,
    preview::validated_indexed_source,
};
use crate::error::CommandError;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use viewer_application::{
    ActiveProject,
    scheduler::{DerivedWorkClass, DerivedWorkScheduler, TaskCoordinator},
};
use viewer_domain::{
    file::{FileKind, FileNode},
    video::{VideoFailureKind, VideoMetadata, VideoProbeStatus},
};
use viewer_infrastructure::{
    search::index::SessionIndex,
    video_probe::{MediaFileIdentity, VideoMetadataProbe, VideoProbeError},
};

pub(crate) struct VideoIndexRuntime {
    cancellation: CancellationToken,
    wake: tokio::sync::mpsc::Sender<()>,
    worker: Mutex<Option<JoinHandle<()>>>,
    errors: Arc<Mutex<Vec<CommandError>>>,
}

trait PendingVideoQueue: Send + Sync {
    fn pending_video_nodes_after(
        &self,
        after: Option<&FileNode>,
        limit: usize,
    ) -> Result<Vec<FileNode>, viewer_infrastructure::search::index::SessionIndexError>;
}

impl PendingVideoQueue for SessionIndex {
    fn pending_video_nodes_after(
        &self,
        after: Option<&FileNode>,
        limit: usize,
    ) -> Result<Vec<FileNode>, viewer_infrastructure::search::index::SessionIndexError> {
        SessionIndex::pending_video_nodes_after(self, after, limit)
    }
}

#[derive(Clone)]
struct VideoWorkerServices {
    active: ActiveProject,
    coordinator: Arc<TaskCoordinator>,
    index: Arc<SessionIndex>,
    pending: Arc<dyn PendingVideoQueue>,
    probe: Arc<dyn VideoMetadataProbe>,
    scheduler: Arc<DerivedWorkScheduler>,
    errors: Arc<Mutex<Vec<CommandError>>>,
}

impl VideoIndexRuntime {
    pub(crate) fn new(
        active: ActiveProject,
        coordinator: Arc<TaskCoordinator>,
        index: Arc<SessionIndex>,
        probe: Arc<dyn VideoMetadataProbe>,
        scheduler: Arc<DerivedWorkScheduler>,
    ) -> Self {
        Self::new_with_queue(
            active,
            coordinator,
            Arc::clone(&index),
            index,
            probe,
            scheduler,
        )
    }

    fn new_with_queue(
        active: ActiveProject,
        coordinator: Arc<TaskCoordinator>,
        index: Arc<SessionIndex>,
        pending: Arc<dyn PendingVideoQueue>,
        probe: Arc<dyn VideoMetadataProbe>,
        scheduler: Arc<DerivedWorkScheduler>,
    ) -> Self {
        let cancellation = CancellationToken::new();
        let (wake, mut incoming) = tokio::sync::mpsc::channel(1);
        let worker_cancellation = cancellation.clone();
        let errors = Arc::new(Mutex::new(Vec::new()));
        let worker_errors = Arc::clone(&errors);
        let services = VideoWorkerServices {
            active,
            coordinator,
            index,
            pending,
            probe,
            scheduler,
            errors: Arc::clone(&worker_errors),
        };
        let worker = tokio::spawn(async move {
            let mut retry_without_wake = false;
            loop {
                if retry_without_wake {
                    tokio::select! {
                        _ = worker_cancellation.cancelled() => return,
                        _ = tokio::time::sleep(Duration::from_millis(50)) => {}
                    }
                } else {
                    tokio::select! {
                        _ = worker_cancellation.cancelled() => return,
                        signal = incoming.recv() => if signal.is_none() { return; },
                    }
                }
                let drain = tokio::spawn(drain_pending_video_nodes(
                    services.clone(),
                    worker_cancellation.clone(),
                ));
                retry_without_wake = match drain.await {
                    Ok(retry) => retry,
                    Err(_) => {
                        record_runtime_error(&worker_errors, video_probe_runtime_failed());
                        true
                    }
                };
            }
        });
        Self {
            cancellation,
            wake,
            worker: Mutex::new(Some(worker)),
            errors,
        }
    }

    pub(crate) fn notify_after_publication(&self) -> Result<(), CommandError> {
        if self.cancellation.is_cancelled() {
            return Ok(());
        }
        match self.wake.try_send(()) {
            Ok(()) | Err(tokio::sync::mpsc::error::TrySendError::Full(())) => Ok(()),
            Err(tokio::sync::mpsc::error::TrySendError::Closed(())) => {
                Err(video_probe_runtime_failed())
            }
        }
    }

    pub(crate) async fn cancel_and_wait(&self) -> Result<(), CommandError> {
        self.cancellation.cancel();
        let worker = self
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(worker) = worker
            && worker.await.is_err()
        {
            record_runtime_error(&self.errors, video_probe_runtime_failed());
        }
        self.errors
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .first()
            .cloned()
            .map_or(Ok(()), Err)
    }
}

pub(crate) async fn cancel_video_worker_before_session_teardown(runtime: &VideoIndexRuntime) {
    let _ = runtime.cancel_and_wait().await;
}

async fn drain_pending_video_nodes(
    services: VideoWorkerServices,
    cancellation: CancellationToken,
) -> bool {
    const PAGE_SIZE: usize = 4;
    let mut cursor = None;
    let mut retry = false;
    loop {
        if cancellation.is_cancelled() {
            return false;
        }
        let page = match services
            .pending
            .pending_video_nodes_after(cursor.as_ref(), PAGE_SIZE)
        {
            Ok(page) => page,
            Err(_) => {
                record_runtime_error(&services.errors, video_probe_index_unavailable());
                return true;
            }
        };
        if page.is_empty() {
            return retry;
        }
        for node in &page {
            if let Err(error) = probe_video_nodes(
                services.active.clone(),
                Arc::clone(&services.coordinator),
                Arc::clone(&services.index),
                Arc::clone(&services.probe),
                Arc::clone(&services.scheduler),
                cancellation.clone(),
                vec![node.clone()],
            )
            .await
            {
                record_runtime_error(&services.errors, error);
                retry = true;
            }
        }
        cursor = page.last().cloned();
        if page.len() < PAGE_SIZE {
            return retry;
        }
    }
}

fn record_runtime_error(errors: &Mutex<Vec<CommandError>>, error: CommandError) {
    let mut errors = errors
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if errors.iter().all(|recorded| recorded.code != error.code) {
        errors.push(error);
    }
}

fn video_probe_runtime_failed() -> CommandError {
    CommandError::new(
        "video_probe_runtime_failed",
        crate::error::ErrorCategory::Internal,
        "视频信息处理意外终止，请重新打开项目。",
        true,
    )
}

fn video_probe_index_unavailable() -> CommandError {
    CommandError::new(
        "video_probe_index_unavailable",
        crate::error::ErrorCategory::Consistency,
        "视频信息索引暂时不可用，请重新打开项目。",
        true,
    )
}

async fn probe_video_nodes(
    active: ActiveProject,
    coordinator: Arc<TaskCoordinator>,
    index: Arc<SessionIndex>,
    probe: Arc<dyn VideoMetadataProbe>,
    scheduler: Arc<DerivedWorkScheduler>,
    cancellation: CancellationToken,
    nodes: Vec<FileNode>,
) -> Result<(), CommandError> {
    for node in nodes
        .into_iter()
        .filter(|node| node.kind == FileKind::Video)
    {
        let permit = tokio::select! {
            _ = cancellation.cancelled() => return Ok(()),
            permit = scheduler.acquire(DerivedWorkClass::VideoProbe) => permit,
        };
        if cancellation.is_cancelled()
            || !coordinator.is_publishable(active.session_id, active.generation)
        {
            return Ok(());
        }
        let current = match index.indexed_node(node.entity_id) {
            Ok(Some(current)) if current.node == node => current,
            Ok(_) => continue,
            Err(error) => return Err(CommandError::from(error)),
        };
        if !current
            .video_metadata
            .as_ref()
            .is_some_and(|metadata| metadata.probe_status == VideoProbeStatus::Pending)
        {
            continue;
        }

        let (source, source_size, source_modified_ns, source_identity) =
            match validated_video_source(&active, &node) {
                VideoSourceState::Current(source) => source,
                VideoSourceState::Failed(kind) => {
                    let metadata = failed_metadata(kind);
                    drop(permit);
                    persist_video_metadata(
                        &active,
                        coordinator.as_ref(),
                        index.as_ref(),
                        &cancellation,
                        &node,
                        &metadata,
                    )?;
                    continue;
                }
                VideoSourceState::Changed => {
                    drop(permit);
                    continue;
                }
            };
        let metadata = {
            let probe_task = {
                let probe = Arc::clone(&probe);
                let cancellation = cancellation.clone();
                let probe_source = source.clone();
                let expected_identity = source_identity.clone();
                tokio::spawn(async move {
                    probe
                        .probe_identity_bound(&probe_source, &expected_identity, cancellation)
                        .await
                })
            };
            match probe_task.await {
                Ok(Ok(metadata)) => metadata,
                Ok(Err(VideoProbeError::Failed(kind))) => failed_metadata(kind),
                Ok(Err(VideoProbeError::Cancelled)) => return Ok(()),
                Ok(Err(VideoProbeError::SourceChanged)) => {
                    drop(permit);
                    continue;
                }
                Err(_) => {
                    return persist_probe_panic(
                        &active,
                        coordinator.as_ref(),
                        index.as_ref(),
                        &cancellation,
                        &node,
                    );
                }
            }
        };
        if !matches!(
            validated_video_source(&active, &node),
            VideoSourceState::Current((ref current, current_size, current_modified_ns, ref current_identity))
                if current == &source
                    && current_size == source_size
                    && current_modified_ns == source_modified_ns
                    && current_identity == &source_identity
        ) {
            drop(permit);
            continue;
        }
        drop(permit);

        persist_video_metadata(
            &active,
            coordinator.as_ref(),
            index.as_ref(),
            &cancellation,
            &node,
            &metadata,
        )?;
    }
    Ok(())
}

fn persist_video_metadata(
    active: &ActiveProject,
    coordinator: &TaskCoordinator,
    index: &SessionIndex,
    cancellation: &CancellationToken,
    node: &FileNode,
    metadata: &VideoMetadata,
) -> Result<(), CommandError> {
    if cancellation.is_cancelled() {
        return Ok(());
    }
    let persisted = coordinator.run_if_current(active.session_id, active.generation, || {
        if cancellation.is_cancelled() {
            return Ok(false);
        }
        index.replace_video_metadata_if_current_node(node, metadata, active.generation)
    });
    let Some(result) = persisted else {
        return Ok(());
    };
    if let Err(error) = result
        && !is_stale_derived_write_error(&error)
    {
        return Err(CommandError::from(error));
    }
    Ok(())
}

enum VideoSourceState {
    Current((std::path::PathBuf, u64, i128, MediaFileIdentity)),
    Failed(VideoFailureKind),
    Changed,
}

fn validated_video_source(active: &ActiveProject, node: &FileNode) -> VideoSourceState {
    match validated_indexed_source(active, node) {
        Ok((source, size, modified)) if size == node.size && modified == node.modified_ns => {
            let metadata = match std::fs::symlink_metadata(&source) {
                Ok(metadata) => metadata,
                Err(_) => return VideoSourceState::Changed,
            };
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || entity_id_for_metadata(&metadata, &node.relative_path) != node.entity_id
                || metadata.len() != size
                || modified_ns(&metadata) != modified
            {
                return VideoSourceState::Changed;
            }
            VideoSourceState::Current((
                source,
                size,
                modified,
                MediaFileIdentity::from_metadata(&metadata),
            ))
        }
        Ok(_) => VideoSourceState::Changed,
        Err(()) => match std::fs::symlink_metadata(active.root.join(node.relative_path.as_str())) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                VideoSourceState::Failed(VideoFailureKind::Missing)
            }
            Err(_) => VideoSourceState::Failed(VideoFailureKind::Unreadable),
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                VideoSourceState::Failed(VideoFailureKind::Unreadable)
            }
            Ok(_) => VideoSourceState::Changed,
        },
    }
}

fn persist_probe_panic(
    active: &ActiveProject,
    coordinator: &TaskCoordinator,
    index: &SessionIndex,
    cancellation: &CancellationToken,
    node: &FileNode,
) -> Result<(), CommandError> {
    let metadata = failed_metadata(VideoFailureKind::EngineInitialization);
    let persisted = coordinator.run_if_current(active.session_id, active.generation, || {
        if cancellation.is_cancelled() {
            return Ok(false);
        }
        index.replace_video_metadata_if_current_node(node, &metadata, active.generation)
    });
    if let Some(Err(error)) = persisted
        && !is_stale_derived_write_error(&error)
    {
        return Err(CommandError::from(error));
    }
    Err(video_probe_runtime_failed())
}

fn failed_metadata(kind: VideoFailureKind) -> VideoMetadata {
    VideoMetadata {
        duration_us: None,
        display_width: None,
        display_height: None,
        rotation_degrees: 0,
        frame_rate_millihertz: None,
        video_codec: None,
        audio_codec: None,
        probe_status: VideoProbeStatus::Failed(kind),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PendingVideoQueue, VideoIndexRuntime, cancel_video_worker_before_session_teardown,
        probe_video_nodes,
    };
    use crate::state::{entity_id_for_metadata, modified_ns};
    use async_trait::async_trait;
    use std::{
        fs,
        path::Path,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        time::{Duration, Instant},
    };
    use tokio_util::sync::CancellationToken;
    use viewer_application::{
        ActiveProject, BrowseIndexPort, ProjectAccess,
        scheduler::{DerivedWorkScheduler, TaskCoordinator},
    };
    use viewer_domain::{
        ProjectId, RelativePath, SessionId,
        file::{FileKind, FileNode},
        video::{VideoMetadata, VideoProbeStatus},
    };
    use viewer_infrastructure::{
        search::index::SessionIndex,
        video_probe::{MediaFileIdentity, VideoMetadataProbe, VideoProbeError},
    };

    struct BlockedProbe {
        started: tokio::sync::Notify,
        release: tokio::sync::Notify,
    }

    #[async_trait]
    impl VideoMetadataProbe for BlockedProbe {
        async fn probe(
            &self,
            _canonical_path: &Path,
            cancellation: CancellationToken,
        ) -> Result<VideoMetadata, VideoProbeError> {
            self.started.notify_one();
            tokio::select! {
                _ = cancellation.cancelled() => Err(VideoProbeError::Cancelled),
                _ = self.release.notified() => Ok(ready_metadata()),
            }
        }

        async fn probe_identity_bound(
            &self,
            canonical_path: &Path,
            _expected_identity: &MediaFileIdentity,
            cancellation: CancellationToken,
        ) -> Result<VideoMetadata, VideoProbeError> {
            self.probe(canonical_path, cancellation).await
        }
    }

    struct PanicProbe;

    struct CancellationRecordingProbe {
        started: tokio::sync::Notify,
        cancellation_observed: AtomicBool,
    }

    #[async_trait]
    impl VideoMetadataProbe for CancellationRecordingProbe {
        async fn probe(
            &self,
            _canonical_path: &Path,
            cancellation: CancellationToken,
        ) -> Result<VideoMetadata, VideoProbeError> {
            self.started.notify_one();
            cancellation.cancelled().await;
            self.cancellation_observed.store(true, Ordering::Release);
            Err(VideoProbeError::Cancelled)
        }

        async fn probe_identity_bound(
            &self,
            canonical_path: &Path,
            _expected_identity: &MediaFileIdentity,
            cancellation: CancellationToken,
        ) -> Result<VideoMetadata, VideoProbeError> {
            self.probe(canonical_path, cancellation).await
        }
    }

    #[tokio::test]
    async fn setup_failure_cancels_and_awaits_worker_before_session_teardown() {
        let fixture = MultiFixture::new(1);
        let probe = Arc::new(CancellationRecordingProbe {
            started: tokio::sync::Notify::new(),
            cancellation_observed: AtomicBool::new(false),
        });
        let runtime = VideoIndexRuntime::new(
            fixture.active.clone(),
            Arc::clone(&fixture.coordinator),
            Arc::clone(&fixture.index),
            probe.clone(),
            Arc::new(DerivedWorkScheduler::default()),
        );
        runtime.notify_after_publication().unwrap();
        probe.started.notified().await;

        cancel_video_worker_before_session_teardown(&runtime).await;

        assert!(probe.cancellation_observed.load(Ordering::Acquire));
    }

    #[async_trait]
    impl VideoMetadataProbe for PanicProbe {
        async fn probe(
            &self,
            _canonical_path: &Path,
            _cancellation: CancellationToken,
        ) -> Result<VideoMetadata, VideoProbeError> {
            panic!("stale work launched the probe process")
        }

        async fn probe_identity_bound(
            &self,
            canonical_path: &Path,
            _expected_identity: &MediaFileIdentity,
            cancellation: CancellationToken,
        ) -> Result<VideoMetadata, VideoProbeError> {
            self.probe(canonical_path, cancellation).await
        }
    }

    struct CountingReadyProbe {
        calls: AtomicUsize,
    }

    struct IdentityBoundOnlyProbe {
        bound_calls: AtomicUsize,
    }

    #[async_trait]
    impl VideoMetadataProbe for IdentityBoundOnlyProbe {
        async fn probe(
            &self,
            _canonical_path: &Path,
            _cancellation: CancellationToken,
        ) -> Result<VideoMetadata, VideoProbeError> {
            panic!("worker bypassed the Task 4 identity-bound probe contract")
        }

        async fn probe_identity_bound(
            &self,
            _canonical_path: &Path,
            _expected_identity: &MediaFileIdentity,
            cancellation: CancellationToken,
        ) -> Result<VideoMetadata, VideoProbeError> {
            if cancellation.is_cancelled() {
                return Err(VideoProbeError::Cancelled);
            }
            self.bound_calls.fetch_add(1, Ordering::AcqRel);
            Ok(ready_metadata())
        }
    }

    #[tokio::test]
    async fn worker_uses_task4_identity_bound_probe_contract() {
        let fixture = Fixture::new();
        let probe = Arc::new(IdentityBoundOnlyProbe {
            bound_calls: AtomicUsize::new(0),
        });

        probe_video_nodes(
            fixture.active.clone(),
            Arc::clone(&fixture.coordinator),
            Arc::clone(&fixture.index),
            probe.clone(),
            Arc::new(DerivedWorkScheduler::default()),
            CancellationToken::new(),
            vec![fixture.node.clone()],
        )
        .await
        .unwrap();

        assert_eq!(probe.bound_calls.load(Ordering::Acquire), 1);
        assert_eq!(fixture.metadata().probe_status, VideoProbeStatus::Ready);
    }

    #[async_trait]
    impl VideoMetadataProbe for CountingReadyProbe {
        async fn probe(
            &self,
            _canonical_path: &Path,
            cancellation: CancellationToken,
        ) -> Result<VideoMetadata, VideoProbeError> {
            if cancellation.is_cancelled() {
                return Err(VideoProbeError::Cancelled);
            }
            self.calls.fetch_add(1, Ordering::AcqRel);
            Ok(ready_metadata())
        }

        async fn probe_identity_bound(
            &self,
            canonical_path: &Path,
            _expected_identity: &MediaFileIdentity,
            cancellation: CancellationToken,
        ) -> Result<VideoMetadata, VideoProbeError> {
            self.probe(canonical_path, cancellation).await
        }
    }

    struct PanicOnceProbe {
        panicked: AtomicBool,
    }

    enum IdentityMutation {
        Replace,
        Rewrite,
    }

    struct MutatingProbe(IdentityMutation);

    #[async_trait]
    impl VideoMetadataProbe for MutatingProbe {
        async fn probe(
            &self,
            canonical_path: &Path,
            _cancellation: CancellationToken,
        ) -> Result<VideoMetadata, VideoProbeError> {
            match self.0 {
                IdentityMutation::Replace => {
                    fs::rename(canonical_path, canonical_path.with_extension("old")).unwrap();
                    fs::write(canonical_path, b"replacement candidate bytes").unwrap();
                }
                IdentityMutation::Rewrite => {
                    fs::write(canonical_path, b"mutated candidate bytes").unwrap();
                }
            }
            Ok(ready_metadata())
        }

        async fn probe_identity_bound(
            &self,
            canonical_path: &Path,
            _expected_identity: &MediaFileIdentity,
            cancellation: CancellationToken,
        ) -> Result<VideoMetadata, VideoProbeError> {
            self.probe(canonical_path, cancellation).await
        }
    }

    #[tokio::test]
    async fn path_replacement_during_probe_cannot_persist_old_anchor_metadata() {
        assert_identity_mutation_is_rejected(IdentityMutation::Replace).await;
    }

    #[tokio::test]
    async fn in_place_content_mutation_during_probe_cannot_persist_metadata() {
        assert_identity_mutation_is_rejected(IdentityMutation::Rewrite).await;
    }

    async fn assert_identity_mutation_is_rejected(mutation: IdentityMutation) {
        let fixture = Fixture::new();
        probe_video_nodes(
            fixture.active.clone(),
            Arc::clone(&fixture.coordinator),
            Arc::clone(&fixture.index),
            Arc::new(MutatingProbe(mutation)),
            Arc::new(DerivedWorkScheduler::default()),
            CancellationToken::new(),
            vec![fixture.node.clone()],
        )
        .await
        .unwrap();
        assert_eq!(fixture.metadata().probe_status, VideoProbeStatus::Pending);
    }

    #[async_trait]
    impl VideoMetadataProbe for PanicOnceProbe {
        async fn probe(
            &self,
            _canonical_path: &Path,
            cancellation: CancellationToken,
        ) -> Result<VideoMetadata, VideoProbeError> {
            if !self.panicked.swap(true, Ordering::AcqRel) {
                panic!("injected video probe panic");
            }
            if cancellation.is_cancelled() {
                Err(VideoProbeError::Cancelled)
            } else {
                Ok(ready_metadata())
            }
        }

        async fn probe_identity_bound(
            &self,
            canonical_path: &Path,
            _expected_identity: &MediaFileIdentity,
            cancellation: CancellationToken,
        ) -> Result<VideoMetadata, VideoProbeError> {
            self.probe(canonical_path, cancellation).await
        }
    }

    struct FailFirstPendingQuery {
        index: Arc<SessionIndex>,
        failed: AtomicBool,
    }

    struct PanicFirstPendingQuery {
        index: Arc<SessionIndex>,
        panicked: AtomicBool,
    }

    impl PendingVideoQueue for PanicFirstPendingQuery {
        fn pending_video_nodes_after(
            &self,
            after: Option<&FileNode>,
            limit: usize,
        ) -> Result<Vec<FileNode>, viewer_infrastructure::search::index::SessionIndexError>
        {
            if !self.panicked.swap(true, Ordering::AcqRel) {
                panic!("injected pending queue panic");
            }
            self.index.pending_video_nodes_after(after, limit)
        }
    }

    impl PendingVideoQueue for FailFirstPendingQuery {
        fn pending_video_nodes_after(
            &self,
            after: Option<&FileNode>,
            limit: usize,
        ) -> Result<Vec<FileNode>, viewer_infrastructure::search::index::SessionIndexError>
        {
            if !self.failed.swap(true, Ordering::AcqRel) {
                return Err(
                    viewer_infrastructure::search::index::SessionIndexError::InvalidSearchQuery,
                );
            }
            self.index.pending_video_nodes_after(after, limit)
        }
    }

    #[tokio::test]
    async fn coalesced_wake_is_nonblocking_and_drains_more_than_legacy_capacity() {
        let fixture = MultiFixture::new(11);
        let probe = Arc::new(CountingReadyProbe {
            calls: AtomicUsize::new(0),
        });
        let runtime = VideoIndexRuntime::new(
            fixture.active.clone(),
            Arc::clone(&fixture.coordinator),
            Arc::clone(&fixture.index),
            probe.clone(),
            Arc::new(DerivedWorkScheduler::default()),
        );

        let started = Instant::now();
        for _ in 0..100 {
            runtime.notify_after_publication().unwrap();
        }
        assert!(started.elapsed() < Duration::from_secs(1));
        fixture.wait_until_terminal().await;
        assert_eq!(probe.calls.load(Ordering::Acquire), 11);
        runtime.cancel_and_wait().await.unwrap();
    }

    #[tokio::test]
    async fn probe_panic_isolated_and_later_pending_candidate_completes() {
        let fixture = MultiFixture::new(2);
        let runtime = VideoIndexRuntime::new(
            fixture.active.clone(),
            Arc::clone(&fixture.coordinator),
            Arc::clone(&fixture.index),
            Arc::new(PanicOnceProbe {
                panicked: AtomicBool::new(false),
            }),
            Arc::new(DerivedWorkScheduler::default()),
        );
        runtime.notify_after_publication().unwrap();
        fixture.wait_until_terminal().await;
        let statuses = fixture.statuses();
        assert!(matches!(
            statuses[0],
            VideoProbeStatus::Failed(viewer_domain::video::VideoFailureKind::EngineInitialization)
        ));
        assert_eq!(statuses[1], VideoProbeStatus::Ready);
        assert_eq!(
            runtime.cancel_and_wait().await.unwrap_err().code,
            "video_probe_runtime_failed"
        );
    }

    #[tokio::test]
    async fn transient_pending_query_error_is_retried_without_stranding_candidates() {
        let fixture = MultiFixture::new(3);
        let pending = Arc::new(FailFirstPendingQuery {
            index: Arc::clone(&fixture.index),
            failed: AtomicBool::new(false),
        });
        let runtime = VideoIndexRuntime::new_with_queue(
            fixture.active.clone(),
            Arc::clone(&fixture.coordinator),
            Arc::clone(&fixture.index),
            pending,
            Arc::new(CountingReadyProbe {
                calls: AtomicUsize::new(0),
            }),
            Arc::new(DerivedWorkScheduler::default()),
        );
        runtime.notify_after_publication().unwrap();
        fixture.wait_until_terminal().await;
        assert!(
            fixture
                .statuses()
                .into_iter()
                .all(|status| status == VideoProbeStatus::Ready)
        );
        assert_eq!(
            runtime.cancel_and_wait().await.unwrap_err().code,
            "video_probe_index_unavailable"
        );
    }

    #[tokio::test]
    async fn pending_queue_panic_restarts_supervision_and_drains_later_candidates() {
        let fixture = MultiFixture::new(3);
        let pending = Arc::new(PanicFirstPendingQuery {
            index: Arc::clone(&fixture.index),
            panicked: AtomicBool::new(false),
        });
        let runtime = VideoIndexRuntime::new_with_queue(
            fixture.active.clone(),
            Arc::clone(&fixture.coordinator),
            Arc::clone(&fixture.index),
            pending,
            Arc::new(CountingReadyProbe {
                calls: AtomicUsize::new(0),
            }),
            Arc::new(DerivedWorkScheduler::default()),
        );
        runtime.notify_after_publication().unwrap();
        fixture.wait_until_terminal().await;
        assert!(
            fixture
                .statuses()
                .into_iter()
                .all(|status| status == VideoProbeStatus::Ready)
        );
        assert_eq!(
            runtime.cancel_and_wait().await.unwrap_err().code,
            "video_probe_runtime_failed"
        );
    }

    #[tokio::test]
    async fn stale_generation_is_rejected_before_probe_launch() {
        let fixture = Fixture::new();
        fixture
            .coordinator
            .bump_generation(fixture.active.session_id)
            .unwrap();

        probe_video_nodes(
            fixture.active.clone(),
            Arc::clone(&fixture.coordinator),
            Arc::clone(&fixture.index),
            Arc::new(PanicProbe),
            Arc::new(DerivedWorkScheduler::default()),
            CancellationToken::new(),
            vec![fixture.node.clone()],
        )
        .await
        .unwrap();

        assert_eq!(fixture.metadata().probe_status, VideoProbeStatus::Pending);
    }

    #[tokio::test]
    async fn stale_generation_result_never_replaces_the_pending_anchor() {
        let fixture = Fixture::new();
        let probe = Arc::new(BlockedProbe {
            started: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        });
        let worker = {
            let fixture = fixture.clone_handles();
            let probe = probe.clone();
            tokio::spawn(async move {
                probe_video_nodes(
                    fixture.active,
                    fixture.coordinator,
                    fixture.index,
                    probe,
                    Arc::new(DerivedWorkScheduler::default()),
                    CancellationToken::new(),
                    vec![fixture.node],
                )
                .await
            })
        };
        probe.started.notified().await;

        fixture
            .coordinator
            .bump_generation(fixture.active.session_id)
            .unwrap();
        probe.release.notify_one();
        worker.await.unwrap().unwrap();

        assert_eq!(fixture.metadata().probe_status, VideoProbeStatus::Pending);
    }

    #[tokio::test]
    async fn changed_same_generation_node_rejects_the_older_probe_result() {
        let fixture = Fixture::new();
        let probe = Arc::new(BlockedProbe {
            started: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        });
        let worker = {
            let fixture = fixture.clone_handles();
            let probe = probe.clone();
            tokio::spawn(async move {
                probe_video_nodes(
                    fixture.active,
                    fixture.coordinator,
                    fixture.index,
                    probe,
                    Arc::new(DerivedWorkScheduler::default()),
                    CancellationToken::new(),
                    vec![fixture.node],
                )
                .await
            })
        };
        probe.started.notified().await;

        fs::write(fixture.active.root.join("clip.mp4"), b"newer bytes").unwrap();
        let changed = FileNode {
            size: 11,
            modified_ns: fixture.node.modified_ns + 1,
            ..fixture.node.clone()
        };
        fixture
            .index
            .upsert_batch(std::slice::from_ref(&changed), fixture.active.generation)
            .unwrap();
        probe.release.notify_one();
        worker.await.unwrap().unwrap();

        assert_eq!(fixture.metadata().probe_status, VideoProbeStatus::Pending);
    }

    struct Fixture {
        _project: tempfile::TempDir,
        active: ActiveProject,
        coordinator: Arc<TaskCoordinator>,
        index: Arc<SessionIndex>,
        node: FileNode,
    }

    struct FixtureHandles {
        active: ActiveProject,
        coordinator: Arc<TaskCoordinator>,
        index: Arc<SessionIndex>,
        node: FileNode,
    }

    struct MultiFixture {
        _project: tempfile::TempDir,
        active: ActiveProject,
        coordinator: Arc<TaskCoordinator>,
        index: Arc<SessionIndex>,
    }

    impl MultiFixture {
        fn new(count: usize) -> Self {
            let project = tempfile::tempdir().unwrap();
            let root = project.path().canonicalize().unwrap();
            let index = Arc::new(SessionIndex::open(root.join("session.sqlite")).unwrap());
            let coordinator = Arc::new(TaskCoordinator::default());
            let session_id = SessionId::new();
            let generation = coordinator.begin_session(session_id);
            let active = ActiveProject {
                project_id: ProjectId::new(),
                session_id,
                generation,
                root: root.clone(),
                display_name: "fixture".into(),
                access: ProjectAccess::ReadWrite,
            };
            let nodes = (0..count)
                .map(|position| {
                    let name = format!("clip-{position:03}.mp4");
                    let path = root.join(&name);
                    fs::write(&path, format!("candidate-{position}")).unwrap();
                    let metadata = fs::symlink_metadata(&path).unwrap();
                    let relative_path = RelativePath::parse(&name).unwrap();
                    FileNode {
                        entity_id: entity_id_for_metadata(&metadata, &relative_path),
                        relative_path,
                        kind: FileKind::Video,
                        size: metadata.len(),
                        modified_ns: modified_ns(&metadata),
                    }
                })
                .collect::<Vec<_>>();
            index.upsert_batch(&nodes, generation).unwrap();
            Self {
                _project: project,
                active,
                coordinator,
                index,
            }
        }

        async fn wait_until_terminal(&self) {
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    if self
                        .statuses()
                        .into_iter()
                        .all(|status| !matches!(status, VideoProbeStatus::Pending))
                    {
                        return;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("durable pending queue must eventually drain");
        }

        fn statuses(&self) -> Vec<VideoProbeStatus> {
            self.index
                .all_indexed_nodes()
                .unwrap()
                .into_iter()
                .filter_map(|indexed| indexed.video_metadata.map(|metadata| metadata.probe_status))
                .collect()
        }
    }

    impl Fixture {
        fn new() -> Self {
            let project = tempfile::tempdir().unwrap();
            fs::write(project.path().join("clip.mp4"), b"candidate").unwrap();
            let root = project.path().canonicalize().unwrap();
            let index =
                Arc::new(SessionIndex::open(project.path().join("session.sqlite")).unwrap());
            let coordinator = Arc::new(TaskCoordinator::default());
            let session_id = SessionId::new();
            let generation = coordinator.begin_session(session_id);
            let active = ActiveProject {
                project_id: ProjectId::new(),
                session_id,
                generation,
                root,
                display_name: "fixture".into(),
                access: ProjectAccess::ReadWrite,
            };
            let relative_path = RelativePath::parse("clip.mp4").unwrap();
            let metadata = fs::metadata(project.path().join("clip.mp4")).unwrap();
            let node = FileNode {
                entity_id: entity_id_for_metadata(&metadata, &relative_path),
                relative_path,
                kind: FileKind::Video,
                size: metadata.len(),
                modified_ns: modified_ns(&metadata),
            };
            index
                .upsert_batch(std::slice::from_ref(&node), generation)
                .unwrap();
            Self {
                _project: project,
                active,
                coordinator,
                index,
                node,
            }
        }

        fn clone_handles(&self) -> FixtureHandles {
            FixtureHandles {
                active: self.active.clone(),
                coordinator: Arc::clone(&self.coordinator),
                index: Arc::clone(&self.index),
                node: self.node.clone(),
            }
        }

        fn metadata(&self) -> VideoMetadata {
            self.index
                .all_indexed_nodes()
                .unwrap()
                .into_iter()
                .find(|indexed| indexed.node.entity_id == self.node.entity_id)
                .unwrap()
                .video_metadata
                .unwrap()
        }
    }

    fn ready_metadata() -> VideoMetadata {
        VideoMetadata {
            duration_us: Some(2_000_000),
            display_width: Some(1920),
            display_height: Some(1080),
            rotation_degrees: 0,
            frame_rate_millihertz: Some(30_000),
            video_codec: Some("h264".into()),
            audio_codec: None,
            probe_status: VideoProbeStatus::Ready,
        }
    }
}
