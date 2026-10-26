//! Batch image processing vocabulary (0.2.4, ADR 0008).
//!
//! Batch conversion produces new files by default and is never undoable —
//! the preflight preview is the safety net, matching the existing
//! "copies are not undone" convention for file operations.

use crate::EntityId;
use serde::{Deserialize, Serialize};

/// Target formats the batch encoder can produce. Availability is probed at
/// runtime (`CGImageDestinationCopyTypeIdentifiers`); formats the host
/// cannot encode are rejected during planning instead of mid-batch.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchImageTargetFormat {
    Jpeg,
    Png,
    Heic,
    Tiff,
    Webp,
}

impl BatchImageTargetFormat {
    /// File extension used for converted outputs (lowercase, no dot).
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Heic => "heic",
            Self::Tiff => "tiff",
            Self::Webp => "webp",
        }
    }
}

/// Resize rule for converted outputs. `LongestEdge` only ever shrinks;
/// upsampling a smaller image is not offered.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchImageResize {
    None,
    LongestEdge(u32),
}

/// Where converted outputs are written. Outputs are always new files; the
/// original sources are only touched by an explicit overwrite strategy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchImageOutput {
    /// `name{suffix}.ext` next to each source.
    Sibling { suffix: String },
    /// A subfolder of each source's folder (created on demand).
    Subfolder { name: String },
}

/// What to do when the computed destination already exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchImageConflictStrategy {
    Skip,
    Suffix,
    Overwrite,
}

/// Lossy compression quality in thousandths (0..=1000); lossless targets
/// ignore it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BatchImageQuality(pub u16);

impl BatchImageQuality {
    pub const DEFAULT: Self = Self(850);

    pub const fn is_valid(self) -> bool {
        self.0 <= 1_000
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BatchImageSettings {
    pub format: BatchImageTargetFormat,
    pub quality: BatchImageQuality,
    pub resize: BatchImageResize,
    pub output: BatchImageOutput,
    pub conflict: BatchImageConflictStrategy,
}

/// Per-file disposition computed during planning (the preflight preview).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchImagePlanDisposition {
    /// Destination is free; the file will be converted.
    Convert { destination: String },
    /// Destination exists and the skip strategy applies.
    SkippedExisting,
    /// Destination exists and the suffix strategy found a free name.
    ConvertSuffixed { destination: String },
    /// Destination exists and the caller explicitly chose overwrite.
    OverwriteExisting { destination: String },
    /// The source cannot participate (unsupported/unreadable kind).
    Rejected { reason: BatchImageRejectReason },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchImageRejectReason {
    NotAnImage,
    SourceMissing,
    UnwritableFormat,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BatchImagePlanRow {
    pub entity_id: EntityId,
    pub relative_path: String,
    pub disposition: BatchImagePlanDisposition,
}

/// Per-file execution outcome.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchImageItemOutcome {
    Converted {
        destination: String,
        width: u32,
        height: u32,
        size_bytes: u64,
    },
    Skipped {
        reason: BatchImageSkipReason,
    },
    Failed {
        reason: BatchImageFailureReason,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchImageSkipReason {
    Conflict,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchImageFailureReason {
    DecodeFailed,
    EncodeFailed,
    PlacementFailed,
    SourceChanged,
}

#[cfg(test)]
mod tests {
    use super::{BatchImageQuality, BatchImageTargetFormat};

    #[test]
    fn target_formats_use_stable_lowercase_extensions() {
        assert_eq!(BatchImageTargetFormat::Jpeg.extension(), "jpg");
        assert_eq!(BatchImageTargetFormat::Heic.extension(), "heic");
        assert_eq!(BatchImageTargetFormat::Webp.extension(), "webp");
    }

    #[test]
    fn quality_bounds_reject_out_of_range_values() {
        assert!(BatchImageQuality::DEFAULT.is_valid());
        assert!(BatchImageQuality(1_000).is_valid());
        assert!(!BatchImageQuality(1_001).is_valid());
    }
}
