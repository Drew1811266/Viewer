use encoding_rs::GB18030;
use std::fs;
use viewer_domain::file::FileKind;
use viewer_infrastructure::search::text::{TextExtractor, TextStatus};

#[test]
fn text_index_supports_utf16_and_gb18030_without_lossy_replacement() {
    let directory = tempfile::tempdir().unwrap();
    let utf16_le = directory.path().join("utf16.txt");
    let mut utf16_bytes = vec![0xff, 0xfe];
    for unit in "产品说明".encode_utf16() {
        utf16_bytes.extend_from_slice(&unit.to_le_bytes());
    }
    fs::write(&utf16_le, utf16_bytes).unwrap();
    let gb18030 = directory.path().join("prompt.md");
    let (gb_bytes, _, had_errors) = GB18030.encode("白色陶瓷杯");
    assert!(!had_errors);
    fs::write(&gb18030, gb_bytes.as_ref()).unwrap();

    assert_eq!(
        TextExtractor::extract(&utf16_le).unwrap(),
        TextStatus::Indexed("产品说明".into())
    );
    assert_eq!(
        TextExtractor::extract(&gb18030).unwrap(),
        TextStatus::Indexed("白色陶瓷杯".into())
    );
}

#[test]
fn text_index_keeps_invalid_and_oversized_files_isolated() {
    let directory = tempfile::tempdir().unwrap();
    let invalid = directory.path().join("invalid.txt");
    let oversized = directory.path().join("oversized.md");
    fs::write(&invalid, [0xff, 0xff, 0xff]).unwrap();
    fs::write(&oversized, vec![b'a'; 10 * 1024 * 1024 + 1]).unwrap();

    assert_eq!(
        TextExtractor::extract(&invalid).unwrap(),
        TextStatus::UnsupportedEncoding
    );
    assert_eq!(
        TextExtractor::extract(&oversized).unwrap(),
        TextStatus::TooLarge
    );
}

#[test]
fn unsupported_images_and_other_files_are_not_eligible_for_derived_work() {
    for kind in [FileKind::UnsupportedImage, FileKind::Other] {
        assert!(!kind.is_previewable_image());
        assert!(!kind.is_previewable_text());
    }
}
