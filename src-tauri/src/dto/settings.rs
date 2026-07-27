use serde::{Deserialize, Serialize};
use viewer_application::{ThumbnailDensity, VIEWER_SETTINGS_SCHEMA_VERSION, ViewerSettings};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThumbnailDensityDto {
    Compact,
    Standard,
    Large,
}

impl From<ThumbnailDensity> for ThumbnailDensityDto {
    fn from(value: ThumbnailDensity) -> Self {
        match value {
            ThumbnailDensity::Compact => Self::Compact,
            ThumbnailDensity::Standard => Self::Standard,
            ThumbnailDensity::Large => Self::Large,
        }
    }
}

impl From<ThumbnailDensityDto> for ThumbnailDensity {
    fn from(value: ThumbnailDensityDto) -> Self {
        match value {
            ThumbnailDensityDto::Compact => Self::Compact,
            ThumbnailDensityDto::Standard => Self::Standard,
            ThumbnailDensityDto::Large => Self::Large,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerSettingsDto {
    pub schema_version: u32,
    pub thumbnail_density: ThumbnailDensityDto,
}

impl From<ViewerSettings> for ViewerSettingsDto {
    fn from(value: ViewerSettings) -> Self {
        Self {
            schema_version: VIEWER_SETTINGS_SCHEMA_VERSION,
            thumbnail_density: value.thumbnail_density.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ThumbnailDensityDto, ViewerSettingsDto};
    use viewer_application::{ThumbnailDensity, ViewerSettings};

    #[test]
    fn viewer_settings_serialize_to_the_frozen_camel_case_shape() {
        assert_eq!(
            serde_json::to_value(ViewerSettingsDto::from(ViewerSettings {
                thumbnail_density: ThumbnailDensity::Compact,
            }))
            .unwrap(),
            serde_json::json!({
                "schemaVersion": 1,
                "thumbnailDensity": "compact"
            })
        );
    }

    #[test]
    fn thumbnail_density_accepts_only_the_public_values() {
        assert!(
            serde_json::from_value::<ThumbnailDensityDto>(serde_json::json!("thumbnail_density"))
                .is_err()
        );
        assert!(serde_json::from_value::<ThumbnailDensityDto>(serde_json::json!("dense")).is_err());
    }
}
