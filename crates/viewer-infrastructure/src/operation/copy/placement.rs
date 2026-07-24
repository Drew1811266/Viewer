#[cfg(not(target_os = "macos"))]
use super::evidence::snapshot_sync;
use super::file_reference::file_snapshot;
#[cfg(target_os = "macos")]
use super::file_reference::{
    open_bound_parent, openat_file, renameat_no_replace, resolve_file_reference,
    validate_bound_parent_location,
};
#[cfg(target_os = "macos")]
use super::staged::{MacStagedCopyLease, bound_copy_cleanup_error};
#[cfg(not(target_os = "macos"))]
use std::fs;
use std::{fs::File, path::Path};
use viewer_application::{FileOperationError, FileSnapshot, watcher::FileIdentity};

pub(crate) fn sync_parent(path: &Path) -> Result<(), FileOperationError> {
    let parent = path.parent().ok_or(FileOperationError::OutsideProject)?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| FileOperationError::io("sync containing directory", parent, &error))
}

#[cfg(target_os = "macos")]
pub(super) fn rename_verified_sync(
    source: &Path,
    destination: &Path,
    expected: &FileSnapshot,
    source_parent_identity: FileIdentity,
    destination_parent_identity: FileIdentity,
) -> Result<(), FileOperationError> {
    rename_verified_sync_with_hook(
        source,
        destination,
        expected,
        source_parent_identity,
        destination_parent_identity,
        || {},
    )
}

#[cfg(target_os = "macos")]
pub(super) fn rename_verified_sync_with_hook<F>(
    source: &Path,
    destination: &Path,
    expected: &FileSnapshot,
    source_parent_identity: FileIdentity,
    destination_parent_identity: FileIdentity,
    after_leaf_open: F,
) -> Result<(), FileOperationError>
where
    F: FnOnce(),
{
    rename_verified_sync_with_hooks(
        source,
        destination,
        expected,
        source_parent_identity,
        destination_parent_identity,
        after_leaf_open,
        || {},
    )
}

#[cfg(target_os = "macos")]
pub(super) fn rename_verified_sync_with_hooks<F, G>(
    source: &Path,
    destination: &Path,
    expected: &FileSnapshot,
    source_parent_identity: FileIdentity,
    destination_parent_identity: FileIdentity,
    after_leaf_open: F,
    after_rename: G,
) -> Result<(), FileOperationError>
where
    F: FnOnce(),
    G: FnOnce(),
{
    let source_parent_path = source.parent().ok_or(FileOperationError::OutsideProject)?;
    let destination_parent_path = destination
        .parent()
        .ok_or(FileOperationError::OutsideProject)?;
    let source_parent = open_bound_parent(source_parent_path, source_parent_identity)?;
    let destination_parent =
        open_bound_parent(destination_parent_path, destination_parent_identity)?;
    let source_file = openat_file(
        &source_parent,
        source,
        libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        0,
        "open bound rename source",
    )?;
    if !snapshot_matches_bound_move(expected, &file_snapshot(&source_file)?) {
        return Err(FileOperationError::IdentityChanged);
    }
    after_leaf_open();
    validate_bound_parent_location(&source_parent, source_parent_path, source_parent_identity)?;
    validate_bound_parent_location(
        &destination_parent,
        destination_parent_path,
        destination_parent_identity,
    )?;
    renameat_no_replace(&source_parent, source, &destination_parent, destination)?;
    after_rename();
    if let Err(validation_error) =
        validate_bound_parent_location(&source_parent, source_parent_path, source_parent_identity)
            .and_then(|()| {
                validate_bound_parent_location(
                    &destination_parent,
                    destination_parent_path,
                    destination_parent_identity,
                )
            })
    {
        match renameat_no_replace(&destination_parent, destination, &source_parent, source) {
            Ok(()) => return Err(validation_error),
            Err(FileOperationError::DestinationExists) => {
                // The source name was occupied while rolling back. Keep the
                // expected leaf at the journal-visible destination, but only
                // after proving that path still names the moved identity.
                let destination_file = openat_file(
                    &destination_parent,
                    destination,
                    libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                    0,
                    "verify recovery-bound rename destination",
                )?;
                if snapshot_matches_bound_move(expected, &file_snapshot(&destination_file)?) {
                    return Err(validation_error);
                }
                return Err(FileOperationError::IdentityChanged);
            }
            Err(rollback_error) => return Err(rollback_error),
        }
    }
    let destination_file = openat_file(
        &destination_parent,
        destination,
        libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        0,
        "verify bound rename destination",
    )?;
    if snapshot_matches_bound_move(expected, &file_snapshot(&destination_file)?) {
        return Ok(());
    }
    // The leaf changed after its fd was validated. Restore the unexpected
    // entry to its original name without overwrite and stop the operation.
    renameat_no_replace(&destination_parent, destination, &source_parent, source)?;
    Err(FileOperationError::IdentityChanged)
}

#[cfg(target_os = "macos")]
pub(super) fn place_bound_staged_copy_sync(
    lease: Box<MacStagedCopyLease>,
    destination: &Path,
    expected_parent: FileIdentity,
) -> Result<(), FileOperationError> {
    place_bound_staged_copy_sync_with_hooks(
        lease,
        destination,
        expected_parent,
        || Ok(()),
        || Ok(()),
    )
}

#[cfg(target_os = "macos")]
pub(super) fn place_bound_staged_copy_sync_with_hooks<F, G>(
    mut lease: Box<MacStagedCopyLease>,
    destination: &Path,
    expected_parent: FileIdentity,
    before_sync: F,
    before_final_validation: G,
) -> Result<(), FileOperationError>
where
    F: FnOnce() -> Result<(), FileOperationError>,
    G: FnOnce() -> Result<(), FileOperationError>,
{
    match try_place_bound_staged_copy_sync_with_hooks(
        &lease,
        destination,
        expected_parent,
        before_sync,
        before_final_validation,
    ) {
        Ok(()) => {
            lease.armed = false;
            Ok(())
        }
        Err(primary) => {
            if validate_recoverable_staged_destination(&lease, destination, expected_parent).is_ok()
            {
                lease.armed = false;
                return Err(primary);
            }
            let error = bound_copy_cleanup_error(
                &lease.temporary_reference,
                &lease.temporary_parent,
                &lease.temporary_parent_path,
                &lease.temporary_path,
                primary,
            );
            if !matches!(
                error,
                FileOperationError::RegisteredTemporaryCleanupRequired { .. }
            ) {
                lease.armed = false;
            }
            Err(error)
        }
    }
}

#[cfg(target_os = "macos")]
fn validate_recoverable_staged_destination(
    lease: &MacStagedCopyLease,
    destination: &Path,
    expected_parent: FileIdentity,
) -> Result<(), FileOperationError> {
    if expected_parent != lease.temporary_parent_identity
        || destination.parent() != Some(lease.temporary_parent_path.as_path())
    {
        return Err(FileOperationError::IdentityChanged);
    }
    validate_bound_parent_location(
        &lease.temporary_parent,
        &lease.temporary_parent_path,
        lease.temporary_parent_identity,
    )?;
    if resolve_file_reference(&lease.temporary_reference, destination)? != destination {
        return Err(FileOperationError::IdentityChanged);
    }
    let destination_file = openat_file(
        &lease.temporary_parent,
        destination,
        libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        0,
        "verify recoverable staged copy destination",
    )?;
    if !snapshot_matches_bound_move(
        &lease.temporary_snapshot,
        &file_snapshot(&destination_file)?,
    ) {
        return Err(FileOperationError::IdentityChanged);
    }
    validate_bound_parent_location(
        &lease.temporary_parent,
        &lease.temporary_parent_path,
        lease.temporary_parent_identity,
    )?;
    if resolve_file_reference(&lease.temporary_reference, destination)? != destination {
        return Err(FileOperationError::IdentityChanged);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn try_place_bound_staged_copy_sync_with_hooks<F, G>(
    lease: &MacStagedCopyLease,
    destination: &Path,
    expected_parent: FileIdentity,
    before_sync: F,
    before_final_validation: G,
) -> Result<(), FileOperationError>
where
    F: FnOnce() -> Result<(), FileOperationError>,
    G: FnOnce() -> Result<(), FileOperationError>,
{
    if expected_parent != lease.temporary_parent_identity
        || destination.parent() != Some(lease.temporary_parent_path.as_path())
    {
        return Err(FileOperationError::IdentityChanged);
    }
    validate_bound_parent_location(
        &lease.temporary_parent,
        &lease.temporary_parent_path,
        lease.temporary_parent_identity,
    )?;
    if resolve_file_reference(&lease.temporary_reference, &lease.temporary_path)?
        != lease.temporary_path
    {
        return Err(FileOperationError::IdentityChanged);
    }
    let temporary_file = openat_file(
        &lease.temporary_parent,
        &lease.temporary_path,
        libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        0,
        "open bound staged copy for placement",
    )?;
    if !snapshot_matches_bound_move(&lease.temporary_snapshot, &file_snapshot(&temporary_file)?) {
        return Err(FileOperationError::IdentityChanged);
    }
    renameat_no_replace(
        &lease.temporary_parent,
        &lease.temporary_path,
        &lease.temporary_parent,
        destination,
    )?;
    if resolve_file_reference(&lease.temporary_reference, &lease.temporary_path)? != destination {
        let _ = renameat_no_replace(
            &lease.temporary_parent,
            destination,
            &lease.temporary_parent,
            &lease.temporary_path,
        );
        return Err(FileOperationError::IdentityChanged);
    }
    validate_bound_parent_location(
        &lease.temporary_parent,
        &lease.temporary_parent_path,
        lease.temporary_parent_identity,
    )?;
    let destination_file = openat_file(
        &lease.temporary_parent,
        destination,
        libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        0,
        "verify bound staged copy placement",
    )?;
    if !snapshot_matches_bound_move(
        &lease.temporary_snapshot,
        &file_snapshot(&destination_file)?,
    ) {
        return Err(FileOperationError::IdentityChanged);
    }
    before_sync()?;
    lease.temporary_parent.sync_all().map_err(|error| {
        FileOperationError::io(
            "sync bound staged copy placement directory",
            &lease.temporary_parent_path,
            &error,
        )
    })?;
    before_final_validation()?;
    validate_bound_parent_location(
        &lease.temporary_parent,
        &lease.temporary_parent_path,
        lease.temporary_parent_identity,
    )?;
    if resolve_file_reference(&lease.temporary_reference, destination)? != destination {
        return Err(FileOperationError::OutsideProject);
    }
    Ok(())
}

pub(super) fn snapshot_matches_bound_move(expected: &FileSnapshot, actual: &FileSnapshot) -> bool {
    expected.volume_id == actual.volume_id
        && expected.len == actual.len
        && expected.file_id == actual.file_id
        && expected.modified_ns == actual.modified_ns
}

#[cfg(not(target_os = "macos"))]
pub(super) fn rename_verified_sync(
    source: &Path,
    destination: &Path,
    expected: &FileSnapshot,
    _source_parent_identity: FileIdentity,
    _destination_parent_identity: FileIdentity,
) -> Result<(), FileOperationError> {
    if snapshot_sync(source)? != *expected {
        return Err(FileOperationError::IdentityChanged);
    }
    rename_no_replace_sync(source, destination)
}

#[cfg(not(target_os = "macos"))]
pub(super) fn verify_parent_identity(
    path: &Path,
    expected: FileIdentity,
) -> Result<(), FileOperationError> {
    let parent = path.parent().ok_or(FileOperationError::OutsideProject)?;
    let metadata = fs::symlink_metadata(parent)
        .map_err(|error| FileOperationError::io("inspect bound directory", parent, &error))?;
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

#[cfg(target_os = "macos")]
pub(super) fn rename_no_replace_sync(
    source: &Path,
    destination: &Path,
) -> Result<(), FileOperationError> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};

    let source_c =
        CString::new(source.as_os_str().as_bytes()).map_err(|_| FileOperationError::Io {
            action: "rename staged file",
            path: source.to_path_buf(),
            message: "path contains a NUL byte".into(),
        })?;
    let destination_c =
        CString::new(destination.as_os_str().as_bytes()).map_err(|_| FileOperationError::Io {
            action: "rename staged file",
            path: destination.to_path_buf(),
            message: "path contains a NUL byte".into(),
        })?;
    // SAFETY: Both C strings are NUL-terminated filesystem paths. RENAME_EXCL
    // asks Darwin to atomically refuse an existing destination.
    let result =
        unsafe { libc::renamex_np(source_c.as_ptr(), destination_c.as_ptr(), libc::RENAME_EXCL) };
    if result == 0 {
        Ok(())
    } else {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            Err(FileOperationError::DestinationExists)
        } else {
            Err(FileOperationError::io(
                "atomically rename staged file",
                destination,
                &error,
            ))
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn rename_no_replace_sync(
    source: &Path,
    destination: &Path,
) -> Result<(), FileOperationError> {
    if destination.exists() {
        return Err(FileOperationError::DestinationExists);
    }
    fs::rename(source, destination).map_err(|error| {
        FileOperationError::io("atomically rename staged file", destination, &error)
    })
}
