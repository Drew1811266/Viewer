//! Batch image processing wire types (0.2.4, ADR 0008).

use serde::{Deserialize, Serialize};
use viewer_domain::batch_image::{
    BatchImageConflictStrategy, BatchImageItemOutcome, BatchImageOutput, BatchImagePlanDisposition,
    BatchImagePlanRow, BatchImageQuality, BatchImageResize, BatchImageSettings,
    BatchImageTargetFormat,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BatchImageSettingsDto {
    pub format: BatchImageTargetFormat,
    pub quality_milli: u16,
    pub resize: BatchImageResize,
    pub output: BatchImageOutput,
    pub conflict: BatchImageConflictStrategy,
}

impl TryFrom<BatchImageSettingsDto> for BatchImageSettings {
    type Error = &'static str;

    fn try_from(value: BatchImageSettingsDto) -> Result<Self, Self::Error> {
        let settings = Self {
            format: value.format,
            quality: BatchImageQuality(value.quality_milli),
            resize: value.resize,
            output: value.output,
            conflict: value.conflict,
        };
        settings.validate()?;
        Ok(settings)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchImagePlanRowDto {
    pub entity_id: String,
    pub relative_path: String,
    pub disposition: BatchImagePlanDisposition,
}

impl From<BatchImagePlanRow> for BatchImagePlanRowDto {
    fn from(row: BatchImagePlanRow) -> Self {
        Self {
            entity_id: row.entity_id.to_string(),
            relative_path: row.relative_path,
            disposition: row.disposition,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchImageOutcomeDto {
    pub entity_id: String,
    pub outcome: BatchImageItemOutcome,
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
                    outcome,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BatchImageSettingsDto;
    use viewer_domain::batch_image::{
        BatchImageConflictStrategy, BatchImageOutput, BatchImageResize, BatchImageSettings,
        BatchImageTargetFormat,
    };

    fn dto(output: BatchImageOutput) -> BatchImageSettingsDto {
        BatchImageSettingsDto {
            format: BatchImageTargetFormat::Jpeg,
            quality_milli: 850,
            resize: BatchImageResize::None,
            output,
            conflict: BatchImageConflictStrategy::Skip,
        }
    }

    #[test]
    fn wire_settings_reject_output_traversal() {
        let error = BatchImageSettings::try_from(dto(BatchImageOutput::Subfolder {
            name: "../outside".into(),
        }))
        .unwrap_err();
        assert!(error.contains("path separator") || error.contains("traversal"));
    }

    #[test]
    fn wire_settings_accept_a_plain_subfolder() {
        let settings = BatchImageSettings::try_from(dto(BatchImageOutput::Subfolder {
            name: "converted".into(),
        }))
        .unwrap();
        assert_eq!(
            settings.output,
            BatchImageOutput::Subfolder {
                name: "converted".into()
            }
        );
    }
}
