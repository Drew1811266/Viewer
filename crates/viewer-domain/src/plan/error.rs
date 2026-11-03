use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanErrorCode {
    UnsupportedVersion,
    InvalidSchema,
    DuplicateId,
    UnknownValue,
    UnknownGroup,
    AssetMissing,
    CountMismatch,
    RelocationConflict,
    TooLarge,
}

impl PlanErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "unsupported_version",
            Self::InvalidSchema => "invalid_schema",
            Self::DuplicateId => "duplicate_id",
            Self::UnknownValue => "unknown_value",
            Self::UnknownGroup => "unknown_group",
            Self::AssetMissing => "asset_missing",
            Self::CountMismatch => "count_mismatch",
            Self::RelocationConflict => "relocation_conflict",
            Self::TooLarge => "too_large",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanError {
    pub path: String,
    pub code: PlanErrorCode,
    pub message: String,
}

impl PlanError {
    pub fn new(
        path: impl Into<String>,
        code: PlanErrorCode,
        message: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for PlanError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_display_their_path_and_message() {
        let error = PlanError::new(
            "shots[3].references[1]",
            PlanErrorCode::AssetMissing,
            "asset is not in the reference store",
        );
        assert_eq!(
            error.to_string(),
            "shots[3].references[1]: asset is not in the reference store"
        );
    }
}
