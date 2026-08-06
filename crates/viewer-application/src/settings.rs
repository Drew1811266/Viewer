use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const VIEWER_SETTINGS_SCHEMA_VERSION: u32 = 1;

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ViewerSettings {
    pub thumbnail_density: ThumbnailDensity,
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

    pub fn update_thumbnail_density(
        &self,
        thumbnail_density: ThumbnailDensity,
    ) -> Result<ViewerSettings, ViewerSettingsError> {
        let settings = ViewerSettings { thumbnail_density };
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
    fn default_settings_use_standard_thumbnail_density() {
        assert_eq!(
            ViewerSettings::default().thumbnail_density,
            ThumbnailDensity::Standard
        );
    }

    #[test]
    fn updating_larger_thumbnail_densities_saves_one_complete_settings_value() {
        for thumbnail_density in [ThumbnailDensity::ExtraLarge, ThumbnailDensity::Maximum] {
            let store = Arc::new(MemorySettingsPort::default());
            let service = ViewerSettingsService::new(store.clone());

            let settings = service
                .update_thumbnail_density(thumbnail_density)
                .expect("settings update");

            assert_eq!(settings, ViewerSettings { thumbnail_density });
            assert_eq!(
                store.saved.lock().expect("settings lock").as_slice(),
                &[ViewerSettings { thumbnail_density }]
            );
        }
    }

    #[test]
    fn updating_thumbnail_density_returns_unavailable_when_save_fails() {
        let store = Arc::new(MemorySettingsPort::default());
        store.fail.store(true, Ordering::Release);
        let service = ViewerSettingsService::new(store);

        assert_eq!(
            service.update_thumbnail_density(ThumbnailDensity::Maximum),
            Err(ViewerSettingsError::Unavailable)
        );
    }
}
