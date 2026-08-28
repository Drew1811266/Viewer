use std::ffi::{CStr, CString};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub(super) enum AtomicCreateOnceError {
    AlreadyExists,
    Io(io::Error),
}

pub(super) fn atomic_replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let (parent, name) = destination(path)?;
    atomic_replace_at(&parent, name, bytes)
}

pub(super) fn atomic_create_once(path: &Path, bytes: &[u8]) -> Result<(), AtomicCreateOnceError> {
    let (parent, name) = destination(path).map_err(AtomicCreateOnceError::Io)?;
    atomic_create_once_at(&parent, name, bytes)
}

fn destination(path: &Path) -> io::Result<(File, &str)> {
    let parent = path.parent().ok_or_else(invalid_name)?;
    let name = path
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(invalid_name)?;
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(parent)?;
    Ok((directory, name))
}

pub(super) fn leaf_name(name: &str) -> io::Result<CString> {
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\']) {
        return Err(invalid_name());
    }
    CString::new(name).map_err(|_| invalid_name())
}

fn invalid_name() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "expected a single repository filename",
    )
}

pub(super) fn open_at(parent: &File, name: &CStr, flags: i32) -> io::Result<File> {
    // The owned descriptor anchors lookup; no component in `name` can escape it.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            0o600,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn create_temporary(parent: &File) -> io::Result<(CString, File)> {
    for _ in 0..64 {
        let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let name = leaf_name(&format!(
            ".viewer-review-{}-{sequence:016x}.tmp",
            std::process::id()
        ))?;
        match open_at(parent, &name, libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL) {
            Ok(file) => return Ok((name, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate repository temporary file",
    ))
}

pub(super) fn atomic_replace_at(parent: &File, name: &str, bytes: &[u8]) -> io::Result<()> {
    atomic_replace_at_with_barrier(parent, name, bytes, || Ok(()))
}

pub(super) fn atomic_replace_at_with_barrier(
    parent: &File,
    name: &str,
    bytes: &[u8],
    before_directory_sync: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    publish(
        parent,
        name,
        false,
        |file| file.write_all(bytes),
        before_directory_sync,
    )
}

pub(super) fn atomic_create_once_at(
    parent: &File,
    name: &str,
    bytes: &[u8],
) -> Result<(), AtomicCreateOnceError> {
    atomic_create_once_with(parent, name, |file| file.write_all(bytes))
}

pub(super) fn atomic_create_once_with(
    parent: &File,
    name: &str,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> Result<(), AtomicCreateOnceError> {
    publish(parent, name, true, write, || Ok(())).map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            AtomicCreateOnceError::AlreadyExists
        } else {
            AtomicCreateOnceError::Io(error)
        }
    })
}

fn publish(
    parent: &File,
    name: &str,
    once: bool,
    write: impl FnOnce(&mut File) -> io::Result<()>,
    before_directory_sync: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    let destination = leaf_name(name)?;
    let (temporary, mut file) = create_temporary(parent)?;
    let result = (|| {
        write(&mut file)?;
        file.sync_all()?;
        drop(file);
        let fd = parent.as_raw_fd();
        if once {
            rename_exclusive(parent, &temporary, &destination)?;
        } else if unsafe { libc::renameat(fd, temporary.as_ptr(), fd, destination.as_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // Publication has happened. A failed durability barrier must never restore
        // the previous index: callers must resolve the command from the head.
        before_directory_sync()?;
        parent.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = unlink_temporary(parent, &temporary);
    }
    result
}

fn rename_exclusive(parent: &File, temporary: &CStr, destination: &CStr) -> io::Result<()> {
    let fd = parent.as_raw_fd();
    // Both names are validated single components in the same owned directory.
    // Never publish via link/unlink: an interruption would leave nlink == 2,
    // making an otherwise valid immutable object fail the repository's safety checks.
    #[cfg(target_os = "macos")]
    let status = unsafe {
        libc::renameatx_np(
            fd,
            temporary.as_ptr(),
            fd,
            destination.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    #[cfg(target_os = "linux")]
    let status = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            fd,
            temporary.as_ptr(),
            fd,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        if status == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (fd, temporary, destination);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "exclusive rename is required",
        ))
    }
}

fn unlink_temporary(parent: &File, temporary: &CStr) -> io::Result<()> {
    if unsafe { libc::unlinkat(parent.as_raw_fd(), temporary.as_ptr(), 0) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub(super) fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, symlink};

    #[test]
    fn interrupted_create_once_keeps_single_linked_bytes_and_is_retryable() {
        const CHILD_DIRECTORY: &str = "VIEWER_ATOMIC_INTERRUPT_TEST_DIRECTORY";
        if let Some(path) = std::env::var_os(CHILD_DIRECTORY) {
            let parent = File::open(path).unwrap();
            publish(
                &parent,
                "state.json",
                true,
                |file| file.write_all(b"immutable"),
                || std::process::exit(73),
            )
            .unwrap();
            panic!("child did not interrupt publication");
        }
        let root = tempfile::TempDir::new().unwrap();
        // Exit at the publication/barrier boundary without stack unwinding or cleanup.
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "review::atomic::tests::interrupted_create_once_keeps_single_linked_bytes_and_is_retryable",
            ])
            .env(CHILD_DIRECTORY, root.path())
            .output()
            .unwrap();
        assert_eq!(child.status.code(), Some(73), "{child:?}");
        let path = root.path().join("state.json");
        assert_eq!(fs::metadata(&path).unwrap().nlink(), 1);
        assert_eq!(fs::read(&path).unwrap(), b"immutable");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        let parent = File::open(root.path()).unwrap();
        assert!(matches!(
            atomic_create_once_at(&parent, "state.json", b"replacement"),
            Err(AtomicCreateOnceError::AlreadyExists)
        ));
        assert_eq!(fs::read(path).unwrap(), b"immutable");
    }

    #[test]
    fn descriptor_publication_never_follows_a_replaced_parent_path() {
        let root = tempfile::TempDir::new().unwrap();
        let outside = tempfile::TempDir::new().unwrap();
        let original = root.path().join("original");
        fs::create_dir(&original).unwrap();
        let parent = File::open(&original).unwrap();
        fs::rename(&original, root.path().join("moved")).unwrap();
        symlink(outside.path(), &original).unwrap();
        atomic_replace_at(&parent, "index.json", b"committed").unwrap();
        assert_eq!(
            fs::read(root.path().join("moved/index.json")).unwrap(),
            b"committed"
        );
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
    }

    #[test]
    fn descriptor_create_once_preserves_existing_bytes_and_rejects_path_components() {
        let root = tempfile::TempDir::new().unwrap();
        let parent = File::open(root.path()).unwrap();
        atomic_create_once_at(&parent, "state.json", b"first").unwrap();
        assert!(matches!(
            atomic_create_once_at(&parent, "state.json", b"other"),
            Err(AtomicCreateOnceError::AlreadyExists)
        ));
        assert_eq!(fs::read(root.path().join("state.json")).unwrap(), b"first");
        assert!(atomic_replace_at(&parent, "../escape", b"bad").is_err());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn failure_at_index_directory_sync_reports_error_without_rolling_back_published_bytes() {
        let root = tempfile::TempDir::new().unwrap();
        let parent = File::open(root.path()).unwrap();
        atomic_replace_at(&parent, "index.json", b"old").unwrap();
        let result = atomic_replace_at_with_barrier(&parent, "index.json", b"new", || {
            Err(io::Error::other("injected directory sync failure"))
        });
        assert!(result.is_err());
        assert_eq!(fs::read(root.path().join("index.json")).unwrap(), b"new");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}
