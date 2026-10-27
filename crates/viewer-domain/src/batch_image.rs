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

/// Maximum edge accepted by [`BatchImageResize::Exact`]. The exact mode
/// allocates a bitmap of the requested geometry, so an unbounded value would
/// let a bad preview request exhaust memory.
pub const MAX_BATCH_EXACT_EDGE: u32 = 8_192;

/// Resize rule for converted outputs. `LongestEdge` and `Percent` only ever
/// shrink; `Exact` writes the requested width x height and may change the
/// source aspect ratio.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchImageResize {
    None,
    LongestEdge(u32),
    Percent(u16),
    Exact { width: u32, height: u32 },
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

impl BatchImageOutput {
    /// Output names are single path components: they may not introduce a
    /// directory separator, an absolute path, or a parent traversal.
    pub fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::Sibling { suffix } => validate_component(suffix, true),
            Self::Subfolder { name } => validate_component(name, false),
        }
    }
}

fn validate_component(value: &str, allow_empty: bool) -> Result<(), &'static str> {
    if !allow_empty && value.is_empty() {
        return Err("output name must not be empty");
    }
    if value == "." || value == ".." {
        return Err("output name must not be a traversal component");
    }
    if value
        .chars()
        .any(|character| character == '/' || character == '\\' || character == '\0')
    {
        return Err("output name must not contain a path separator");
    }
    Ok(())
}

impl BatchImageSettings {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.quality.is_valid() {
            return Err("quality must be within 0..=1000");
        }
        if let BatchImageResize::LongestEdge(edge) = self.resize
            && edge == 0
        {
            return Err("longest edge must be positive");
        }
        if let BatchImageResize::Percent(percent) = self.resize
            && !(1..=100).contains(&percent)
        {
            return Err("percentage must be within 1..=100");
        }
        if let BatchImageResize::Exact { width, height } = self.resize
            && (width == 0
                || height == 0
                || width > MAX_BATCH_EXACT_EDGE
                || height > MAX_BATCH_EXACT_EDGE)
        {
            return Err("exact dimensions must be within 1..=8192");
        }
        self.output.validate()
    }
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
    /// An empty suffix plus the source's own format would target the source
    /// itself; batch output must always be a new file.
    SamePath,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BatchImagePlanRow {
    pub entity_id: EntityId,
    pub relative_path: String,
    pub disposition: BatchImagePlanDisposition,
    /// Source file size, or `0` when metadata was unavailable.
    #[serde(default)]
    pub source_size_bytes: u64,
    /// Estimated output geometry; `0` when the source header was unavailable.
    #[serde(default)]
    pub target_width: u32,
    #[serde(default)]
    pub target_height: u32,
    /// Coarse output-size estimate for the preflight UI; `0` when unknown.
    #[serde(default)]
    pub estimated_size_bytes: u64,
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
    use super::{
        BatchImageConflictStrategy, BatchImageOutput, BatchImageQuality, BatchImageResize,
        BatchImageSettings, BatchImageTargetFormat,
    };

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

    #[test]
    fn output_names_reject_path_components() {
        for output in [
            BatchImageOutput::Subfolder {
                name: "../outside".into(),
            },
            BatchImageOutput::Subfolder {
                name: "/tmp/outside".into(),
            },
            BatchImageOutput::Subfolder {
                name: "nested/inside".into(),
            },
            BatchImageOutput::Subfolder { name: "..".into() },
            BatchImageOutput::Subfolder {
                name: String::new(),
            },
            BatchImageOutput::Sibling {
                suffix: "/../../outside".into(),
            },
        ] {
            assert!(output.validate().is_err(), "{output:?} should be rejected");
        }
        assert!(
            BatchImageOutput::Subfolder {
                name: "converted".into()
            }
            .validate()
            .is_ok()
        );
        assert!(
            BatchImageOutput::Sibling {
                suffix: "-converted".into()
            }
            .validate()
            .is_ok()
        );
        assert!(
            BatchImageOutput::Sibling {
                suffix: String::new()
            }
            .validate()
            .is_ok()
        );
    }

    #[test]
    fn settings_validation_covers_quality_resize_and_output() {
        let mut settings = BatchImageSettings {
            format: BatchImageTargetFormat::Jpeg,
            quality: BatchImageQuality::DEFAULT,
            resize: BatchImageResize::None,
            output: BatchImageOutput::Subfolder {
                name: "converted".into(),
            },
            conflict: BatchImageConflictStrategy::Skip,
        };
        assert!(settings.validate().is_ok());
        settings.quality = BatchImageQuality(1_001);
        assert!(settings.validate().is_err());
        settings.quality = BatchImageQuality::DEFAULT;
        settings.resize = BatchImageResize::LongestEdge(0);
        assert!(settings.validate().is_err());
        settings.resize = BatchImageResize::Percent(0);
        assert!(settings.validate().is_err());
        settings.resize = BatchImageResize::Percent(101);
        assert!(settings.validate().is_err());
        settings.resize = BatchImageResize::Percent(50);
        assert!(settings.validate().is_ok());
        settings.resize = BatchImageResize::Exact {
            width: 0,
            height: 100,
        };
        assert!(settings.validate().is_err());
        settings.resize = BatchImageResize::Exact {
            width: 8_193,
            height: 100,
        };
        assert!(settings.validate().is_err());
        settings.resize = BatchImageResize::Exact {
            width: 640,
            height: 480,
        };
        assert!(settings.validate().is_ok());
        settings.resize = BatchImageResize::None;
        settings.output = BatchImageOutput::Subfolder {
            name: "../outside".into(),
        };
        assert!(settings.validate().is_err());
    }
}
