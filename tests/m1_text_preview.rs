use std::fs;
use viewer_application::text::{
    MAX_TEXT_PREVIEW_BYTES, TextEncoding, TextPreviewError, TextPreviewPort,
};
use viewer_infrastructure::text::preview::TextPreviewReader;

fn write_fixture(bytes: &[u8]) -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().unwrap();
    fs::write(file.path(), bytes).unwrap();
    file
}

fn utf16_bytes(text: &str, little_endian: bool) -> Vec<u8> {
    let mut bytes = if little_endian {
        vec![0xff, 0xfe]
    } else {
        vec![0xfe, 0xff]
    };
    for unit in text.encode_utf16() {
        bytes.extend(if little_endian {
            unit.to_le_bytes()
        } else {
            unit.to_be_bytes()
        });
    }
    bytes
}

#[test]
fn reader_detects_utf8_utf16_and_gb18030_without_mutating_source() {
    let fixtures = [
        (
            b"\xef\xbb\xbfhello\r\nviewer".to_vec(),
            "hello\nviewer",
            TextEncoding::Utf8,
        ),
        (
            utf16_bytes("你好\rViewer", true),
            "你好\nViewer",
            TextEncoding::Utf16Le,
        ),
        (
            utf16_bytes("预览\r\nViewer", false),
            "预览\nViewer",
            TextEncoding::Utf16Be,
        ),
        (vec![0xd6, 0xd0, 0xce, 0xc4], "中文", TextEncoding::Gb18030),
    ];

    for (bytes, expected, encoding) in fixtures {
        let file = write_fixture(&bytes);
        let before = fs::read(file.path()).unwrap();

        let preview = TextPreviewReader.read(file.path(), None).unwrap();

        assert_eq!(preview.text, expected);
        assert_eq!(preview.encoding, encoding);
        assert!(!preview.truncated);
        assert_eq!(fs::read(file.path()).unwrap(), before);
    }
}

#[test]
fn manual_encoding_applies_only_to_the_current_read() {
    let file = write_fixture(&[0xd6, 0xd0, 0xce, 0xc4]);
    let preview = TextPreviewReader
        .read(file.path(), Some(TextEncoding::Gb18030))
        .unwrap();
    assert_eq!(preview.text, "中文");
    assert_eq!(preview.encoding, TextEncoding::Gb18030);

    let utf8 = write_fixture("中文".as_bytes());
    let automatic = TextPreviewReader.read(utf8.path(), None).unwrap();
    assert_eq!(automatic.encoding, TextEncoding::Utf8);
}

#[test]
fn reader_stops_at_ten_mib_on_a_utf8_character_boundary() {
    let mut bytes = vec![b'a'; MAX_TEXT_PREVIEW_BYTES - 1];
    bytes.extend("界".as_bytes());
    bytes.extend(b"trailing");
    let file = write_fixture(&bytes);

    let preview = TextPreviewReader
        .read(file.path(), Some(TextEncoding::Utf8))
        .unwrap();

    assert!(preview.truncated);
    assert!(preview.text.len() <= MAX_TEXT_PREVIEW_BYTES);
    assert!(std::str::from_utf8(preview.text.as_bytes()).is_ok());
    assert!(!preview.text.contains('界'));
}

#[test]
fn invalid_automatic_encoding_requests_an_explicit_choice() {
    let file = write_fixture(&[0xff, 0xff, 0xff]);

    let result = TextPreviewReader.read(file.path(), None);

    assert!(matches!(result, Err(TextPreviewError::EncodingRequired)));
}

#[test]
fn unreadable_source_is_isolated_as_an_io_error() {
    let directory = tempfile::tempdir().unwrap();

    let result = TextPreviewReader.read(&directory.path().join("missing.txt"), None);

    assert!(matches!(result, Err(TextPreviewError::Io(_))));
}
