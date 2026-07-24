use super::publication::OperationRecord;
use crate::state::DesktopEventSink;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};
use tokio::sync::{Mutex as AsyncMutex, oneshot, watch};
use viewer_application::file_commands::{
    BatchId, BatchProgress, BatchResultPage, ConflictResolution, FileCommand, FileCommandItem,
    FileCommandKind, FileCommandPreflight, FileCommandPreflightState, FileCommandService,
    FileCommandServiceError,
};
use viewer_domain::{SessionId, operation::BatchLifecycle, search::Generation};

const MAX_RETAINED_BATCHES: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperationStarted {
    pub batch_id: BatchId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum OperationRuntimeError {
    #[error(transparent)]
    Service(#[from] FileCommandServiceError),
    #[error("operation batch was not found in this session")]
    BatchNotFound,
    #[error("operation batch failed before producing results")]
    BatchFailed,
}

#[derive(Default)]
struct OperationRuntimeState {
    active: Option<BatchId>,
    order: Vec<BatchId>,
    records: HashMap<BatchId, Arc<OperationRecord>>,
}

pub struct OperationRuntime {
    service: Arc<FileCommandService>,
    events: Arc<dyn DesktopEventSink>,
    start_gate: AsyncMutex<()>,
    state: Arc<Mutex<OperationRuntimeState>>,
}

impl OperationRuntime {
    pub fn new(service: Arc<FileCommandService>, events: Arc<dyn DesktopEventSink>) -> Self {
        Self {
            service,
            events,
            start_gate: AsyncMutex::new(()),
            state: Arc::new(Mutex::new(OperationRuntimeState::default())),
        }
    }

    pub async fn preflight(
        &self,
        command: FileCommand,
    ) -> Result<FileCommandPreflight, OperationRuntimeError> {
        self.service.preview(command).await.map_err(Into::into)
    }

    pub async fn start(
        &self,
        session_id: SessionId,
        generation: Generation,
        kind: FileCommandKind,
        items: Vec<FileCommandItem>,
        resolutions: Vec<ConflictResolution>,
    ) -> Result<OperationStarted, OperationRuntimeError> {
        let _start_guard = self.start_gate.lock().await;
        if self.lock_state().active.is_some() {
            return Err(FileCommandServiceError::BatchActive.into());
        }
        let preflight = self
            .service
            .preflight(FileCommand {
                session_id,
                generation,
                kind,
                items,
            })
            .await?;
        if let Err(error) = validate_conflict_resolutions(preflight.rows(), &resolutions) {
            self.service.discard_preflight(preflight.batch_id()).await?;
            return Err(error.into());
        }
        if !preflight.is_executable() {
            self.service.discard_preflight(preflight.batch_id()).await?;
            return Err(FileCommandServiceError::PreflightBlocked.into());
        }
        let batch_id = preflight.batch_id();
        let initial = preflight.initial_progress();
        let record = Arc::new(OperationRecord::new(initial.clone()));
        {
            let mut state = self.lock_state();
            state.active = Some(batch_id);
            state.order.push(batch_id);
            state.records.insert(batch_id, Arc::clone(&record));
            while state.order.len() > MAX_RETAINED_BATCHES {
                let expired = state.order.remove(0);
                if state.active != Some(expired) {
                    state.records.remove(&expired);
                }
            }
        }
        self.events.emit_operation(session_id, generation, initial);

        let (progress_tx, mut progress_rx) = watch::channel(record.progress());
        let (stop_progress_tx, mut stop_progress_rx) = oneshot::channel();
        let progress_record = Arc::clone(&record);
        let progress_events = Arc::clone(&self.events);
        let progress_forwarder = tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    _ = &mut stop_progress_rx => break,
                    changed = progress_rx.changed() => {
                        if changed.is_err() {
                            break;
                        }
                        let progress = progress_rx.borrow_and_update().clone();
                        if progress.lifecycle != BatchLifecycle::Completed {
                            progress_record.publish(progress.clone());
                            progress_events.emit_operation(session_id, generation, progress);
                        }
                    }
                }
            }
        });

        let service = Arc::clone(&self.service);
        let state = Arc::clone(&self.state);
        let terminal_events = Arc::clone(&self.events);
        tokio::spawn(async move {
            let result = service
                .execute(preflight, &resolutions, Some(progress_tx))
                .await;
            let completed = match &result {
                Ok(summary) => BatchProgress {
                    batch_id,
                    lifecycle: BatchLifecycle::Completed,
                    requested: summary.requested(),
                    completed: summary.completed(),
                    failed: summary.failed(),
                    skipped: summary.skipped(),
                    cancelled: summary.cancelled(),
                    active_entity_id: None,
                },
                Err(_) => {
                    let requested = record.progress().requested;
                    BatchProgress {
                        batch_id,
                        lifecycle: BatchLifecycle::Completed,
                        requested,
                        completed: 0,
                        failed: requested,
                        skipped: 0,
                        cancelled: 0,
                        active_entity_id: None,
                    }
                }
            };
            let _ = stop_progress_tx.send(());
            let _ = progress_forwarder.await;
            record.publish(completed.clone());
            record.store_result(result);
            {
                let mut state = state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if state.active == Some(batch_id) {
                    state.active = None;
                }
            }
            terminal_events.emit_operation(session_id, generation, completed);
            record.mark_complete();
        });
        Ok(OperationStarted { batch_id })
    }

    pub fn status(&self, batch_id: BatchId) -> Result<BatchProgress, OperationRuntimeError> {
        Ok(self.record(batch_id)?.progress())
    }

    pub fn results(
        &self,
        batch_id: BatchId,
        offset: usize,
        limit: usize,
    ) -> Result<BatchResultPage, OperationRuntimeError> {
        let record = self.record(batch_id)?;
        if record
            .failure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_some()
        {
            return Err(OperationRuntimeError::BatchFailed);
        }
        record
            .summary
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map(|summary| summary.result_page(offset, limit))
            .ok_or(OperationRuntimeError::BatchNotFound)
    }

    pub fn cancel(&self, batch_id: BatchId) -> Result<bool, OperationRuntimeError> {
        let record = self.record(batch_id)?;
        let accepted = self.lock_state().active == Some(batch_id) && !record.is_complete();
        if accepted {
            self.service.request_cancel(batch_id);
        }
        Ok(accepted)
    }

    pub fn active_batch(&self) -> Option<BatchId> {
        self.lock_state().active
    }

    pub async fn wait(&self, batch_id: BatchId) -> Result<(), OperationRuntimeError> {
        let record = self.record(batch_id)?;
        let mut completion = record.subscribe_completion();
        loop {
            if *completion.borrow_and_update() {
                let failure = *record
                    .failure
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                return failure.map_or(Ok(()), |error| Err(error.into()));
            }
            completion
                .changed()
                .await
                .expect("operation record owns its completion sender");
        }
    }

    pub async fn cancel_and_wait_active(&self) {
        if let Some(batch_id) = self.active_batch() {
            let _ = self.cancel(batch_id);
            let _ = self.wait(batch_id).await;
        }
    }

    fn record(&self, batch_id: BatchId) -> Result<Arc<OperationRecord>, OperationRuntimeError> {
        self.lock_state()
            .records
            .get(&batch_id)
            .cloned()
            .ok_or(OperationRuntimeError::BatchNotFound)
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, OperationRuntimeState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn validate_conflict_resolutions(
    rows: &[viewer_application::file_commands::FileCommandPreflightRow],
    resolutions: &[ConflictResolution],
) -> Result<(), FileCommandServiceError> {
    let conflicts = rows
        .iter()
        .filter(|row| row.state == FileCommandPreflightState::Conflict)
        .map(|row| row.entity_id)
        .collect::<HashSet<_>>();
    let mut explicit = HashSet::new();
    let mut carried = false;
    for row in rows {
        if row.state != FileCommandPreflightState::Conflict {
            continue;
        }
        let matching = resolutions
            .iter()
            .find(|resolution| resolution.entity_id == row.entity_id);
        if matching.is_none() && !carried {
            return Err(FileCommandServiceError::MissingConflictResolution);
        }
        if matching.is_some_and(|resolution| resolution.apply_to_remaining) {
            carried = true;
        }
    }
    for resolution in resolutions {
        if !conflicts.contains(&resolution.entity_id) || !explicit.insert(resolution.entity_id) {
            return Err(FileCommandServiceError::InvalidConflictResolution);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::{
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
        time::Duration,
    };
    use tokio::sync::Notify;
    use viewer_application::{
        LocalFileCommandPort, ProjectAccess,
        file_commands::{
            BatchResultCode, FileCommandAction, FileCommandCancellation, FileCommandItemExecution,
            LocalFileCommandError, LocalFileCommandOutcome, LocalFileCommandPreflightItem,
        },
        scheduler::TaskCoordinator,
    };
    use viewer_domain::{EntityId, RelativePath};

    #[derive(Default)]
    struct TestPort {
        block_execution: AtomicBool,
        started: Notify,
        release: Notify,
        returned: Notify,
    }

    #[async_trait]
    impl LocalFileCommandPort for TestPort {
        async fn preflight(
            &self,
            _batch_id: BatchId,
            command: &FileCommand,
        ) -> Result<Vec<LocalFileCommandPreflightItem>, LocalFileCommandError> {
            Ok(command
                .items
                .iter()
                .enumerate()
                .map(|(index, item)| LocalFileCommandPreflightItem {
                    entity_id: item.entity_id,
                    relative_path: RelativePath::parse(&format!("item-{index}.txt")).unwrap(),
                    state: FileCommandPreflightState::Ready,
                })
                .collect())
        }

        async fn execute_item(
            &self,
            _request: FileCommandItemExecution,
            _cancellation: FileCommandCancellation,
        ) -> Result<LocalFileCommandOutcome, LocalFileCommandError> {
            self.started.notify_one();
            if self.block_execution.load(Ordering::Acquire) {
                self.release.notified().await;
            }
            self.returned.notify_one();
            Ok(LocalFileCommandOutcome::completed(
                BatchResultCode::MovedToTrash,
            ))
        }
    }

    #[derive(Default)]
    struct RecordingEvents {
        progress: Mutex<Vec<BatchProgress>>,
    }

    impl DesktopEventSink for RecordingEvents {
        fn emit_scan(&self, _event: crate::dto::ScanEventDto) {}

        fn emit_operation(
            &self,
            _session_id: SessionId,
            _generation: Generation,
            event: BatchProgress,
        ) {
            self.progress
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(event);
        }
    }

    struct CancelSessionOnAdmission {
        coordinator: Arc<TaskCoordinator>,
        session_id: SessionId,
        cancelled: AtomicBool,
        progress: Mutex<Vec<BatchProgress>>,
    }

    struct BlockingTerminalEvents {
        terminal_entered: Notify,
        release_terminal: AtomicBool,
        terminal_returned: AtomicBool,
    }

    impl DesktopEventSink for BlockingTerminalEvents {
        fn emit_scan(&self, _event: crate::dto::ScanEventDto) {}

        fn emit_operation(
            &self,
            _session_id: SessionId,
            _generation: Generation,
            event: BatchProgress,
        ) {
            if event.lifecycle != BatchLifecycle::Completed {
                return;
            }
            self.terminal_entered.notify_one();
            while !self.release_terminal.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
            self.terminal_returned.store(true, Ordering::Release);
        }
    }

    #[derive(Default)]
    struct AdmissionPort {
        preflight_started: Notify,
        release_preflight: Notify,
        execute_calls: AtomicUsize,
    }

    #[async_trait]
    impl LocalFileCommandPort for AdmissionPort {
        async fn preflight(
            &self,
            _batch_id: BatchId,
            command: &FileCommand,
        ) -> Result<Vec<LocalFileCommandPreflightItem>, LocalFileCommandError> {
            self.preflight_started.notify_one();
            self.release_preflight.notified().await;
            Ok(command
                .items
                .iter()
                .enumerate()
                .map(|(index, item)| LocalFileCommandPreflightItem {
                    entity_id: item.entity_id,
                    relative_path: RelativePath::parse(&format!("item-{index}.txt")).unwrap(),
                    state: FileCommandPreflightState::Ready,
                })
                .collect())
        }

        async fn execute_item(
            &self,
            _request: FileCommandItemExecution,
            _cancellation: FileCommandCancellation,
        ) -> Result<LocalFileCommandOutcome, LocalFileCommandError> {
            self.execute_calls.fetch_add(1, Ordering::AcqRel);
            Ok(LocalFileCommandOutcome::completed(
                BatchResultCode::MovedToTrash,
            ))
        }
    }

    impl DesktopEventSink for CancelSessionOnAdmission {
        fn emit_scan(&self, _event: crate::dto::ScanEventDto) {}

        fn emit_operation(
            &self,
            _session_id: SessionId,
            _generation: Generation,
            event: BatchProgress,
        ) {
            self.progress
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(event.clone());
            if event.lifecycle == BatchLifecycle::Queued
                && !self.cancelled.swap(true, Ordering::AcqRel)
            {
                self.coordinator.cancel_session(self.session_id);
            }
        }
    }

    fn command_item() -> FileCommandItem {
        FileCommandItem {
            entity_id: EntityId::new(),
            action: FileCommandAction::Trash,
        }
    }

    #[tokio::test]
    async fn execution_failure_after_admission_publishes_terminal_progress() {
        let session_id = SessionId::new();
        let coordinator = Arc::new(TaskCoordinator::default());
        let generation = coordinator.begin_session(session_id);
        let port = Arc::new(TestPort::default());
        let service = Arc::new(FileCommandService::new(
            session_id,
            ProjectAccess::ReadWrite,
            Arc::clone(&coordinator),
            port,
        ));
        let events = Arc::new(CancelSessionOnAdmission {
            coordinator,
            session_id,
            cancelled: AtomicBool::new(false),
            progress: Mutex::new(Vec::new()),
        });
        let runtime = OperationRuntime::new(service, events.clone());

        let started = runtime
            .start(
                session_id,
                generation,
                FileCommandKind::Trash,
                vec![command_item()],
                Vec::new(),
            )
            .await
            .unwrap();
        assert_eq!(
            runtime.wait(started.batch_id).await,
            Err(OperationRuntimeError::Service(
                FileCommandServiceError::StaleSession
            ))
        );

        let status = runtime.status(started.batch_id).unwrap();
        assert_eq!(status.lifecycle, BatchLifecycle::Completed);
        assert_eq!(status.failed, status.requested);
        assert_eq!(status.processed(), status.requested);
        let published = events.progress.lock().unwrap();
        assert_eq!(
            published.last().unwrap().lifecycle,
            BatchLifecycle::Completed
        );
        let terminal_positions = published
            .iter()
            .enumerate()
            .filter_map(|(index, progress)| {
                (progress.lifecycle == BatchLifecycle::Completed).then_some(index)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            terminal_positions.len(),
            1,
            "terminal progress is published once"
        );
        assert!(
            published[terminal_positions[0]..]
                .iter()
                .all(|progress| progress.lifecycle == BatchLifecycle::Completed),
            "no non-terminal progress may follow completion"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wait_does_not_complete_until_terminal_event_publication_returns() {
        let session_id = SessionId::new();
        let coordinator = Arc::new(TaskCoordinator::default());
        let generation = coordinator.begin_session(session_id);
        let service = Arc::new(FileCommandService::new(
            session_id,
            ProjectAccess::ReadWrite,
            coordinator,
            Arc::new(TestPort::default()),
        ));
        let events = Arc::new(BlockingTerminalEvents {
            terminal_entered: Notify::new(),
            release_terminal: AtomicBool::new(false),
            terminal_returned: AtomicBool::new(false),
        });
        let runtime = Arc::new(OperationRuntime::new(service, events.clone()));
        let started = runtime
            .start(
                session_id,
                generation,
                FileCommandKind::Trash,
                vec![command_item()],
                Vec::new(),
            )
            .await
            .unwrap();

        events.terminal_entered.notified().await;
        assert!(
            tokio::time::timeout(Duration::from_millis(25), runtime.wait(started.batch_id),)
                .await
                .is_err(),
            "completion must remain latched false while the terminal sink is in flight"
        );
        events.release_terminal.store(true, Ordering::Release);
        runtime.wait(started.batch_id).await.unwrap();
        assert!(events.terminal_returned.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn cancel_intent_is_visible_before_service_registration_and_first_item() {
        let session_id = SessionId::new();
        let coordinator = Arc::new(TaskCoordinator::default());
        let generation = coordinator.begin_session(session_id);
        let port = Arc::new(AdmissionPort::default());
        let service = Arc::new(FileCommandService::new(
            session_id,
            ProjectAccess::ReadWrite,
            coordinator,
            port.clone(),
        ));
        let lane = service.write_lane();
        let events = Arc::new(RecordingEvents::default());
        let runtime = Arc::new(OperationRuntime::new(service, events));

        let start_runtime = Arc::clone(&runtime);
        let start = tokio::spawn(async move {
            start_runtime
                .start(
                    session_id,
                    generation,
                    FileCommandKind::Trash,
                    vec![command_item()],
                    Vec::new(),
                )
                .await
        });
        port.preflight_started.notified().await;

        // Queue a lane holder before preflight releases its temporary lane
        // guard. The runtime will be admitted while service registration waits.
        let lane_acquired = Arc::new(Notify::new());
        let release_lane = Arc::new(Notify::new());
        let holder_acquired = Arc::clone(&lane_acquired);
        let holder_release = Arc::clone(&release_lane);
        let holder = tokio::spawn(async move {
            let _lane = lane.lock().await;
            holder_acquired.notify_one();
            holder_release.notified().await;
        });
        port.release_preflight.notify_one();
        let started = start.await.unwrap().unwrap();
        lane_acquired.notified().await;
        assert!(runtime.cancel(started.batch_id).unwrap());
        release_lane.notify_one();
        holder.await.unwrap();

        tokio::time::timeout(Duration::from_secs(1), runtime.wait(started.batch_id))
            .await
            .expect("cancelled admission must settle without polling")
            .unwrap();
        let results = runtime.results(started.batch_id, 0, 10).unwrap();
        assert_eq!(results.items.len(), 1);
        assert_eq!(
            results.items[0].status,
            viewer_domain::operation::BatchItemStatus::Cancelled
        );
        assert_eq!(port.execute_calls.load(Ordering::Acquire), 0);
    }

    #[tokio::test]
    async fn wait_does_not_finish_before_the_runtime_clears_its_active_batch() {
        use std::{sync::mpsc, thread, time::Duration};

        let session_id = SessionId::new();
        let coordinator = Arc::new(TaskCoordinator::default());
        let generation = coordinator.begin_session(session_id);
        let port = Arc::new(TestPort::default());
        port.block_execution.store(true, Ordering::Release);
        let service = Arc::new(FileCommandService::new(
            session_id,
            ProjectAccess::ReadWrite,
            coordinator,
            port.clone(),
        ));
        let events = Arc::new(RecordingEvents::default());
        let runtime = Arc::new(OperationRuntime::new(service, events));
        let started = runtime
            .start(
                session_id,
                generation,
                FileCommandKind::Trash,
                vec![command_item()],
                Vec::new(),
            )
            .await
            .unwrap();
        port.started.notified().await;
        let record = runtime.record(started.batch_id).unwrap();
        let holder_runtime = Arc::clone(&runtime);
        let holder_record = Arc::clone(&record);
        let (locked_tx, locked_rx) = mpsc::sync_channel(0);
        let holder = thread::spawn(move || {
            let state = holder_runtime
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            assert_eq!(state.active, Some(started.batch_id));
            locked_tx.send(()).unwrap();
            let deadline = std::time::Instant::now() + Duration::from_millis(100);
            let completed_while_active = loop {
                if holder_record.is_complete() {
                    break true;
                }
                if std::time::Instant::now() >= deadline {
                    break false;
                }
                thread::yield_now();
            };
            drop(state);
            completed_while_active
        });
        locked_rx.recv().unwrap();
        port.release.notify_one();
        port.returned.notified().await;
        tokio::time::sleep(Duration::from_millis(1)).await;
        let completed_while_active = holder.join().unwrap();
        assert!(
            !completed_while_active,
            "completion publication must happen only after active is cleared"
        );
        runtime.wait(started.batch_id).await.unwrap();
        assert_eq!(runtime.active_batch(), None);
    }
}
