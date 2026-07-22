use async_trait::async_trait;
use std::{
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use viewer_application::{
    FileContentEvidence, FileMutationPort, FileOperationError, FileSnapshot,
    file_commands::FileCommandCancellation, watcher::FileIdentity,
};

const COPY_BUFFER_BYTES: usize = 1024 * 1024;
#[cfg(target_os = "macos")]
const RENAME_NOFOLLOW_ANY: u32 = 0x0000_0010;

#[cfg(target_os = "macos")]
#[repr(C)]
struct BoundFileReference {
    hidden: [u8; 80],
}

#[cfg(target_os = "macos")]
#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    fn FSPathMakeRef(
        path: *const u8,
        reference: *mut BoundFileReference,
        is_directory: *mut u8,
    ) -> i32;
    fn FSRefMakePath(
        reference: *const BoundFileReference,
        path: *mut u8,
        max_path_size: u32,
    ) -> i32;
    fn FSUnlinkObject(reference: *const BoundFileReference) -> i32;
}

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

#[cfg(target_os = "macos")]
fn bind_file_reference(
    path: &Path,
    expected: &FileSnapshot,
) -> Result<BoundFileReference, FileOperationError> {
    use std::{ffi::CString, mem::MaybeUninit, os::unix::ffi::OsStrExt};

    let encoded =
        CString::new(path.as_os_str().as_bytes()).map_err(|_| FileOperationError::Io {
            action: "bind temporary file reference",
            path: path.to_path_buf(),
            message: "path contains a NUL byte".into(),
        })?;
    let mut reference = MaybeUninit::<BoundFileReference>::uninit();
    // SAFETY: `encoded` is NUL terminated and `reference` points to writable
    // storage of the exact opaque FSRef size declared by CoreServices.
    let status = unsafe {
        FSPathMakeRef(
            encoded.as_ptr().cast(),
            reference.as_mut_ptr(),
            std::ptr::null_mut(),
        )
    };
    if status != 0 {
        return Err(file_reference_error(
            "bind temporary file reference",
            path,
            status,
        ));
    }
    // SAFETY: CoreServices initialized the reference after returning success.
    let reference = unsafe { reference.assume_init() };
    let resolved = resolve_file_reference(&reference, path)?;
    let file = open_path_no_follow(&resolved, "verify temporary file reference")?;
    if !snapshot_matches_bound_move(expected, &file_snapshot(&file)?) {
        return Err(FileOperationError::IdentityChanged);
    }
    Ok(reference)
}

#[cfg(target_os = "macos")]
fn resolve_file_reference(
    reference: &BoundFileReference,
    error_path: &Path,
) -> Result<PathBuf, FileOperationError> {
    use std::{ffi::CStr, os::unix::ffi::OsStrExt};

    let mut buffer = vec![0_u8; libc::PATH_MAX as usize];
    // SAFETY: `buffer` is writable for the supplied length and `reference`
    // was initialized by CoreServices.
    let status = unsafe {
        FSRefMakePath(
            reference,
            buffer.as_mut_ptr(),
            u32::try_from(buffer.len()).expect("PATH_MAX fits in u32"),
        )
    };
    if status != 0 {
        return Err(file_reference_error(
            "resolve temporary file reference",
            error_path,
            status,
        ));
    }
    let resolved = unsafe { CStr::from_ptr(buffer.as_ptr().cast()) };
    Ok(PathBuf::from(std::ffi::OsStr::from_bytes(
        resolved.to_bytes(),
    )))
}

#[cfg(target_os = "macos")]
fn open_path_no_follow(path: &Path, action: &'static str) -> Result<File, FileOperationError> {
    use std::{ffi::CString, os::fd::FromRawFd, os::unix::ffi::OsStrExt};

    let encoded =
        CString::new(path.as_os_str().as_bytes()).map_err(|_| FileOperationError::Io {
            action,
            path: path.to_path_buf(),
            message: "path contains a NUL byte".into(),
        })?;
    // SAFETY: The C string is NUL terminated. O_NOFOLLOW_ANY rejects links in
    // every path component before the descriptor is accepted.
    let fd = unsafe {
        libc::open(
            encoded.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW_ANY,
        )
    };
    if fd < 0 {
        return Err(FileOperationError::io(
            action,
            path,
            &std::io::Error::last_os_error(),
        ));
    }
    // SAFETY: `open` returned a new owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[cfg(target_os = "macos")]
fn unlink_file_reference(
    reference: &BoundFileReference,
    error_path: &Path,
) -> Result<(), FileOperationError> {
    // SAFETY: `reference` was initialized by CoreServices and stays live for
    // the duration of the call. FSUnlinkObject removes that identity, not the
    // current occupant of a pathname.
    let status = unsafe { FSUnlinkObject(reference) };
    if status == 0 || status == -43 {
        Ok(())
    } else {
        Err(file_reference_error(
            "unlink temporary file reference",
            error_path,
            status,
        ))
    }
}

#[cfg(target_os = "macos")]
fn file_reference_error(action: &'static str, path: &Path, status: i32) -> FileOperationError {
    FileOperationError::Io {
        action,
        path: path.to_path_buf(),
        message: format!("CoreServices returned OSStatus {status}"),
    }
}

#[cfg(target_os = "macos")]
fn open_bound_parent(path: &Path, expected: FileIdentity) -> Result<File, FileOperationError> {
    use std::{ffi::CString, os::fd::FromRawFd, os::unix::ffi::OsStrExt};
    let encoded =
        CString::new(path.as_os_str().as_bytes()).map_err(|_| FileOperationError::Io {
            action: "open bound directory",
            path: path.to_path_buf(),
            message: "path contains a NUL byte".into(),
        })?;
    // SAFETY: `encoded` is NUL terminated. O_NOFOLLOW_ANY rejects a symlink
    // in every component before an fd is accepted as the bound directory.
    let fd = unsafe {
        libc::open(
            encoded.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW_ANY,
        )
    };
    if fd < 0 {
        return Err(FileOperationError::io(
            "open bound directory",
            path,
            &std::io::Error::last_os_error(),
        ));
    }
    // SAFETY: `open` returned a new owned descriptor.
    let directory = unsafe { File::from_raw_fd(fd) };
    let metadata = directory
        .metadata()
        .map_err(|error| FileOperationError::io("inspect bound directory", path, &error))?;
    use std::os::unix::fs::MetadataExt;
    if !metadata.is_dir() || metadata.dev() != expected.volume || metadata.ino() != expected.file {
        return Err(FileOperationError::IdentityChanged);
    }
    Ok(directory)
}

#[cfg(target_os = "macos")]
fn encoded_file_name(
    path: &Path,
    action: &'static str,
) -> Result<std::ffi::CString, FileOperationError> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let name = path.file_name().ok_or(FileOperationError::OutsideProject)?;
    CString::new(name.as_bytes()).map_err(|_| FileOperationError::Io {
        action,
        path: path.to_path_buf(),
        message: "file name contains a NUL byte".into(),
    })
}

#[cfg(target_os = "macos")]
fn openat_file(
    directory: &File,
    path: &Path,
    flags: i32,
    mode: libc::mode_t,
    action: &'static str,
) -> Result<File, FileOperationError> {
    use std::os::fd::{AsRawFd, FromRawFd};
    let name = encoded_file_name(path, action)?;
    // SAFETY: The directory fd is live, the name is one NUL-terminated
    // component, and a mode is supplied for the O_CREAT case.
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            flags,
            libc::c_uint::from(mode),
        )
    };
    if fd < 0 {
        return Err(FileOperationError::io(
            action,
            path,
            &std::io::Error::last_os_error(),
        ));
    }
    // SAFETY: `openat` returned a new owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[cfg(target_os = "macos")]
fn renameat_no_replace(
    source_parent: &File,
    source: &Path,
    destination_parent: &File,
    destination: &Path,
) -> Result<(), FileOperationError> {
    use std::os::fd::AsRawFd;
    let source_name = encoded_file_name(source, "encode bound rename source")?;
    let destination_name = encoded_file_name(destination, "encode bound rename destination")?;
    let flags = libc::RENAME_EXCL | RENAME_NOFOLLOW_ANY;
    // SAFETY: Both descriptors bind verified directories and both C strings
    // contain one relative component. The flags reject overwrite and links.
    let result = unsafe {
        libc::renameatx_np(
            source_parent.as_raw_fd(),
            source_name.as_ptr(),
            destination_parent.as_raw_fd(),
            destination_name.as_ptr(),
            flags,
        )
    };
    if result == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.kind() == std::io::ErrorKind::AlreadyExists {
        Err(FileOperationError::DestinationExists)
    } else {
        Err(FileOperationError::io(
            "atomically rename bound file",
            destination,
            &error,
        ))
    }
}

fn file_snapshot(file: &File) -> Result<FileSnapshot, FileOperationError> {
    let metadata = file.metadata().map_err(|error| {
        FileOperationError::io("inspect bound file", Path::new("bound-file"), &error)
    })?;
    if !metadata.is_file() {
        return Err(FileOperationError::SourceMissing);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(FileSnapshot {
            len: metadata.len(),
            volume_id: metadata.dev(),
            file_id: Some(u128::from(metadata.ino())),
            modified_ns: Some(unix_timestamp_ns(metadata.mtime(), metadata.mtime_nsec())),
            changed_ns: Some(unix_timestamp_ns(metadata.ctime(), metadata.ctime_nsec())),
        })
    }
    #[cfg(not(unix))]
    {
        Ok(FileSnapshot {
            len: metadata.len(),
            volume_id: 0,
            file_id: None,
            modified_ns: None,
            changed_ns: None,
        })
    }
}

#[cfg(target_os = "macos")]
fn create_registered_temporary_verified_sync(
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
fn create_registered_temporary_verified_sync(
    path: &Path,
    _expected_parent: FileIdentity,
) -> Result<(), FileOperationError> {
    create_temporary_sync(path)
}

#[cfg(target_os = "macos")]
fn rename_verified_sync(
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
fn rename_verified_sync_with_hook<F>(
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
fn rename_verified_sync_with_hooks<F, G>(
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
fn validate_bound_parent_location(
    directory: &File,
    expected_path: &Path,
    expected_identity: FileIdentity,
) -> Result<(), FileOperationError> {
    use std::{ffi::CStr, os::fd::AsRawFd, os::unix::ffi::OsStrExt};
    let metadata = directory.metadata().map_err(|error| {
        FileOperationError::io("revalidate bound rename parent", expected_path, &error)
    })?;
    use std::os::unix::fs::MetadataExt;
    if metadata.dev() != expected_identity.volume || metadata.ino() != expected_identity.file {
        return Err(FileOperationError::IdentityChanged);
    }
    let mut path = vec![0_i8; libc::PATH_MAX as usize];
    let result = unsafe { libc::fcntl(directory.as_raw_fd(), libc::F_GETPATH, path.as_mut_ptr()) };
    if result < 0 {
        return Err(FileOperationError::io(
            "resolve bound rename parent",
            expected_path,
            &std::io::Error::last_os_error(),
        ));
    }
    let current = unsafe { CStr::from_ptr(path.as_ptr()) };
    if current.to_bytes() != expected_path.as_os_str().as_bytes() {
        return Err(FileOperationError::OutsideProject);
    }
    Ok(())
}

fn snapshot_matches_bound_move(expected: &FileSnapshot, actual: &FileSnapshot) -> bool {
    expected.volume_id == actual.volume_id
        && expected.len == actual.len
        && expected.file_id == actual.file_id
        && expected.modified_ns == actual.modified_ns
}

#[cfg(not(target_os = "macos"))]
fn rename_verified_sync(
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

#[cfg(target_os = "macos")]
fn copy_and_hash_cancellable_verified_sync(
    source: &Path,
    temporary: &Path,
    cancellation: &FileCommandCancellation,
    expected_source: &FileSnapshot,
    source_parent_identity: FileIdentity,
    temporary_parent_identity: FileIdentity,
) -> Result<(u64, [u8; 32]), FileOperationError> {
    let source_parent = open_bound_parent(
        source.parent().ok_or(FileOperationError::OutsideProject)?,
        source_parent_identity,
    )?;
    let temporary_parent = open_bound_parent(
        temporary
            .parent()
            .ok_or(FileOperationError::OutsideProject)?,
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
    let temporary_file = openat_file(
        &temporary_parent,
        temporary,
        libc::O_RDWR | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        0,
        "open bound copy temporary",
    )?;
    let mut reader = BufReader::with_capacity(COPY_BUFFER_BYTES, source_file);
    let mut writer = BufWriter::with_capacity(COPY_BUFFER_BYTES, temporary_file);
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut hasher = blake3::Hasher::new();
    let mut length = 0_u64;
    loop {
        if cancellation.is_cancelled() {
            return Err(FileOperationError::Cancelled);
        }
        let read = reader
            .read(&mut buffer)
            .map_err(|error| FileOperationError::io("read bound copy source", source, &error))?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read]).map_err(|error| {
            FileOperationError::io("write bound copy temporary", temporary, &error)
        })?;
        length = length
            .checked_add(read as u64)
            .ok_or(FileOperationError::VerificationFailed)?;
        hasher.update(&buffer[..read]);
    }
    if cancellation.is_cancelled() {
        return Err(FileOperationError::Cancelled);
    }
    writer
        .flush()
        .map_err(|error| FileOperationError::io("flush bound copy temporary", temporary, &error))?;
    let mut temporary_file = writer.into_inner().map_err(|error| {
        FileOperationError::io("finish bound copy temporary", temporary, error.error())
    })?;
    temporary_file
        .sync_all()
        .map_err(|error| FileOperationError::io("sync bound copy temporary", temporary, &error))?;
    let copied = (length, *hasher.finalize().as_bytes());
    temporary_file.seek(SeekFrom::Start(0)).map_err(|error| {
        FileOperationError::io("rewind bound copy temporary", temporary, &error)
    })?;
    let mut verify = BufReader::with_capacity(COPY_BUFFER_BYTES, temporary_file);
    let mut verified_hasher = blake3::Hasher::new();
    let mut verified_len = 0_u64;
    loop {
        let read = verify.read(&mut buffer).map_err(|error| {
            FileOperationError::io("verify bound copy temporary", temporary, &error)
        })?;
        if read == 0 {
            break;
        }
        verified_len = verified_len
            .checked_add(read as u64)
            .ok_or(FileOperationError::VerificationFailed)?;
        verified_hasher.update(&buffer[..read]);
    }
    let verified = (verified_len, *verified_hasher.finalize().as_bytes());
    if copied != verified || file_snapshot(reader.get_ref())? != *expected_source {
        return Err(FileOperationError::IdentityChanged);
    }
    Ok(copied)
}

#[cfg(target_os = "macos")]
fn create_copy_and_hash_cancellable_verified_sync_with_hook<F>(
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
fn create_copy_and_hash_cancellable_verified_sync_with_hooks<F, G, H>(
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
    let source_parent = open_bound_parent(
        source.parent().ok_or(FileOperationError::OutsideProject)?,
        source_parent_identity,
    )?;
    let temporary_parent_path = temporary
        .parent()
        .ok_or(FileOperationError::OutsideProject)?;
    let temporary_parent = open_bound_parent(temporary_parent_path, temporary_parent_identity)?;
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
    let temporary_reference = match bind_file_reference(temporary, &temporary_snapshot) {
        Ok(reference) => reference,
        Err(primary) => return Err(unbound_copy_cleanup_obligation(primary, temporary)),
    };
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
        copy_open_files_and_evidence(
            source_file,
            temporary_file,
            source,
            temporary,
            cancellation,
            expected_source,
        )
    });
    match copied {
        Ok(evidence) => Ok(evidence),
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
fn bound_copy_cleanup_error(
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
fn bound_copy_cleanup_error_with_sync<F>(
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
fn create_copy_and_hash_cancellable_verified_sync_with_hook<F>(
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
fn verify_parent_identity(path: &Path, expected: FileIdentity) -> Result<(), FileOperationError> {
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

fn copy_open_files_and_evidence(
    source_file: File,
    temporary_file: File,
    source: &Path,
    temporary: &Path,
    cancellation: &FileCommandCancellation,
    expected_source: &FileSnapshot,
) -> Result<FileContentEvidence, FileOperationError> {
    let mut reader = BufReader::with_capacity(COPY_BUFFER_BYTES, source_file);
    let mut writer = BufWriter::with_capacity(COPY_BUFFER_BYTES, temporary_file);
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut hasher = blake3::Hasher::new();
    let mut length = 0_u64;
    loop {
        if cancellation.is_cancelled() {
            return Err(FileOperationError::Cancelled);
        }
        let read = reader
            .read(&mut buffer)
            .map_err(|error| FileOperationError::io("read bound copy source", source, &error))?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read]).map_err(|error| {
            FileOperationError::io("write bound copy temporary", temporary, &error)
        })?;
        length = length
            .checked_add(read as u64)
            .ok_or(FileOperationError::VerificationFailed)?;
        hasher.update(&buffer[..read]);
    }
    if cancellation.is_cancelled() {
        return Err(FileOperationError::Cancelled);
    }
    writer
        .flush()
        .map_err(|error| FileOperationError::io("flush bound copy temporary", temporary, &error))?;
    let mut temporary_file = writer.into_inner().map_err(|error| {
        FileOperationError::io("finish bound copy temporary", temporary, error.error())
    })?;
    temporary_file
        .sync_all()
        .map_err(|error| FileOperationError::io("sync bound copy temporary", temporary, &error))?;
    let copied = (length, *hasher.finalize().as_bytes());
    temporary_file.seek(SeekFrom::Start(0)).map_err(|error| {
        FileOperationError::io("rewind bound copy temporary", temporary, &error)
    })?;
    let mut verify = BufReader::with_capacity(COPY_BUFFER_BYTES, temporary_file);
    let mut verified_hasher = blake3::Hasher::new();
    let mut verified_len = 0_u64;
    loop {
        let read = verify.read(&mut buffer).map_err(|error| {
            FileOperationError::io("verify bound copy temporary", temporary, &error)
        })?;
        if read == 0 {
            break;
        }
        verified_len = verified_len
            .checked_add(read as u64)
            .ok_or(FileOperationError::VerificationFailed)?;
        verified_hasher.update(&buffer[..read]);
    }
    if copied != (verified_len, *verified_hasher.finalize().as_bytes())
        || file_snapshot(reader.get_ref())? != *expected_source
    {
        return Err(FileOperationError::IdentityChanged);
    }
    let snapshot = file_snapshot(verify.get_ref())?;
    if snapshot.len != copied.0 {
        return Err(FileOperationError::VerificationFailed);
    }
    Ok(FileContentEvidence {
        snapshot,
        hash: copied.1,
    })
}

#[cfg(not(target_os = "macos"))]
fn copy_and_hash_cancellable_verified_sync(
    source: &Path,
    temporary: &Path,
    cancellation: &FileCommandCancellation,
    expected_source: &FileSnapshot,
    _source_parent_identity: FileIdentity,
    _temporary_parent_identity: FileIdentity,
) -> Result<(u64, [u8; 32]), FileOperationError> {
    if snapshot_sync(source)? != *expected_source {
        return Err(FileOperationError::IdentityChanged);
    }
    copy_and_hash_cancellable_sync(source, temporary, cancellation)
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

pub(crate) fn hash_file_sync(path: &Path) -> Result<(u64, [u8; 32]), FileOperationError> {
    let file = File::open(path)
        .map_err(|error| FileOperationError::io("open file for verification", path, &error))?;
    let mut reader = BufReader::with_capacity(COPY_BUFFER_BYTES, file);
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut hasher = blake3::Hasher::new();
    let mut length = 0_u64;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| FileOperationError::io("verify copied file", path, &error))?;
        if read == 0 {
            break;
        }
        length = length
            .checked_add(read as u64)
            .ok_or(FileOperationError::VerificationFailed)?;
        hasher.update(&buffer[..read]);
    }
    Ok((length, *hasher.finalize().as_bytes()))
}

pub(crate) fn sync_parent(path: &Path) -> Result<(), FileOperationError> {
    let parent = path.parent().ok_or(FileOperationError::OutsideProject)?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| FileOperationError::io("sync containing directory", parent, &error))
}

fn snapshot_sync(path: &Path) -> Result<FileSnapshot, FileOperationError> {
    let metadata = fs::metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            FileOperationError::SourceMissing
        } else {
            FileOperationError::io("read file metadata", path, &error)
        }
    })?;
    if !metadata.is_file() {
        return Err(FileOperationError::SourceMissing);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(FileSnapshot {
            len: metadata.len(),
            volume_id: metadata.dev(),
            file_id: Some(u128::from(metadata.ino())),
            modified_ns: Some(unix_timestamp_ns(metadata.mtime(), metadata.mtime_nsec())),
            changed_ns: Some(unix_timestamp_ns(metadata.ctime(), metadata.ctime_nsec())),
        })
    }
    #[cfg(not(unix))]
    {
        Ok(FileSnapshot {
            len: metadata.len(),
            volume_id: 0,
            file_id: None,
            modified_ns: system_modified_ns(&metadata),
            changed_ns: None,
        })
    }
}

#[cfg(unix)]
fn unix_timestamp_ns(seconds: i64, nanoseconds: i64) -> i128 {
    i128::from(seconds)
        .saturating_mul(1_000_000_000)
        .saturating_add(i128::from(nanoseconds))
}

#[cfg(not(unix))]
fn system_modified_ns(metadata: &fs::Metadata) -> Option<i128> {
    metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i128::try_from(duration.as_nanos()).ok())
}

fn copy_and_hash_sync(
    source: &Path,
    temporary: &Path,
) -> Result<(u64, [u8; 32]), FileOperationError> {
    let input = File::open(source)
        .map_err(|error| FileOperationError::io("open copy source", source, &error))?;
    let output = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(temporary)
        .map_err(|error| {
            FileOperationError::io("open registered copy temporary", temporary, &error)
        })?;
    let mut reader = BufReader::with_capacity(COPY_BUFFER_BYTES, input);
    let mut writer = BufWriter::with_capacity(COPY_BUFFER_BYTES, output);
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut hasher = blake3::Hasher::new();
    let mut length = 0_u64;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| FileOperationError::io("read copy source", source, &error))?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read]).map_err(|error| {
            FileOperationError::io("write registered copy temporary", temporary, &error)
        })?;
        length = length
            .checked_add(read as u64)
            .ok_or_else(|| FileOperationError::Io {
                action: "copy file",
                path: source.to_path_buf(),
                message: "file length overflow".into(),
            })?;
        hasher.update(&buffer[..read]);
    }
    writer.flush().map_err(|error| {
        FileOperationError::io("flush registered copy temporary", temporary, &error)
    })?;
    writer.get_ref().sync_all().map_err(|error| {
        FileOperationError::io("sync registered copy temporary", temporary, &error)
    })?;
    Ok((length, *hasher.finalize().as_bytes()))
}

fn copy_and_hash_cancellable_sync(
    source: &Path,
    temporary: &Path,
    cancellation: &FileCommandCancellation,
) -> Result<(u64, [u8; 32]), FileOperationError> {
    let input = File::open(source)
        .map_err(|error| FileOperationError::io("open copy source", source, &error))?;
    let output = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(temporary)
        .map_err(|error| {
            FileOperationError::io("open registered copy temporary", temporary, &error)
        })?;
    let mut reader = BufReader::with_capacity(COPY_BUFFER_BYTES, input);
    let mut writer = BufWriter::with_capacity(COPY_BUFFER_BYTES, output);
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut hasher = blake3::Hasher::new();
    let mut length = 0_u64;
    loop {
        if cancellation.is_cancelled() {
            return Err(FileOperationError::Cancelled);
        }
        let read = reader
            .read(&mut buffer)
            .map_err(|error| FileOperationError::io("read copy source", source, &error))?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read]).map_err(|error| {
            FileOperationError::io("write registered copy temporary", temporary, &error)
        })?;
        length = length
            .checked_add(read as u64)
            .ok_or(FileOperationError::VerificationFailed)?;
        hasher.update(&buffer[..read]);
    }
    if cancellation.is_cancelled() {
        return Err(FileOperationError::Cancelled);
    }
    writer.flush().map_err(|error| {
        FileOperationError::io("flush registered copy temporary", temporary, &error)
    })?;
    writer.get_ref().sync_all().map_err(|error| {
        FileOperationError::io("sync registered copy temporary", temporary, &error)
    })?;
    Ok((length, *hasher.finalize().as_bytes()))
}

#[cfg(target_os = "macos")]
fn rename_no_replace_sync(source: &Path, destination: &Path) -> Result<(), FileOperationError> {
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
fn rename_no_replace_sync(source: &Path, destination: &Path) -> Result<(), FileOperationError> {
    if destination.exists() {
        return Err(FileOperationError::DestinationExists);
    }
    fs::rename(source, destination).map_err(|error| {
        FileOperationError::io("atomically rename staged file", destination, &error)
    })
}

fn worker_error(
    action: &'static str,
    path: &Path,
    error: tokio::task::JoinError,
) -> FileOperationError {
    FileOperationError::Io {
        action,
        path: PathBuf::from(path),
        message: error.to_string(),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    #[test]
    fn bound_copy_keeps_the_created_leaf_open_when_its_name_is_replaced_by_a_hardlink() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let source = root.join("source.bin");
        let temporary = root.join(".viewer-copy-test.part");
        let victim = root.join("victim.bin");
        fs::write(&source, b"selected bytes").unwrap();
        fs::write(&victim, b"victim sentinel").unwrap();
        let parent_metadata = fs::metadata(&root).unwrap();
        let parent_identity = FileIdentity {
            volume: parent_metadata.dev(),
            file: parent_metadata.ino(),
        };
        let source_snapshot = snapshot_sync(&source).unwrap();

        let evidence = create_copy_and_hash_cancellable_verified_sync_with_hook(
            &source,
            &temporary,
            &FileCommandCancellation::default(),
            &source_snapshot,
            parent_identity,
            parent_identity,
            || {
                fs::remove_file(&temporary).unwrap();
                fs::hard_link(&victim, &temporary).unwrap();
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(fs::read(&victim).unwrap(), b"victim sentinel");
        assert_eq!(fs::read(&temporary).unwrap(), b"victim sentinel");
        assert_ne!(snapshot_sync(&temporary).unwrap(), evidence.snapshot);
        assert_eq!(evidence.snapshot.len, b"selected bytes".len() as u64);
        assert_eq!(evidence.hash, *blake3::hash(b"selected bytes").as_bytes());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn identity_bound_cleanup_unlinks_the_created_leaf_after_its_name_is_swapped() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let temporary = root.join(".viewer-copy-test.part");
        let parked = root.join("parked-created-temp.part");
        let replacement = root.join("replacement.bin");
        fs::write(&temporary, b"created temporary").unwrap();
        fs::write(&replacement, b"replacement sentinel").unwrap();
        let expected = snapshot_sync(&temporary).unwrap();
        let reference = bind_file_reference(&temporary, &expected).unwrap();

        fs::rename(&temporary, &parked).unwrap();
        fs::rename(&replacement, &temporary).unwrap();
        unlink_file_reference(&reference, &temporary).unwrap();

        assert_eq!(fs::read(&temporary).unwrap(), b"replacement sentinel");
        assert!(!parked.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn registered_copy_error_removes_the_partially_written_created_identity() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let source = root.join("source.bin");
        let temporary = root.join(".viewer-copy-test.part");
        fs::write(&source, b"selected bytes").unwrap();
        let parent_metadata = fs::metadata(&root).unwrap();
        let parent_identity = FileIdentity {
            volume: parent_metadata.dev(),
            file: parent_metadata.ino(),
        };
        let source_snapshot = snapshot_sync(&source).unwrap();

        let result = create_copy_and_hash_cancellable_verified_sync_with_hook(
            &source,
            &temporary,
            &FileCommandCancellation::default(),
            &source_snapshot,
            parent_identity,
            parent_identity,
            || {
                fs::write(&temporary, b"partial bytes").unwrap();
                Err(FileOperationError::Cancelled)
            },
        );

        assert!(matches!(result, Err(FileOperationError::Cancelled)));
        assert!(!temporary.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn registered_copy_bind_failure_becomes_a_cleanup_obligation() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let source = root.join("source.bin");
        let temporary = root.join(".viewer-copy-test.part");
        let parked = root.join("parked-created-temp.part");
        fs::write(&source, b"selected bytes").unwrap();
        let parent_metadata = fs::metadata(&root).unwrap();
        let parent_identity = FileIdentity {
            volume: parent_metadata.dev(),
            file: parent_metadata.ino(),
        };
        let source_snapshot = snapshot_sync(&source).unwrap();

        let result = create_copy_and_hash_cancellable_verified_sync_with_hooks(
            &source,
            &temporary,
            &FileCommandCancellation::default(),
            &source_snapshot,
            parent_identity,
            parent_identity,
            || {
                fs::rename(&temporary, &parked).unwrap();
                fs::write(&temporary, b"replacement sentinel").unwrap();
                Ok(())
            },
            || Ok(()),
            || panic!("copy hook must not run after setup failure"),
        );

        match result {
            Err(FileOperationError::RegisteredTemporaryCleanupRequired { primary, cleanup }) => {
                assert_eq!(*primary, FileOperationError::IdentityChanged);
                assert_ne!(*cleanup, FileOperationError::Cancelled);
            }
            other => panic!("expected structured setup obligation, got {other:?}"),
        }
        assert_eq!(fs::read(&temporary).unwrap(), b"replacement sentinel");
        assert!(parked.is_file());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn registered_copy_bound_setup_error_cleans_identity_and_preserves_primary() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let source = root.join("source.bin");
        let temporary = root.join(".viewer-copy-test.part");
        fs::write(&source, b"selected bytes").unwrap();
        let parent_metadata = fs::metadata(&root).unwrap();
        let parent_identity = FileIdentity {
            volume: parent_metadata.dev(),
            file: parent_metadata.ino(),
        };
        let source_snapshot = snapshot_sync(&source).unwrap();
        let setup_error = FileOperationError::Io {
            action: "injected directory sync failure",
            path: temporary.clone(),
            message: "injected failure".into(),
        };

        let result = create_copy_and_hash_cancellable_verified_sync_with_hooks(
            &source,
            &temporary,
            &FileCommandCancellation::default(),
            &source_snapshot,
            parent_identity,
            parent_identity,
            || Ok(()),
            || Err(setup_error.clone()),
            || panic!("copy hook must not run after bound setup failure"),
        );

        assert_eq!(result, Err(setup_error));
        assert!(!temporary.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn registered_copy_error_never_removes_a_replacement_at_the_temporary_name() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let source = root.join("source.bin");
        let temporary = root.join(".viewer-copy-test.part");
        let parked = root.join("parked-created-temp.part");
        let replacement = root.join("replacement.bin");
        fs::write(&source, b"selected bytes").unwrap();
        fs::write(&replacement, b"replacement sentinel").unwrap();
        let parent_metadata = fs::metadata(&root).unwrap();
        let parent_identity = FileIdentity {
            volume: parent_metadata.dev(),
            file: parent_metadata.ino(),
        };
        let source_snapshot = snapshot_sync(&source).unwrap();

        let result = create_copy_and_hash_cancellable_verified_sync_with_hook(
            &source,
            &temporary,
            &FileCommandCancellation::default(),
            &source_snapshot,
            parent_identity,
            parent_identity,
            || {
                fs::rename(&temporary, &parked).unwrap();
                fs::rename(&replacement, &temporary).unwrap();
                Err(FileOperationError::Cancelled)
            },
        );

        match result {
            Err(FileOperationError::RegisteredTemporaryCleanupRequired { primary, cleanup }) => {
                assert_eq!(*primary, FileOperationError::Cancelled);
                assert_eq!(*cleanup, FileOperationError::IdentityChanged);
            }
            other => panic!("expected replacement cleanup obligation, got {other:?}"),
        }
        assert_eq!(fs::read(&temporary).unwrap(), b"replacement sentinel");
        assert!(!parked.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn registered_copy_cleanup_sync_failure_retains_the_recovery_obligation() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let temporary = root.join(".viewer-copy-test.part");
        fs::write(&temporary, b"partial bytes").unwrap();
        let temporary_snapshot = snapshot_sync(&temporary).unwrap();
        let temporary_reference = bind_file_reference(&temporary, &temporary_snapshot).unwrap();
        let sync_error = FileOperationError::Io {
            action: "injected cleanup directory sync failure",
            path: root,
            message: "injected failure".into(),
        };

        let result = bound_copy_cleanup_error_with_sync(
            &temporary_reference,
            &temporary,
            FileOperationError::Cancelled,
            || Err(sync_error.clone()),
        );

        match result {
            FileOperationError::RegisteredTemporaryCleanupRequired { primary, cleanup } => {
                assert_eq!(*primary, FileOperationError::Cancelled);
                assert_eq!(*cleanup, sync_error);
            }
            other => panic!("expected durable cleanup obligation, got {other:?}"),
        }
        assert!(!temporary.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn registered_copy_cleanup_failure_preserves_primary_and_owned_temporary() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let source = root.join("source.bin");
        let temporary = root.join(".viewer-copy-test.part");
        fs::write(&source, b"selected bytes").unwrap();
        let parent_metadata = fs::metadata(&root).unwrap();
        let parent_identity = FileIdentity {
            volume: parent_metadata.dev(),
            file: parent_metadata.ino(),
        };
        let source_snapshot = snapshot_sync(&source).unwrap();

        let result = create_copy_and_hash_cancellable_verified_sync_with_hook(
            &source,
            &temporary,
            &FileCommandCancellation::default(),
            &source_snapshot,
            parent_identity,
            parent_identity,
            || {
                fs::write(&temporary, b"partial bytes").unwrap();
                fs::set_permissions(&root, fs::Permissions::from_mode(0o500)).unwrap();
                Err(FileOperationError::Cancelled)
            },
        );
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();

        match result {
            Err(FileOperationError::RegisteredTemporaryCleanupRequired { primary, cleanup }) => {
                assert_eq!(*primary, FileOperationError::Cancelled);
                assert_ne!(*cleanup, FileOperationError::Cancelled);
            }
            other => panic!("expected structured cleanup failure, got {other:?}"),
        }
        assert_eq!(fs::read(&temporary).unwrap(), b"partial bytes");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn verified_rename_rolls_back_a_leaf_swap_after_the_validated_fd_is_opened() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let source = root.join("source.bin");
        let destination = root.join("destination.bin");
        let parked = root.join("parked-original.bin");
        fs::write(&source, b"selected bytes").unwrap();
        let parent_metadata = fs::metadata(&root).unwrap();
        let parent_identity = FileIdentity {
            volume: parent_metadata.dev(),
            file: parent_metadata.ino(),
        };
        let expected = snapshot_sync(&source).unwrap();

        let result = rename_verified_sync_with_hook(
            &source,
            &destination,
            &expected,
            parent_identity,
            parent_identity,
            || {
                fs::rename(&source, &parked).unwrap();
                fs::write(&source, b"replacement").unwrap();
            },
        );

        assert_eq!(result, Err(FileOperationError::IdentityChanged));
        assert_eq!(fs::read(&source).unwrap(), b"replacement");
        assert_eq!(fs::read(&parked).unwrap(), b"selected bytes");
        assert!(!destination.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn verified_rename_rejects_a_parent_fd_reparented_outside_after_leaf_validation() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_root = fs::canonicalize(outside.path()).unwrap();
        let source_parent = root.join("source");
        let destination_parent = root.join("destination");
        fs::create_dir(&source_parent).unwrap();
        fs::create_dir(&destination_parent).unwrap();
        let source = source_parent.join("item.bin");
        let destination = destination_parent.join("item.bin");
        fs::write(&source, b"selected bytes").unwrap();
        let source_metadata = fs::metadata(&source_parent).unwrap();
        let destination_metadata = fs::metadata(&destination_parent).unwrap();
        let expected = snapshot_sync(&source).unwrap();
        let moved_parent = outside_root.join("moved-source");

        let result = rename_verified_sync_with_hook(
            &source,
            &destination,
            &expected,
            FileIdentity {
                volume: source_metadata.dev(),
                file: source_metadata.ino(),
            },
            FileIdentity {
                volume: destination_metadata.dev(),
                file: destination_metadata.ino(),
            },
            || {
                fs::rename(&source_parent, &moved_parent).unwrap();
                fs::create_dir(&source_parent).unwrap();
                fs::write(source_parent.join("item.bin"), b"replacement").unwrap();
            },
        );

        assert!(matches!(result, Err(FileOperationError::OutsideProject)));
        assert_eq!(
            fs::read(moved_parent.join("item.bin")).unwrap(),
            b"selected bytes"
        );
        assert_eq!(fs::read(&source).unwrap(), b"replacement");
        assert!(!destination.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn verified_rename_rolls_back_when_a_parent_is_reparented_after_the_syscall() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_root = fs::canonicalize(outside.path()).unwrap();
        let source_parent = root.join("source");
        let destination_parent = root.join("destination");
        fs::create_dir(&source_parent).unwrap();
        fs::create_dir(&destination_parent).unwrap();
        let source = source_parent.join("item.bin");
        let destination = destination_parent.join("item.bin");
        fs::write(&source, b"selected bytes").unwrap();
        let source_metadata = fs::metadata(&source_parent).unwrap();
        let destination_metadata = fs::metadata(&destination_parent).unwrap();
        let expected = snapshot_sync(&source).unwrap();
        let moved_parent = outside_root.join("moved-source");

        let result = rename_verified_sync_with_hooks(
            &source,
            &destination,
            &expected,
            FileIdentity {
                volume: source_metadata.dev(),
                file: source_metadata.ino(),
            },
            FileIdentity {
                volume: destination_metadata.dev(),
                file: destination_metadata.ino(),
            },
            || {},
            || fs::rename(&source_parent, &moved_parent).unwrap(),
        );

        assert!(matches!(result, Err(FileOperationError::OutsideProject)));
        assert_eq!(
            fs::read(moved_parent.join("item.bin")).unwrap(),
            b"selected bytes"
        );
        assert!(!destination.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn verified_rename_keeps_a_recoverable_destination_when_reparent_rollback_is_blocked() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_root = fs::canonicalize(outside.path()).unwrap();
        let source_parent = root.join("source");
        let destination_parent = root.join("destination");
        fs::create_dir(&source_parent).unwrap();
        fs::create_dir(&destination_parent).unwrap();
        let source = source_parent.join("item.bin");
        let destination = destination_parent.join("item.bin");
        fs::write(&source, b"selected bytes").unwrap();
        let source_metadata = fs::metadata(&source_parent).unwrap();
        let destination_metadata = fs::metadata(&destination_parent).unwrap();
        let expected = snapshot_sync(&source).unwrap();
        let moved_parent = outside_root.join("moved-source");

        let result = rename_verified_sync_with_hooks(
            &source,
            &destination,
            &expected,
            FileIdentity {
                volume: source_metadata.dev(),
                file: source_metadata.ino(),
            },
            FileIdentity {
                volume: destination_metadata.dev(),
                file: destination_metadata.ino(),
            },
            || {},
            || {
                fs::rename(&source_parent, &moved_parent).unwrap();
                fs::write(moved_parent.join("item.bin"), b"replacement").unwrap();
            },
        );

        assert!(matches!(result, Err(FileOperationError::OutsideProject)));
        assert_eq!(fs::read(&destination).unwrap(), b"selected bytes");
        assert_eq!(
            fs::read(moved_parent.join("item.bin")).unwrap(),
            b"replacement"
        );
    }
}
