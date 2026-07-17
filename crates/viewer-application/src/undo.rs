use crate::{FileMutationPort, FileOperationError, FileSnapshot};
use std::path::Path;
use viewer_domain::{EntityId, OperationId, RelativePath, SessionId, operation::OperationKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UndoAction {
    File {
        entity_id: EntityId,
        current: RelativePath,
        restore: RelativePath,
        expected: FileSnapshot,
    },
    ReviewState {
        entity_id: EntityId,
        previous: Option<String>,
    },
    Favorite {
        entity_id: EntityId,
        previous: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UndoBatch {
    pub batch_id: OperationId,
    pub kind: OperationKind,
    pub actions: Vec<UndoAction>,
}

#[derive(Debug, thiserror::Error)]
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
}
