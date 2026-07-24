#[cfg(not(target_os = "macos"))]
use super::evidence::copy_open_files_and_evidence;
#[cfg(target_os = "macos")]
use super::evidence::{copy_open_files_and_evidence_with_validation, worker_error};
use super::file_reference::file_snapshot;
#[cfg(target_os = "macos")]
use super::file_reference::{
    BoundFileReference, bind_open_file_reference, open_bound_parent, openat_file,
    resolve_file_reference, unlink_file_reference, validate_bound_parent_location,
};
#[cfg(target_os = "macos")]
use super::placement::place_bound_staged_copy_sync;
#[cfg(not(target_os = "macos"))]
use super::placement::{sync_parent, verify_parent_identity};
#[cfg(target_os = "macos")]
use async_trait::async_trait;
#[cfg(not(target_os = "macos"))]
use std::fs::OpenOptions;
#[cfg(target_os = "macos")]
use std::{fs, path::PathBuf};
use std::{fs::File, path::Path};
use viewer_application::{
    FileContentEvidence, FileOperationError, FileSnapshot, file_commands::FileCommandCancellation,
    watcher::FileIdentity,
};
#[cfg(target_os = "macos")]
use viewer_application::{StagedCopy, StagedCopyLeasePort};

#[cfg(target_os = "macos")]
pub(super) struct MacStagedCopyLease {
    pub(super) temporary_reference: BoundFileReference,
    pub(super) temporary_parent: File,
    pub(super) temporary_parent_path: PathBuf,
    pub(super) temporary_path: PathBuf,
    pub(super) temporary_snapshot: FileSnapshot,
    pub(super) temporary_parent_identity: FileIdentity,
    pub(super) armed: bool,
}

#[cfg(target_os = "macos")]
pub(super) struct MacStagedCopyBuild {
    evidence: FileContentEvidence,
    pub(super) lease: MacStagedCopyLease,
}

#[cfg(target_os = "macos")]
impl MacStagedCopyBuild {
    fn into_staged(self) -> StagedCopy {
        StagedCopy::new(self.evidence, Box::new(self.lease))
    }

    fn into_legacy_evidence(mut self) -> FileContentEvidence {
        self.lease.armed = false;
        self.evidence
    }
}

#[cfg(target_os = "macos")]
impl Drop for MacStagedCopyLease {
    fn drop(&mut self) {
        if self.armed {
            let _ = unlink_file_reference(&self.temporary_reference, &self.temporary_path);
            let _ = self.temporary_parent.sync_all();
            self.armed = false;
        }
    }
}

#[cfg(target_os = "macos")]
pub(super) fn create_registered_temporary_verified_sync(
    path: &Path,
    expected_parent: FileIdentity,
) -> Result<(), FileOperationError> {
    let parent = path.parent().ok_or(FileOperationError::OutsideProject)?;
    let directory = open_bound_parent(parent, expected_parent)?;
    let file = openat_file(
        &directory,
        path,
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        0o600,
        "create bound registered temporary",
    )?;
    file.sync_all()
        .map_err(|error| FileOperationError::io("sync bound registered temporary", path, &error))?;
    directory
        .sync_all()
        .map_err(|error| FileOperationError::io("sync bound directory", parent, &error))
}

#[cfg(not(target_os = "macos"))]
pub(super) fn create_registered_temporary_verified_sync(
    path: &Path,
    _expected_parent: FileIdentity,
) -> Result<(), FileOperationError> {
    create_temporary_sync(path)
}

#[cfg(target_os = "macos")]
#[async_trait]
impl StagedCopyLeasePort for MacStagedCopyLease {
    async fn place(
        self: Box<Self>,
        destination: &Path,
        expected_parent: FileIdentity,
    ) -> Result<(), FileOperationError> {
        let destination = destination.to_path_buf();
        let error_path = destination.clone();
        tokio::task::spawn_blocking(move || {
            place_bound_staged_copy_sync(self, &destination, expected_parent)
        })
        .await
        .map_err(|error| worker_error("bound staged placement worker", &error_path, error))?
    }
}

#[cfg(target_os = "macos")]
pub(super) fn create_copy_and_hash_cancellable_verified_sync_with_hook<F>(
    source: &Path,
    temporary: &Path,
    cancellation: &FileCommandCancellation,
    expected_source: &FileSnapshot,
    source_parent_identity: FileIdentity,
    temporary_parent_identity: FileIdentity,
    after_create: F,
) -> Result<FileContentEvidence, FileOperationError>
where
    F: FnOnce() -> Result<(), FileOperationError>,
{
    create_copy_and_hash_cancellable_verified_sync_with_hooks(
        source,
        temporary,
        cancellation,
        expected_source,
        source_parent_identity,
        temporary_parent_identity,
        || Ok(()),
        || Ok(()),
        after_create,
    )
}

#[cfg(target_os = "macos")]
#[allow(clippy::too_many_arguments)]
pub(super) fn create_copy_and_hash_cancellable_verified_sync_with_hooks<F, G, H>(
    source: &Path,
    temporary: &Path,
    cancellation: &FileCommandCancellation,
    expected_source: &FileSnapshot,
    source_parent_identity: FileIdentity,
    temporary_parent_identity: FileIdentity,
    after_os_create: F,
    after_identity_bound: G,
    after_bound_create: H,
) -> Result<FileContentEvidence, FileOperationError>
where
    F: FnOnce() -> Result<(), FileOperationError>,
    G: FnOnce() -> Result<(), FileOperationError>,
    H: FnOnce() -> Result<(), FileOperationError>,
{
    build_staged_copy_cancellable_verified_sync_with_hooks(
        source,
        temporary,
        cancellation,
        expected_source,
        source_parent_identity,
        temporary_parent_identity,
        after_os_create,
        after_identity_bound,
        after_bound_create,
    )
    .map(MacStagedCopyBuild::into_legacy_evidence)
}

#[cfg(target_os = "macos")]
#[allow(clippy::too_many_arguments)]
pub(super) fn create_staged_copy_cancellable_verified_sync_with_hooks<F, G, H>(
    source: &Path,
    temporary: &Path,
    cancellation: &FileCommandCancellation,
    expected_source: &FileSnapshot,
    source_parent_identity: FileIdentity,
    temporary_parent_identity: FileIdentity,
    after_os_create: F,
    after_identity_bound: G,
    after_bound_create: H,
) -> Result<StagedCopy, FileOperationError>
where
    F: FnOnce() -> Result<(), FileOperationError>,
    G: FnOnce() -> Result<(), FileOperationError>,
    H: FnOnce() -> Result<(), FileOperationError>,
{
    build_staged_copy_cancellable_verified_sync_with_hooks(
        source,
        temporary,
        cancellation,
        expected_source,
        source_parent_identity,
        temporary_parent_identity,
        after_os_create,
        after_identity_bound,
        after_bound_create,
    )
    .map(MacStagedCopyBuild::into_staged)
}

#[cfg(target_os = "macos")]
#[allow(clippy::too_many_arguments)]
pub(super) fn build_staged_copy_cancellable_verified_sync_with_hooks<F, G, H>(
    source: &Path,
    temporary: &Path,
    cancellation: &FileCommandCancellation,
    expected_source: &FileSnapshot,
    source_parent_identity: FileIdentity,
    temporary_parent_identity: FileIdentity,
    after_os_create: F,
    after_identity_bound: G,
    after_bound_create: H,
) -> Result<MacStagedCopyBuild, FileOperationError>
where
    F: FnOnce() -> Result<(), FileOperationError>,
    G: FnOnce() -> Result<(), FileOperationError>,
    H: FnOnce() -> Result<(), FileOperationError>,
{
    let source_parent_path = source.parent().ok_or(FileOperationError::OutsideProject)?;
    let source_parent = open_bound_parent(source_parent_path, source_parent_identity)?;
    let temporary_parent_path = temporary
        .parent()
        .ok_or(FileOperationError::OutsideProject)?;
    let temporary_parent = open_bound_parent(temporary_parent_path, temporary_parent_identity)?;
    validate_bound_parent_location(&source_parent, source_parent_path, source_parent_identity)?;
    validate_bound_parent_location(
        &temporary_parent,
        temporary_parent_path,
        temporary_parent_identity,
    )?;
    let source_file = openat_file(
        &source_parent,
        source,
        libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        0,
        "open bound copy source",
    )?;
    if file_snapshot(&source_file)? != *expected_source {
        return Err(FileOperationError::IdentityChanged);
    }
    validate_bound_parent_location(
        &temporary_parent,
        temporary_parent_path,
        temporary_parent_identity,
    )?;
    let temporary_file = openat_file(
        &temporary_parent,
        temporary,
        libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        0o600,
        "create bound registered copy temporary",
    )?;
    if let Err(primary) = after_os_create() {
        return Err(unbound_copy_cleanup_obligation(primary, temporary));
    }
    let temporary_snapshot = match file_snapshot(&temporary_file) {
        Ok(snapshot) => snapshot,
        Err(primary) => return Err(unbound_copy_cleanup_obligation(primary, temporary)),
    };
    let temporary_reference =
        match bind_open_file_reference(&temporary_file, &temporary_snapshot, temporary) {
            Ok(reference) => reference,
            Err(primary) => return Err(unbound_copy_cleanup_obligation(primary, temporary)),
        };
    let temporary_identity_path = match resolve_file_reference(&temporary_reference, temporary) {
        Ok(path) => path,
        Err(primary) => {
            return Err(bound_copy_cleanup_error(
                &temporary_reference,
                &temporary_parent,
                temporary_parent_path,
                temporary,
                primary,
            ));
        }
    };
    let binding_validation =
        validate_bound_parent_location(&source_parent, source_parent_path, source_parent_identity)
            .and_then(|()| {
                validate_bound_parent_location(
                    &temporary_parent,
                    temporary_parent_path,
                    temporary_parent_identity,
                )
            })
            .and_then(|()| {
                if temporary_identity_path == temporary {
                    Ok(())
                } else {
                    Err(FileOperationError::IdentityChanged)
                }
            });
    if let Err(primary) = binding_validation {
        return Err(bound_copy_cleanup_error(
            &temporary_reference,
            &temporary_parent,
            temporary_parent_path,
            temporary,
            primary,
        ));
    }
    let sync_result = after_identity_bound().and_then(|()| {
        temporary_parent.sync_all().map_err(|error| {
            FileOperationError::io(
                "sync bound registered copy directory",
                temporary_parent_path,
                &error,
            )
        })
    });
    if let Err(primary) = sync_result {
        return Err(bound_copy_cleanup_error(
            &temporary_reference,
            &temporary_parent,
            temporary_parent_path,
            temporary,
            primary,
        ));
    }
    let copied = after_bound_create().and_then(|()| {
        copy_open_files_and_evidence_with_validation(
            source_file,
            temporary_file,
            source,
            temporary,
            cancellation,
            expected_source,
            || {
                validate_bound_parent_location(
                    &source_parent,
                    source_parent_path,
                    source_parent_identity,
                )?;
                validate_bound_parent_location(
                    &temporary_parent,
                    temporary_parent_path,
                    temporary_parent_identity,
                )
            },
        )
    });
    match copied {
        Ok(evidence) => {
            let staged_snapshot = evidence.snapshot.clone();
            Ok(MacStagedCopyBuild {
                evidence,
                lease: MacStagedCopyLease {
                    temporary_reference,
                    temporary_parent,
                    temporary_parent_path: temporary_parent_path.to_path_buf(),
                    temporary_path: temporary.to_path_buf(),
                    temporary_snapshot: staged_snapshot,
                    temporary_parent_identity,
                    armed: true,
                },
            })
        }
        Err(primary) => Err(bound_copy_cleanup_error(
            &temporary_reference,
            &temporary_parent,
            temporary_parent_path,
            temporary,
            primary,
        )),
    }
}

#[cfg(target_os = "macos")]
pub(super) fn bound_copy_cleanup_error(
    temporary_reference: &BoundFileReference,
    temporary_parent: &File,
    temporary_parent_path: &Path,
    temporary: &Path,
    primary: FileOperationError,
) -> FileOperationError {
    bound_copy_cleanup_error_with_sync(temporary_reference, temporary, primary, || {
        temporary_parent.sync_all().map_err(|error| {
            FileOperationError::io(
                "sync registered temporary cleanup directory",
                temporary_parent_path,
                &error,
            )
        })
    })
}

#[cfg(target_os = "macos")]
pub(super) fn bound_copy_cleanup_error_with_sync<F>(
    temporary_reference: &BoundFileReference,
    temporary: &Path,
    primary: FileOperationError,
    sync_parent: F,
) -> FileOperationError
where
    F: FnOnce() -> Result<(), FileOperationError>,
{
    let cleanup = match unlink_file_reference(temporary_reference, temporary) {
        Ok(()) => match sync_parent() {
            Ok(()) => match fs::symlink_metadata(temporary) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return primary,
                Ok(_) => FileOperationError::IdentityChanged,
                Err(error) => FileOperationError::io(
                    "inspect registered temporary after identity cleanup",
                    temporary,
                    &error,
                ),
            },
            Err(cleanup) => cleanup,
        },
        Err(cleanup) => cleanup,
    };
    FileOperationError::RegisteredTemporaryCleanupRequired {
        primary: Box::new(primary),
        cleanup: Box::new(cleanup),
    }
}

fn unbound_copy_cleanup_obligation(
    primary: FileOperationError,
    temporary: &Path,
) -> FileOperationError {
    FileOperationError::RegisteredTemporaryCleanupRequired {
        primary: Box::new(primary),
        cleanup: Box::new(FileOperationError::Io {
            action: "bind registered temporary cleanup identity",
            path: temporary.to_path_buf(),
            message: "temporary creation succeeded before a cleanup identity was available".into(),
        }),
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn create_copy_and_hash_cancellable_verified_sync_with_hook<F>(
    source: &Path,
    temporary: &Path,
    cancellation: &FileCommandCancellation,
    expected_source: &FileSnapshot,
    source_parent_identity: FileIdentity,
    temporary_parent_identity: FileIdentity,
    after_create: F,
) -> Result<FileContentEvidence, FileOperationError>
where
    F: FnOnce() -> Result<(), FileOperationError>,
{
    verify_parent_identity(source, source_parent_identity)?;
    verify_parent_identity(temporary, temporary_parent_identity)?;
    let source_file = File::open(source)
        .map_err(|error| FileOperationError::io("open bound copy source", source, &error))?;
    if file_snapshot(&source_file)? != *expected_source {
        return Err(FileOperationError::IdentityChanged);
    }
    let temporary_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(temporary)
        .map_err(|error| {
            FileOperationError::io("create bound registered copy temporary", temporary, &error)
        })?;
    if let Err(primary) = sync_parent(temporary) {
        return Err(unbound_copy_cleanup_obligation(primary, temporary));
    }
    match after_create().and_then(|()| {
        copy_open_files_and_evidence(
            source_file,
            temporary_file,
            source,
            temporary,
            cancellation,
            expected_source,
        )
    }) {
        Ok(evidence) => Ok(evidence),
        Err(primary) => Err(unbound_copy_cleanup_obligation(primary, temporary)),
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn create_temporary_sync(path: &Path) -> Result<(), FileOperationError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| FileOperationError::io("create registered temporary", path, &error))?;
    sync_parent(path)
}
