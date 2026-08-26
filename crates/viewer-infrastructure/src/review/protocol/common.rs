use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;

pub const MAX_REVIEW_DOCUMENT_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_REVIEW_INDEX_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ReviewProtocolError {
    #[error("review protocol version is unsupported")]
    UnsupportedVersion,
    #[error("review protocol data is invalid")]
    InvalidData,
    #[error("review protocol limit was exceeded")]
    LimitExceeded,
}

pub(super) fn detect_protocol<'a>(
    bytes: &[u8],
    max_bytes: u64,
    supported: &'a [&'a str],
) -> Result<&'a str, ReviewProtocolError> {
    if bytes.len() as u64 > max_bytes {
        return Err(ReviewProtocolError::LimitExceeded);
    }
    let envelope: StoredVersionEnvelope =
        serde_json::from_slice(bytes).map_err(|_| ReviewProtocolError::InvalidData)?;
    supported
        .iter()
        .copied()
        .find(|protocol| *protocol == envelope.protocol_version)
        .ok_or(ReviewProtocolError::UnsupportedVersion)
}

pub(super) fn decode_document<T>(
    bytes: &[u8],
    max_bytes: u64,
    expected_protocol: &str,
) -> Result<T, ReviewProtocolError>
where
    T: DeserializeOwned,
{
    let supported = [expected_protocol];
    let protocol = detect_protocol(bytes, max_bytes, &supported)?;
    debug_assert_eq!(protocol, expected_protocol);
    serde_json::from_slice(bytes).map_err(|_| ReviewProtocolError::InvalidData)
}

pub(super) fn encode_document<T>(value: &T, max_bytes: u64) -> Result<Vec<u8>, ReviewProtocolError>
where
    T: Serialize,
{
    let mut encoded =
        serde_json::to_vec_pretty(value).map_err(|_| ReviewProtocolError::InvalidData)?;
    encoded.push(b'\n');
    if encoded.len() as u64 > max_bytes {
        Err(ReviewProtocolError::LimitExceeded)
    } else {
        Ok(encoded)
    }
}

pub(super) fn parse_digest(value: &str) -> Result<[u8; 32], ReviewProtocolError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(ReviewProtocolError::InvalidData);
    }
    let mut result = [0_u8; 32];
    for (index, slot) in result.iter_mut().enumerate() {
        let high = hex_nibble(value.as_bytes()[index * 2])?;
        let low = hex_nibble(value.as_bytes()[index * 2 + 1])?;
        *slot = (high << 4) | low;
    }
    Ok(result)
}

pub(super) fn encode_digest(value: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(64);
    for byte in value {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn hex_nibble(value: u8) -> Result<u8, ReviewProtocolError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(ReviewProtocolError::InvalidData),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredVersionEnvelope {
    protocol_version: String,
}
