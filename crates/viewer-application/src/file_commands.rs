use crate::{
    ProjectAccess, ports::LocalFileCommandPort, scheduler::TaskCoordinator, undo::UndoStack,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::{Mutex as AsyncMutex, watch};
use viewer_domain::{
    EntityId, RelativePath, SessionId, operation::ConflictPolicy, search::Generation,
};

pub use viewer_domain::operation::{
    BatchId, BatchItemResult, BatchItemStatus, BatchLifecycle, BatchProgress, BatchResultCode,
    BatchResultPage, BatchSummary, MAX_BATCH_RESULT_PAGE_SIZE,
};

pub const MAX_FILE_COMMAND_ITEMS: usize = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileCommandKind {
    Rename,
    Copy,
    Move,
    Trash,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FileCommandAction {
    Rename {
        proposed_name: String,
        edit_extension: bool,
    },
    Copy {
        destination_folder: EntityId,
    },
    Move {
        destination_folder: EntityId,
    },
    Trash,
}

impl FileCommandAction {
    pub const fn kind(&self) -> FileCommandKind {
        match self {
            Self::Rename { .. } => FileCommandKind::Rename,
            Self::Copy { .. } => FileCommandKind::Copy,
            Self::Move { .. } => FileCommandKind::Move,
            Self::Trash => FileCommandKind::Trash,
        }
    }
}

impl From<FileCommandKind> for viewer_domain::operation::OperationKind {
    fn from(kind: FileCommandKind) -> Self {
        match kind {
            FileCommandKind::Rename => Self::Rename,
            FileCommandKind::Copy => Self::Copy,
            FileCommandKind::Move => Self::Move,
            FileCommandKind::Trash => Self::Trash,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileCommandItem {
    pub entity_id: EntityId,
    pub action: FileCommandAction,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileCommand {
    pub session_id: SessionId,
    pub generation: Generation,
    pub kind: FileCommandKind,
    pub items: Vec<FileCommandItem>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileCommandPreflightState {
    Ready,
    Conflict,
    Blocked(BatchResultCode),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalFileCommandPreflightItem {
    pub entity_id: EntityId,
    pub relative_path: RelativePath,
    pub state: FileCommandPreflightState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileCommandPreflightRow {
    pub entity_id: EntityId,
    pub relative_path: RelativePath,
    pub state: FileCommandPreflightState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileCommandPreflight {
    batch_id: BatchId,
    kind: FileCommandKind,
    rows: Vec<FileCommandPreflightRow>,
    executable: bool,
    command: FileCommand,
}

impl FileCommandPreflight {
    pub const fn batch_id(&self) -> BatchId {
        self.batch_id
    }

    pub const fn kind(&self) -> FileCommandKind {
        self.kind
    }

    pub fn rows(&self) -> &[FileCommandPreflightRow] {
        &self.rows
    }

    pub const fn is_executable(&self) -> bool {
        self.executable
    }

    pub fn initial_progress(&self) -> BatchProgress {
        BatchProgress {
            batch_id: self.batch_id,
            lifecycle: BatchLifecycle::Queued,
            requested: u32::try_from(self.rows.len()).unwrap_or(u32::MAX),
            completed: 0,
            failed: 0,
            skipped: 0,
            cancelled: 0,
            active_entity_id: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConflictResolution {
    pub entity_id: EntityId,
    pub policy: ConflictPolicy,
    pub apply_to_remaining: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileCommandItemExecution {
    pub batch_id: BatchId,
    pub kind: FileCommandKind,
    pub item: FileCommandItem,
    pub relative_path: RelativePath,
    pub conflict_policy: Option<ConflictPolicy>,
}

#[derive(Clone, Debug, Default)]
pub struct FileCommandCancellation {
    cancelled: Arc<AtomicBool>,
}

impl FileCommandCancellation {
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalFileCommandOutcome {
    status: BatchItemStatus,
    code: BatchResultCode,
}

impl LocalFileCommandOutcome {
    pub const fn completed(code: BatchResultCode) -> Self {
        Self {
            status: BatchItemStatus::Completed,
            code,
        }
    }

    pub const fn failed(code: BatchResultCode) -> Self {
        Self {
            status: BatchItemStatus::Failed,
            code,
        }
    }

    pub const fn skipped(code: BatchResultCode) -> Self {
        Self {
            status: BatchItemStatus::Skipped,
            code,
        }
    }

    pub const fn cancelled(code: BatchResultCode) -> Self {
        Self {
            status: BatchItemStatus::Cancelled,
            code,
        }
    }

    pub const fn status(self) -> BatchItemStatus {
        self.status
    }

    pub const fn code(self) -> BatchResultCode {
        self.code
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LocalFileCommandError {
    #[error("local file command backend is unavailable")]
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum FileCommandServiceError {
    #[error("the current project is read-only")]
    ReadOnly,
    #[error("the file command belongs to a stale project session")]
    StaleSession,
    #[error("at least one file target is required")]
    EmptyTargets,
    #[error("the file command has too many targets")]
    TooManyTargets,
    #[error("file command targets must be unique")]
    DuplicateTarget,
    #[error("file command actions do not match the batch kind")]
    ActionKindMismatch,
    #[error("another file command batch is active")]
    BatchActive,
    #[error("the file command preflight contains blocked rows")]
    PreflightBlocked,
    #[error("a conflict decision is required")]
    MissingConflictResolution,
    #[error("a conflict decision does not match this preflight")]
    InvalidConflictResolution,
    #[error("the file command backend returned an invalid preflight")]
    PreflightContract,
    #[error("the file command preflight was already executed")]
    AlreadyExecuted,
    #[error("the file command backend is unavailable")]
    BackendUnavailable,
}

#[derive(Clone)]
struct ActiveBatch {
    batch_id: BatchId,
    cancellation: FileCommandCancellation,
    progress: Arc<Mutex<BatchProgress>>,
    progress_sink: Option<watch::Sender<BatchProgress>>,
}

#[derive(Default)]
struct ServiceState {
    active: Option<ActiveBatch>,
    claimed: HashSet<BatchId>,
}

pub struct FileCommandService {
    session_id: SessionId,
    access: ProjectAccess,
    coordinator: Arc<TaskCoordinator>,
    port: Arc<dyn LocalFileCommandPort>,
    write_lane: Arc<AsyncMutex<()>>,
    undo_stack: Option<Arc<Mutex<UndoStack>>>,
    state: Mutex<ServiceState>,
}

impl FileCommandService {
    pub fn new(
        session_id: SessionId,
        access: ProjectAccess,
        coordinator: Arc<TaskCoordinator>,
        port: Arc<dyn LocalFileCommandPort>,
    ) -> Self {
        Self {
            session_id,
            access,
            coordinator,
            port,
            write_lane: Arc::new(AsyncMutex::new(())),
            undo_stack: None,
            state: Mutex::new(ServiceState::default()),
        }
    }

    pub fn new_with_undo(
        session_id: SessionId,
        access: ProjectAccess,
        coordinator: Arc<TaskCoordinator>,
        port: Arc<dyn LocalFileCommandPort>,
        write_lane: Arc<AsyncMutex<()>>,
        undo_stack: Arc<Mutex<UndoStack>>,
    ) -> Self {
        Self {
            session_id,
            access,
            coordinator,
            port,
            write_lane,
            undo_stack: Some(undo_stack),
            state: Mutex::new(ServiceState::default()),
        }
    }

    pub fn write_lane(&self) -> Arc<AsyncMutex<()>> {
        Arc::clone(&self.write_lane)
    }

    pub async fn preflight(
        &self,
        command: FileCommand,
    ) -> Result<FileCommandPreflight, FileCommandServiceError> {
        self.validate_command(&command)?;
        let _lane = self
            .write_lane
            .try_lock()
            .map_err(|_| FileCommandServiceError::BatchActive)?;
        let batch_id = BatchId::new();
        let rows = self
            .port
            .preflight(batch_id, &command)
            .await
            .map_err(|_| FileCommandServiceError::BackendUnavailable)?;
        self.ensure_current(&command)?;
        if rows.len() != command.items.len()
            || rows
                .iter()
                .zip(&command.items)
                .any(|(row, item)| row.entity_id != item.entity_id)
        {
            return Err(FileCommandServiceError::PreflightContract);
        }
        let rows = rows
            .into_iter()
            .map(|row| FileCommandPreflightRow {
                entity_id: row.entity_id,
                relative_path: row.relative_path,
                state: row.state,
            })
            .collect::<Vec<_>>();
        let executable = rows
            .iter()
            .all(|row| !matches!(row.state, FileCommandPreflightState::Blocked(_)));
        Ok(FileCommandPreflight {
            batch_id,
            kind: command.kind,
            rows,
            executable,
            command,
        })
    }

    pub async fn execute(
        &self,
        preflight: FileCommandPreflight,
        resolutions: &[ConflictResolution],
        progress_sink: Option<watch::Sender<BatchProgress>>,
    ) -> Result<BatchSummary, FileCommandServiceError> {
        self.validate_command(&preflight.command)?;
        if !preflight.executable {
            return Err(FileCommandServiceError::PreflightBlocked);
        }
        let policies = resolve_conflicts(&preflight.rows, resolutions)?;
        let _lane = self
            .write_lane
            .try_lock()
            .map_err(|_| FileCommandServiceError::BatchActive)?;
        let cancellation = FileCommandCancellation::default();
        let mut progress = preflight.initial_progress();
        progress.lifecycle = BatchLifecycle::Running;
        let shared_progress = Arc::new(Mutex::new(progress.clone()));
        {
            let mut state = self.lock_state();
            if !state.claimed.insert(preflight.batch_id) {
                return Err(FileCommandServiceError::AlreadyExecuted);
            }
            state.active = Some(ActiveBatch {
                batch_id: preflight.batch_id,
                cancellation: cancellation.clone(),
                progress: Arc::clone(&shared_progress),
                progress_sink: progress_sink.clone(),
            });
        }
        let mut active_registration = ActiveBatchRegistration {
            state: &self.state,
            batch_id: preflight.batch_id,
            finished: false,
        };

        publish_progress(&shared_progress, &progress_sink, &progress);
        let mut results = Vec::with_capacity(preflight.rows.len());
        for (index, ((item, row), policy)) in preflight
            .command
            .items
            .iter()
            .zip(&preflight.rows)
            .zip(&policies)
            .enumerate()
        {
            let pending_code = if !self.is_current(&preflight.command) {
                Some(BatchResultCode::SessionStale)
            } else if cancellation.is_cancelled() {
                Some(BatchResultCode::Cancelled)
            } else {
                None
            };
            if let Some(code) = pending_code {
                progress.lifecycle = BatchLifecycle::Cancelling;
                progress.active_entity_id = None;
                for ((pending_item, pending_row), pending_policy) in preflight.command.items
                    [index..]
                    .iter()
                    .zip(&preflight.rows[index..])
                    .zip(&policies[index..])
                {
                    let outcome = self
                        .port
                        .settle_unstarted(
                            FileCommandItemExecution {
                                batch_id: preflight.batch_id,
                                kind: preflight.kind,
                                item: pending_item.clone(),
                                relative_path: pending_row.relative_path.clone(),
                                conflict_policy: *pending_policy,
                            },
                            LocalFileCommandOutcome::cancelled(code),
                        )
                        .await
                        .unwrap_or_else(|_| {
                            LocalFileCommandOutcome::failed(BatchResultCode::BackendUnavailable)
                        });
                    results.push(BatchItemResult {
                        entity_id: pending_item.entity_id,
                        relative_path: pending_row.relative_path.clone(),
                        status: outcome.status,
                        code: outcome.code,
                    });
                    increment_progress(&mut progress, outcome.status);
                }
                publish_progress(&shared_progress, &progress_sink, &progress);
                break;
            }

            progress.active_entity_id = Some(item.entity_id);
            publish_progress(&shared_progress, &progress_sink, &progress);
            let request = FileCommandItemExecution {
                batch_id: preflight.batch_id,
                kind: preflight.kind,
                item: item.clone(),
                relative_path: row.relative_path.clone(),
                conflict_policy: *policy,
            };
            let outcome = if *policy == Some(ConflictPolicy::Skip) {
                self.port
                    .settle_unstarted(
                        request,
                        LocalFileCommandOutcome::skipped(BatchResultCode::ConflictSkipped),
                    )
                    .await
                    .unwrap_or_else(|_| {
                        LocalFileCommandOutcome::failed(BatchResultCode::BackendUnavailable)
                    })
            } else {
                self.port
                    .execute_item(request, cancellation.clone())
                    .await
                    .unwrap_or_else(|_| {
                        LocalFileCommandOutcome::failed(BatchResultCode::BackendUnavailable)
                    })
            };
            results.push(BatchItemResult {
                entity_id: item.entity_id,
                relative_path: row.relative_path.clone(),
                status: outcome.status,
                code: outcome.code,
            });
            increment_progress(&mut progress, outcome.status);
            if cancellation.is_cancelled() {
                progress.lifecycle = BatchLifecycle::Cancelling;
            }
            progress.active_entity_id = None;
            publish_progress(&shared_progress, &progress_sink, &progress);
        }
        let summary =
            BatchSummary::try_from_results(preflight.batch_id, progress.requested, results)
                .ok_or(FileCommandServiceError::BackendUnavailable)?;
        if matches!(
            preflight.kind,
            FileCommandKind::Rename | FileCommandKind::Move
        ) {
            let actions = self
                .port
                .take_undo_actions(preflight.batch_id)
                .await
                .unwrap_or_default();
            if let Some(stack) = self.undo_stack.as_ref()
                && !actions.is_empty()
            {
                stack
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .record_batch(preflight.batch_id, preflight.kind.into(), actions);
            }
        }
        active_registration.finish();
        progress.lifecycle = BatchLifecycle::Completed;
        progress.active_entity_id = None;
        debug_assert!(progress.counts_are_consistent());
        publish_progress(&shared_progress, &progress_sink, &progress);
        Ok(summary)
    }

    pub fn cancel_pending(&self, batch_id: BatchId) -> bool {
        let state = self.lock_state();
        let Some(active) = &state.active else {
            return false;
        };
        if active.batch_id != batch_id {
            return false;
        }
        active.cancellation.cancel();
        let mut progress = active
            .progress
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        progress.lifecycle = BatchLifecycle::Cancelling;
        if let Some(sink) = &active.progress_sink {
            sink.send_replace(progress.clone());
        }
        true
    }

    fn validate_command(&self, command: &FileCommand) -> Result<(), FileCommandServiceError> {
        if self.access != ProjectAccess::ReadWrite {
            return Err(FileCommandServiceError::ReadOnly);
        }
        self.ensure_current(command)?;
        if command.items.is_empty() {
            return Err(FileCommandServiceError::EmptyTargets);
        }
        if command.items.len() > MAX_FILE_COMMAND_ITEMS {
            return Err(FileCommandServiceError::TooManyTargets);
        }
        let mut entities = HashSet::with_capacity(command.items.len());
        for item in &command.items {
            if !entities.insert(item.entity_id) {
                return Err(FileCommandServiceError::DuplicateTarget);
            }
            if item.action.kind() != command.kind {
                return Err(FileCommandServiceError::ActionKindMismatch);
            }
        }
        Ok(())
    }

    fn ensure_current(&self, command: &FileCommand) -> Result<(), FileCommandServiceError> {
        self.is_current(command)
            .then_some(())
            .ok_or(FileCommandServiceError::StaleSession)
    }

    fn is_current(&self, command: &FileCommand) -> bool {
        command.session_id == self.session_id
            && self
                .coordinator
                .is_publishable(command.session_id, command.generation)
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, ServiceState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

struct ActiveBatchRegistration<'a> {
    state: &'a Mutex<ServiceState>,
    batch_id: BatchId,
    finished: bool,
}

impl ActiveBatchRegistration<'_> {
    fn finish(&mut self) {
        self.clear_active();
        self.finished = true;
    }

    fn clear_active(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state
            .active
            .as_ref()
            .is_some_and(|active| active.batch_id == self.batch_id)
        {
            state.active = None;
        }
    }
}

impl Drop for ActiveBatchRegistration<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.clear_active();
        }
    }
}

fn resolve_conflicts(
    rows: &[FileCommandPreflightRow],
    resolutions: &[ConflictResolution],
) -> Result<Vec<Option<ConflictPolicy>>, FileCommandServiceError> {
    let conflicts = rows
        .iter()
        .filter(|row| row.state == FileCommandPreflightState::Conflict)
        .map(|row| row.entity_id)
        .collect::<HashSet<_>>();
    let mut explicit = HashMap::with_capacity(resolutions.len());
    for resolution in resolutions {
        if !conflicts.contains(&resolution.entity_id)
            || explicit.insert(resolution.entity_id, *resolution).is_some()
        {
            return Err(FileCommandServiceError::InvalidConflictResolution);
        }
    }
    let mut carried = None;
    let mut policies = Vec::with_capacity(rows.len());
    for row in rows {
        if row.state != FileCommandPreflightState::Conflict {
            policies.push(None);
            continue;
        }
        let resolution = explicit.get(&row.entity_id).copied();
        let policy = resolution.map(|choice| choice.policy).or(carried);
        let Some(policy) = policy else {
            return Err(FileCommandServiceError::MissingConflictResolution);
        };
        if resolution.is_some_and(|choice| choice.apply_to_remaining) {
            carried = Some(policy);
        }
        policies.push(Some(policy));
    }
    Ok(policies)
}

fn publish_progress(
    shared: &Mutex<BatchProgress>,
    sink: &Option<watch::Sender<BatchProgress>>,
    progress: &BatchProgress,
) {
    let mut published = progress.clone();
    let mut current = shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if current.lifecycle == BatchLifecycle::Cancelling
        && published.lifecycle == BatchLifecycle::Running
    {
        published.lifecycle = BatchLifecycle::Cancelling;
    }
    *current = published.clone();
    drop(current);
    if let Some(sink) = sink {
        sink.send_replace(published);
    }
}

fn increment_progress(progress: &mut BatchProgress, status: BatchItemStatus) {
    match status {
        BatchItemStatus::Completed => progress.completed += 1,
        BatchItemStatus::Failed => progress.failed += 1,
        BatchItemStatus::Skipped => progress.skipped += 1,
        BatchItemStatus::Cancelled => progress.cancelled += 1,
    }
}
