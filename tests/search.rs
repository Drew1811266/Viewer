use rusqlite::{Connection, OptionalExtension};
use std::fs;
use viewer_domain::{
    EntityId, RelativePath,
    file::{FileKind, FileNode},
};
use viewer_infrastructure::search::{
    index::SessionIndex,
    text::{TextExtractor, TextStatus},
};

#[test]
fn text_index_extracts_utf8_bom_empty_and_normalizes_newlines() {
    let directory = tempfile::tempdir().unwrap();
    let plain = directory.path().join("plain.txt");
    let bom = directory.path().join("bom.md");
    let empty = directory.path().join("empty.txt");
    fs::write(&plain, "第一行\r\nsecond\rline").unwrap();
    fs::write(&bom, b"\xef\xbb\xbfproduct prompt").unwrap();
    fs::write(&empty, b"").unwrap();

    assert_eq!(
        TextExtractor::extract(&plain).unwrap(),
        TextStatus::Indexed("第一行\nsecond\nline".into())
    );
    assert_eq!(
        TextExtractor::extract(&bom).unwrap(),
        TextStatus::Indexed("product prompt".into())
    );
    assert_eq!(
        TextExtractor::extract(&empty).unwrap(),
        TextStatus::Indexed(String::new())
    );
}

#[test]
fn text_index_isolates_an_unsupported_encoding() {
    let directory = tempfile::tempdir().unwrap();
    let invalid = directory.path().join("invalid.txt");
    let valid = directory.path().join("valid.txt");
    fs::write(&invalid, [0xff, 0xfe, 0xfd]).unwrap();
    fs::write(&valid, "仍可索引").unwrap();

    assert_eq!(
        TextExtractor::extract(&invalid).unwrap(),
        TextStatus::UnsupportedEncoding
    );
    assert_eq!(
        TextExtractor::extract(&valid).unwrap(),
        TextStatus::Indexed("仍可索引".into())
    );
}

#[test]
fn text_index_skips_files_larger_than_ten_mib() {
    let directory = tempfile::tempdir().unwrap();
    let oversized = directory.path().join("oversized.md");
    fs::write(&oversized, vec![b'a'; 11 * 1024 * 1024]).unwrap();

    assert_eq!(
        TextExtractor::extract(&oversized).unwrap(),
        TextStatus::TooLarge
    );
}

#[test]
fn text_index_replacement_is_atomic_and_removes_stale_rows() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("session.sqlite");
    let entity_id = EntityId::new();
    let relative_path = RelativePath::parse("notes.txt").unwrap();
    let index = SessionIndex::open(&database).unwrap();
    index
        .upsert_batch(&[FileNode {
            entity_id,
            relative_path: relative_path.clone(),
            kind: FileKind::Text,
            size: 5,
            modified_ns: 10,
        }])
        .unwrap();
    index
        .replace_text(
            entity_id,
            &relative_path,
            &TextStatus::Indexed("产品说明".into()),
        )
        .unwrap();
    index.close().unwrap();

    let connection = Connection::open(&database).unwrap();
    let body = connection
        .query_row(
            "SELECT body FROM text_fts WHERE entity_id = ?1",
            [entity_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .unwrap();
    assert_eq!(body, "产品说明");
    drop(connection);

    let index = SessionIndex::open(&database).unwrap();
    index
        .replace_text(entity_id, &relative_path, &TextStatus::TooLarge)
        .unwrap();
    index.close().unwrap();
    let connection = Connection::open(&database).unwrap();
    let stale_body = connection
        .query_row(
            "SELECT body FROM text_fts WHERE entity_id = ?1",
            [entity_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .unwrap();
    assert_eq!(stale_body, None);
}
