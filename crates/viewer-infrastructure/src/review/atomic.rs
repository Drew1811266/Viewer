use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) enum AtomicCreateOnceError {
    AlreadyExists,
    Io(io::Error),
}

pub(super) fn atomic_replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing parent directory"))?;
    let temporary = create_temporary(parent)?;
    let temporary_path = temporary.0.clone();
    let result = write_and_replace(temporary, path, parent, bytes);
    if result.is_err() {
        let _ = fs::remove_file(temporary_path);
    }
    result
}

pub(super) fn atomic_create_once(path: &Path, bytes: &[u8]) -> Result<(), AtomicCreateOnceError> {
    let parent = path.parent().ok_or_else(|| {
        AtomicCreateOnceError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing parent directory",
        ))
    })?;
    let temporary = create_temporary(parent).map_err(AtomicCreateOnceError::Io)?;
    let temporary_path = temporary.0.clone();
    let result = write_and_create_once(temporary, path, parent, bytes);
    if result.is_err() {
        let _ = fs::remove_file(temporary_path);
    }
    result
}

fn create_temporary(parent: &Path) -> io::Result<(PathBuf, File)> {
    for _ in 0..64 {
        let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".viewer-review-{}-{sequence:016x}.tmp",
            std::process::id()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)
        {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate repository temporary file",
    ))
}

fn write_and_replace(
    temporary: (PathBuf, File),
    destination: &Path,
    parent: &Path,
    bytes: &[u8],
) -> io::Result<()> {
    let (temporary_path, mut file) = temporary;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary_path, destination)?;
    sync_directory(parent)
}

fn write_and_create_once(
    temporary: (PathBuf, File),
    destination: &Path,
    parent: &Path,
    bytes: &[u8],
) -> Result<(), AtomicCreateOnceError> {
    let (temporary_path, mut file) = temporary;
    file.write_all(bytes).map_err(AtomicCreateOnceError::Io)?;
    file.sync_all().map_err(AtomicCreateOnceError::Io)?;
    drop(file);
    match fs::hard_link(&temporary_path, destination) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Err(AtomicCreateOnceError::AlreadyExists);
        }
        Err(error) => return Err(AtomicCreateOnceError::Io(error)),
    }
    sync_directory(parent).map_err(AtomicCreateOnceError::Io)?;
    fs::remove_file(&temporary_path).map_err(AtomicCreateOnceError::Io)?;
    sync_directory(parent).map_err(AtomicCreateOnceError::Io)
}

pub(super) fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}
