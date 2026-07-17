use std::path::Path;

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
