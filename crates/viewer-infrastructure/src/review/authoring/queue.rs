use viewer_application::review_workspace::{ReviewCommitError, ReviewMaterializationFailure};

pub(super) const MATERIALIZATION_LEASE_MS: i64 = 30_000;

pub(super) fn failure_code(value: ReviewMaterializationFailure) -> &'static str {
    match value {
        ReviewMaterializationFailure::SourceChanged => "source_changed",
        ReviewMaterializationFailure::SourceMissing => "source_missing",
        ReviewMaterializationFailure::SourceUnreadable => "source_unreadable",
        ReviewMaterializationFailure::RenderFailed => "render_failed",
        ReviewMaterializationFailure::Integrity => "integrity",
        ReviewMaterializationFailure::LimitExceeded => "limit_exceeded",
        ReviewMaterializationFailure::Io => "io",
    }
}

pub(super) fn parse_failure(
    value: &str,
) -> Result<ReviewMaterializationFailure, ReviewCommitError> {
    Ok(match value {
        "source_changed" => ReviewMaterializationFailure::SourceChanged,
        "source_missing" => ReviewMaterializationFailure::SourceMissing,
        "source_unreadable" => ReviewMaterializationFailure::SourceUnreadable,
        "render_failed" => ReviewMaterializationFailure::RenderFailed,
        "integrity" => ReviewMaterializationFailure::Integrity,
        "limit_exceeded" => ReviewMaterializationFailure::LimitExceeded,
        "io" => ReviewMaterializationFailure::Io,
        _ => return Err(ReviewCommitError::Integrity),
    })
}
