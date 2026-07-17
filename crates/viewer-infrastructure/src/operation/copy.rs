use async_trait::async_trait;
use std::{
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
};
use viewer_application::{
    FileMutationPort, FileOperationError, FileSnapshot, file_commands::FileCommandCancellation,
};

const COPY_BUFFER_BYTES: usize = 1024 * 1024;

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

    async fn rename(&self, source: &Path, destination: &Path) -> Result<(), FileOperationError> {
        let source = source.to_path_buf();
        let destination = destination.to_path_buf();
        let error_path = destination.clone();
        tokio::task::spawn_blocking(move || rename_no_replace_sync(&source, &destination))
            .await
            .map_err(|error| worker_error("rename worker", &error_path, error))?
    }

    async fn remove_registered_temporary(&self, path: &Path) -> Result<(), FileOperationError> {
        let path = path.to_path_buf();
        let error_path = path.clone();
        tokio::task::spawn_blocking(move || match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(FileOperationError::io(
                "remove registered temporary",
                &path,
                &error,
            )),
        })
        .await
        .map_err(|error| worker_error("temporary cleanup worker", &error_path, error))?
    }
}

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
        })
    }
    #[cfg(not(unix))]
    {
        Ok(FileSnapshot {
            len: metadata.len(),
            volume_id: 0,
            file_id: None,
        })
    }
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
