#[cfg(unix)]
pub(super) use super::evidence::unix_timestamp_ns;
use super::evidence::{
    copy_and_hash_cancellable_sync, copy_and_hash_cancellable_verified_sync, copy_and_hash_sync,
    snapshot_sync, worker_error,
};
pub(super) use super::placement::snapshot_matches_bound_move;
use super::placement::{rename_no_replace_sync, rename_verified_sync};
#[cfg(target_os = "macos")]
use super::staged::create_staged_copy_cancellable_verified_sync_with_hooks;
use super::staged::{
    create_copy_and_hash_cancellable_verified_sync_with_hook,
    create_registered_temporary_verified_sync,
};
use async_trait::async_trait;
use std::{path::Path, sync::Arc};
use viewer_application::{
    FileContentEvidence, FileMutationPort, FileOperationError, FileSnapshot, StagedCopy,
    file_commands::FileCommandCancellation, watcher::FileIdentity,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct LocalFileMutation;

#[async_trait]
impl FileMutationPort for LocalFileMutation {
    async fn snapshot(&self, path: &Path) -> Result<FileSnapshot, FileOperationError> {
        let path = path.to_path_buf();
        let error_path = path.clone();
        tokio::task::spawn_blocking(move || snapshot_sync(&path))
            .await
            .map_err(|error| worker_error("snapshot worker", &error_path, error))?
    }

    async fn copy_and_hash(
        &self,
        source: &Path,
        temporary: &Path,
    ) -> Result<(u64, [u8; 32]), FileOperationError> {
        let source = source.to_path_buf();
        let temporary = temporary.to_path_buf();
        let error_path = temporary.clone();
        tokio::task::spawn_blocking(move || copy_and_hash_sync(&source, &temporary))
            .await
            .map_err(|error| worker_error("copy worker", &error_path, error))?
    }

    async fn copy_and_hash_cancellable(
        &self,
        source: &Path,
        temporary: &Path,
        cancellation: &FileCommandCancellation,
    ) -> Result<(u64, [u8; 32]), FileOperationError> {
        let source = source.to_path_buf();
        let temporary = temporary.to_path_buf();
        let error_path = temporary.clone();
        let cancellation = cancellation.clone();
        tokio::task::spawn_blocking(move || {
            copy_and_hash_cancellable_sync(&source, &temporary, &cancellation)
        })
        .await
        .map_err(|error| worker_error("cancellable copy worker", &error_path, error))?
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
        let source = source.to_path_buf();
        let temporary = temporary.to_path_buf();
        let error_path = temporary.clone();
        let cancellation = cancellation.clone();
        let expected_source = expected_source.clone();
        tokio::task::spawn_blocking(move || {
            copy_and_hash_cancellable_verified_sync(
                &source,
                &temporary,
                &cancellation,
                &expected_source,
                source_parent,
                temporary_parent,
            )
        })
        .await
        .map_err(|error| worker_error("bound copy worker", &error_path, error))?
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
        let source = source.to_path_buf();
        let temporary = temporary.to_path_buf();
        let error_path = temporary.clone();
        let cancellation = cancellation.clone();
        let expected_source = expected_source.clone();
        tokio::task::spawn_blocking(move || {
            create_copy_and_hash_cancellable_verified_sync_with_hook(
                &source,
                &temporary,
                &cancellation,
                &expected_source,
                source_parent,
                temporary_parent,
                || Ok(()),
            )
        })
        .await
        .map_err(|error| worker_error("bound registered copy worker", &error_path, error))?
    }

    #[cfg(target_os = "macos")]
    async fn create_staged_copy_cancellable_verified(
        self: Arc<Self>,
        source: &Path,
        temporary: &Path,
        cancellation: &FileCommandCancellation,
        expected_source: &FileSnapshot,
        source_parent: FileIdentity,
        temporary_parent: FileIdentity,
    ) -> Result<StagedCopy, FileOperationError> {
        let source = source.to_path_buf();
        let temporary = temporary.to_path_buf();
        let error_path = temporary.clone();
        let cancellation = cancellation.clone();
        let expected_source = expected_source.clone();
        tokio::task::spawn_blocking(move || {
            create_staged_copy_cancellable_verified_sync_with_hooks(
                &source,
                &temporary,
                &cancellation,
                &expected_source,
                source_parent,
                temporary_parent,
                || Ok(()),
                || Ok(()),
                || Ok(()),
            )
        })
        .await
        .map_err(|error| worker_error("bound staged copy worker", &error_path, error))?
    }

    async fn rename(&self, source: &Path, destination: &Path) -> Result<(), FileOperationError> {
        let source = source.to_path_buf();
        let destination = destination.to_path_buf();
        let error_path = destination.clone();
        tokio::task::spawn_blocking(move || rename_no_replace_sync(&source, &destination))
            .await
            .map_err(|error| worker_error("rename worker", &error_path, error))?
    }

    async fn rename_verified(
        &self,
        source: &Path,
        destination: &Path,
        expected: &FileSnapshot,
        source_parent: FileIdentity,
        destination_parent: FileIdentity,
    ) -> Result<(), FileOperationError> {
        let source = source.to_path_buf();
        let destination = destination.to_path_buf();
        let error_path = destination.clone();
        let expected = expected.clone();
        tokio::task::spawn_blocking(move || {
            rename_verified_sync(
                &source,
                &destination,
                &expected,
                source_parent,
                destination_parent,
            )
        })
        .await
        .map_err(|error| worker_error("bound rename worker", &error_path, error))?
    }

    async fn create_registered_temporary(
        &self,
        path: &Path,
        expected_parent: FileIdentity,
    ) -> Result<(), FileOperationError> {
        let path = path.to_path_buf();
        let error_path = path.clone();
        tokio::task::spawn_blocking(move || {
            create_registered_temporary_verified_sync(&path, expected_parent)
        })
        .await
        .map_err(|error| worker_error("bound temporary worker", &error_path, error))?
    }
}
