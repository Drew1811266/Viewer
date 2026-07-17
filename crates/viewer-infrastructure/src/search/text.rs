use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};

pub const MAX_INDEXED_TEXT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextStatus {
    Indexed(String),
    UnsupportedEncoding,
    TooLarge,
}

#[derive(Debug, thiserror::Error)]
pub enum TextExtractError {
    #[error("text file could not be read: {0}")]
    Io(#[from] io::Error),
}

#[derive(Clone, Copy, Debug)]
pub struct TextExtractor;

impl TextExtractor {
    pub fn extract(path: impl AsRef<Path>) -> Result<TextStatus, TextExtractError> {
        let file = File::open(path)?;
        let mut bytes = Vec::with_capacity(MAX_INDEXED_TEXT_BYTES.min(64 * 1024));
        file.take((MAX_INDEXED_TEXT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_INDEXED_TEXT_BYTES {
            return Ok(TextStatus::TooLarge);
        }
        let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
        let Ok(text) = std::str::from_utf8(bytes) else {
            return Ok(TextStatus::UnsupportedEncoding);
        };
        Ok(TextStatus::Indexed(
            text.replace("\r\n", "\n").replace('\r', "\n"),
        ))
    }
}
