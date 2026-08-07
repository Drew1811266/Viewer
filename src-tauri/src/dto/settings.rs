use serde::{Deserialize, Serialize};
use viewer_application::{
    MagnifierArea, MagnifierMagnification, MagnifierPreferences, MagnifierShape, ThumbnailDensity,
    VIEWER_SETTINGS_SCHEMA_VERSION, ViewerSettings,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThumbnailDensityDto {
    Compact,
    Standard,
    Large,
    ExtraLarge,
    Maximum,
}

impl From<ThumbnailDensity> for ThumbnailDensityDto {
    fn from(value: ThumbnailDensity) -> Self {
        match value {
            ThumbnailDensity::Compact => Self::Compact,
            ThumbnailDensity::Standard => Self::Standard,
            ThumbnailDensity::Large => Self::Large,
            ThumbnailDensity::ExtraLarge => Self::ExtraLarge,
            ThumbnailDensity::Maximum => Self::Maximum,
        }
    }
}

impl From<ThumbnailDensityDto> for ThumbnailDensity {
    fn from(value: ThumbnailDensityDto) -> Self {
        match value {
            ThumbnailDensityDto::Compact => Self::Compact,
            ThumbnailDensityDto::Standard => Self::Standard,
            ThumbnailDensityDto::Large => Self::Large,
            ThumbnailDensityDto::ExtraLarge => Self::ExtraLarge,
            ThumbnailDensityDto::Maximum => Self::Maximum,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MagnifierShapeDto {
    Circle,
    RoundedRectangle,
}

impl From<MagnifierShape> for MagnifierShapeDto {
    fn from(value: MagnifierShape) -> Self {
        match value {
            MagnifierShape::Circle => Self::Circle,
            MagnifierShape::RoundedRectangle => Self::RoundedRectangle,
        }
    }
}

impl From<MagnifierShapeDto> for MagnifierShape {
    fn from(value: MagnifierShapeDto) -> Self {
        match value {
            MagnifierShapeDto::Circle => Self::Circle,
            MagnifierShapeDto::RoundedRectangle => Self::RoundedRectangle,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MagnifierAreaDto {
    Small,
    Medium,
    Large,
}

impl From<MagnifierArea> for MagnifierAreaDto {
    fn from(value: MagnifierArea) -> Self {
        match value {
            MagnifierArea::Small => Self::Small,
            MagnifierArea::Medium => Self::Medium,
            MagnifierArea::Large => Self::Large,
        }
    }
}

impl From<MagnifierAreaDto> for MagnifierArea {
    fn from(value: MagnifierAreaDto) -> Self {
        match value {
            MagnifierAreaDto::Small => Self::Small,
            MagnifierAreaDto::Medium => Self::Medium,
            MagnifierAreaDto::Large => Self::Large,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MagnifierPreferencesDto {
    pub shape: MagnifierShapeDto,
    pub magnification: f64,
    pub area: MagnifierAreaDto,
}

impl From<MagnifierPreferences> for MagnifierPreferencesDto {
    fn from(value: MagnifierPreferences) -> Self {
        Self {
            shape: value.shape.into(),
            magnification: f64::from(value.magnification),
            area: value.area.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerSettingsDto {
    pub schema_version: u32,
    pub thumbnail_density: ThumbnailDensityDto,
    pub magnifier: MagnifierPreferencesDto,
}

impl From<ViewerSettings> for ViewerSettingsDto {
    fn from(value: ViewerSettings) -> Self {
        Self {
            schema_version: VIEWER_SETTINGS_SCHEMA_VERSION,
            thumbnail_density: value.thumbnail_density.into(),
            magnifier: value.magnifier.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MagnifierPreferencesUpdateDto {
    pub shape: MagnifierShapeDto,
    pub magnification: f64,
    pub area: MagnifierAreaDto,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ViewerSettingsUpdateDto {
    pub thumbnail_density: ThumbnailDensityDto,
    pub magnifier: MagnifierPreferencesUpdateDto,
}

impl TryFrom<ViewerSettingsUpdateDto> for ViewerSettings {
    type Error = ();

    fn try_from(value: ViewerSettingsUpdateDto) -> Result<Self, Self::Error> {
        Ok(Self {
            thumbnail_density: value.thumbnail_density.into(),
            magnifier: MagnifierPreferences {
                shape: value.magnifier.shape.into(),
                magnification: MagnifierMagnification::try_from(value.magnifier.magnification)?,
                area: value.magnifier.area.into(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{ThumbnailDensityDto, ViewerSettingsDto, ViewerSettingsUpdateDto};
    use viewer_application::{
        MagnifierArea, MagnifierMagnification, MagnifierPreferences, MagnifierShape,
        ThumbnailDensity, ViewerSettings,
    };

    #[test]
    fn viewer_settings_serialize_to_the_frozen_camel_case_shape() {
        assert_eq!(
            serde_json::to_value(ViewerSettingsDto::from(ViewerSettings {
                thumbnail_density: ThumbnailDensity::Compact,
                magnifier: MagnifierPreferences {
                    shape: MagnifierShape::RoundedRectangle,
                    magnification: MagnifierMagnification::OnePointFive,
                    area: MagnifierArea::Medium,
                },
            }))
            .unwrap(),
            serde_json::json!({
                "schemaVersion": 3,
                "thumbnailDensity": "compact",
                "magnifier": {
                    "shape": "rounded_rectangle",
                    "magnification": 1.5,
                    "area": "medium"
                }
            })
        );
    }

    #[test]
    fn complete_settings_input_converts_to_the_bounded_domain_value() {
        let input = serde_json::from_value::<ViewerSettingsUpdateDto>(serde_json::json!({
            "thumbnailDensity": "maximum",
            "magnifier": {
                "shape": "rounded_rectangle",
                "magnification": 3,
                "area": "large"
            }
        }))
        .unwrap();

        assert_eq!(
            ViewerSettings::try_from(input),
            Ok(ViewerSettings {
                thumbnail_density: ThumbnailDensity::Maximum,
                magnifier: MagnifierPreferences {
                    shape: MagnifierShape::RoundedRectangle,
                    magnification: MagnifierMagnification::Three,
                    area: MagnifierArea::Large,
                },
            })
        );
    }

    #[test]
    fn complete_settings_input_rejects_invalid_magnification_and_unknown_fields() {
        for magnification in [0.0, 1.0, 1.4, 2.5, 4.0, 6.0] {
            let input = serde_json::from_value::<ViewerSettingsUpdateDto>(serde_json::json!({
                "thumbnailDensity": "standard",
                "magnifier": {
                    "shape": "circle",
                    "magnification": magnification,
                    "area": "small"
                }
            }))
            .unwrap();
            assert_eq!(ViewerSettings::try_from(input), Err(()));
        }

        for invalid in [
            serde_json::json!({
                "thumbnailDensity": "standard",
                "magnifier": { "shape": "square", "magnification": 1.5, "area": "small" }
            }),
            serde_json::json!({
                "thumbnailDensity": "standard",
                "magnifier": { "shape": "circle", "magnification": 1.5, "area": "huge" }
            }),
            serde_json::json!({
                "thumbnailDensity": "standard",
                "magnifier": {
                    "shape": "circle",
                    "magnification": 1.5,
                    "area": "small",
                    "unexpected": true
                }
            }),
            serde_json::json!({
                "thumbnailDensity": "standard",
                "magnifier": { "shape": "circle", "magnification": "1.5", "area": "small" }
            }),
        ] {
            assert!(serde_json::from_value::<ViewerSettingsUpdateDto>(invalid).is_err());
        }
    }

    #[test]
    fn thumbnail_density_accepts_only_the_public_values() {
        for public_value in ["compact", "standard", "large", "extra_large", "maximum"] {
            let density =
                serde_json::from_value::<ThumbnailDensityDto>(serde_json::json!(public_value))
                    .unwrap();
            assert_eq!(
                serde_json::to_value(density).unwrap(),
                serde_json::json!(public_value)
            );
        }
        assert!(
            serde_json::from_value::<ThumbnailDensityDto>(serde_json::json!("thumbnail_density"))
                .is_err()
        );
        assert!(serde_json::from_value::<ThumbnailDensityDto>(serde_json::json!("dense")).is_err());
    }
}
