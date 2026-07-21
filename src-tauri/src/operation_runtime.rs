use crate::state::DesktopEventSink;
use async_trait::async_trait;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::{Mutex as AsyncMutex, oneshot, watch};
use viewer_application::{
    BrowseIndexPort, ClockPort, CommitStage, MetadataCommitOutcome, OperationCommit,
    OperationCommitError, OperationCommitPort, VolumePort,
    file_commands::{
        BatchId, BatchProgress, BatchResultPage, BatchSummary, ConflictResolution, FileCommand,
        FileCommandItem, FileCommandKind, FileCommandPreflight, FileCommandPreflightState,
        FileCommandService, FileCommandServiceError,
    },
    metadata::{
        FileCopyProjection, FileMoveProjection, FilePathMove, OperationProjectionPort,
        PortableMetadataPort,
    },
};
use viewer_domain::{
    EntityId, RelativePath, SessionId,
    file::FileNode,
    operation::{BatchLifecycle, ConflictPolicy, OperationKind},
    search::Generation,
};
use viewer_infrastructure::operation::{
    journal::{JournalItem, OperationJournal},
    service::LocalFileCommandAdapter,
};

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

struct OperationRecord {
    progress: Mutex<BatchProgress>,
    summary: Mutex<Option<BatchSummary>>,
    failure: Mutex<Option<FileCommandServiceError>>,
    complete: watch::Sender<bool>,
}

impl OperationRecord {
    fn new(progress: BatchProgress) -> Self {
        let (complete, _) = watch::channel(false);
        Self {
            progress: Mutex::new(progress),
            summary: Mutex::new(None),
            failure: Mutex::new(None),
            complete,
        }
    }

    fn progress(&self) -> BatchProgress {
        self.progress
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn publish(&self, progress: BatchProgress) {
        *self
            .progress
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = progress;
    }

    fn store_result(&self, result: Result<BatchSummary, FileCommandServiceError>) {
        match result {
            Ok(summary) => {
                *self
                    .summary
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(summary);
            }
            Err(error) => {
                *self
                    .failure
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(error);
            }
        }
    }

    fn mark_complete(&self) {
        self.complete.send_replace(true);
    }

    fn is_complete(&self) -> bool {
        *self.complete.borrow()
    }

    fn subscribe_completion(&self) -> watch::Receiver<bool> {
        self.complete.subscribe()
    }
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

pub struct DesktopOperationCommitPort {
    root: PathBuf,
    index: Arc<dyn BrowseIndexPort>,
    projection: Arc<dyn OperationProjectionPort>,
    metadata: Arc<dyn PortableMetadataPort>,
    volume: Arc<dyn VolumePort>,
    clock: Arc<dyn ClockPort>,
    journal: Arc<OperationJournal>,
    allow_missing_index: bool,
}

impl DesktopOperationCommitPort {
    pub fn new(
        root: PathBuf,
        index: Arc<dyn BrowseIndexPort>,
        projection: Arc<dyn OperationProjectionPort>,
        metadata: Arc<dyn PortableMetadataPort>,
        volume: Arc<dyn VolumePort>,
        clock: Arc<dyn ClockPort>,
        journal: Arc<OperationJournal>,
    ) -> Self {
        Self {
            root,
            index,
            projection,
            metadata,
            volume,
            clock,
            journal,
            allow_missing_index: false,
        }
    }

    pub fn for_recovery(
        root: PathBuf,
        index: Arc<dyn BrowseIndexPort>,
        projection: Arc<dyn OperationProjectionPort>,
        metadata: Arc<dyn PortableMetadataPort>,
        volume: Arc<dyn VolumePort>,
        clock: Arc<dyn ClockPort>,
        journal: Arc<OperationJournal>,
    ) -> Self {
        Self {
            root,
            index,
            projection,
            metadata,
            volume,
            clock,
            journal,
            allow_missing_index: true,
        }
    }

    fn journal_batch(
        &self,
        operation_id: viewer_domain::OperationId,
        stage: CommitStage,
    ) -> Result<(JournalItem, Vec<JournalItem>), OperationCommitError> {
        let current = self
            .journal
            .item(operation_id)
            .map_err(|_| OperationCommitError::new(stage, "journal_unavailable"))?
            .ok_or_else(|| OperationCommitError::new(stage, "journal_item_missing"))?;
        let batch = self
            .journal
            .incomplete_items()
            .map_err(|_| OperationCommitError::new(stage, "journal_unavailable"))?
            .into_iter()
            .filter(|item| item.batch_id == current.batch_id)
            .collect();
        Ok((current, batch))
    }

    fn blocking_cycle_moves(
        &self,
        current: &JournalItem,
        batch: &[JournalItem],
        destination: &RelativePath,
    ) -> Result<Vec<FilePathMove>, OperationCommitError> {
        batch
            .iter()
            .filter(|item| item.operation_id != current.operation_id && item.source == *destination)
            .filter_map(|item| item.temporary.as_ref().map(|temporary| (item, temporary)))
            .map(|(item, temporary)| {
                self.has_marker(&item.source).map(|present| {
                    present.then(|| FilePathMove {
                        source: item.source.clone(),
                        destination: temporary.clone(),
                    })
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|moves| moves.into_iter().flatten().collect())
    }

    fn has_marker(&self, path: &RelativePath) -> Result<bool, OperationCommitError> {
        self.metadata
            .markers_for_paths(std::slice::from_ref(path))
            .map(|markers| markers.iter().any(|marker| marker.relative_path == *path))
            .map_err(|_| metadata_commit_error("metadata_unavailable"))
    }

    fn marker_source(
        &self,
        current: &JournalItem,
    ) -> Result<Option<RelativePath>, OperationCommitError> {
        let paths = current.temporary.as_ref().map_or_else(
            || vec![current.source.clone()],
            |temporary| vec![temporary.clone(), current.source.clone()],
        );
        let markers = self
            .metadata
            .markers_for_paths(&paths)
            .map_err(|_| metadata_commit_error("metadata_unavailable"))?;
        Ok(paths
            .into_iter()
            .find(|path| markers.iter().any(|marker| marker.relative_path == *path)))
    }

    fn replaced_destination_marker(
        &self,
        current: &JournalItem,
        batch: &[JournalItem],
        destination: &RelativePath,
    ) -> Option<RelativePath> {
        if current.conflict_policy != ConflictPolicy::Replace {
            return None;
        }
        let is_cycle_blocker = batch.iter().any(|item| {
            item.operation_id != current.operation_id
                && item.source == *destination
                && item.temporary.is_some()
        });
        if is_cycle_blocker {
            return None;
        }
        Some(destination.clone())
    }

    fn metadata_moves(
        &self,
        current: &JournalItem,
        batch: &[JournalItem],
        destination: &RelativePath,
    ) -> Result<Vec<FilePathMove>, OperationCommitError> {
        if current.kind == OperationKind::Copy {
            return Ok(Vec::new());
        }
        let mut moves = self.blocking_cycle_moves(current, batch, destination)?;
        if let Some(source) = self.marker_source(current)? {
            moves.push(FilePathMove {
                source,
                destination: destination.clone(),
            });
        }
        Ok(moves)
    }

    fn source(&self, entity_id: EntityId) -> Result<Option<FileNode>, OperationCommitError> {
        let node = self
            .index
            .node(entity_id)
            .map_err(|_| index_commit_error("index_unavailable"))?;
        match node {
            Some(node) => Ok(Some(node)),
            None if self.allow_missing_index => Ok(None),
            None => Err(index_commit_error("projection_stale")),
        }
    }

    fn case_sensitive(&self) -> Result<bool, OperationCommitError> {
        self.volume
            .is_case_sensitive(&self.root)
            .map_err(|_| index_commit_error("volume_unavailable"))
    }

    fn destination_node(
        &self,
        path: &RelativePath,
        kind: viewer_domain::file::FileKind,
    ) -> Result<FileNode, OperationCommitError> {
        let candidate = self.root.join(path.as_str());
        let metadata = fs::symlink_metadata(&candidate)
            .map_err(|_| index_commit_error("destination_unavailable"))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(index_commit_error("destination_invalid"));
        }
        let canonical = fs::canonicalize(&candidate)
            .map_err(|_| index_commit_error("destination_unavailable"))?;
        if !canonical.starts_with(&self.root) {
            return Err(index_commit_error("destination_outside_project"));
        }
        Ok(FileNode {
            entity_id: entity_id_for_metadata(&metadata, path),
            relative_path: path.clone(),
            kind,
            size: metadata.len(),
            modified_ns: modified_ns(&metadata),
        })
    }
}

#[async_trait]
impl OperationCommitPort for DesktopOperationCommitPort {
    async fn commit_metadata(&self, commit: &OperationCommit) -> Result<(), OperationCommitError> {
        if commit.kind == OperationKind::Trash {
            return Ok(());
        }
        let destination = commit
            .destination
            .as_ref()
            .ok_or_else(|| metadata_commit_error("destination_missing"))?;
        let (current, batch) = self.journal_batch(commit.operation_id, CommitStage::Metadata)?;
        let case_sensitive = self
            .volume
            .is_case_sensitive(&self.root)
            .map_err(|_| metadata_commit_error("volume_unavailable"))?;
        let moves = self.metadata_moves(&current, &batch, destination)?;
        if moves.is_empty() {
            return Ok(());
        }
        self.metadata
            .move_paths(&moves, case_sensitive, self.clock.unix_millis())
            .map_err(|_| metadata_commit_error("metadata_unavailable"))?;
        Ok(())
    }

    async fn commit_metadata_barrier(
        &self,
        commit: &OperationCommit,
    ) -> Result<MetadataCommitOutcome, OperationCommitError> {
        if commit.kind == OperationKind::Trash {
            return Ok(MetadataCommitOutcome::CallerAdvancesJournal);
        }
        let destination = commit
            .destination
            .as_ref()
            .ok_or_else(|| metadata_commit_error("destination_missing"))?;
        let (current, batch) = self.journal_batch(commit.operation_id, CommitStage::Metadata)?;
        if current.conflict_policy != ConflictPolicy::Replace {
            self.commit_metadata(commit).await?;
            return Ok(MetadataCommitOutcome::CallerAdvancesJournal);
        }

        let case_sensitive = self
            .volume
            .is_case_sensitive(&self.root)
            .map_err(|_| metadata_commit_error("volume_unavailable"))?;
        let moves = self.metadata_moves(&current, &batch, destination)?;
        let replaced_destination = self.replaced_destination_marker(&current, &batch, destination);
        self.metadata
            .commit_replace(
                current.operation_id,
                replaced_destination.as_ref(),
                &moves,
                case_sensitive,
                self.clock.unix_millis(),
            )
            .map_err(|_| metadata_commit_error("metadata_unavailable"))?;
        Ok(MetadataCommitOutcome::JournalAdvanced)
    }

    async fn sync_index(&self, commit: &OperationCommit) -> Result<(), OperationCommitError> {
        let Some(source) = self.source(commit.entity_id)? else {
            return Ok(());
        };
        let case_sensitive = self.case_sensitive()?;
        match commit.kind {
            OperationKind::Rename | OperationKind::Move => {
                let destination_path = commit
                    .destination
                    .clone()
                    .ok_or_else(|| index_commit_error("destination_missing"))?;
                let destination = self.destination_node(&destination_path, source.kind)?;
                let (current, batch) =
                    self.journal_batch(commit.operation_id, CommitStage::Index)?;
                let mut moves = Vec::new();
                if let Some(blocker) = self
                    .index
                    .node_by_relative_path(&destination_path)
                    .map_err(|_| index_commit_error("index_unavailable"))?
                    .filter(|blocker| blocker.entity_id != source.entity_id)
                {
                    if let Some(temporary) = batch
                        .iter()
                        .find(|item| {
                            item.operation_id != current.operation_id
                                && item.entity_id == blocker.entity_id
                                && item.source == destination_path
                        })
                        .and_then(|item| item.temporary.clone())
                    {
                        let temporary_destination = FileNode {
                            relative_path: temporary,
                            ..blocker.clone()
                        };
                        moves.push(FileMoveProjection {
                            source: blocker,
                            destination: temporary_destination,
                        });
                    } else {
                        self.projection
                            .apply_trash(&[blocker])
                            .map_err(|_| index_commit_error("projection_stale"))?;
                    }
                }
                moves.push(FileMoveProjection {
                    source,
                    destination,
                });
                self.projection
                    .apply_move(&moves, case_sensitive)
                    .map_err(|_| index_commit_error("projection_stale"))
            }
            OperationKind::Copy => {
                let destination_path = commit
                    .destination
                    .as_ref()
                    .ok_or_else(|| index_commit_error("destination_missing"))?;
                let destination = self.destination_node(destination_path, source.kind)?;
                if let Some(replaced) = self
                    .index
                    .node_by_relative_path(destination_path)
                    .map_err(|_| index_commit_error("index_unavailable"))?
                    .filter(|replaced| replaced.entity_id != destination.entity_id)
                {
                    self.projection
                        .apply_trash(&[replaced])
                        .map_err(|_| index_commit_error("projection_stale"))?;
                }
                self.projection
                    .apply_copy(
                        &[FileCopyProjection {
                            source,
                            destination,
                        }],
                        case_sensitive,
                    )
                    .map_err(|_| index_commit_error("projection_stale"))
            }
            OperationKind::Trash => self
                .projection
                .apply_trash(&[source])
                .map_err(|_| index_commit_error("projection_stale")),
            OperationKind::SetReviewState | OperationKind::SetFavorite => {
                Err(index_commit_error("operation_kind_invalid"))
            }
        }
    }
}

fn metadata_commit_error(code: &str) -> OperationCommitError {
    OperationCommitError::new(CommitStage::Metadata, code)
}

fn index_commit_error(code: &str) -> OperationCommitError {
    OperationCommitError::new(CommitStage::Index, code)
}

#[cfg(unix)]
fn entity_id_for_metadata(metadata: &fs::Metadata, _path: &RelativePath) -> EntityId {
    use std::os::unix::fs::MetadataExt;
    EntityId::from_u128((u128::from(metadata.dev()) << 64) | u128::from(metadata.ino()))
}

#[cfg(not(unix))]
fn entity_id_for_metadata(metadata: &fs::Metadata, path: &RelativePath) -> EntityId {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.as_str().hash(&mut hasher);
    metadata.len().hash(&mut hasher);
    EntityId::from_u128(u128::from(hasher.finish()))
}

#[cfg(unix)]
fn modified_ns(metadata: &fs::Metadata) -> i128 {
    use std::os::unix::fs::MetadataExt;
    i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec())
}

#[cfg(not(unix))]
fn modified_ns(metadata: &fs::Metadata) -> i128 {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos() as i128)
}

pub fn adapter_as_undo_port(
    adapter: Arc<LocalFileCommandAdapter>,
) -> Arc<dyn viewer_application::undo::UndoFilePort> {
    adapter
}

#[cfg(test)]
mod tests {
    use super::*;
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
    async fn completion_latch_remembers_finish_for_late_and_multiple_waiters() {
        let initial = BatchProgress {
            batch_id: BatchId::new(),
            lifecycle: BatchLifecycle::Queued,
            requested: 1,
            completed: 0,
            failed: 0,
            skipped: 0,
            cancelled: 0,
            active_entity_id: None,
        };
        let record = Arc::new(OperationRecord::new(initial));
        let mut before = record.subscribe_completion();
        record.store_result(Ok(BatchSummary::try_from_results(
            BatchId::new(),
            0,
            Vec::new(),
        )
        .unwrap()));
        record.mark_complete();

        tokio::time::timeout(Duration::from_millis(100), before.changed())
            .await
            .expect("an already-registered waiter must be woken")
            .expect("completion channel remains open");
        assert!(*before.borrow_and_update());

        let mut late = record.subscribe_completion();
        assert!(
            *late.borrow_and_update(),
            "late waiter observes the latched value"
        );
        let waiters = (0..8)
            .map(|_| {
                let mut completion = record.subscribe_completion();
                tokio::spawn(async move {
                    if !*completion.borrow_and_update() {
                        completion.changed().await.unwrap();
                    }
                    assert!(*completion.borrow_and_update());
                })
            })
            .collect::<Vec<_>>();
        for waiter in waiters {
            tokio::time::timeout(Duration::from_millis(100), waiter)
                .await
                .expect("every completion waiter finishes")
                .unwrap();
        }
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
