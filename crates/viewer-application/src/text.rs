use std::path::{Path, PathBuf};

pub const MAX_TEXT_PREVIEW_BYTES: usize = 10 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextEncoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    Gb18030,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextPreview {
    pub text: String,
    pub encoding: TextEncoding,
    pub truncated: bool,
    /// blake3 fingerprint (hex) of the bounded bytes the preview decoded; a
    /// save must echo it back so the writer can refuse blind overwrites of
    /// externally modified files (ADR 0008).
    pub fingerprint: String,
    /// Whether the source bytes contained CRLF line endings, so an edited
    /// save can round-trip the original newline style.
    pub uses_crlf: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TextPreviewError {
    #[error("text file could not be read: {0}")]
    Io(String),
    #[error("text encoding requires an explicit choice")]
    EncodingRequired,
}

pub trait TextPreviewPort: Send + Sync {
    fn read(
        &self,
        source: &Path,
        encoding: Option<TextEncoding>,
    ) -> Result<TextPreview, TextPreviewError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextNewlineStyle {
    Lf,
    Crlf,
}

#[derive(Clone, Copy, Debug)]
pub struct TextSaveRequest<'a> {
    pub source: &'a Path,
    pub content: &'a str,
    pub expected_fingerprint: &'a str,
    pub newline_style: TextNewlineStyle,
    pub acknowledge_encoding_conversion: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct TextCopyRequest<'a> {
    pub source: &'a Path,
    pub content: &'a str,
    pub newline_style: TextNewlineStyle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextCopyOutcome {
    pub path: PathBuf,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextSaveOutcome {
    /// blake3 fingerprint (hex) of the newly written bytes; the caller keeps
    /// it as the expected fingerprint for the next save of the same load.
    pub fingerprint: String,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TextSaveError {
    #[error("text file could not be written: {0}")]
    Io(String),
    #[error("text file changed since it was loaded")]
    Conflict,
    #[error("text file is too large to edit")]
    TooLarge,
    #[error("saving requires an explicit encoding conversion acknowledgement")]
    EncodingAcknowledgementRequired,
}

pub trait TextSavePort: Send + Sync {
    fn save(&self, request: TextSaveRequest<'_>) -> Result<TextSaveOutcome, TextSaveError>;
    /// Write `content` next to `source` under a fresh `-copy` name without
    /// touching the source. Never overwrites an existing file.
    fn save_copy(&self, request: TextCopyRequest<'_>) -> Result<TextCopyOutcome, TextSaveError>;
}
