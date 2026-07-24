#[cfg(target_os = "macos")]
use super::local::snapshot_matches_bound_move;
#[cfg(unix)]
use super::local::unix_timestamp_ns;
use std::{
    fs::File,
    path::{Path, PathBuf},
};
use viewer_application::{FileOperationError, FileSnapshot, watcher::FileIdentity};

#[cfg(target_os = "macos")]
const RENAME_NOFOLLOW_ANY: u32 = 0x0000_0010;

#[cfg(target_os = "macos")]
#[repr(C)]
pub(super) struct BoundFileReference {
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

#[cfg(target_os = "macos")]
pub(super) fn bind_file_reference(
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
pub(super) fn bind_open_file_reference(
    file: &File,
    expected: &FileSnapshot,
    error_path: &Path,
) -> Result<BoundFileReference, FileOperationError> {
    bind_open_file_reference_with_hook(file, expected, error_path, |_, _| {})
}

#[cfg(target_os = "macos")]
pub(super) fn bind_open_file_reference_with_hook<F>(
    file: &File,
    expected: &FileSnapshot,
    error_path: &Path,
    mut before_bind: F,
) -> Result<BoundFileReference, FileOperationError>
where
    F: FnMut(usize, &Path),
{
    const BIND_ATTEMPTS: usize = 3;
    let mut last_error = None;
    for attempt in 0..BIND_ATTEMPTS {
        let current_path = match resolve_open_file_path(file, error_path) {
            Ok(path) => path,
            Err(error) => {
                last_error = Some(error);
                std::thread::yield_now();
                continue;
            }
        };
        before_bind(attempt, &current_path);
        match bind_file_reference(&current_path, expected) {
            Ok(reference) => return Ok(reference),
            Err(error) => {
                last_error = Some(error);
                std::thread::yield_now();
            }
        }
    }
    Err(last_error.unwrap_or(FileOperationError::IdentityChanged))
}

#[cfg(target_os = "macos")]
pub(super) fn resolve_file_reference(
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
fn resolve_open_file_path(file: &File, error_path: &Path) -> Result<PathBuf, FileOperationError> {
    use std::{ffi::CStr, os::fd::AsRawFd, os::unix::ffi::OsStrExt};

    let mut path = vec![0_i8; libc::PATH_MAX as usize];
    // SAFETY: `path` is writable for PATH_MAX bytes and the file descriptor
    // remains live for the duration of the call.
    let result = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETPATH, path.as_mut_ptr()) };
    if result < 0 {
        return Err(FileOperationError::io(
            "resolve open temporary identity path",
            error_path,
            &std::io::Error::last_os_error(),
        ));
    }
    let current = unsafe { CStr::from_ptr(path.as_ptr()) };
    Ok(PathBuf::from(std::ffi::OsStr::from_bytes(
        current.to_bytes(),
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
pub(super) fn unlink_file_reference(
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
pub(super) fn open_bound_parent(
    path: &Path,
    expected: FileIdentity,
) -> Result<File, FileOperationError> {
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
pub(super) fn openat_file(
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
pub(super) fn renameat_no_replace(
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

pub(super) fn file_snapshot(file: &File) -> Result<FileSnapshot, FileOperationError> {
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
pub(super) fn validate_bound_parent_location(
    directory: &File,
    expected_path: &Path,
    expected_identity: FileIdentity,
) -> Result<(), FileOperationError> {
    use std::{ffi::CStr, os::fd::AsRawFd, os::unix::ffi::OsStrExt};
    let metadata = directory.metadata().map_err(|error| {
        FileOperationError::io("revalidate bound operation parent", expected_path, &error)
    })?;
    use std::os::unix::fs::MetadataExt;
    if metadata.dev() != expected_identity.volume || metadata.ino() != expected_identity.file {
        return Err(FileOperationError::IdentityChanged);
    }
    let mut path = vec![0_i8; libc::PATH_MAX as usize];
    let result = unsafe { libc::fcntl(directory.as_raw_fd(), libc::F_GETPATH, path.as_mut_ptr()) };
    if result < 0 {
        return Err(FileOperationError::io(
            "resolve bound operation parent",
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
