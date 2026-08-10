use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const VIEWER_SETTINGS_SCHEMA_VERSION: u32 = 4;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThumbnailDensity {
    Compact,
    #[default]
    Standard,
    Large,
    ExtraLarge,
    Maximum,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MagnifierShape {
    #[default]
    Circle,
    RoundedRectangle,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MagnifierMagnification {
    #[default]
    Two,
    Three,
    Four,
}

impl TryFrom<f64> for MagnifierMagnification {
    type Error = ();

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        match value {
            2.0 => Ok(Self::Two),
            3.0 => Ok(Self::Three),
            4.0 => Ok(Self::Four),
            _ => Err(()),
        }
    }
}

impl From<MagnifierMagnification> for f64 {
    fn from(value: MagnifierMagnification) -> Self {
        match value {
            MagnifierMagnification::Two => 2.0,
            MagnifierMagnification::Three => 3.0,
            MagnifierMagnification::Four => 4.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MagnifierArea {
    #[default]
    Small,
    Medium,
    Large,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MagnifierPreferences {
    pub shape: MagnifierShape,
    pub magnification: MagnifierMagnification,
    pub area: MagnifierArea,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ViewerSettings {
    pub thumbnail_density: ThumbnailDensity,
    pub magnifier: MagnifierPreferences,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ViewerSettingsError {
    #[error("viewer settings are unavailable")]
    Unavailable,
}

pub trait ViewerSettingsPort: Send + Sync {
    fn load(&self) -> ViewerSettings;
    fn save(&self, settings: ViewerSettings) -> Result<(), ViewerSettingsError>;
}

pub struct ViewerSettingsService {
    store: Arc<dyn ViewerSettingsPort>,
}

impl ViewerSettingsService {
    pub fn new(store: Arc<dyn ViewerSettingsPort>) -> Self {
        Self { store }
    }

    pub fn load(&self) -> ViewerSettings {
        self.store.load()
    }

    pub fn update(&self, settings: ViewerSettings) -> Result<ViewerSettings, ViewerSettingsError> {
        self.store.save(settings)?;
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };

    #[derive(Default)]
    struct MemorySettingsPort {
        saved: Mutex<Vec<ViewerSettings>>,
        fail: AtomicBool,
    }

    impl ViewerSettingsPort for MemorySettingsPort {
        fn load(&self) -> ViewerSettings {
            self.saved
                .lock()
                .expect("settings lock")
                .last()
                .copied()
                .unwrap_or_default()
        }

        fn save(&self, settings: ViewerSettings) -> Result<(), ViewerSettingsError> {
            if self.fail.load(Ordering::Acquire) {
                return Err(ViewerSettingsError::Unavailable);
            }
            self.saved.lock().expect("settings lock").push(settings);
            Ok(())
        }
    }

    #[test]
    fn default_settings_use_schema_four_and_small_circle_two_x_magnifier() {
        assert_eq!(VIEWER_SETTINGS_SCHEMA_VERSION, 4);
        assert_eq!(
            ViewerSettings::default(),
            ViewerSettings {
                thumbnail_density: ThumbnailDensity::Standard,
                magnifier: MagnifierPreferences {
                    shape: MagnifierShape::Circle,
                    magnification: MagnifierMagnification::Two,
                    area: MagnifierArea::Small,
                },
            }
        );
    }

    #[test]
    fn magnification_accepts_only_the_three_public_values() {
        for (public_value, expected) in [
            (2.0, MagnifierMagnification::Two),
            (3.0, MagnifierMagnification::Three),
            (4.0, MagnifierMagnification::Four),
        ] {
            let parsed = MagnifierMagnification::try_from(public_value).expect("public value");
            assert_eq!(parsed, expected);
            assert_eq!(f64::from(parsed), public_value);
        }
        for rejected in [0.0, 1.5, 2.5, 5.0, f64::INFINITY, f64::NAN] {
            assert!(MagnifierMagnification::try_from(rejected).is_err());
        }
    }

    #[test]
    fn updating_settings_saves_one_complete_value() {
        let store = Arc::new(MemorySettingsPort::default());
        let service = ViewerSettingsService::new(store.clone());
        let expected = ViewerSettings {
            thumbnail_density: ThumbnailDensity::Maximum,
            magnifier: MagnifierPreferences {
                shape: MagnifierShape::RoundedRectangle,
                magnification: MagnifierMagnification::Three,
                area: MagnifierArea::Large,
            },
        };

        let saved = service.update(expected).expect("settings update");

        assert_eq!(saved, expected);
        assert_eq!(
            store.saved.lock().expect("settings lock").as_slice(),
            &[expected]
        );
    }

    #[test]
    fn updating_settings_returns_unavailable_without_saving_when_store_fails() {
        let store = Arc::new(MemorySettingsPort::default());
        store.fail.store(true, Ordering::Release);
        let service = ViewerSettingsService::new(store.clone());

        assert_eq!(
            service.update(ViewerSettings::default()),
            Err(ViewerSettingsError::Unavailable)
        );
        assert!(store.saved.lock().expect("settings lock").is_empty());
    }
}
