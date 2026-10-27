//! Atomic text write-back (0.2.3, ADR 0008).
//!
//! Saves refuse to fly blind: the current bytes on disk are re-read and
//! fingerprinted first, and any mismatch with the load-time fingerprint is a
//! conflict instead of an overwrite. The write itself goes to a temporary
//! file in the target directory and is renamed into place, so a crash either
//! leaves the original file untouched or the new content fully visible.

use std::{fs::File, io::Read};
use viewer_application::text::{
    MAX_TEXT_PREVIEW_BYTES, TextCopyOutcome, TextCopyRequest, TextNewlineStyle, TextSaveError,
    TextSaveOutcome, TextSavePort, TextSaveRequest,
};

#[derive(Clone, Copy, Debug)]
pub struct AtomicTextWriter;

impl TextSavePort for AtomicTextWriter {
    fn save(&self, request: TextSaveRequest<'_>) -> Result<TextSaveOutcome, TextSaveError> {
        let TextSaveRequest {
            source,
            content,
            expected_fingerprint,
            newline_style,
            acknowledge_encoding_conversion,
        } = request;

        if content.len() > MAX_TEXT_PREVIEW_BYTES {
            return Err(TextSaveError::TooLarge);
        }

        let file = File::open(source).map_err(io_error)?;
        let mut current = Vec::new();
        file.take((MAX_TEXT_PREVIEW_BYTES + 4) as u64)
            .read_to_end(&mut current)
            .map_err(io_error)?;
        if current.len() > MAX_TEXT_PREVIEW_BYTES {
            return Err(TextSaveError::TooLarge);
        }

        let current_fingerprint = blake3::hash(&current).to_hex().to_string();
        if current_fingerprint != expected_fingerprint {
            return Err(TextSaveError::Conflict);
        }

        let stripped = current
            .strip_prefix(&[0xef, 0xbb, 0xbf])
            .unwrap_or(&current);
        let original_is_utf8 = std::str::from_utf8(stripped).is_ok();
        if !original_is_utf8 && !acknowledge_encoding_conversion {
            return Err(TextSaveError::EncodingAcknowledgementRequired);
        }

        let bytes = encode_content(content, newline_style);

        let parent = source
            .parent()
            .ok_or_else(|| io_error_message("text file has no parent directory"))?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(io_error)?;
        std::io::Write::write_all(&mut temporary, &bytes).map_err(io_error)?;
        temporary.as_file().sync_all().map_err(io_error)?;
        temporary
            .persist(source)
            .map_err(|error| io_error_message(&error.to_string()))?;

        Ok(TextSaveOutcome {
            fingerprint: blake3::hash(&bytes).to_hex().to_string(),
        })
    }

    fn save_copy(&self, request: TextCopyRequest<'_>) -> Result<TextCopyOutcome, TextSaveError> {
        let TextCopyRequest {
            source,
            content,
            newline_style,
        } = request;
        if content.len() > MAX_TEXT_PREVIEW_BYTES {
            return Err(TextSaveError::TooLarge);
        }
        let parent = source
            .parent()
            .ok_or_else(|| io_error_message("text file has no parent directory"))?;
        let stem = source
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| io_error_message("text file has no usable name"))?;
        let extension = source.extension().and_then(|extension| extension.to_str());
        let bytes = encode_content(content, newline_style);
        let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(io_error)?;
        std::io::Write::write_all(&mut temporary, &bytes).map_err(io_error)?;
        temporary.as_file().sync_all().map_err(io_error)?;
        let mut candidate = temporary;
        for index in 0..1_000_u32 {
            let name = if index == 0 {
                format!("{stem}-copy")
            } else {
                format!("{stem}-copy-{}", index + 1)
            };
            let name = match extension {
                Some(extension) => format!("{name}.{extension}"),
                None => name,
            };
            let destination = parent.join(name);
            match candidate.persist_noclobber(&destination) {
                Ok(_) => {
                    return Ok(TextCopyOutcome {
                        path: destination,
                        fingerprint: blake3::hash(&bytes).to_hex().to_string(),
                    });
                }
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                    candidate = error.file;
                }
                Err(error) => return Err(io_error_message(&error.to_string())),
            }
        }
        Err(io_error_message("too many text copy candidates"))
    }
}

fn encode_content(content: &str, newline_style: TextNewlineStyle) -> Vec<u8> {
    match newline_style {
        TextNewlineStyle::Lf => content.as_bytes().to_vec(),
        TextNewlineStyle::Crlf => {
            let mut converted = String::with_capacity(content.len() + content.len() / 8);
            for line in content.split('\n') {
                converted.push_str(line);
                converted.push_str("\r\n");
            }
            converted.truncate(converted.len() - 2);
            converted.into_bytes()
        }
    }
}

fn io_error(error: std::io::Error) -> TextSaveError {
    TextSaveError::Io(error.to_string())
}

fn io_error_message(message: &str) -> TextSaveError {
    TextSaveError::Io(message.to_owned())
}
