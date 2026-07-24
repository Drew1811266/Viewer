use super::file_reference::file_snapshot;
#[cfg(target_os = "macos")]
use super::file_reference::{open_bound_parent, openat_file};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use viewer_application::{
    FileContentEvidence, FileOperationError, FileSnapshot, file_commands::FileCommandCancellation,
    watcher::FileIdentity,
};

pub(super) const COPY_BUFFER_BYTES: usize = 1024 * 1024;

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

#[cfg(target_os = "macos")]
pub(super) fn copy_and_hash_cancellable_verified_sync(
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

#[cfg(not(target_os = "macos"))]
pub(super) fn copy_open_files_and_evidence(
    source_file: File,
    temporary_file: File,
    source: &Path,
    temporary: &Path,
    cancellation: &FileCommandCancellation,
    expected_source: &FileSnapshot,
) -> Result<FileContentEvidence, FileOperationError> {
    copy_open_files_and_evidence_with_validation(
        source_file,
        temporary_file,
        source,
        temporary,
        cancellation,
        expected_source,
        || Ok(()),
    )
}

pub(super) fn copy_open_files_and_evidence_with_validation<F>(
    source_file: File,
    temporary_file: File,
    source: &Path,
    temporary: &Path,
    cancellation: &FileCommandCancellation,
    expected_source: &FileSnapshot,
    mut validate_parents: F,
) -> Result<FileContentEvidence, FileOperationError>
where
    F: FnMut() -> Result<(), FileOperationError>,
{
    let mut reader = BufReader::with_capacity(COPY_BUFFER_BYTES, source_file);
    let mut writer = BufWriter::with_capacity(COPY_BUFFER_BYTES, temporary_file);
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut hasher = blake3::Hasher::new();
    let mut length = 0_u64;
    loop {
        validate_parents()?;
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
        validate_parents()?;
        length = length
            .checked_add(read as u64)
            .ok_or(FileOperationError::VerificationFailed)?;
        hasher.update(&buffer[..read]);
    }
    if cancellation.is_cancelled() {
        return Err(FileOperationError::Cancelled);
    }
    validate_parents()?;
    writer
        .flush()
        .map_err(|error| FileOperationError::io("flush bound copy temporary", temporary, &error))?;
    validate_parents()?;
    let mut temporary_file = writer.into_inner().map_err(|error| {
        FileOperationError::io("finish bound copy temporary", temporary, error.error())
    })?;
    temporary_file
        .sync_all()
        .map_err(|error| FileOperationError::io("sync bound copy temporary", temporary, &error))?;
    validate_parents()?;
    let copied = (length, *hasher.finalize().as_bytes());
    temporary_file.seek(SeekFrom::Start(0)).map_err(|error| {
        FileOperationError::io("rewind bound copy temporary", temporary, &error)
    })?;
    let mut verify = BufReader::with_capacity(COPY_BUFFER_BYTES, temporary_file);
    let mut verified_hasher = blake3::Hasher::new();
    let mut verified_len = 0_u64;
    loop {
        validate_parents()?;
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
    validate_parents()?;
    if copied != (verified_len, *verified_hasher.finalize().as_bytes())
        || file_snapshot(reader.get_ref())? != *expected_source
    {
        return Err(FileOperationError::IdentityChanged);
    }
    let snapshot = file_snapshot(verify.get_ref())?;
    if snapshot.len != copied.0 {
        return Err(FileOperationError::VerificationFailed);
    }
    validate_parents()?;
    Ok(FileContentEvidence {
        snapshot,
        hash: copied.1,
    })
}

#[cfg(not(target_os = "macos"))]
pub(super) fn copy_and_hash_cancellable_verified_sync(
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

pub(super) fn snapshot_sync(path: &Path) -> Result<FileSnapshot, FileOperationError> {
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
pub(super) fn unix_timestamp_ns(seconds: i64, nanoseconds: i64) -> i128 {
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

pub(super) fn copy_and_hash_sync(
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

pub(super) fn copy_and_hash_cancellable_sync(
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

pub(super) fn worker_error(
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
