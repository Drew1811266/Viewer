use super::{ReviewWorkspaceErrorCode as Code, ReviewWorkspaceErrorDto as Error};
use serde::Serialize;
use std::io::{self, Write};
use viewer_application::review_workspace::{ReviewCommitReceipt, ReviewWorkspaceError};

const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;

/// Measure the encoded wire value without retaining an expanded JSON buffer. In particular,
/// shared Domain Arc values may expand many times during serialization. Tauri only receives
/// the typed DTO after this bounded pass; a committed write never loses its receipt here.
pub fn review_response<T: Serialize>(
    value: T,
    committed: Option<ReviewCommitReceipt>,
) -> Result<T, Error> {
    let mut budget = Budget {
        remaining: MAX_RESPONSE_BYTES,
        exceeded: false,
    };
    if serde_json::to_writer(&mut budget, &value).is_err() {
        return Err(match committed {
            Some(receipt) => ReviewWorkspaceError::CommittedViewUnavailable(receipt).into(),
            None => Error::new(if budget.exceeded {
                Code::LimitExceeded
            } else {
                Code::InvalidData
            }),
        });
    }
    Ok(value)
}

struct Budget {
    remaining: usize,
    exceeded: bool,
}
impl Write for Budget {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.remaining {
            self.exceeded = true;
            return Err(io::Error::other("review response limit exceeded"));
        }
        self.remaining -= bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn response_budget_accepts_exact_boundary_without_allocating_output() {
        let mut budget = Budget {
            remaining: 8,
            exceeded: false,
        };
        budget.write_all(b"12345678").unwrap();
        assert_eq!(budget.remaining, 0);
        assert!(!budget.exceeded);
        assert!(budget.write_all(b"9").is_err());
        assert!(budget.exceeded);
    }
}
