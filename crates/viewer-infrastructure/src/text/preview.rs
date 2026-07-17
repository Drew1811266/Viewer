use encoding_rs::GB18030;
use std::{borrow::Cow, fs::File, io::Read, path::Path};
use viewer_application::text::{
    MAX_TEXT_PREVIEW_BYTES, TextEncoding, TextPreview, TextPreviewError, TextPreviewPort,
};

#[derive(Clone, Copy, Debug)]
pub struct TextPreviewReader;

impl TextPreviewPort for TextPreviewReader {
    fn read(
        &self,
        source: &Path,
        requested: Option<TextEncoding>,
    ) -> Result<TextPreview, TextPreviewError> {
        let file = File::open(source).map_err(io_error)?;
        let mut bytes = Vec::with_capacity(MAX_TEXT_PREVIEW_BYTES.min(64 * 1024));
        file.take((MAX_TEXT_PREVIEW_BYTES + 4) as u64)
            .read_to_end(&mut bytes)
            .map_err(io_error)?;
        let truncated = bytes.len() > MAX_TEXT_PREVIEW_BYTES;
        let bounded = &bytes[..bytes.len().min(MAX_TEXT_PREVIEW_BYTES)];

        let (encoding, text) = if let Some(encoding) = requested {
            let text = decode(bounded, encoding, truncated)?;
            (encoding, text)
        } else if bounded.starts_with(&[0xef, 0xbb, 0xbf]) {
            (
                TextEncoding::Utf8,
                decode(bounded, TextEncoding::Utf8, truncated)?,
            )
        } else if bounded.starts_with(&[0xff, 0xfe]) {
            (
                TextEncoding::Utf16Le,
                decode(bounded, TextEncoding::Utf16Le, truncated)?,
            )
        } else if bounded.starts_with(&[0xfe, 0xff]) {
            (
                TextEncoding::Utf16Be,
                decode(bounded, TextEncoding::Utf16Be, truncated)?,
            )
        } else if let Ok(text) = decode(bounded, TextEncoding::Utf8, truncated) {
            (TextEncoding::Utf8, text)
        } else {
            (
                TextEncoding::Gb18030,
                decode(bounded, TextEncoding::Gb18030, truncated)?,
            )
        };

        Ok(TextPreview {
            text: normalize_newlines(text),
            encoding,
            truncated,
        })
    }
}

fn decode(
    bytes: &[u8],
    encoding: TextEncoding,
    may_end_mid_character: bool,
) -> Result<String, TextPreviewError> {
    let bytes = match encoding {
        TextEncoding::Utf8 => bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes),
        TextEncoding::Utf16Le => bytes.strip_prefix(&[0xff, 0xfe]).unwrap_or(bytes),
        TextEncoding::Utf16Be => bytes.strip_prefix(&[0xfe, 0xff]).unwrap_or(bytes),
        TextEncoding::Gb18030 => bytes,
    };
    let max_trim = if may_end_mid_character { 3 } else { 0 };
    for trim in 0..=max_trim.min(bytes.len()) {
        let candidate = &bytes[..bytes.len() - trim];
        let decoded = match encoding {
            TextEncoding::Utf8 => std::str::from_utf8(candidate).ok().map(Cow::from),
            TextEncoding::Utf16Le => decode_utf16(candidate, u16::from_le_bytes).map(Cow::from),
            TextEncoding::Utf16Be => decode_utf16(candidate, u16::from_be_bytes).map(Cow::from),
            TextEncoding::Gb18030 => {
                GB18030.decode_without_bom_handling_and_without_replacement(candidate)
            }
        };
        if let Some(decoded) = decoded {
            return Ok(decoded.into_owned());
        }
    }
    Err(TextPreviewError::EncodingRequired)
}

fn decode_utf16(bytes: &[u8], convert: fn([u8; 2]) -> u16) -> Option<String> {
    if bytes.len() & 1 == 1 {
        return None;
    }
    let units = bytes
        .chunks_exact(2)
        .map(|chunk| convert([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&units).ok()
}

fn normalize_newlines(text: String) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn io_error(error: std::io::Error) -> TextPreviewError {
    TextPreviewError::Io(error.to_string())
}
