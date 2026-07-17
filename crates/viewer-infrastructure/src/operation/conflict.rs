use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use viewer_application::{
    ClockPort, FaultInjector, FileMutationPort, FileOperationError, InjectedCrash, NoFaults,
    TrashPort,
};
use viewer_domain::{
    OperationId, RelativePath,
    operation::{ConflictPolicy, OperationItemPlan, OperationKind, OperationState},
};

use super::{
    copy::{hash_file_sync, sync_parent},
    journal::{JournalError, OperationJournal},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConflictResult {
    Skipped,
    Placed(PathBuf),
}

#[derive(Debug, thiserror::Error)]
pub enum ConflictError {
    #[error(transparent)]
    File(#[from] FileOperationError),
    #[error(
        "previous destination was moved to the system Trash, but placement from {source:?} to {destination:?} failed: {cause}"
    )]
    PlacementAfterTrash {
        source: PathBuf,
        destination: PathBuf,
        #[source]
        cause: FileOperationError,
    },
}

pub struct ConflictExecutor {
    mutation: Arc<dyn FileMutationPort>,
    trash: Arc<dyn TrashPort>,
}

impl ConflictExecutor {
    pub fn new(mutation: Arc<dyn FileMutationPort>, trash: Arc<dyn TrashPort>) -> Self {
        Self { mutation, trash }
    }

    pub async fn place(
        &self,
        source: &Path,
        requested_destination: &Path,
        policy: ConflictPolicy,
    ) -> Result<ConflictResult, ConflictError> {
        if !requested_destination.exists() {
            self.mutation.rename(source, requested_destination).await?;
            return Ok(ConflictResult::Placed(requested_destination.to_path_buf()));
        }

        match policy {
            ConflictPolicy::Skip => Ok(ConflictResult::Skipped),
            ConflictPolicy::KeepBoth => {
                let destination = keep_both_destination(requested_destination)?;
                self.mutation.rename(source, &destination).await?;
                Ok(ConflictResult::Placed(destination))
            }
            ConflictPolicy::Replace => {
                self.trash.trash(requested_destination).await?;
                if let Err(cause) = self.mutation.rename(source, requested_destination).await {
                    return Err(ConflictError::PlacementAfterTrash {
                        source: source.to_path_buf(),
                        destination: requested_destination.to_path_buf(),
                        cause,
                    });
                }
                Ok(ConflictResult::Placed(requested_destination.to_path_buf()))
            }
        }
    }
}

pub fn keep_both_destination(destination: &Path) -> Result<PathBuf, FileOperationError> {
    let parent = destination
        .parent()
        .ok_or(FileOperationError::OutsideProject)?;
    let stem = destination
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or(FileOperationError::OutsideProject)?;
    let extension = destination.extension().and_then(|value| value.to_str());

    for index in 1_u32.. {
        let suffix = if index == 1 {
            " copy".to_owned()
        } else {
            format!(" copy {index}")
        };
        let file_name = match extension {
            Some(extension) => format!("{stem}{suffix}.{extension}"),
            None => format!("{stem}{suffix}"),
        };
        let candidate = parent.join(file_name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    unreachable!("u32 keep-both suffix space cannot be exhausted")
}

#[derive(Debug, thiserror::Error)]
pub enum ReplaceError {
    #[error(transparent)]
    File(#[from] FileOperationError),
    #[error(transparent)]
    Journal(#[from] JournalError),
    #[error(transparent)]
    Injected(#[from] InjectedCrash),
    #[error("replace executor requires rename or move with Replace conflict policy")]
    WrongOperation,
    #[error("previous destination is in the system Trash, but replacement placement failed: {0}")]
    PlacementAfterTrash(FileOperationError),
}

impl ReplaceError {
    pub const fn injected_state(&self) -> Option<OperationState> {
        match self {
            Self::Injected(error) => Some(error.state),
            _ => None,
        }
    }
}

pub struct ReplaceExecutor {
    project_root: PathBuf,
    journal: Arc<OperationJournal>,
    mutation: Arc<dyn FileMutationPort>,
    trash: Arc<dyn TrashPort>,
    clock: Arc<dyn ClockPort>,
    faults: Arc<dyn FaultInjector>,
}

impl ReplaceExecutor {
    pub fn new(
        project_root: impl AsRef<Path>,
        journal: Arc<OperationJournal>,
        mutation: Arc<dyn FileMutationPort>,
        trash: Arc<dyn TrashPort>,
        clock: Arc<dyn ClockPort>,
    ) -> Result<Self, FileOperationError> {
        Self::with_faults(
            project_root,
            journal,
            mutation,
            trash,
            clock,
            Arc::new(NoFaults),
        )
    }

    pub fn with_faults(
        project_root: impl AsRef<Path>,
        journal: Arc<OperationJournal>,
        mutation: Arc<dyn FileMutationPort>,
        trash: Arc<dyn TrashPort>,
        clock: Arc<dyn ClockPort>,
        faults: Arc<dyn FaultInjector>,
    ) -> Result<Self, FileOperationError> {
        let project_root = std::fs::canonicalize(project_root.as_ref()).map_err(|error| {
            FileOperationError::io(
                "canonicalize replace project root",
                project_root.as_ref(),
                &error,
            )
        })?;
        Ok(Self {
            project_root,
            journal,
            mutation,
            trash,
            clock,
            faults,
        })
    }

    pub async fn execute(&self, item: &OperationItemPlan) -> Result<(), ReplaceError> {
        if !matches!(item.kind, OperationKind::Rename | OperationKind::Move)
            || item.conflict_policy != ConflictPolicy::Replace
        {
            return Err(ReplaceError::WrongOperation);
        }
        let destination_relative = item
            .destination
            .as_ref()
            .ok_or(FileOperationError::DestinationRequired)?;
        let source = self.resolve_existing(&item.source)?;
        let destination = self.resolve_existing(destination_relative)?;
        let temporary_relative = replace_temporary_path(&item.source, item.operation_id)?;
        let temporary = self.resolve_destination(&temporary_relative)?;

        let before = self.mutation.snapshot(&source).await?;
        let expected = hash_path(&source).await?;
        let after = self.mutation.snapshot(&source).await?;
        if before != after || before.len != expected.0 {
            return Err(FileOperationError::IdentityChanged.into());
        }
        self.journal.record_prepared_evidence(
            item.operation_id,
            Some(&temporary_relative),
            expected.0,
            expected.1,
            self.now(),
        )?;
        self.after_persist(item.operation_id, OperationState::Prepared)?;

        self.mutation.rename(&source, &temporary).await?;
        sync_path(&temporary).await?;
        self.journal.advance(
            item.operation_id,
            OperationState::Prepared,
            OperationState::Staged,
            self.now(),
        )?;
        self.after_persist(item.operation_id, OperationState::Staged)?;

        self.trash.trash(&destination).await?;
        if let Err(error) = self.mutation.rename(&temporary, &destination).await {
            return Err(ReplaceError::PlacementAfterTrash(error));
        }
        sync_path(&destination).await?;
        if hash_path(&destination).await? != expected {
            return Err(FileOperationError::VerificationFailed.into());
        }
        self.journal.record_fs_applied(
            item.operation_id,
            OperationState::Staged,
            expected.0,
            expected.1,
            self.now(),
        )?;
        self.after_persist(item.operation_id, OperationState::FsApplied)?;

        for (current, next) in [
            (OperationState::FsApplied, OperationState::Verified),
            (OperationState::Verified, OperationState::MetaCommitted),
            (OperationState::MetaCommitted, OperationState::IndexSynced),
            (OperationState::IndexSynced, OperationState::Completed),
        ] {
            self.journal
                .advance(item.operation_id, current, next, self.now())?;
            self.after_persist(item.operation_id, next)?;
        }
        Ok(())
    }

    fn resolve_existing(&self, relative: &RelativePath) -> Result<PathBuf, FileOperationError> {
        let candidate = self.project_root.join(relative.as_str());
        let metadata = std::fs::symlink_metadata(&candidate).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                FileOperationError::SourceMissing
            } else {
                FileOperationError::io("inspect replace path", &candidate, &error)
            }
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(FileOperationError::OutsideProject);
        }
        let canonical = std::fs::canonicalize(&candidate)
            .map_err(|error| FileOperationError::io("resolve replace path", &candidate, &error))?;
        if !canonical.starts_with(&self.project_root) {
            return Err(FileOperationError::OutsideProject);
        }
        Ok(canonical)
    }

    fn resolve_destination(&self, relative: &RelativePath) -> Result<PathBuf, FileOperationError> {
        let candidate = self.project_root.join(relative.as_str());
        let parent = candidate
            .parent()
            .ok_or(FileOperationError::OutsideProject)?;
        let canonical_parent = std::fs::canonicalize(parent).map_err(|error| {
            FileOperationError::io("resolve replace temporary parent", parent, &error)
        })?;
        if !canonical_parent.starts_with(&self.project_root) {
            return Err(FileOperationError::OutsideProject);
        }
        Ok(canonical_parent.join(
            candidate
                .file_name()
                .ok_or(FileOperationError::OutsideProject)?,
        ))
    }

    fn now(&self) -> i64 {
        self.clock.unix_millis()
    }

    fn after_persist(
        &self,
        operation_id: OperationId,
        state: OperationState,
    ) -> Result<(), InjectedCrash> {
        self.faults.after_persist(operation_id, state)
    }
}

pub(crate) fn replace_temporary_path(
    source: &RelativePath,
    operation_id: OperationId,
) -> Result<RelativePath, FileOperationError> {
    let source = Path::new(source.as_str());
    let temporary = match source.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => {
            parent.join(format!(".viewer-replace-{operation_id}.part"))
        }
        _ => PathBuf::from(format!(".viewer-replace-{operation_id}.part")),
    };
    RelativePath::parse(
        temporary
            .to_str()
            .ok_or(FileOperationError::OutsideProject)?,
    )
    .map_err(|_| FileOperationError::ReservedPath)
}

async fn hash_path(path: &Path) -> Result<(u64, [u8; 32]), FileOperationError> {
    let path = path.to_path_buf();
    let error_path = path.clone();
    tokio::task::spawn_blocking(move || hash_file_sync(&path))
        .await
        .map_err(|error| FileOperationError::Io {
            action: "replace fingerprint worker",
            path: error_path,
            message: error.to_string(),
        })?
}

async fn sync_path(path: &Path) -> Result<(), FileOperationError> {
    let path = path.to_path_buf();
    let error_path = path.clone();
    tokio::task::spawn_blocking(move || sync_parent(&path))
        .await
        .map_err(|error| FileOperationError::Io {
            action: "replace directory sync worker",
            path: error_path,
            message: error.to_string(),
        })?
}
