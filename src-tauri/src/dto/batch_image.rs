//! Batch image processing wire types (0.2.4, ADR 0008).
//!
//! The UI uses internally tagged (`kind`) unions for the nested policy and
//! result values. Keep those shapes in dedicated DTOs: the domain enums stay
//! implementation-oriented and are never serialized across the IPC boundary.

use serde::{Deserialize, Serialize};
use viewer_domain::batch_image::{
    BatchImageConflictStrategy, BatchImageFailureReason, BatchImageItemOutcome, BatchImageOutput,
    BatchImagePlanDisposition, BatchImagePlanRow, BatchImageQuality, BatchImageRejectReason,
    BatchImageResize, BatchImageSettings, BatchImageSkipReason, BatchImageTargetFormat,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BatchImageResizeDto {
    None,
    LongestEdge { edge: u32 },
}

impl From<BatchImageResizeDto> for BatchImageResize {
    fn from(value: BatchImageResizeDto) -> Self {
        match value {
            BatchImageResizeDto::None => Self::None,
            BatchImageResizeDto::LongestEdge { edge } => Self::LongestEdge(edge),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BatchImageOutputDto {
    Sibling { suffix: String },
    Subfolder { name: String },
}

impl From<BatchImageOutputDto> for BatchImageOutput {
    fn from(value: BatchImageOutputDto) -> Self {
        match value {
            BatchImageOutputDto::Sibling { suffix } => Self::Sibling { suffix },
            BatchImageOutputDto::Subfolder { name } => Self::Subfolder { name },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchImageSettingsDto {
    pub format: BatchImageTargetFormat,
    pub quality_milli: u16,
    pub resize: BatchImageResizeDto,
    pub output: BatchImageOutputDto,
    pub conflict: BatchImageConflictStrategy,
}

impl TryFrom<BatchImageSettingsDto> for BatchImageSettings {
    type Error = &'static str;

    fn try_from(value: BatchImageSettingsDto) -> Result<Self, Self::Error> {
        let settings = Self {
            format: value.format,
            quality: BatchImageQuality(value.quality_milli),
            resize: value.resize.into(),
            output: value.output.into(),
            conflict: value.conflict,
        };
        settings.validate()?;
        Ok(settings)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BatchImagePlanDispositionDto {
    Convert { destination: String },
    SkippedExisting,
    ConvertSuffixed { destination: String },
    OverwriteExisting { destination: String },
    Rejected { reason: BatchImageRejectReason },
}

impl From<BatchImagePlanDisposition> for BatchImagePlanDispositionDto {
    fn from(value: BatchImagePlanDisposition) -> Self {
        match value {
            BatchImagePlanDisposition::Convert { destination } => Self::Convert { destination },
            BatchImagePlanDisposition::SkippedExisting => Self::SkippedExisting,
            BatchImagePlanDisposition::ConvertSuffixed { destination } => {
                Self::ConvertSuffixed { destination }
            }
            BatchImagePlanDisposition::OverwriteExisting { destination } => {
                Self::OverwriteExisting { destination }
            }
            BatchImagePlanDisposition::Rejected { reason } => Self::Rejected { reason },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchImagePlanRowDto {
    pub entity_id: String,
    pub relative_path: String,
    pub disposition: BatchImagePlanDispositionDto,
}

impl From<BatchImagePlanRow> for BatchImagePlanRowDto {
    fn from(row: BatchImagePlanRow) -> Self {
        Self {
            entity_id: row.entity_id.to_string(),
            relative_path: row.relative_path,
            disposition: row.disposition.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BatchImageItemOutcomeDto {
    Converted {
        destination: String,
        width: u32,
        height: u32,
        #[serde(rename = "sizeBytes")]
        size_bytes: u64,
    },
    Skipped {
        reason: BatchImageSkipReason,
    },
    Failed {
        reason: BatchImageFailureReason,
    },
}

impl From<BatchImageItemOutcome> for BatchImageItemOutcomeDto {
    fn from(value: BatchImageItemOutcome) -> Self {
        match value {
            BatchImageItemOutcome::Converted {
                destination,
                width,
                height,
                size_bytes,
            } => Self::Converted {
                destination,
                width,
                height,
                size_bytes,
            },
            BatchImageItemOutcome::Skipped { reason } => Self::Skipped { reason },
            BatchImageItemOutcome::Failed { reason } => Self::Failed { reason },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchImageOutcomeDto {
    pub entity_id: String,
    pub outcome: BatchImageItemOutcomeDto,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchImageExecutionDto {
    pub batch_id: String,
    pub outcomes: Vec<BatchImageOutcomeDto>,
}

impl From<viewer_application::BatchImageExecution> for BatchImageExecutionDto {
    fn from(execution: viewer_application::BatchImageExecution) -> Self {
        Self {
            batch_id: execution.batch_id.to_string(),
            outcomes: execution
                .outcomes
                .into_iter()
                .map(|(entity_id, outcome)| BatchImageOutcomeDto {
                    entity_id: entity_id.to_string(),
                    outcome: outcome.into(),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BatchImageExecutionDto, BatchImageItemOutcomeDto, BatchImageOutputDto,
        BatchImagePlanRowDto, BatchImageResizeDto, BatchImageSettingsDto,
    };
    use serde_json::json;
    use viewer_domain::EntityId;
    use viewer_domain::batch_image::{
        BatchImageConflictStrategy, BatchImageItemOutcome, BatchImageOutput,
        BatchImagePlanDisposition, BatchImagePlanRow, BatchImageResize, BatchImageSettings,
        BatchImageTargetFormat,
    };

    #[test]
    fn wire_settings_use_kind_tags() {
        let value = json!({
            "format": "jpeg",
            "qualityMilli": 850,
            "resize": { "kind": "longest_edge", "edge": 1600 },
            "output": { "kind": "sibling", "suffix": "-converted" },
            "conflict": "suffix"
        });
        let dto: BatchImageSettingsDto = serde_json::from_value(value).unwrap();
        let settings = BatchImageSettings::try_from(dto).unwrap();
        assert_eq!(settings.resize, BatchImageResize::LongestEdge(1_600));
        assert_eq!(
            settings.output,
            BatchImageOutput::Sibling {
                suffix: "-converted".into()
            }
        );
    }

    #[test]
    fn wire_settings_reject_output_traversal() {
        let error = BatchImageSettings::try_from(BatchImageSettingsDto {
            format: BatchImageTargetFormat::Jpeg,
            quality_milli: 850,
            resize: BatchImageResizeDto::None,
            output: BatchImageOutputDto::Subfolder {
                name: "../outside".into(),
            },
            conflict: BatchImageConflictStrategy::Skip,
        })
        .unwrap_err();
        assert!(error.contains("path separator") || error.contains("traversal"));
    }

    #[test]
    fn plan_and_outcome_serialize_with_kind_tags() {
        let row = BatchImagePlanRowDto::from(BatchImagePlanRow {
            entity_id: EntityId::new(),
            relative_path: "photo.jpg".into(),
            disposition: BatchImagePlanDisposition::Convert {
                destination: "photo-converted.webp".into(),
            },
        });
        let value = serde_json::to_value(&row).unwrap();
        assert_eq!(value["disposition"]["kind"], "convert");
        assert_eq!(value["disposition"]["destination"], "photo-converted.webp");

        let outcome = BatchImageItemOutcomeDto::from(BatchImageItemOutcome::Converted {
            destination: "photo-converted.webp".into(),
            width: 10,
            height: 8,
            size_bytes: 7,
        });
        let value = serde_json::to_value(&outcome).unwrap();
        assert_eq!(value["kind"], "converted");
        assert_eq!(value["sizeBytes"], 7);
    }

    #[test]
    fn execution_wraps_outcomes_in_camel_case() {
        let execution = BatchImageExecutionDto {
            batch_id: "batch-1".into(),
            outcomes: vec![super::BatchImageOutcomeDto {
                entity_id: "entity-1".into(),
                outcome: BatchImageItemOutcomeDto::Skipped {
                    reason: viewer_domain::batch_image::BatchImageSkipReason::Conflict,
                },
            }],
        };
        let value = serde_json::to_value(&execution).unwrap();
        assert_eq!(value["batchId"], "batch-1");
        assert_eq!(value["outcomes"][0]["entityId"], "entity-1");
        assert_eq!(value["outcomes"][0]["outcome"]["kind"], "skipped");
    }
}
