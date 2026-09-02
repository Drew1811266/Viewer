//! External readers share native repository validation, but have no writer capability.
mod current;
mod delta;
mod heads;
mod history;
mod legacy;
mod project;
mod request;
mod source;

use crate::review::{ReviewProtocolError, v3};
use serde::Serialize;
use serde_json::Value;
use std::io::{Read, Write};
use v3::ReadErrorCode;
use viewer_application::review_workspace::ReviewCommitError;

const PROTOCOL: &str = "viewer.review.reader/1";
const MAX_REQUEST_BYTES: u64 = 64 * 1024;

#[derive(Debug, Serialize)]
struct Failure {
    code: ReadErrorCode,
    message: String,
}
impl Failure {
    fn new(code: ReadErrorCode, message: &str) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    fn integrity(message: &str) -> Self {
        Self::new(ReadErrorCode::Integrity, message)
    }
    fn publication_pending() -> Self {
        Self::new(
            ReadErrorCode::PublicationPending,
            "review publication is still being generated",
        )
    }
}
impl From<ReviewProtocolError> for Failure {
    fn from(error: ReviewProtocolError) -> Self {
        match error {
            ReviewProtocolError::UnsupportedVersion => Self::new(
                ReadErrorCode::UnsupportedVersion,
                "unsupported review protocol version; use read-current for v3",
            ),
            ReviewProtocolError::LimitExceeded => Self::new(
                ReadErrorCode::LimitExceeded,
                "review protocol size or item limit exceeded",
            ),
            ReviewProtocolError::InvalidData => Self::integrity("review protocol data is invalid"),
        }
    }
}
impl From<ReviewCommitError> for Failure {
    fn from(error: ReviewCommitError) -> Self {
        match error {
            ReviewCommitError::LimitExceeded => Self::new(
                ReadErrorCode::LimitExceeded,
                "review file size or item limit exceeded",
            ),
            ReviewCommitError::UnsupportedProtocol => {
                ReviewProtocolError::UnsupportedVersion.into()
            }
            ReviewCommitError::Io => Self::new(ReadErrorCode::Io, "review file could not be read"),
            _ => Self::integrity("review file identity, path or content verification failed"),
        }
    }
}

#[derive(Serialize)]
#[serde(untagged)]
enum Response {
    Success {
        #[serde(rename = "protocolVersion")]
        protocol: &'static str,
        result: Value,
    },
    Error {
        #[serde(rename = "protocolVersion")]
        protocol: &'static str,
        error: Failure,
    },
}

fn read_request(input: impl Read) -> Result<Value, Failure> {
    let mut bytes = Vec::new();
    input
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Failure::new(ReadErrorCode::Io, "cannot read reader request"))?;
    if bytes.len() as u64 > MAX_REQUEST_BYTES {
        return Err(Failure::new(
            ReadErrorCode::LimitExceeded,
            "reader request exceeds 64 KiB",
        ));
    }
    let request: request::Request = serde_json::from_slice(&bytes)
        .map_err(|_| Failure::integrity("invalid or unsupported closed reader request"))?;
    request.validate()?;
    let gate_current = matches!(
        request.operation,
        request::Operation::Current | request::Operation::List
    );
    let project = project::Project::open(&request.project_root, gate_current)?;
    match request.operation {
        request::Operation::LegacyLatest | request::Operation::LegacyList => {
            legacy::read(&project, &request)
        }
        request::Operation::History => history::read(&project.into_current()?, &request),
        request::Operation::Current | request::Operation::List => {
            current::read(&project.into_current()?, &request)
        }
    }
}

/// Process a single bounded request. Errors are JSON too; no partial success is emitted.
pub fn run_review_reader(input: impl Read, mut output: impl Write) -> bool {
    let (response, success) = match read_request(input) {
        Ok(result) => (
            Response::Success {
                protocol: PROTOCOL,
                result,
            },
            true,
        ),
        Err(error) => (
            Response::Error {
                protocol: PROTOCOL,
                error,
            },
            false,
        ),
    };
    let (bytes, success) =
        match v3::encode_bounded_json(&response, crate::review::MAX_REVIEW_DOCUMENT_BYTES) {
            Ok(bytes) => (bytes, success),
            Err(error) => {
                let response = Response::Error {
                    protocol: PROTOCOL,
                    error: error.into(),
                };
                let Ok(bytes) = v3::encode_bounded_json(&response, 8192) else {
                    return false;
                };
                (bytes, false)
            }
        };
    output.write_all(&bytes).is_ok() && success
}
