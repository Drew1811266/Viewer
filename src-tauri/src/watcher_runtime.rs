use crate::state::DesktopEventSink;
use std::{collections::VecDeque, path::PathBuf, sync::Arc, time::Duration};
use tokio::{sync::watch, task::JoinHandle};
use viewer_application::{
    ClockPort, WatchSubscription, WatcherError, WatcherPort,
    watcher::{ReconcileRequest, WATCHER_DEBOUNCE_MS},
};
use viewer_domain::{SessionId, search::Generation};
use viewer_infrastructure::scan::{
    reconcile::{ExpectedChangeLedger, ReconcilePlanner},
    reconcile_service::ProjectReconciler,
};

const MAX_QUEUED_RECONCILES: usize = 64;

pub struct WatcherRuntime {
    subscription: Option<Box<dyn WatchSubscription>>,
    task: Option<JoinHandle<()>>,
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
    ) -> Result<Self, WatcherError> {
        let (sink, mut incoming) = tokio::sync::mpsc::channel(64);
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
                        let reason = request.reason;
                        let summary = reconciler.reconcile(request).await.unwrap_or(
                            viewer_application::watcher::ReconcileSummary {
                                reason,
                                added: 0,
                                removed: 0,
                                modified: 0,
                                moved: 0,
                                marker_paths_moved: 0,
                                failed: 1,
                            },
                        );
                        events.emit_project_changed(session_id, generation, summary);
                    }
                }
            }
        });
        Ok(Self {
            subscription: Some(subscription),
            task: Some(task),
        })
    }

    pub async fn stop(&mut self) {
        self.subscription.take();
        if let Some(task) = self.task.take() {
            task.abort();
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
