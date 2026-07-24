use super::local::COPY_BUFFER_BYTES;
use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};
use viewer_application::FileOperationError;

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
