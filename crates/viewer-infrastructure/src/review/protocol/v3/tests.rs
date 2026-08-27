use super::bounded::BoundedJsonBuffer;
use super::*;
use std::io::Write;

#[test]
fn json_buffer_refuses_growth_past_the_document_limit() {
    let mut buffer = BoundedJsonBuffer::new(8);
    buffer.write_all(b"12345678").unwrap();
    assert!(buffer.write_all(b"9").is_err());
    assert_eq!(buffer.bytes, b"12345678");
    assert_eq!(
        encode_document(&vec!["long payload"; 20], 8),
        Err(ReviewProtocolError::LimitExceeded)
    );
}
