use crate::text::preview::TextPreviewReader;
use std::{fs, io, path::Path};
use viewer_application::{TextPreviewPort, text::TextPreviewError};

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
        let path = path.as_ref();
        if fs::metadata(path)?.len() > MAX_INDEXED_TEXT_BYTES as u64 {
            return Ok(TextStatus::TooLarge);
        }
        match TextPreviewReader.read(path, None) {
            Ok(preview) if preview.truncated => Ok(TextStatus::TooLarge),
            Ok(preview) => Ok(TextStatus::Indexed(preview.text)),
            Err(TextPreviewError::EncodingRequired) => Ok(TextStatus::UnsupportedEncoding),
            Err(TextPreviewError::Io(message)) => {
                Err(TextExtractError::Io(io::Error::other(message)))
            }
        }
    }
}
