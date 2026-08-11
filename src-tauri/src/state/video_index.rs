use super::{is_stale_derived_write_error, preview::validated_indexed_source};
use crate::error::CommandError;
use std::sync::{Arc, Mutex};
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
    video_probe::{VideoMetadataProbe, VideoProbeError},
};

pub(crate) struct VideoIndexRuntime {
    cancellation: CancellationToken,
    queue: tokio::sync::mpsc::Sender<Vec<FileNode>>,
    worker: Mutex<Option<JoinHandle<Result<(), CommandError>>>>,
}

impl VideoIndexRuntime {
    pub(crate) fn new(
        active: ActiveProject,
        coordinator: Arc<TaskCoordinator>,
        index: Arc<SessionIndex>,
        probe: Arc<dyn VideoMetadataProbe>,
        scheduler: Arc<DerivedWorkScheduler>,
    ) -> Self {
        let cancellation = CancellationToken::new();
        let (queue, mut incoming) =
            tokio::sync::mpsc::channel(DerivedWorkClass::VideoProbe.queue_capacity());
        let worker_cancellation = cancellation.clone();
        let worker = tokio::spawn(async move {
            loop {
                let nodes = tokio::select! {
                    _ = worker_cancellation.cancelled() => return Ok(()),
                    nodes = incoming.recv() => match nodes {
                        Some(nodes) => nodes,
                        None => return Ok(()),
                    },
                };
                probe_video_nodes(
                    active.clone(),
                    Arc::clone(&coordinator),
                    Arc::clone(&index),
                    Arc::clone(&probe),
                    Arc::clone(&scheduler),
                    worker_cancellation.clone(),
                    nodes,
                )
                .await?;
            }
        });
        Self {
            cancellation,
            queue,
            worker: Mutex::new(Some(worker)),
        }
    }

    pub(crate) async fn enqueue_after_publication(&self, nodes: Vec<FileNode>) {
        if self.cancellation.is_cancelled() {
            return;
        }
        tokio::select! {
            _ = self.cancellation.cancelled() => {}
            _ = self.queue.send(nodes) => {}
        }
    }

    pub(crate) async fn cancel_and_wait(&self) {
        self.cancellation.cancel();
        let worker = self
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(worker) = worker {
            let _ = worker.await;
        }
    }
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

        let metadata = match validated_indexed_source(&active, &node) {
            Ok((source, _, _)) => match probe.probe(&source, cancellation.clone()).await {
                Ok(metadata) => metadata,
                Err(VideoProbeError::Failed(kind)) => failed_metadata(kind),
                Err(VideoProbeError::Cancelled) => return Ok(()),
            },
            Err(()) => failed_metadata(source_failure(&active, &node)),
        };
        drop(permit);

        if cancellation.is_cancelled() {
            return Ok(());
        }
        let persisted = coordinator.run_if_current(active.session_id, active.generation, || {
            if cancellation.is_cancelled() {
                return Ok(false);
            }
            index.replace_video_metadata_if_current_node(&node, &metadata, active.generation)
        });
        let Some(result) = persisted else {
            return Ok(());
        };
        if let Err(error) = result {
            if is_stale_derived_write_error(&error) {
                continue;
            }
            return Err(CommandError::from(error));
        }
    }
    Ok(())
}

fn source_failure(active: &ActiveProject, node: &FileNode) -> VideoFailureKind {
    match std::fs::symlink_metadata(active.root.join(node.relative_path.as_str())) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => VideoFailureKind::Missing,
        _ => VideoFailureKind::Unreadable,
    }
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
    use super::probe_video_nodes;
    use crate::state::entity_id_for_metadata;
    use async_trait::async_trait;
    use std::{fs, path::Path, sync::Arc};
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
        video_probe::{VideoMetadataProbe, VideoProbeError},
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
    }

    struct PanicProbe;

    #[async_trait]
    impl VideoMetadataProbe for PanicProbe {
        async fn probe(
            &self,
            _canonical_path: &Path,
            _cancellation: CancellationToken,
        ) -> Result<VideoMetadata, VideoProbeError> {
            panic!("stale work launched the probe process")
        }
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
                size: 9,
                modified_ns: 1,
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
