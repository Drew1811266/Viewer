use crate::{
    ClockPort, FileMutationPort, FileOperationError, FileSnapshot, ProjectAccess,
    metadata::{
        MarkerProjectionPort, MarkerRestore, MarkerStoreError, MarkerTarget, PortableMetadataPort,
    },
};
use async_trait::async_trait;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use viewer_domain::{
    EntityId, OperationId, RelativePath, SessionId, file::Marker, operation::OperationKind,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UndoAction {
    File {
        entity_id: EntityId,
        current: RelativePath,
        restore: RelativePath,
        expected: FileSnapshot,
    },
    ReviewState {
        target: MarkerTarget,
        previous: Marker,
        expected: Marker,
    },
    Favorite {
        target: MarkerTarget,
        previous: Marker,
        expected: Marker,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UndoBatch {
    pub batch_id: OperationId,
    pub kind: OperationKind,
    pub actions: Vec<UndoAction>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum UndoError {
    #[error(transparent)]
    File(#[from] FileOperationError),
    #[error("undo target is outside the current project")]
    OutsideProject,
    #[error("undo target identity no longer matches the recorded file")]
    IdentityChanged,
    #[error("undo restore destination is occupied by another filesystem item")]
    DestinationOccupied,
}

#[async_trait]
pub trait UndoFilePort: Send + Sync {
    async fn reverse_batch(&self, project_root: &Path, batch: &UndoBatch) -> Result<(), UndoError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UndoReceipt {
    pub batch_id: OperationId,
    pub kind: OperationKind,
    pub action_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum UndoServiceError {
    #[error("the undo request belongs to a stale project session")]
    StaleSession,
    #[error("the current project is read-only")]
    ReadOnly,
    #[error(transparent)]
    Undo(#[from] UndoError),
    #[error(transparent)]
    Store(#[from] MarkerStoreError),
    #[error("undo was committed but the session projection needs rebuilding")]
    CommittedButProjectionStale,
    #[error("the undo stack changed while the inverse operation was running")]
    StackChanged,
}

pub struct UndoStack {
    session_id: SessionId,
    batches: Vec<UndoBatch>,
}

impl UndoStack {
    pub const fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            batches: Vec::new(),
        }
    }

    pub fn record_batch(
        &mut self,
        batch_id: OperationId,
        kind: OperationKind,
        actions: Vec<UndoAction>,
    ) -> bool {
        let allowed = match kind {
            OperationKind::Rename | OperationKind::Move => actions
                .iter()
                .all(|action| matches!(action, UndoAction::File { .. })),
            OperationKind::SetReviewState => actions
                .iter()
                .all(|action| matches!(action, UndoAction::ReviewState { .. })),
            OperationKind::SetFavorite => actions
                .iter()
                .all(|action| matches!(action, UndoAction::Favorite { .. })),
            OperationKind::Copy | OperationKind::Trash => false,
        };
        if !allowed || actions.is_empty() {
            return false;
        }
        self.batches.push(UndoBatch {
            batch_id,
            kind,
            actions,
        });
        true
    }

    pub async fn pop_validated(
        &mut self,
        project_root: &Path,
        mutation: &dyn FileMutationPort,
    ) -> Result<Option<UndoBatch>, UndoError> {
        let Some(batch) = self.batches.last() else {
            return Ok(None);
        };
        let root = std::fs::canonicalize(project_root).map_err(|error| {
            FileOperationError::io("canonicalize undo project", project_root, &error)
        })?;
        for action in &batch.actions {
            let UndoAction::File {
                current,
                restore,
                expected,
                ..
            } = action
            else {
                continue;
            };
            let candidate = root.join(current.as_str());
            let source_metadata = std::fs::symlink_metadata(&candidate).map_err(|error| {
                FileOperationError::io("inspect undo source", &candidate, &error)
            })?;
            if source_metadata.file_type().is_symlink() || !source_metadata.is_file() {
                return Err(UndoError::OutsideProject);
            }
            let canonical = std::fs::canonicalize(&candidate).map_err(|error| {
                FileOperationError::io("resolve undo target", &candidate, &error)
            })?;
            if !canonical.starts_with(&root) {
                return Err(UndoError::OutsideProject);
            }
            let restore_candidate = root.join(restore.as_str());
            let restore_parent = restore_candidate
                .parent()
                .ok_or(UndoError::OutsideProject)?;
            let canonical_restore_parent =
                std::fs::canonicalize(restore_parent).map_err(|error| {
                    FileOperationError::io("resolve undo restore parent", restore_parent, &error)
                })?;
            if !canonical_restore_parent.starts_with(&root) {
                return Err(UndoError::OutsideProject);
            }
            let restore_path = canonical_restore_parent.join(
                restore_candidate
                    .file_name()
                    .ok_or(UndoError::OutsideProject)?,
            );
            match std::fs::symlink_metadata(&restore_path) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() || !metadata.is_file() {
                        return Err(UndoError::DestinationOccupied);
                    }
                    let canonical_restore =
                        std::fs::canonicalize(&restore_path).map_err(|error| {
                            FileOperationError::io(
                                "resolve occupied undo destination",
                                &restore_path,
                                &error,
                            )
                        })?;
                    if canonical_restore != canonical {
                        return Err(UndoError::DestinationOccupied);
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(FileOperationError::io(
                        "inspect undo restore destination",
                        &restore_path,
                        &error,
                    )
                    .into());
                }
            }
            if mutation.snapshot(&canonical).await? != *expected {
                return Err(UndoError::IdentityChanged);
            }
        }
        Ok(self.batches.pop())
    }

    pub fn close_session(&mut self, session_id: SessionId) {
        if session_id == self.session_id {
            self.batches.clear();
        }
    }

    pub fn len(&self) -> usize {
        self.batches.len()
    }

    pub fn is_empty(&self) -> bool {
        self.batches.is_empty()
    }

    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub fn last(&self) -> Option<UndoBatch> {
        self.batches.last().cloned()
    }

    pub fn consume(&mut self, batch_id: OperationId) -> bool {
        if self
            .batches
            .last()
            .is_some_and(|batch| batch.batch_id == batch_id)
        {
            self.batches.pop();
            true
        } else {
            false
        }
    }
}

pub struct UndoService {
    session_id: SessionId,
    access: ProjectAccess,
    project_root: PathBuf,
    mutation: Arc<dyn FileMutationPort>,
    files: Arc<dyn UndoFilePort>,
    store: Arc<dyn PortableMetadataPort>,
    projection: Arc<dyn MarkerProjectionPort>,
    clock: Arc<dyn ClockPort>,
    stack: Arc<Mutex<UndoStack>>,
    write_lane: Arc<tokio::sync::Mutex<()>>,
}

impl UndoService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_id: SessionId,
        access: ProjectAccess,
        project_root: PathBuf,
        mutation: Arc<dyn FileMutationPort>,
        files: Arc<dyn UndoFilePort>,
        store: Arc<dyn PortableMetadataPort>,
        projection: Arc<dyn MarkerProjectionPort>,
        clock: Arc<dyn ClockPort>,
        stack: Arc<Mutex<UndoStack>>,
        write_lane: Arc<tokio::sync::Mutex<()>>,
    ) -> Self {
        Self {
            session_id,
            access,
            project_root,
            mutation,
            files,
            store,
            projection,
            clock,
            stack,
            write_lane,
        }
    }

    pub async fn undo_last(
        &self,
        expected_session: SessionId,
    ) -> Result<Option<UndoReceipt>, UndoServiceError> {
        if expected_session != self.session_id {
            return Err(UndoServiceError::StaleSession);
        }
        if self.access != ProjectAccess::ReadWrite {
            return Err(UndoServiceError::ReadOnly);
        }
        let _lane = self.write_lane.lock().await;
        let batch = self
            .stack
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .last();
        let Some(batch) = batch else {
            return Ok(None);
        };
        match batch.kind {
            OperationKind::Rename | OperationKind::Move => {
                validate_file_batch(&self.project_root, self.mutation.as_ref(), &batch).await?;
                self.files.reverse_batch(&self.project_root, &batch).await?;
                self.consume(batch.batch_id)?;
            }
            OperationKind::SetReviewState | OperationKind::SetFavorite => {
                let restores = marker_restores(&batch)?;
                let changes = self
                    .store
                    .restore_batch(&restores, self.clock.unix_millis())?;
                self.consume(batch.batch_id)?;
                if self.projection.sync_markers(&changes).is_err() {
                    return Err(UndoServiceError::CommittedButProjectionStale);
                }
            }
            OperationKind::Copy | OperationKind::Trash => {
                return Err(UndoServiceError::StackChanged);
            }
        }
        Ok(Some(UndoReceipt {
            batch_id: batch.batch_id,
            kind: batch.kind,
            action_count: u32::try_from(batch.actions.len()).unwrap_or(u32::MAX),
        }))
    }

    fn consume(&self, batch_id: OperationId) -> Result<(), UndoServiceError> {
        self.stack
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .consume(batch_id)
            .then_some(())
            .ok_or(UndoServiceError::StackChanged)
    }
}

fn marker_restores(batch: &UndoBatch) -> Result<Vec<MarkerRestore>, UndoServiceError> {
    batch
        .actions
        .iter()
        .map(|action| match action {
            UndoAction::ReviewState {
                target,
                previous,
                expected,
            }
            | UndoAction::Favorite {
                target,
                previous,
                expected,
            } => Ok(MarkerRestore {
                target: target.clone(),
                expected: *expected,
                previous: *previous,
            }),
            UndoAction::File { .. } => Err(UndoServiceError::StackChanged),
        })
        .collect()
}

async fn validate_file_batch(
    project_root: &Path,
    mutation: &dyn FileMutationPort,
    batch: &UndoBatch,
) -> Result<(), UndoError> {
    let root = std::fs::canonicalize(project_root).map_err(|error| {
        FileOperationError::io("canonicalize undo project", project_root, &error)
    })?;
    let current_paths = batch
        .actions
        .iter()
        .filter_map(|action| match action {
            UndoAction::File { current, .. } => Some(root.join(current.as_str())),
            UndoAction::ReviewState { .. } | UndoAction::Favorite { .. } => None,
        })
        .collect::<std::collections::HashSet<_>>();
    for action in &batch.actions {
        let UndoAction::File {
            current,
            restore,
            expected,
            ..
        } = action
        else {
            return Err(UndoError::OutsideProject);
        };
        let candidate = root.join(current.as_str());
        let metadata = std::fs::symlink_metadata(&candidate)
            .map_err(|error| FileOperationError::io("inspect undo source", &candidate, &error))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(UndoError::OutsideProject);
        }
        let canonical = std::fs::canonicalize(&candidate)
            .map_err(|error| FileOperationError::io("resolve undo source", &candidate, &error))?;
        if canonical != candidate || !canonical.starts_with(&root) {
            return Err(UndoError::OutsideProject);
        }
        if mutation.snapshot(&canonical).await? != *expected {
            return Err(UndoError::IdentityChanged);
        }
        let restore_candidate = root.join(restore.as_str());
        let parent = restore_candidate
            .parent()
            .ok_or(UndoError::OutsideProject)?;
        let canonical_parent = std::fs::canonicalize(parent).map_err(|error| {
            FileOperationError::io("resolve undo restore parent", parent, &error)
        })?;
        if canonical_parent != parent || !canonical_parent.starts_with(&root) {
            return Err(UndoError::OutsideProject);
        }
        match std::fs::symlink_metadata(&restore_candidate) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    return Err(UndoError::DestinationOccupied);
                }
                if !current_paths.contains(&restore_candidate) {
                    let canonical_restore =
                        std::fs::canonicalize(&restore_candidate).map_err(|error| {
                            FileOperationError::io(
                                "resolve occupied undo restore destination",
                                &restore_candidate,
                                &error,
                            )
                        })?;
                    if canonical_restore != canonical {
                        return Err(UndoError::DestinationOccupied);
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(FileOperationError::io(
                    "inspect undo restore destination",
                    &restore_candidate,
                    &error,
                )
                .into());
            }
        }
    }
    Ok(())
}
