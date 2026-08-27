use super::super::common::ReviewProtocolError;
use serde::Serialize;
use std::io::{self, Write};

pub(super) struct BoundedJsonBuffer {
    pub(super) bytes: Vec<u8>,
    limit: u64,
    exceeded: bool,
}
impl BoundedJsonBuffer {
    pub(super) fn new(limit: u64) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
            exceeded: false,
        }
    }
}
impl Write for BoundedJsonBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() as u64 > self.limit.saturating_sub(self.bytes.len() as u64) {
            self.exceeded = true;
            return Err(io::Error::other("review JSON exceeds its byte limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(super) fn encode_document<T: Serialize>(
    value: &T,
    max_bytes: u64,
) -> Result<Vec<u8>, ReviewProtocolError> {
    let mut buffer = BoundedJsonBuffer::new(max_bytes);
    if serde_json::to_writer_pretty(&mut buffer, value).is_err() {
        return Err(if buffer.exceeded {
            ReviewProtocolError::LimitExceeded
        } else {
            ReviewProtocolError::InvalidData
        });
    }
    buffer
        .write_all(b"\n")
        .map_err(|_| ReviewProtocolError::LimitExceeded)?;
    Ok(buffer.bytes)
}
