use crate::{
    FileContentEvidence, FileOperationError, FileSnapshot, ImageArtifact, ImageError, ImageRequest,
    file_commands::{
        BatchId, FileCommand, FileCommandCancellation, FileCommandItemExecution,
        LocalFileCommandError, LocalFileCommandOutcome, LocalFileCommandPreflightItem,
    },
    finder_drag::{FinderDragError, PreparedFinderDrag},
    scan::{ScanError, ScanRequest, ScanSink},
    search::SearchError,
    watcher::FileIdentity,
};

pub trait FinderDragPort {
    fn begin_drag(&self, selection: &PreparedFinderDrag) -> Result<(), FinderDragError>;
}
use async_trait::async_trait;
use std::fmt;
use std::path::{Path, PathBuf};
use viewer_domain::{
    SessionId,
    image::ImageProbe,
    search::{Generation, SearchPage, SearchQuery},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectAccess {
    ReadWrite,
    ReadOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectProbeOperation {
    ReadMetadata,
    ReadDirectory,
    CreateWriteProbe,
    RemoveWriteProbe,
}

impl fmt::Display for ProjectProbeOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::ReadMetadata => "read project root metadata",
            Self::ReadDirectory => "read project root directory",
            Self::CreateWriteProbe => "create project write probe",
            Self::RemoveWriteProbe => "remove project write probe",
        };
        formatter.write_str(label)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProjectProbeError {
    #[error("project root is not a directory: {path}")]
    NotDirectory { path: PathBuf },
    #[error("failed to {operation} at {path}: {message}")]
    Io {
        operation: ProjectProbeOperation,
        path: PathBuf,
        message: String,
    },
}

impl ProjectProbeError {
    pub fn io(
        operation: ProjectProbeOperation,
        path: impl Into<PathBuf>,
        error: &std::io::Error,
    ) -> Self {
        Self::Io {
            operation,
            path: path.into(),
            message: error.to_string(),
        }
    }
}

pub trait ProjectProbePort: Send + Sync {
    fn probe(&self, root: &Path) -> Result<ProjectAccess, ProjectProbeError>;
}

pub trait ClockPort: Send + Sync {
    fn unix_millis(&self) -> i64;
}

#[async_trait]
pub trait FileMutationPort: Send + Sync {
    async fn snapshot(&self, path: &Path) -> Result<FileSnapshot, FileOperationError>;

    async fn copy_and_hash(
        &self,
        source: &Path,
        temporary: &Path,
    ) -> Result<(u64, [u8; 32]), FileOperationError>;

    async fn copy_and_hash_cancellable(
        &self,
        source: &Path,
        temporary: &Path,
        cancellation: &FileCommandCancellation,
    ) -> Result<(u64, [u8; 32]), FileOperationError> {
        if cancellation.is_cancelled() {
            return Err(FileOperationError::Cancelled);
        }
        let result = self.copy_and_hash(source, temporary).await?;
        if cancellation.is_cancelled() {
            return Err(FileOperationError::Cancelled);
        }
        Ok(result)
    }

    async fn copy_and_hash_cancellable_verified(
        &self,
        source: &Path,
        temporary: &Path,
        cancellation: &FileCommandCancellation,
        expected_source: &FileSnapshot,
        source_parent: FileIdentity,
        temporary_parent: FileIdentity,
    ) -> Result<(u64, [u8; 32]), FileOperationError> {
        verify_directory_identity(
            source.parent().ok_or(FileOperationError::OutsideProject)?,
            source_parent,
        )?;
        verify_directory_identity(
            temporary
                .parent()
                .ok_or(FileOperationError::OutsideProject)?,
            temporary_parent,
        )?;
        if self.snapshot(source).await? != *expected_source {
            return Err(FileOperationError::IdentityChanged);
        }
        self.copy_and_hash_cancellable(source, temporary, cancellation)
            .await
    }

    async fn create_and_copy_cancellable_verified(
        &self,
        source: &Path,
        temporary: &Path,
        cancellation: &FileCommandCancellation,
        expected_source: &FileSnapshot,
        source_parent: FileIdentity,
        temporary_parent: FileIdentity,
    ) -> Result<FileContentEvidence, FileOperationError> {
        self.create_registered_temporary(temporary, temporary_parent)
            .await?;
        let (len, hash) = self
            .copy_and_hash_cancellable_verified(
                source,
                temporary,
                cancellation,
                expected_source,
                source_parent,
                temporary_parent,
            )
            .await?;
        let snapshot = self.snapshot(temporary).await?;
        if snapshot.len != len {
            return Err(FileOperationError::IdentityChanged);
        }
        Ok(FileContentEvidence { snapshot, hash })
    }

    async fn rename(&self, source: &Path, destination: &Path) -> Result<(), FileOperationError>;

    async fn rename_verified(
        &self,
        source: &Path,
        destination: &Path,
        expected: &FileSnapshot,
        source_parent: FileIdentity,
        destination_parent: FileIdentity,
    ) -> Result<(), FileOperationError> {
        verify_directory_identity(
            source.parent().ok_or(FileOperationError::OutsideProject)?,
            source_parent,
        )?;
        verify_directory_identity(
            destination
                .parent()
                .ok_or(FileOperationError::OutsideProject)?,
            destination_parent,
        )?;
        if !snapshot_matches_bound_move(expected, &self.snapshot(source).await?) {
            return Err(FileOperationError::IdentityChanged);
        }
        self.rename(source, destination).await
    }

    async fn create_registered_temporary(
        &self,
        path: &Path,
        expected_parent: FileIdentity,
    ) -> Result<(), FileOperationError> {
        use std::{fs::OpenOptions, io::Write};
        verify_directory_identity(
            path.parent().ok_or(FileOperationError::OutsideProject)?,
            expected_parent,
        )?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| FileOperationError::io("create registered temporary", path, &error))?;
        file.flush()
            .map_err(|error| FileOperationError::io("flush registered temporary", path, &error))?;
        file.sync_all()
            .map_err(|error| FileOperationError::io("sync registered temporary", path, &error))
    }

    async fn remove_registered_temporary(&self, path: &Path) -> Result<(), FileOperationError>;

    async fn remove_registered_temporary_verified(
        &self,
        path: &Path,
        expected_parent: FileIdentity,
    ) -> Result<(), FileOperationError> {
        verify_directory_identity(
            path.parent().ok_or(FileOperationError::OutsideProject)?,
            expected_parent,
        )?;
        self.remove_registered_temporary(path).await
    }

    async fn remove_registered_temporary_bound(
        &self,
        path: &Path,
        expected_parent: FileIdentity,
        expected_leaf: Option<&FileSnapshot>,
    ) -> Result<(), FileOperationError> {
        if let Some(expected) = expected_leaf
            && !snapshot_matches_bound_move(expected, &self.snapshot(path).await?)
        {
            return Err(FileOperationError::IdentityChanged);
        }
        self.remove_registered_temporary_verified(path, expected_parent)
            .await
    }
}

fn verify_directory_identity(
    path: &Path,
    expected: FileIdentity,
) -> Result<(), FileOperationError> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| FileOperationError::io("inspect bound directory", path, &error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(FileOperationError::OutsideProject);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.dev() != expected.volume || metadata.ino() != expected.file {
            return Err(FileOperationError::IdentityChanged);
        }
    }
    Ok(())
}

fn snapshot_matches_bound_move(expected: &FileSnapshot, actual: &FileSnapshot) -> bool {
    expected.volume_id == actual.volume_id
        && expected.len == actual.len
        && expected.file_id == actual.file_id
        && expected.modified_ns == actual.modified_ns
}

#[async_trait]
pub trait TrashPort: Send + Sync {
    async fn trash(&self, path: &Path) -> Result<(), FileOperationError>;

    async fn trash_verified(
        &self,
        path: &Path,
        expected: &FileSnapshot,
        expected_parent: FileIdentity,
    ) -> Result<(), FileOperationError> {
        verify_directory_identity(
            path.parent().ok_or(FileOperationError::OutsideProject)?,
            expected_parent,
        )?;
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|error| FileOperationError::io("inspect bound Trash leaf", path, &error))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(FileOperationError::IdentityChanged);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let actual = FileSnapshot {
                len: metadata.len(),
                volume_id: metadata.dev(),
                file_id: Some(u128::from(metadata.ino())),
                modified_ns: Some(
                    i128::from(metadata.mtime()) * 1_000_000_000
                        + i128::from(metadata.mtime_nsec()),
                ),
                changed_ns: Some(
                    i128::from(metadata.ctime()) * 1_000_000_000
                        + i128::from(metadata.ctime_nsec()),
                ),
            };
            if !snapshot_matches_bound_move(expected, &actual) {
                return Err(FileOperationError::IdentityChanged);
            }
        }
        self.trash(path).await
    }
}

pub trait VolumePort: Send + Sync {
    fn volume_id(&self, path: &Path) -> Result<u64, FileOperationError>;
    fn is_case_sensitive(&self, path: &Path) -> Result<bool, FileOperationError>;
    fn name_max(&self, path: &Path) -> Result<usize, FileOperationError>;
}

#[async_trait]
pub trait LocalFileCommandPort: Send + Sync {
    async fn preflight(
        &self,
        batch_id: BatchId,
        command: &FileCommand,
    ) -> Result<Vec<LocalFileCommandPreflightItem>, LocalFileCommandError>;

    async fn discard_preflight(&self, _batch_id: BatchId) -> Result<(), LocalFileCommandError> {
        Ok(())
    }

    async fn execute_item(
        &self,
        request: FileCommandItemExecution,
        cancellation: FileCommandCancellation,
    ) -> Result<LocalFileCommandOutcome, LocalFileCommandError>;

    async fn settle_unstarted(
        &self,
        _request: FileCommandItemExecution,
        outcome: LocalFileCommandOutcome,
    ) -> Result<LocalFileCommandOutcome, LocalFileCommandError> {
        Ok(outcome)
    }

    async fn take_undo_actions(
        &self,
        _batch_id: BatchId,
    ) -> Result<Vec<crate::undo::UndoAction>, LocalFileCommandError> {
        Ok(Vec::new())
    }
}

#[async_trait]
pub trait ScanPort: Send + Sync {
    async fn scan(&self, request: ScanRequest, sink: ScanSink) -> Result<(), ScanError>;
}

#[async_trait]
pub trait SearchPort: Send + Sync {
    async fn search(
        &self,
        session_id: SessionId,
        generation: Generation,
        query: SearchQuery,
    ) -> Result<SearchPage, SearchError>;
}

#[async_trait]
pub trait SearchSnippetPort: Send + Sync {
    async fn text_snippet(
        &self,
        session_id: SessionId,
        generation: Generation,
        entity_id: viewer_domain::EntityId,
        query: String,
    ) -> Result<Option<String>, SearchError>;
}

#[async_trait]
pub trait ImagePort: Send + Sync {
    async fn probe(&self, source: &Path) -> Result<ImageProbe, ImageError>;
    async fn render(&self, request: ImageRequest) -> Result<ImageArtifact, ImageError>;
    async fn cancel_session(&self, session_id: SessionId);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    struct TypedProbe;

    impl ProjectProbePort for TypedProbe {
        fn probe(&self, root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
            Err(ProjectProbeError::NotDirectory {
                path: root.to_path_buf(),
            })
        }
    }

    #[test]
    fn project_probe_port_exposes_a_synchronous_typed_error() {
        let result: Result<ProjectAccess, ProjectProbeError> =
            TypedProbe.probe(Path::new("not-a-directory"));
        assert_eq!(
            result,
            Err(ProjectProbeError::NotDirectory {
                path: PathBuf::from("not-a-directory"),
            })
        );
    }
}
