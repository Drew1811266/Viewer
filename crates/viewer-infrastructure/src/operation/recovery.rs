use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use viewer_application::{ClockPort, FileMutationPort, FileOperationError, TrashPort};
use viewer_domain::{
    OperationId, RelativePath,
    operation::{ConflictPolicy, OperationKind, OperationState},
};

use super::{
    conflict::replace_temporary_path,
    copy::{hash_file_sync, sync_parent},
    executor::{CopyError, CopyExecutor, CopyResumeResult, temporary_relative_path},
    journal::{JournalError, JournalItem, OperationJournal},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryActionKind {
    CleanedTemporary,
    RestoredSource,
    RestoredSourcePreviousDestinationInTrash,
    Completed,
    MarkedFailed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryAction {
    pub operation_id: OperationId,
    pub kind: RecoveryActionKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NeedsUserReview {
    pub operation_id: OperationId,
    pub reason: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RecoveryReport {
    pub actions: Vec<RecoveryAction>,
    pub needs_user_review: Vec<NeedsUserReview>,
}

#[derive(Debug, thiserror::Error)]
pub enum RecoveryError {
    #[error(transparent)]
    File(#[from] FileOperationError),
    #[error(transparent)]
    Journal(#[from] JournalError),
    #[error(transparent)]
    Copy(#[from] CopyError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CandidateStatus {
    Missing,
    Match,
    Conflict,
}

pub struct RecoveryService {
    project_root: PathBuf,
    journal: Arc<OperationJournal>,
    mutation: Arc<dyn FileMutationPort>,
    _trash: Arc<dyn TrashPort>,
    clock: Arc<dyn ClockPort>,
}

impl RecoveryService {
    pub fn new(
        project_root: impl AsRef<Path>,
        journal: Arc<OperationJournal>,
        mutation: Arc<dyn FileMutationPort>,
        trash: Arc<dyn TrashPort>,
        clock: Arc<dyn ClockPort>,
    ) -> Result<Self, FileOperationError> {
        let project_root = std::fs::canonicalize(project_root.as_ref()).map_err(|error| {
            FileOperationError::io(
                "canonicalize recovery project root",
                project_root.as_ref(),
                &error,
            )
        })?;
        Ok(Self {
            project_root,
            journal,
            mutation,
            _trash: trash,
            clock,
        })
    }

    pub async fn recover_project(&self) -> Result<RecoveryReport, RecoveryError> {
        let mut report = RecoveryReport::default();
        for item in self.journal.incomplete_items()? {
            let outcome = match item.kind {
                OperationKind::Copy => self.recover_copy(&item).await,
                OperationKind::Rename | OperationKind::Move
                    if item.conflict_policy == ConflictPolicy::Replace =>
                {
                    self.recover_replace(&item).await
                }
                OperationKind::Rename | OperationKind::Move => self.recover_rename(&item).await,
                OperationKind::Trash
                | OperationKind::SetReviewState
                | OperationKind::SetFavorite => Ok(RecoveryOutcome::Review(
                    "operation kind has no automatic filesystem recovery protocol".into(),
                )),
            }?;
            match outcome {
                RecoveryOutcome::Action(kind) => report.actions.push(RecoveryAction {
                    operation_id: item.operation_id,
                    kind,
                }),
                RecoveryOutcome::Review(reason) => {
                    report.needs_user_review.push(NeedsUserReview {
                        operation_id: item.operation_id,
                        reason,
                    });
                }
            }
        }
        Ok(report)
    }

    async fn recover_copy(&self, item: &JournalItem) -> Result<RecoveryOutcome, RecoveryError> {
        let source = self.resolve_candidate(&item.source)?;
        let destination = self.resolve_required_destination(item)?;
        let expected_temporary = temporary_relative_path(
            item.destination
                .as_ref()
                .ok_or(FileOperationError::DestinationRequired)?,
            item.operation_id,
        )?;
        if item.temporary.as_ref() != Some(&expected_temporary) {
            return Ok(RecoveryOutcome::Review(
                "copy registered temporary path does not match its operation ID".into(),
            ));
        }
        let temporary = match item.temporary.as_ref() {
            Some(path) => self.resolve_candidate(path)?,
            None => {
                return Ok(RecoveryOutcome::Review(
                    "copy has no registered temporary path".into(),
                ));
            }
        };

        match item.state {
            OperationState::Prepared | OperationState::Staged => {
                if !safe_regular_file(&source) {
                    return Ok(RecoveryOutcome::Review(
                        "copy source is missing, a symlink, or no longer a regular file".into(),
                    ));
                }
                self.mutation
                    .remove_registered_temporary(&temporary)
                    .await?;
                self.journal.fail(
                    item.operation_id,
                    item.state,
                    "recovered_before_verified_copy",
                    self.now(),
                )?;
                Ok(RecoveryOutcome::Action(
                    RecoveryActionKind::CleanedTemporary,
                ))
            }
            OperationState::FsApplied
            | OperationState::Verified
            | OperationState::MetaCommitted
            | OperationState::IndexSynced => {
                let expected = evidence(item)?;
                let temporary_status = candidate_status(&temporary, expected).await?;
                let destination_status = candidate_status(&destination, expected).await?;
                if temporary_status == CandidateStatus::Match
                    && destination_status == CandidateStatus::Match
                {
                    return Ok(RecoveryOutcome::Review(
                        "stored copy evidence matches both temporary and final paths".into(),
                    ));
                }
                if matches!(temporary_status, CandidateStatus::Conflict)
                    || matches!(destination_status, CandidateStatus::Conflict)
                {
                    return Ok(RecoveryOutcome::Review(
                        "a copy recovery path is occupied by unknown content".into(),
                    ));
                }

                if item.state == OperationState::FsApplied
                    && temporary_status == CandidateStatus::Missing
                    && destination_status == CandidateStatus::Match
                {
                    self.finish_from(item, OperationState::FsApplied)?;
                    return Ok(RecoveryOutcome::Action(RecoveryActionKind::Completed));
                }

                let resumable = match item.state {
                    OperationState::FsApplied => temporary_status == CandidateStatus::Match,
                    OperationState::Verified => {
                        temporary_status == CandidateStatus::Match
                            || destination_status == CandidateStatus::Match
                    }
                    OperationState::MetaCommitted | OperationState::IndexSynced => {
                        temporary_status == CandidateStatus::Missing
                            && destination_status == CandidateStatus::Match
                    }
                    _ => false,
                };
                if !resumable {
                    return Ok(RecoveryOutcome::Review(
                        "copy recovery evidence does not identify one safe candidate".into(),
                    ));
                }
                let executor = CopyExecutor::new(
                    &self.project_root,
                    Arc::clone(&self.journal),
                    Arc::clone(&self.mutation),
                    Arc::clone(&self.clock),
                )?;
                let result = executor.resume(item.operation_id).await?;
                Ok(RecoveryOutcome::Action(match result {
                    CopyResumeResult::Completed(_) => RecoveryActionKind::Completed,
                    CopyResumeResult::CleanedTemporary => RecoveryActionKind::CleanedTemporary,
                }))
            }
            OperationState::Completed | OperationState::Failed => Ok(RecoveryOutcome::Review(
                "terminal operation unexpectedly appeared in recovery query".into(),
            )),
        }
    }

    async fn recover_rename(&self, item: &JournalItem) -> Result<RecoveryOutcome, RecoveryError> {
        let expected = match optional_evidence(item) {
            Some(expected) => expected,
            None => {
                return Ok(RecoveryOutcome::Review(
                    "rename has no persisted size/hash identity".into(),
                ));
            }
        };
        let source = self.resolve_candidate(&item.source)?;
        let destination = self.resolve_required_destination(item)?;
        let temporary = item
            .temporary
            .as_ref()
            .map(|path| self.resolve_candidate(path))
            .transpose()?;
        if let Some(registered) = item.temporary.as_ref()
            && !is_expected_rename_temporary(&item.source, item.operation_id, registered)
        {
            return Ok(RecoveryOutcome::Review(
                "rename registered temporary path does not match its operation ID".into(),
            ));
        }
        let source_status = candidate_status(&source, expected).await?;
        let destination_status = candidate_status(&destination, expected).await?;
        let temporary_status = match temporary.as_ref() {
            Some(path) => candidate_status(path, expected).await?,
            None => CandidateStatus::Missing,
        };

        if [source_status, destination_status, temporary_status]
            .into_iter()
            .filter(|status| *status == CandidateStatus::Match)
            .count()
            > 1
        {
            return Ok(RecoveryOutcome::Review(
                "stored rename evidence matches multiple candidate paths".into(),
            ));
        }

        match item.state {
            OperationState::Prepared => {
                if source_status == CandidateStatus::Match
                    && temporary_status == CandidateStatus::Missing
                {
                    self.mark_failed(item, "recovered_before_rename")?;
                    return Ok(RecoveryOutcome::Action(RecoveryActionKind::MarkedFailed));
                }
                if source_status == CandidateStatus::Missing
                    && temporary_status == CandidateStatus::Match
                {
                    self.restore_source(
                        item,
                        temporary.as_ref().expect("matched temporary"),
                        &source,
                    )
                    .await?;
                    return Ok(RecoveryOutcome::Action(RecoveryActionKind::RestoredSource));
                }
            }
            OperationState::Staged => {
                if temporary.is_none()
                    && source_status == CandidateStatus::Match
                    && destination_status == CandidateStatus::Missing
                {
                    self.mark_failed(item, "recovered_before_direct_rename")?;
                    return Ok(RecoveryOutcome::Action(RecoveryActionKind::MarkedFailed));
                }
                if temporary_status == CandidateStatus::Match
                    && destination_status == CandidateStatus::Missing
                {
                    self.place_and_finish(
                        item,
                        temporary.as_ref().expect("matched temporary"),
                        &destination,
                        expected,
                    )
                    .await?;
                    return Ok(RecoveryOutcome::Action(RecoveryActionKind::Completed));
                }
                if temporary_status == CandidateStatus::Match
                    && source_status == CandidateStatus::Missing
                {
                    self.restore_source(
                        item,
                        temporary.as_ref().expect("matched temporary"),
                        &source,
                    )
                    .await?;
                    return Ok(RecoveryOutcome::Action(RecoveryActionKind::RestoredSource));
                }
                if destination_status == CandidateStatus::Match
                    && source_status == CandidateStatus::Missing
                    && temporary_status == CandidateStatus::Missing
                {
                    self.record_applied_and_finish(item, expected)?;
                    return Ok(RecoveryOutcome::Action(RecoveryActionKind::Completed));
                }
            }
            OperationState::FsApplied
            | OperationState::Verified
            | OperationState::MetaCommitted
            | OperationState::IndexSynced => {
                if destination_status == CandidateStatus::Match
                    && source_status == CandidateStatus::Missing
                    && temporary_status == CandidateStatus::Missing
                {
                    self.finish_from(item, item.state)?;
                    return Ok(RecoveryOutcome::Action(RecoveryActionKind::Completed));
                }
            }
            OperationState::Completed | OperationState::Failed => {}
        }

        Ok(RecoveryOutcome::Review(
            "rename paths do not match one evidence-backed recovery decision".into(),
        ))
    }

    async fn recover_replace(&self, item: &JournalItem) -> Result<RecoveryOutcome, RecoveryError> {
        let expected = match optional_evidence(item) {
            Some(expected) => expected,
            None => {
                return Ok(RecoveryOutcome::Review(
                    "replace has no persisted replacement identity".into(),
                ));
            }
        };
        let source = self.resolve_candidate(&item.source)?;
        let destination = self.resolve_required_destination(item)?;
        let temporary = match item.temporary.as_ref() {
            Some(path) => self.resolve_candidate(path)?,
            None => {
                return Ok(RecoveryOutcome::Review(
                    "replace has no registered staged path".into(),
                ));
            }
        };
        if item.temporary.as_ref()
            != Some(&replace_temporary_path(&item.source, item.operation_id)?)
        {
            return Ok(RecoveryOutcome::Review(
                "replace registered temporary path does not match its operation ID".into(),
            ));
        }
        let source_status = candidate_status(&source, expected).await?;
        let destination_status = candidate_status(&destination, expected).await?;
        let temporary_status = candidate_status(&temporary, expected).await?;
        if [source_status, destination_status, temporary_status]
            .into_iter()
            .filter(|status| *status == CandidateStatus::Match)
            .count()
            > 1
        {
            return Ok(RecoveryOutcome::Review(
                "stored replacement evidence matches multiple candidate paths".into(),
            ));
        }

        match item.state {
            OperationState::Prepared => {
                if source_status == CandidateStatus::Match
                    && temporary_status == CandidateStatus::Missing
                {
                    self.mark_failed(item, "recovered_before_replace")?;
                    return Ok(RecoveryOutcome::Action(RecoveryActionKind::MarkedFailed));
                }
                if source_status == CandidateStatus::Missing
                    && temporary_status == CandidateStatus::Match
                {
                    let previous_in_trash = destination_status == CandidateStatus::Missing;
                    self.restore_source(item, &temporary, &source).await?;
                    return Ok(RecoveryOutcome::Action(if previous_in_trash {
                        RecoveryActionKind::RestoredSourcePreviousDestinationInTrash
                    } else {
                        RecoveryActionKind::RestoredSource
                    }));
                }
            }
            OperationState::Staged => {
                if source_status == CandidateStatus::Missing
                    && temporary_status == CandidateStatus::Match
                {
                    let previous_in_trash = destination_status == CandidateStatus::Missing;
                    self.restore_source(item, &temporary, &source).await?;
                    return Ok(RecoveryOutcome::Action(if previous_in_trash {
                        RecoveryActionKind::RestoredSourcePreviousDestinationInTrash
                    } else {
                        RecoveryActionKind::RestoredSource
                    }));
                }
                if destination_status == CandidateStatus::Match
                    && source_status == CandidateStatus::Missing
                    && temporary_status == CandidateStatus::Missing
                {
                    self.record_applied_and_finish(item, expected)?;
                    return Ok(RecoveryOutcome::Action(RecoveryActionKind::Completed));
                }
            }
            OperationState::FsApplied
            | OperationState::Verified
            | OperationState::MetaCommitted
            | OperationState::IndexSynced => {
                if destination_status == CandidateStatus::Match
                    && source_status == CandidateStatus::Missing
                    && temporary_status == CandidateStatus::Missing
                {
                    self.finish_from(item, item.state)?;
                    return Ok(RecoveryOutcome::Action(RecoveryActionKind::Completed));
                }
            }
            OperationState::Completed | OperationState::Failed => {}
        }

        Ok(RecoveryOutcome::Review(
            "replace paths do not match one evidence-backed recovery decision".into(),
        ))
    }

    async fn place_and_finish(
        &self,
        item: &JournalItem,
        source: &Path,
        destination: &Path,
        expected: (u64, [u8; 32]),
    ) -> Result<(), RecoveryError> {
        self.mutation.rename(source, destination).await?;
        sync_path(destination).await?;
        if candidate_status(destination, expected).await? != CandidateStatus::Match {
            return Err(FileOperationError::VerificationFailed.into());
        }
        self.record_applied_and_finish(item, expected)
    }

    async fn restore_source(
        &self,
        item: &JournalItem,
        temporary: &Path,
        source: &Path,
    ) -> Result<(), RecoveryError> {
        if source.exists() {
            return Err(FileOperationError::DestinationExists.into());
        }
        self.mutation.rename(temporary, source).await?;
        sync_path(source).await?;
        self.mark_failed(item, "restored_interrupted_source")
    }

    fn record_applied_and_finish(
        &self,
        item: &JournalItem,
        expected: (u64, [u8; 32]),
    ) -> Result<(), RecoveryError> {
        self.journal.record_fs_applied(
            item.operation_id,
            OperationState::Staged,
            expected.0,
            expected.1,
            self.now(),
        )?;
        self.finish_from(item, OperationState::FsApplied)
    }

    fn finish_from(
        &self,
        item: &JournalItem,
        mut current: OperationState,
    ) -> Result<(), RecoveryError> {
        for next in [
            OperationState::Verified,
            OperationState::MetaCommitted,
            OperationState::IndexSynced,
            OperationState::Completed,
        ] {
            if state_rank(next) <= state_rank(current) {
                continue;
            }
            self.journal
                .advance(item.operation_id, current, next, self.now())?;
            current = next;
        }
        Ok(())
    }

    fn mark_failed(&self, item: &JournalItem, code: &str) -> Result<(), RecoveryError> {
        self.journal
            .fail(item.operation_id, item.state, code, self.now())?;
        Ok(())
    }

    fn resolve_required_destination(&self, item: &JournalItem) -> Result<PathBuf, RecoveryError> {
        let relative = item
            .destination
            .as_ref()
            .ok_or(FileOperationError::DestinationRequired)?;
        self.resolve_candidate(relative).map_err(Into::into)
    }

    fn resolve_candidate(&self, relative: &RelativePath) -> Result<PathBuf, FileOperationError> {
        let candidate = self.project_root.join(relative.as_str());
        let parent = candidate
            .parent()
            .ok_or(FileOperationError::OutsideProject)?;
        let canonical_parent = std::fs::canonicalize(parent).map_err(|error| {
            FileOperationError::io("resolve recovery path parent", parent, &error)
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
}

enum RecoveryOutcome {
    Action(RecoveryActionKind),
    Review(String),
}

fn evidence(item: &JournalItem) -> Result<(u64, [u8; 32]), RecoveryError> {
    optional_evidence(item).ok_or_else(|| {
        FileOperationError::Io {
            action: "read recovery evidence",
            path: PathBuf::from(item.source.as_str()),
            message: "persisted size/hash is incomplete".into(),
        }
        .into()
    })
}

fn optional_evidence(item: &JournalItem) -> Option<(u64, [u8; 32])> {
    item.expected_size.zip(item.expected_hash)
}

async fn candidate_status(
    path: &Path,
    expected: (u64, [u8; 32]),
) -> Result<CandidateStatus, FileOperationError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CandidateStatus::Missing);
        }
        Err(error) => {
            return Err(FileOperationError::io(
                "inspect recovery candidate",
                path,
                &error,
            ));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Ok(CandidateStatus::Conflict);
    }
    let fingerprint_path = path.to_path_buf();
    let error_path = fingerprint_path.clone();
    let actual = tokio::task::spawn_blocking(move || hash_file_sync(&fingerprint_path))
        .await
        .map_err(|error| FileOperationError::Io {
            action: "recovery fingerprint worker",
            path: error_path,
            message: error.to_string(),
        })??;
    Ok(if actual == expected {
        CandidateStatus::Match
    } else {
        CandidateStatus::Conflict
    })
}

fn safe_regular_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
}

fn is_expected_rename_temporary(
    source: &RelativePath,
    operation_id: OperationId,
    registered: &RelativePath,
) -> bool {
    let source = Path::new(source.as_str());
    let expected_name = format!(".viewer-rename-{operation_id}.part");
    let expected = match source.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(expected_name),
        _ => PathBuf::from(expected_name),
    };
    expected.to_str() == Some(registered.as_str())
}

async fn sync_path(path: &Path) -> Result<(), FileOperationError> {
    let path = path.to_path_buf();
    let error_path = path.clone();
    tokio::task::spawn_blocking(move || sync_parent(&path))
        .await
        .map_err(|error| FileOperationError::Io {
            action: "recovery directory sync worker",
            path: error_path,
            message: error.to_string(),
        })?
}

const fn state_rank(state: OperationState) -> u8 {
    match state {
        OperationState::Prepared => 0,
        OperationState::Staged => 1,
        OperationState::FsApplied => 2,
        OperationState::Verified => 3,
        OperationState::MetaCommitted => 4,
        OperationState::IndexSynced => 5,
        OperationState::Completed => 6,
        OperationState::Failed => 7,
    }
}
