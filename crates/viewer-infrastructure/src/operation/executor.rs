use super::{
    copy::{create_temporary_sync, hash_file_sync, sync_parent},
    journal::{JournalError, JournalItem, OperationJournal},
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use viewer_application::{
    ClockPort, FaultInjector, FileMutationPort, FileOperationError, InjectedCrash, NoFaults,
    OperationCommit, OperationCommitError, OperationCommitPort,
};
use viewer_domain::{
    OperationId, RelativePath,
    operation::{OperationItemPlan, OperationKind, OperationState},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CopyResult {
    pub len: u64,
    pub hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CopyResumeResult {
    Completed(CopyResult),
    CleanedTemporary,
}

#[derive(Debug, thiserror::Error)]
pub enum CopyError {
    #[error(transparent)]
    File(#[from] FileOperationError),
    #[error(transparent)]
    Journal(#[from] JournalError),
    #[error(transparent)]
    Injected(#[from] InjectedCrash),
    #[error(transparent)]
    Commit(#[from] OperationCommitError),
    #[error("copy executor received a non-copy operation")]
    WrongOperationKind,
    #[error("operation {0} is not present in the journal")]
    MissingJournalItem(OperationId),
    #[error("operation {0} does not have complete recovery evidence")]
    MissingEvidence(OperationId),
    #[error("operation {operation_id} cannot resume from state {state:?}")]
    NotResumable {
        operation_id: OperationId,
        state: OperationState,
    },
}

impl CopyError {
    pub const fn injected_state(&self) -> Option<OperationState> {
        match self {
            Self::Injected(error) => Some(error.state),
            _ => None,
        }
    }
}

pub struct CopyExecutor {
    project_root: PathBuf,
    journal: Arc<OperationJournal>,
    mutation: Arc<dyn FileMutationPort>,
    clock: Arc<dyn ClockPort>,
    commits: Arc<dyn OperationCommitPort>,
    faults: Arc<dyn FaultInjector>,
}

impl CopyExecutor {
    pub fn new(
        project_root: impl AsRef<Path>,
        journal: Arc<OperationJournal>,
        mutation: Arc<dyn FileMutationPort>,
        clock: Arc<dyn ClockPort>,
        commits: Arc<dyn OperationCommitPort>,
    ) -> Result<Self, FileOperationError> {
        Self::with_faults(
            project_root,
            journal,
            mutation,
            clock,
            commits,
            Arc::new(NoFaults),
        )
    }

    pub fn with_faults(
        project_root: impl AsRef<Path>,
        journal: Arc<OperationJournal>,
        mutation: Arc<dyn FileMutationPort>,
        clock: Arc<dyn ClockPort>,
        commits: Arc<dyn OperationCommitPort>,
        faults: Arc<dyn FaultInjector>,
    ) -> Result<Self, FileOperationError> {
        let project_root = std::fs::canonicalize(project_root.as_ref()).map_err(|error| {
            FileOperationError::io("canonicalize project root", project_root.as_ref(), &error)
        })?;
        Ok(Self {
            project_root,
            journal,
            mutation,
            clock,
            commits,
            faults,
        })
    }

    pub async fn execute(&self, item: &OperationItemPlan) -> Result<CopyResult, CopyError> {
        if item.kind != OperationKind::Copy {
            return Err(CopyError::WrongOperationKind);
        }
        let destination_relative = item
            .destination
            .as_ref()
            .ok_or(FileOperationError::DestinationRequired)?;
        let source = self.resolve_existing(&item.source)?;
        let destination = self.resolve_destination(destination_relative)?;
        if destination.exists() {
            return Err(FileOperationError::DestinationExists.into());
        }
        let temporary_relative = temporary_relative_path(destination_relative, item.operation_id)?;
        let temporary = self.resolve_destination(&temporary_relative)?;

        let temporary_for_create = temporary.clone();
        tokio::task::spawn_blocking(move || create_temporary_sync(&temporary_for_create))
            .await
            .map_err(|error| FileOperationError::Io {
                action: "temporary creation worker",
                path: temporary.clone(),
                message: error.to_string(),
            })??;
        if let Err(error) = self.journal.register_temporary(
            item.operation_id,
            OperationState::Prepared,
            &temporary_relative,
            self.now(),
        ) {
            self.mutation
                .remove_registered_temporary(&temporary)
                .await?;
            return Err(error.into());
        }
        self.after_persist(item.operation_id, OperationState::Prepared)?;
        self.journal.advance(
            item.operation_id,
            OperationState::Prepared,
            OperationState::Staged,
            self.now(),
        )?;
        self.after_persist(item.operation_id, OperationState::Staged)?;

        let source_before = self.mutation.snapshot(&source).await?;
        let copied = match self.mutation.copy_and_hash(&source, &temporary).await {
            Ok(copied) => copied,
            Err(FileOperationError::Cancelled) => {
                self.mutation
                    .remove_registered_temporary(&temporary)
                    .await?;
                self.journal.fail(
                    item.operation_id,
                    OperationState::Staged,
                    "cancelled",
                    self.now(),
                )?;
                return Err(FileOperationError::Cancelled.into());
            }
            Err(error) => return Err(error.into()),
        };
        let source_after = self.mutation.snapshot(&source).await?;
        if source_before != source_after || copied.0 != source_before.len {
            return Err(FileOperationError::IdentityChanged.into());
        }
        self.journal.record_fs_applied(
            item.operation_id,
            OperationState::Staged,
            copied.0,
            copied.1,
            self.now(),
        )?;
        self.after_persist(item.operation_id, OperationState::FsApplied)?;

        self.verify_expected(&temporary, copied).await?;
        self.journal.advance(
            item.operation_id,
            OperationState::FsApplied,
            OperationState::Verified,
            self.now(),
        )?;
        self.after_persist(item.operation_id, OperationState::Verified)?;

        self.place_verified(item.operation_id, &temporary, &destination)
            .await?;

        Ok(CopyResult {
            len: copied.0,
            hash: copied.1,
        })
    }

    pub async fn resume(&self, operation_id: OperationId) -> Result<CopyResumeResult, CopyError> {
        let item = self
            .journal
            .item(operation_id)?
            .ok_or(CopyError::MissingJournalItem(operation_id))?;
        if item.kind != OperationKind::Copy {
            return Err(CopyError::WrongOperationKind);
        }
        let destination_relative = item
            .destination
            .as_ref()
            .ok_or(FileOperationError::DestinationRequired)?;
        let expected_temporary = temporary_relative_path(destination_relative, operation_id)?;
        if item.temporary.as_ref() != Some(&expected_temporary) {
            return Err(CopyError::MissingEvidence(operation_id));
        }
        let temporary = self.resolve_destination(&expected_temporary)?;
        let destination = self.resolve_destination(destination_relative)?;

        match item.state {
            OperationState::Prepared | OperationState::Staged => {
                self.mutation
                    .remove_registered_temporary(&temporary)
                    .await?;
                self.journal.fail(
                    operation_id,
                    item.state,
                    "interrupted_before_verified_copy",
                    self.now(),
                )?;
                Ok(CopyResumeResult::CleanedTemporary)
            }
            OperationState::FsApplied => {
                let expected = expected_copy_evidence(&item)?;
                self.verify_expected(&temporary, expected).await?;
                self.journal.advance(
                    operation_id,
                    OperationState::FsApplied,
                    OperationState::Verified,
                    self.now(),
                )?;
                self.after_persist(operation_id, OperationState::Verified)?;
                self.place_verified(operation_id, &temporary, &destination)
                    .await?;
                Ok(CopyResumeResult::Completed(CopyResult {
                    len: expected.0,
                    hash: expected.1,
                }))
            }
            OperationState::Verified => {
                let expected = expected_copy_evidence(&item)?;
                if temporary.exists() {
                    self.verify_expected(&temporary, expected).await?;
                    self.place_verified(operation_id, &temporary, &destination)
                        .await?;
                } else if destination.exists() {
                    self.verify_expected(&destination, expected).await?;
                    self.finish_journal(operation_id, OperationState::Verified)
                        .await?;
                } else {
                    return Err(CopyError::MissingEvidence(operation_id));
                }
                Ok(CopyResumeResult::Completed(CopyResult {
                    len: expected.0,
                    hash: expected.1,
                }))
            }
            OperationState::MetaCommitted => {
                let expected = expected_copy_evidence(&item)?;
                self.verify_expected(&destination, expected).await?;
                self.finish_journal(operation_id, OperationState::MetaCommitted)
                    .await?;
                Ok(CopyResumeResult::Completed(CopyResult {
                    len: expected.0,
                    hash: expected.1,
                }))
            }
            OperationState::IndexSynced => {
                let expected = expected_copy_evidence(&item)?;
                self.verify_expected(&destination, expected).await?;
                self.journal.complete_item(
                    operation_id,
                    OperationState::IndexSynced,
                    "completed",
                    self.now(),
                )?;
                self.after_persist(operation_id, OperationState::Completed)?;
                Ok(CopyResumeResult::Completed(CopyResult {
                    len: expected.0,
                    hash: expected.1,
                }))
            }
            OperationState::Completed | OperationState::Failed => Err(CopyError::NotResumable {
                operation_id,
                state: item.state,
            }),
        }
    }

    async fn verify_expected(
        &self,
        path: &Path,
        expected: (u64, [u8; 32]),
    ) -> Result<(), CopyError> {
        let verification_path = path.to_path_buf();
        let error_path = verification_path.clone();
        let actual = tokio::task::spawn_blocking(move || hash_file_sync(&verification_path))
            .await
            .map_err(|error| FileOperationError::Io {
                action: "copy verification worker",
                path: error_path,
                message: error.to_string(),
            })??;
        if actual != expected {
            return Err(FileOperationError::VerificationFailed.into());
        }
        Ok(())
    }

    async fn place_verified(
        &self,
        operation_id: OperationId,
        temporary: &Path,
        destination: &Path,
    ) -> Result<(), CopyError> {
        if destination.exists() {
            return Err(FileOperationError::DestinationExists.into());
        }
        self.mutation.rename(temporary, destination).await?;
        let destination_for_sync = destination.to_path_buf();
        let error_path = destination_for_sync.clone();
        tokio::task::spawn_blocking(move || sync_parent(&destination_for_sync))
            .await
            .map_err(|error| FileOperationError::Io {
                action: "destination directory sync worker",
                path: error_path,
                message: error.to_string(),
            })??;
        self.finish_journal(operation_id, OperationState::Verified)
            .await?;
        Ok(())
    }

    async fn finish_journal(
        &self,
        operation_id: OperationId,
        mut current: OperationState,
    ) -> Result<(), CopyError> {
        let item = self
            .journal
            .item(operation_id)?
            .ok_or(CopyError::MissingJournalItem(operation_id))?;
        let commit = operation_commit(&item);
        if current == OperationState::Verified {
            self.commits.commit_metadata(&commit).await?;
            self.journal.advance(
                operation_id,
                OperationState::Verified,
                OperationState::MetaCommitted,
                self.now(),
            )?;
            current = OperationState::MetaCommitted;
            self.after_persist(operation_id, current)?;
        }
        if current == OperationState::MetaCommitted {
            self.commits.sync_index(&commit).await?;
            self.journal.advance(
                operation_id,
                OperationState::MetaCommitted,
                OperationState::IndexSynced,
                self.now(),
            )?;
            current = OperationState::IndexSynced;
            self.after_persist(operation_id, current)?;
        }
        self.journal.complete_item(
            operation_id,
            OperationState::IndexSynced,
            "completed",
            self.now(),
        )?;
        self.after_persist(operation_id, OperationState::Completed)
    }

    fn resolve_existing(&self, relative: &RelativePath) -> Result<PathBuf, FileOperationError> {
        let candidate = self.project_root.join(relative.as_str());
        let canonical = std::fs::canonicalize(&candidate).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                FileOperationError::SourceMissing
            } else {
                FileOperationError::io("resolve operation source", &candidate, &error)
            }
        })?;
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
            FileOperationError::io("resolve operation destination parent", parent, &error)
        })?;
        if !canonical_parent.starts_with(&self.project_root) {
            return Err(FileOperationError::OutsideProject);
        }
        let file_name = candidate
            .file_name()
            .ok_or(FileOperationError::OutsideProject)?;
        Ok(canonical_parent.join(file_name))
    }

    fn now(&self) -> i64 {
        self.clock.unix_millis()
    }

    fn after_persist(
        &self,
        operation_id: OperationId,
        state: OperationState,
    ) -> Result<(), CopyError> {
        self.faults
            .after_persist(operation_id, state)
            .map_err(Into::into)
    }
}

fn operation_commit(item: &JournalItem) -> OperationCommit {
    OperationCommit {
        operation_id: item.operation_id,
        entity_id: item.entity_id,
        kind: item.kind,
        source: item.source.clone(),
        destination: item.destination.clone(),
    }
}

fn expected_copy_evidence(item: &JournalItem) -> Result<(u64, [u8; 32]), CopyError> {
    item.expected_size
        .zip(item.expected_hash)
        .ok_or(CopyError::MissingEvidence(item.operation_id))
}

pub(crate) fn temporary_relative_path(
    destination: &RelativePath,
    operation_id: OperationId,
) -> Result<RelativePath, FileOperationError> {
    let destination = Path::new(destination.as_str());
    let temporary_name = format!(".viewer-copy-{operation_id}.part");
    let temporary = match destination.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(temporary_name),
        _ => PathBuf::from(temporary_name),
    };
    let temporary = temporary
        .to_str()
        .ok_or(FileOperationError::OutsideProject)?;
    RelativePath::parse(temporary).map_err(|_| FileOperationError::ReservedPath)
}
