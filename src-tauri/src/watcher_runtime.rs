use crate::{
    error::CommandError,
    state::{DesktopEventSink, rebuild_derived_nodes},
};
use std::{collections::VecDeque, path::PathBuf, sync::Arc, time::Duration};
use tokio::{sync::watch, task::JoinHandle};
use viewer_application::{
    ActiveProject, BrowseIndexPort, ClockPort, ImagePort, WatchSubscription, WatcherError,
    WatcherPort,
    scheduler::TaskCoordinator,
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

pub struct WatcherDerivedServices {
    pub active: ActiveProject,
    pub coordinator: Arc<TaskCoordinator>,
    pub index: Arc<SessionIndex>,
    pub image: Arc<dyn ImagePort>,
    pub events: Arc<dyn DesktopEventSink>,
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
        let nodes = nodes
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
            nodes,
        )
        .await
    }
}

impl WatcherRuntime {
    #[allow(clippy::too_many_arguments)]
    pub fn start(
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
                        let result = reconciler.reconcile(request).await;
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
    use viewer_application::{
        ImageArtifact, ImageError, ImageRequest, ProjectAccess, WatcherSink,
        metadata::{
            FilePathMove, MarkerChange, MarkerPatch, MarkerRestore, MarkerStoreError, MarkerTarget,
            PortableMarker, PortableMetadataPort,
        },
        scheduler::TaskCoordinator,
        watcher::{ReconcileSummary, WatcherEvent},
    };
    use viewer_domain::{
        EntityId, ProjectId, RelativePath,
        file::{FileKind, FileNode, TextIndexStatus},
        image::ImageProbe,
    };
    use viewer_infrastructure::{
        portable::{PortableMarkerStore, PortableProjectMetadata},
        search::index::SessionIndex,
    };

    #[derive(Default)]
    struct TestWatcher {
        sink: Mutex<Option<WatcherSink>>,
    }

    struct TestSubscription;
    impl WatchSubscription for TestSubscription {}

    struct UnusedImage;

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
            .upsert_batch(&[filesystem_node(project.path(), "before.txt")])
            .unwrap();
        let coordinator = Arc::new(TaskCoordinator::default());
        let session_id = SessionId::new();
        let generation = coordinator.begin_session(session_id);
        let clock = Arc::new(AdjustableClock::default());
        let reconciler = Arc::new(
            ProjectReconciler::new(
                project.path(),
                Arc::clone(&coordinator),
                index,
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
            None,
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

        assert!(
            !stopped_before_drain,
            "stop must wait for an in-flight blocking marker commit"
        );
        assert!(!marker_finished_before_release);
        assert!(markers.finished.load(Ordering::Acquire));
        assert!(events.0.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn watcher_rebuilds_text_derivation_in_its_single_bounded_session() {
        let project = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(project.path()).unwrap();
        fs::create_dir(root.join(".viewer")).unwrap();
        let index = Arc::new(SessionIndex::open(root.join(".viewer/session.sqlite")).unwrap());
        let coordinator = Arc::new(TaskCoordinator::default());
        let session_id = SessionId::new();
        let generation = coordinator.begin_session(session_id);
        let clock = Arc::new(AdjustableClock::default());
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
                active: ActiveProject {
                    project_id: ProjectId::new(),
                    session_id,
                    generation,
                    root: root.clone(),
                    display_name: "fixture".into(),
                    access: ProjectAccess::ReadWrite,
                },
                coordinator,
                index: Arc::clone(&index),
                image: Arc::new(UnusedImage),
                events,
            }),
        )
        .unwrap();
        let note = root.join("note.txt");
        fs::write(&note, b"watcher searchable text").unwrap();
        let sink = watcher.sink.lock().unwrap().as_ref().unwrap().clone();
        sink.send(vec![WatcherEvent::added(note)]).await.unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        clock.0.store(1_000, Ordering::Release);
        let entity_id = filesystem_node(&root, "note.txt").entity_id;

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if index
                    .indexed_node(entity_id)
                    .unwrap()
                    .is_some_and(|node| node.text_status == TextIndexStatus::Ready)
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("watcher-derived text should become searchable");
        runtime.stop().await;
    }
}
