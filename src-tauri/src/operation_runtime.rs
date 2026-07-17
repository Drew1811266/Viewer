use crate::state::DesktopEventSink;
use async_trait::async_trait;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::{Mutex as AsyncMutex, Notify, watch};
use viewer_application::{
    BrowseIndexPort, ClockPort, CommitStage, OperationCommit, OperationCommitError,
    OperationCommitPort, VolumePort,
    file_commands::{
        BatchId, BatchProgress, BatchResultPage, BatchSummary, ConflictResolution, FileCommand,
        FileCommandItem, FileCommandKind, FileCommandPreflightState, FileCommandService,
        FileCommandServiceError,
    },
    metadata::{
        FileCopyProjection, FileMoveProjection, FilePathMove, MarkerRestore, MarkerTarget,
        OperationProjectionPort, PortableMetadataPort,
    },
};
use viewer_domain::{
    EntityId, RelativePath, SessionId,
    file::FileNode,
    operation::{BatchLifecycle, OperationKind},
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
    complete: AtomicBool,
    notify: Notify,
}

impl OperationRecord {
    fn new(progress: BatchProgress) -> Self {
        Self {
            progress: Mutex::new(progress),
            summary: Mutex::new(None),
            failure: Mutex::new(None),
            complete: AtomicBool::new(false),
            notify: Notify::new(),
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

    fn finish(&self, result: Result<BatchSummary, FileCommandServiceError>) {
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
        self.complete.store(true, Ordering::Release);
        self.notify.notify_waiters();
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
        validate_conflict_resolutions(preflight.rows(), &resolutions)?;
        if !preflight.is_executable() {
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
        let progress_record = Arc::clone(&record);
        let progress_events = Arc::clone(&self.events);
        tokio::spawn(async move {
            while progress_rx.changed().await.is_ok() {
                let progress = progress_rx.borrow_and_update().clone();
                progress_record.publish(progress.clone());
                progress_events.emit_operation(session_id, generation, progress);
            }
        });

        let service = Arc::clone(&self.service);
        let state = Arc::clone(&self.state);
        tokio::spawn(async move {
            let result = service
                .execute(preflight, &resolutions, Some(progress_tx))
                .await;
            if let Ok(summary) = &result {
                let completed = BatchProgress {
                    batch_id,
                    lifecycle: BatchLifecycle::Completed,
                    requested: summary.requested(),
                    completed: summary.completed(),
                    failed: summary.failed(),
                    skipped: summary.skipped(),
                    cancelled: summary.cancelled(),
                    active_entity_id: None,
                };
                record.publish(completed);
            }
            record.finish(result);
            let mut state = state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if state.active == Some(batch_id) {
                state.active = None;
            }
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
        if self.service.cancel_pending(batch_id) {
            return Ok(true);
        }
        let accepted =
            self.lock_state().active == Some(batch_id) && !record.complete.load(Ordering::Acquire);
        if accepted {
            let service = Arc::clone(&self.service);
            tokio::spawn(async move {
                loop {
                    if service.cancel_pending(batch_id) || record.complete.load(Ordering::Acquire) {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            });
        }
        Ok(accepted)
    }

    pub fn active_batch(&self) -> Option<BatchId> {
        self.lock_state().active
    }

    pub async fn wait(&self, batch_id: BatchId) -> Result<(), OperationRuntimeError> {
        let record = self.record(batch_id)?;
        loop {
            if record.complete.load(Ordering::Acquire) {
                let failure = *record
                    .failure
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                return failure.map_or(Ok(()), |error| Err(error.into()));
            }
            record.notify.notified().await;
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

    fn clear_replaced_destination_marker(
        &self,
        current: &JournalItem,
        batch: &[JournalItem],
        destination: &RelativePath,
    ) -> Result<(), OperationCommitError> {
        let is_cycle_blocker = batch.iter().any(|item| {
            item.operation_id != current.operation_id
                && item.source == *destination
                && item.temporary.is_some()
        });
        if is_cycle_blocker {
            return Ok(());
        }
        let Some(node) = self
            .index
            .node_by_relative_path(destination)
            .map_err(|_| metadata_commit_error("index_unavailable"))?
        else {
            return Ok(());
        };
        if node.entity_id == current.entity_id {
            return Ok(());
        }
        let Some(marker) = self
            .metadata
            .markers_for_paths(std::slice::from_ref(destination))
            .map_err(|_| metadata_commit_error("metadata_unavailable"))?
            .into_iter()
            .find(|marker| marker.relative_path == *destination)
        else {
            return Ok(());
        };
        self.metadata
            .restore_batch(
                &[MarkerRestore {
                    target: MarkerTarget {
                        entity_id: node.entity_id,
                        relative_path: destination.clone(),
                        kind: node.kind,
                        size: node.size,
                        modified_ns: node.modified_ns,
                    },
                    expected: marker.marker,
                    previous: viewer_domain::file::Marker::default(),
                }],
                self.clock.unix_millis(),
            )
            .map_err(|_| metadata_commit_error("metadata_unavailable"))?;
        Ok(())
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
        self.clear_replaced_destination_marker(&current, &batch, destination)?;
        if commit.kind == OperationKind::Copy {
            return Ok(());
        }
        let case_sensitive = self
            .volume
            .is_case_sensitive(&self.root)
            .map_err(|_| metadata_commit_error("volume_unavailable"))?;
        let mut moves = self.blocking_cycle_moves(&current, &batch, destination)?;
        if let Some(source) = self.marker_source(&current)? {
            moves.push(FilePathMove {
                source,
                destination: destination.clone(),
            });
        }
        if moves.is_empty() {
            return Ok(());
        }
        self.metadata
            .move_paths(&moves, case_sensitive, self.clock.unix_millis())
            .map_err(|_| metadata_commit_error("metadata_unavailable"))?;
        Ok(())
    }

    async fn sync_index(&self, commit: &OperationCommit) -> Result<(), OperationCommitError> {
        let Some(source) = self.source(commit.entity_id)? else {
            return Ok(());
        };
        let case_sensitive = self.case_sensitive()?;
        match commit.kind {
            OperationKind::Rename | OperationKind::Move => {
                let destination = commit
                    .destination
                    .clone()
                    .ok_or_else(|| index_commit_error("destination_missing"))?;
                let (current, batch) =
                    self.journal_batch(commit.operation_id, CommitStage::Index)?;
                let mut moves = Vec::new();
                if let Some(blocker) = self
                    .index
                    .node_by_relative_path(&destination)
                    .map_err(|_| index_commit_error("index_unavailable"))?
                    .filter(|blocker| blocker.entity_id != source.entity_id)
                {
                    if let Some(temporary) = batch
                        .iter()
                        .find(|item| {
                            item.operation_id != current.operation_id
                                && item.entity_id == blocker.entity_id
                                && item.source == destination
                        })
                        .and_then(|item| item.temporary.clone())
                    {
                        moves.push(FileMoveProjection {
                            source: blocker,
                            destination: temporary,
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
