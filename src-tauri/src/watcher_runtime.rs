use crate::{
    error::CommandError,
    state::{DesktopEventSink, VideoIndexRuntime, rebuild_derived_nodes},
};
use std::{collections::VecDeque, path::PathBuf, sync::Arc, time::Duration};
use tokio::{sync::watch, task::JoinHandle};
use viewer_application::{
    ActiveProject, BrowseIndexPort, ClockPort, ImagePort, WatchSubscription, WatcherError,
    WatcherPort,
    scheduler::{DerivedWorkClass, DerivedWorkScheduler, TaskCoordinator},
    watcher::{ReconcileRequest, WATCHER_DEBOUNCE_MS},
};
use viewer_domain::{SessionId, search::Generation};
use viewer_infrastructure::scan::{
    reconcile::{ExpectedChangeLedger, ReconcilePlanner},
    reconcile_service::{ProjectReconcileError, ProjectReconciler},
};
use viewer_infrastructure::search::index::SessionIndex;

const MAX_QUEUED_RECONCILES: usize = 64;

pub struct WatcherRuntime {
    subscription: Option<Box<dyn WatchSubscription>>,
    stop: Option<watch::Sender<bool>>,
    task: Option<JoinHandle<()>>,
}

pub(crate) struct WatcherDerivedServices {
    pub(crate) active: ActiveProject,
    pub(crate) coordinator: Arc<TaskCoordinator>,
    pub(crate) index: Arc<SessionIndex>,
    pub(crate) image: Arc<dyn ImagePort>,
    pub(crate) events: Arc<dyn DesktopEventSink>,
    pub(crate) scheduler: Arc<DerivedWorkScheduler>,
    pub(crate) video_index: Arc<VideoIndexRuntime>,
}

impl WatcherDerivedServices {
    async fn rebuild(&self, roots: &[PathBuf]) -> Result<(), CommandError> {
        if !self
            .coordinator
            .is_publishable(self.active.session_id, self.active.generation)
        {
            return Ok(());
        }
        let nodes =
            BrowseIndexPort::descendants(self.index.as_ref(), None).map_err(CommandError::from)?;
        let nodes: Vec<_> = nodes
            .into_iter()
            .filter(|node| {
                let absolute = self.active.root.join(node.relative_path.as_str());
                roots.iter().any(|root| absolute.starts_with(root))
            })
            .collect();
        rebuild_derived_nodes(
            self.active.clone(),
            Arc::clone(&self.coordinator),
            Arc::clone(&self.index),
            Arc::clone(&self.image),
            Arc::clone(&self.events),
            Arc::clone(&self.scheduler),
            nodes.clone(),
        )
        .await?;
        self.video_index.notify_after_publication()?;
        Ok(())
    }
}

impl WatcherRuntime {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn start(
        root: PathBuf,
        session_id: SessionId,
        generation: Generation,
        watcher: Arc<dyn WatcherPort>,
        ledger: ExpectedChangeLedger,
        reconciler: Arc<ProjectReconciler>,
        clock: Arc<dyn ClockPort>,
        events: Arc<dyn DesktopEventSink>,
        mut scan_ready: watch::Receiver<bool>,
        derived: Option<WatcherDerivedServices>,
    ) -> Result<Self, WatcherError> {
        let (sink, mut incoming) = tokio::sync::mpsc::channel(64);
        let (stop, mut stop_requested) = watch::channel(false);
        let subscription = watcher.watch(&root, sink)?;
        let watched_root = root.clone();
        let mut planner =
            ReconcilePlanner::new(root, session_id, generation, ledger).map_err(|error| {
                WatcherError::Backend {
                    path: PathBuf::new(),
                    message: error.to_string(),
                }
            })?;
        let task = tokio::spawn(async move {
            let mut queued = VecDeque::<ReconcileRequest>::new();
            let mut ready = *scan_ready.borrow();
            let mut tick = tokio::time::interval(Duration::from_millis(
                WATCHER_DEBOUNCE_MS.saturating_div(2).max(25),
            ));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    changed = stop_requested.changed() => {
                        if changed.is_err() || *stop_requested.borrow() { break; }
                    }
                    batch = incoming.recv() => {
                        let Some(batch) = batch else { break };
                        let now = u64::try_from(clock.unix_millis()).unwrap_or(0);
                        for event in batch {
                            if let Some(request) = planner.push(event, now) {
                                enqueue_reconcile(
                                    &mut queued,
                                    request,
                                    &watched_root,
                                    session_id,
                                    generation,
                                );
                            }
                        }
                    }
                    _ = tick.tick() => {
                        let now = u64::try_from(clock.unix_millis()).unwrap_or(0);
                        if let Some(request) = planner.flush(now) {
                            enqueue_reconcile(
                                &mut queued,
                                request,
                                &watched_root,
                                session_id,
                                generation,
                            );
                        }
                    }
                    changed = scan_ready.changed(), if !ready => {
                        if changed.is_err() && !*scan_ready.borrow() { break; }
                        ready = *scan_ready.borrow();
                    }
                }
                if ready {
                    while let Some(request) = queued.pop_front() {
                        if *stop_requested.borrow() {
                            return;
                        }
                        let reason = request.reason;
                        let roots = request.roots.clone();
                        let publication_permit = if let Some(derived) = derived.as_ref() {
                            Some(
                                derived
                                    .scheduler
                                    .acquire(DerivedWorkClass::FolderPublication)
                                    .await,
                            )
                        } else {
                            None
                        };
                        let result = reconciler.reconcile(request).await;
                        drop(publication_permit);
                        if *stop_requested.borrow() {
                            return;
                        }
                        let mut summary = match result {
                            Ok(summary) => summary,
                            Err(ProjectReconcileError::StaleSession) => continue,
                            Err(_) => viewer_application::watcher::ReconcileSummary {
                                reason,
                                added: 0,
                                removed: 0,
                                modified: 0,
                                moved: 0,
                                marker_paths_moved: 0,
                                failed: 1,
                            },
                        };
                        if let Some(derived) = derived.as_ref()
                            && derived.rebuild(&roots).await.is_err()
                        {
                            summary.failed = summary.failed.saturating_add(1);
                        }
                        if *stop_requested.borrow() {
                            return;
                        }
                        events.emit_project_changed(session_id, generation, summary);
                    }
                }
            }
        });
        Ok(Self {
            subscription: Some(subscription),
            stop: Some(stop),
            task: Some(task),
        })
    }

    pub async fn stop(&mut self) {
        self.subscription.take();
        if let Some(stop) = self.stop.take() {
            stop.send_replace(true);
        }
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}

fn enqueue_reconcile(
    queued: &mut VecDeque<ReconcileRequest>,
    request: ReconcileRequest,
    root: &std::path::Path,
    session_id: SessionId,
    generation: Generation,
) {
    if queued.len() < MAX_QUEUED_RECONCILES {
        queued.push_back(request);
        return;
    }
    queued.clear();
    queued.push_back(ReconcileRequest {
        session_id,
        generation,
        roots: vec![root.to_path_buf()],
        reason: viewer_application::watcher::ReconcileReason::Overflow,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::Path,
        sync::{
            Condvar, Mutex,
            atomic::{AtomicBool, AtomicI64, Ordering},
        },
    };
    use tokio_util::sync::CancellationToken;
    use viewer_application::{
        ImageArtifact, ImageError, ImageRequest, ProjectAccess, WatcherSink,
        metadata::{
            FilePathMove, MarkerChange, MarkerPatch, MarkerRestore, MarkerStoreError, MarkerTarget,
            PortableMarker, PortableMetadataPort,
        },
        scheduler::{DerivedWorkClass, DerivedWorkScheduler, TaskCoordinator},
        watcher::{ReconcileSummary, WatcherEvent},
    };
    use viewer_domain::{
        EntityId, ProjectId, RelativePath,
        file::{FileKind, FileNode, TextIndexStatus},
        image::ImageProbe,
        video::{VideoMetadata, VideoProbeStatus},
    };
    use viewer_infrastructure::{
        portable::{PortableMarkerStore, PortableProjectMetadata},
        search::index::SessionIndex,
        video_probe::{MediaFileIdentity, VideoMetadataProbe, VideoProbeError},
    };

    #[derive(Default)]
    struct TestWatcher {
        sink: Mutex<Option<WatcherSink>>,
    }

    struct TestSubscription;
    impl WatchSubscription for TestSubscription {}

    struct UnusedImage;

    struct ReadyVideoProbe;

    struct BlockingFirstVideoProbe {
        blocked: AtomicBool,
        started: tokio::sync::Notify,
        release: tokio::sync::Notify,
    }

    #[async_trait::async_trait]
    impl VideoMetadataProbe for ReadyVideoProbe {
        async fn probe(
            &self,
            _canonical_path: &Path,
            cancellation: CancellationToken,
        ) -> Result<VideoMetadata, VideoProbeError> {
            if cancellation.is_cancelled() {
                return Err(VideoProbeError::Cancelled);
            }
            Ok(VideoMetadata {
                duration_us: Some(1_000_000),
                display_width: Some(640),
                display_height: Some(360),
                rotation_degrees: 0,
                frame_rate_millihertz: Some(24_000),
                video_codec: Some("h264".into()),
                audio_codec: None,
                probe_status: VideoProbeStatus::Ready,
            })
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

    #[async_trait::async_trait]
    impl VideoMetadataProbe for BlockingFirstVideoProbe {
        async fn probe(
            &self,
            _canonical_path: &Path,
            cancellation: CancellationToken,
        ) -> Result<VideoMetadata, VideoProbeError> {
            if !self.blocked.swap(true, Ordering::AcqRel) {
                self.started.notify_one();
                tokio::select! {
                    _ = cancellation.cancelled() => return Err(VideoProbeError::Cancelled),
                    _ = self.release.notified() => {}
                }
            }
            ReadyVideoProbe.probe(_canonical_path, cancellation).await
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

    #[async_trait::async_trait]
    impl ImagePort for UnusedImage {
        async fn probe(&self, _source: &Path) -> Result<ImageProbe, ImageError> {
            Err(ImageError::Unsupported)
        }

        async fn render(&self, _request: ImageRequest) -> Result<ImageArtifact, ImageError> {
            Err(ImageError::Unsupported)
        }

        async fn cancel_session(&self, _session_id: SessionId) {}
    }

    impl WatcherPort for TestWatcher {
        fn watch(
            &self,
            _root: &Path,
            sink: WatcherSink,
        ) -> Result<Box<dyn WatchSubscription>, WatcherError> {
            *self.sink.lock().unwrap() = Some(sink);
            Ok(Box::new(TestSubscription))
        }
    }

    #[derive(Default)]
    struct AdjustableClock(AtomicI64);

    impl ClockPort for AdjustableClock {
        fn unix_millis(&self) -> i64 {
            self.0.load(Ordering::Acquire)
        }
    }

    struct BlockingMarkers {
        delegate: Arc<PortableMarkerStore>,
        started: tokio::sync::Notify,
        release: (Mutex<bool>, Condvar),
        finished: AtomicBool,
    }

    impl BlockingMarkers {
        fn release(&self) {
            let mut released = self.release.0.lock().unwrap();
            *released = true;
            self.release.1.notify_all();
        }
    }

    impl PortableMetadataPort for BlockingMarkers {
        fn markers_for_paths(
            &self,
            paths: &[RelativePath],
        ) -> Result<Vec<PortableMarker>, MarkerStoreError> {
            self.delegate.markers_for_paths(paths)
        }

        fn apply_batch(
            &self,
            targets: &[MarkerTarget],
            patch: MarkerPatch,
            updated_at_ms: i64,
        ) -> Result<Vec<MarkerChange>, MarkerStoreError> {
            self.delegate.apply_batch(targets, patch, updated_at_ms)
        }

        fn restore_batch(
            &self,
            restores: &[MarkerRestore],
            updated_at_ms: i64,
        ) -> Result<Vec<MarkerChange>, MarkerStoreError> {
            self.delegate.restore_batch(restores, updated_at_ms)
        }

        fn move_paths(
            &self,
            moves: &[FilePathMove],
            case_sensitive: bool,
            updated_at_ms: i64,
        ) -> Result<usize, MarkerStoreError> {
            self.started.notify_one();
            let mut released = self.release.0.lock().unwrap();
            while !*released {
                released = self.release.1.wait(released).unwrap();
            }
            drop(released);
            let result = self
                .delegate
                .move_paths(moves, case_sensitive, updated_at_ms);
            self.finished.store(true, Ordering::Release);
            result
        }
    }

    #[derive(Default)]
    struct RecordingEvents(Mutex<Vec<ReconcileSummary>>);

    impl DesktopEventSink for RecordingEvents {
        fn emit_scan(&self, _event: crate::dto::ScanEventDto) {}

        fn emit_project_changed(
            &self,
            _session_id: SessionId,
            _generation: Generation,
            summary: ReconcileSummary,
        ) {
            self.0.lock().unwrap().push(summary);
        }
    }

    fn filesystem_node(root: &Path, relative: &str) -> FileNode {
        use std::os::unix::fs::MetadataExt;
        let metadata = fs::symlink_metadata(root.join(relative)).unwrap();
        FileNode {
            entity_id: EntityId::from_u128(
                (u128::from(metadata.dev()) << 64) | u128::from(metadata.ino()),
            ),
            relative_path: RelativePath::parse(relative).unwrap(),
            kind: FileKind::Text,
            size: metadata.len(),
            modified_ns: i128::from(metadata.mtime()) * 1_000_000_000
                + i128::from(metadata.mtime_nsec()),
        }
    }

    #[tokio::test]
    async fn stop_drains_inflight_marker_work_and_rejects_stale_publication() {
        let project = tempfile::tempdir().unwrap();
        fs::write(project.path().join("before.txt"), b"marker move").unwrap();
        let portable =
            PortableProjectMetadata::open(project.path(), ProjectAccess::ReadWrite, 1).unwrap();
        let database = portable.database_path().unwrap().to_path_buf();
        drop(portable);
        let delegate = Arc::new(PortableMarkerStore::open(&database, true).unwrap());
        let markers = Arc::new(BlockingMarkers {
            delegate,
            started: tokio::sync::Notify::new(),
            release: (Mutex::new(false), Condvar::new()),
            finished: AtomicBool::new(false),
        });
        let index =
            Arc::new(SessionIndex::open(project.path().join(".viewer/session.sqlite")).unwrap());
        index
            .upsert_batch(
                &[filesystem_node(project.path(), "before.txt")],
                viewer_domain::search::Generation::new(1),
            )
            .unwrap();
        let coordinator = Arc::new(TaskCoordinator::default());
        let session_id = SessionId::new();
        let generation = coordinator.begin_session(session_id);
        let clock = Arc::new(AdjustableClock::default());
        let scheduler = Arc::new(DerivedWorkScheduler::default());
        let active_probe = scheduler.acquire(DerivedWorkClass::VideoProbe).await;
        let (launched_probe, mut probe_launch) = tokio::sync::oneshot::channel();
        let next_probe = {
            let scheduler = Arc::clone(&scheduler);
            tokio::spawn(async move {
                let permit = scheduler.acquire(DerivedWorkClass::VideoProbe).await;
                let _ = launched_probe.send(());
                permit
            })
        };
        let reconciler = Arc::new(
            ProjectReconciler::new(
                project.path(),
                Arc::clone(&coordinator),
                Arc::clone(&index),
                Some(markers.clone() as Arc<dyn PortableMetadataPort>),
                clock.clone(),
                Arc::new(tokio::sync::Mutex::new(())),
                true,
            )
            .unwrap(),
        );
        let watcher = Arc::new(TestWatcher::default());
        let events = Arc::new(RecordingEvents::default());
        let (_scan_ready, scan_ready_rx) = watch::channel(true);
        let active = ActiveProject {
            project_id: ProjectId::new(),
            session_id,
            generation,
            root: project.path().canonicalize().unwrap(),
            display_name: "fixture".into(),
            access: ProjectAccess::ReadWrite,
        };
        let video_index = Arc::new(VideoIndexRuntime::new(
            active.clone(),
            Arc::clone(&coordinator),
            Arc::clone(&index),
            Arc::new(ReadyVideoProbe),
            Arc::clone(&scheduler),
        ));
        let mut runtime = WatcherRuntime::start(
            project.path().to_path_buf(),
            session_id,
            generation,
            watcher.clone(),
            ExpectedChangeLedger::default(),
            reconciler,
            clock.clone(),
            events.clone(),
            scan_ready_rx,
            Some(WatcherDerivedServices {
                active,
                coordinator: Arc::clone(&coordinator),
                index: Arc::clone(&index),
                image: Arc::new(UnusedImage),
                events: Arc::clone(&events) as Arc<dyn DesktopEventSink>,
                scheduler: Arc::clone(&scheduler),
                video_index: Arc::clone(&video_index),
            }),
        )
        .unwrap();
        let old = fs::canonicalize(project.path().join("before.txt")).unwrap();
        let new = project.path().join("after.txt");
        fs::rename(&old, &new).unwrap();
        let sink = watcher.sink.lock().unwrap().as_ref().unwrap().clone();
        sink.send(vec![WatcherEvent::renamed(old, new)])
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        clock.0.store(1_000, Ordering::Release);
        tokio::time::timeout(Duration::from_secs(2), markers.started.notified())
            .await
            .expect("reconcile should reach the blocking marker commit");

        drop(active_probe);
        let probe_launched_during_publication =
            tokio::time::timeout(Duration::from_millis(250), &mut probe_launch)
                .await
                .is_ok();

        coordinator.cancel_session(session_id);
        let stopping = tokio::spawn(async move { runtime.stop().await });
        tokio::task::yield_now().await;
        let stopped_before_drain = stopping.is_finished();
        let marker_finished_before_release = markers.finished.load(Ordering::Acquire);
        markers.release();
        tokio::time::timeout(Duration::from_secs(2), stopping)
            .await
            .expect("watcher stop should finish after the marker worker drains")
            .unwrap();
        if !probe_launched_during_publication {
            tokio::time::timeout(Duration::from_secs(2), probe_launch)
                .await
                .expect("next probe should launch after watcher publication")
                .unwrap();
        }
        drop(next_probe.await.unwrap());

        assert!(
            !stopped_before_drain,
            "stop must wait for an in-flight blocking marker commit"
        );
        assert!(!marker_finished_before_release);
        assert!(markers.finished.load(Ordering::Acquire));
        assert!(events.0.lock().unwrap().is_empty());
        assert!(
            !probe_launched_during_publication,
            "next probe launched while watcher publication was in progress"
        );
        video_index.cancel_and_wait().await.unwrap();
    }

    #[tokio::test]
    async fn watcher_publication_never_blocks_behind_more_than_legacy_video_queue_capacity() {
        let project = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(project.path()).unwrap();
        fs::create_dir(root.join(".viewer")).unwrap();
        let index = Arc::new(SessionIndex::open(root.join(".viewer/session.sqlite")).unwrap());
        let coordinator = Arc::new(TaskCoordinator::default());
        let session_id = SessionId::new();
        let generation = coordinator.begin_session(session_id);
        let clock = Arc::new(AdjustableClock::default());
        let scheduler = Arc::new(DerivedWorkScheduler::default());
        let reconciler = Arc::new(
            ProjectReconciler::new(
                &root,
                Arc::clone(&coordinator),
                Arc::clone(&index),
                None,
                clock.clone(),
                Arc::new(tokio::sync::Mutex::new(())),
                true,
            )
            .unwrap(),
        );
        let watcher = Arc::new(TestWatcher::default());
        let events = Arc::new(RecordingEvents::default());
        let (_scan_ready, scan_ready_rx) = watch::channel(true);
        let active = ActiveProject {
            project_id: ProjectId::new(),
            session_id,
            generation,
            root: root.clone(),
            display_name: "fixture".into(),
            access: ProjectAccess::ReadWrite,
        };
        let probe = Arc::new(BlockingFirstVideoProbe {
            blocked: AtomicBool::new(false),
            started: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        });
        let video_index = Arc::new(VideoIndexRuntime::new(
            active.clone(),
            Arc::clone(&coordinator),
            Arc::clone(&index),
            probe.clone(),
            Arc::clone(&scheduler),
        ));
        let mut runtime = WatcherRuntime::start(
            root.clone(),
            session_id,
            generation,
            watcher.clone(),
            ExpectedChangeLedger::default(),
            reconciler,
            clock.clone(),
            events.clone(),
            scan_ready_rx,
            Some(WatcherDerivedServices {
                active,
                coordinator,
                index: Arc::clone(&index),
                image: Arc::new(UnusedImage),
                events: events.clone(),
                scheduler,
                video_index: Arc::clone(&video_index),
            }),
        )
        .unwrap();
        let note = root.join("note.txt");
        fs::write(&note, b"watcher searchable text").unwrap();
        let sink = watcher.sink.lock().unwrap().as_ref().unwrap().clone();
        let mut video_entity_ids = Vec::new();
        for candidate in 0..6 {
            let clip = root.join(format!("clip-{candidate}.mp4"));
            fs::write(&clip, format!("watcher video candidate {candidate}")).unwrap();
            video_entity_ids
                .push(filesystem_node(&root, &format!("clip-{candidate}.mp4")).entity_id);
            let mut batch = vec![WatcherEvent::added(clip)];
            if candidate == 0 {
                batch.push(WatcherEvent::added(note.clone()));
            }
            sink.send(batch).await.unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
            clock
                .0
                .store(i64::from(candidate + 1) * 1_000, Ordering::Release);
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    if events.0.lock().unwrap().len() > candidate as usize {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("watcher publication must not wait for the blocked video worker");
        }
        probe.started.notified().await;
        assert_eq!(events.0.lock().unwrap().len(), 6);
        let entity_id = filesystem_node(&root, "note.txt").entity_id;
        assert!(video_entity_ids.iter().all(|entity_id| {
            index.indexed_node(*entity_id).unwrap().is_some_and(|node| {
                node.video_metadata
                    .is_some_and(|metadata| metadata.probe_status == VideoProbeStatus::Pending)
            })
        }));

        probe.release.notify_one();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if index
                    .indexed_node(entity_id)
                    .unwrap()
                    .is_some_and(|node| node.text_status == TextIndexStatus::Ready)
                    && video_entity_ids.iter().all(|entity_id| {
                        index.indexed_node(*entity_id).unwrap().is_some_and(|node| {
                            node.video_metadata.is_some_and(|metadata| {
                                metadata.probe_status == VideoProbeStatus::Ready
                            })
                        })
                    })
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("watcher-derived text and video should become ready");
        runtime.stop().await;
        video_index.cancel_and_wait().await.unwrap();
    }
}
