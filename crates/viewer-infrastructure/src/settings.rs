use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
    sync::Mutex,
};
use viewer_application::{
    ThumbnailDensity, VIEWER_SETTINGS_SCHEMA_VERSION, ViewerSettings, ViewerSettingsError,
    ViewerSettingsPort,
};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredViewerSettings {
    schema_version: u32,
    thumbnail_density: ThumbnailDensity,
}

pub struct JsonViewerSettingsStore {
    directory: PathBuf,
    write_lock: Mutex<()>,
}

impl JsonViewerSettingsStore {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            write_lock: Mutex::new(()),
        }
    }
}

impl ViewerSettingsPort for JsonViewerSettingsStore {
    fn load(&self) -> ViewerSettings {
        let settings = fs::read_to_string(self.directory.join("settings.json"))
            .ok()
            .and_then(|json| serde_json::from_str::<StoredViewerSettings>(&json).ok())
            .filter(|settings| settings.schema_version == VIEWER_SETTINGS_SCHEMA_VERSION);

        settings
            .map(|settings| ViewerSettings {
                thumbnail_density: settings.thumbnail_density,
            })
            .unwrap_or_default()
    }

    fn save(&self, settings: ViewerSettings) -> Result<(), ViewerSettingsError> {
        let _lock = self
            .write_lock
            .lock()
            .map_err(|_| ViewerSettingsError::Unavailable)?;
        fs::create_dir_all(&self.directory).map_err(|_| ViewerSettingsError::Unavailable)?;

        let serialized = serde_json::to_vec(&StoredViewerSettings {
            schema_version: VIEWER_SETTINGS_SCHEMA_VERSION,
            thumbnail_density: settings.thumbnail_density,
        })
        .map_err(|_| ViewerSettingsError::Unavailable)?;
        let temporary_path = self.directory.join("settings.json.tmp");
        let settings_path = self.directory.join("settings.json");
        let mut temporary =
            File::create(&temporary_path).map_err(|_| ViewerSettingsError::Unavailable)?;
        temporary
            .write_all(&serialized)
            .map_err(|_| ViewerSettingsError::Unavailable)?;
        temporary
            .sync_all()
            .map_err(|_| ViewerSettingsError::Unavailable)?;
        drop(temporary);
        fs::rename(temporary_path, settings_path).map_err(|_| ViewerSettingsError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::JsonViewerSettingsStore;
    use viewer_application::{
        ThumbnailDensity, ViewerSettings, ViewerSettingsError, ViewerSettingsPort,
    };

    #[test]
    fn missing_settings_use_standard() {
        let directory = tempfile::tempdir().unwrap();
        let store = JsonViewerSettingsStore::new(directory.path().to_path_buf());

        assert_eq!(store.load().thumbnail_density, ThumbnailDensity::Standard);
    }

    #[test]
    fn round_trip_uses_the_exact_versioned_shape() {
        let directory = tempfile::tempdir().unwrap();
        let store = JsonViewerSettingsStore::new(directory.path().to_path_buf());
        store
            .save(ViewerSettings {
                thumbnail_density: ThumbnailDensity::Large,
            })
            .unwrap();

        let json = std::fs::read_to_string(directory.path().join("settings.json")).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&json).unwrap(),
            serde_json::json!({
                "schemaVersion": 1,
                "thumbnailDensity": "large"
            })
        );
        assert_eq!(store.load().thumbnail_density, ThumbnailDensity::Large);
    }

    #[test]
    fn version_one_larger_thumbnail_densities_load_and_save_without_a_schema_change() {
        for public_value in ["extra_large", "maximum"] {
            let directory = tempfile::tempdir().unwrap();
            std::fs::write(
                directory.path().join("settings.json"),
                serde_json::json!({
                    "schemaVersion": 1,
                    "thumbnailDensity": public_value,
                })
                .to_string(),
            )
            .unwrap();
            let store = JsonViewerSettingsStore::new(directory.path().to_path_buf());

            let loaded = store.load();
            assert_eq!(
                serde_json::to_value(loaded.thumbnail_density).unwrap(),
                serde_json::json!(public_value)
            );
            store.save(loaded).unwrap();
            let saved = std::fs::read_to_string(directory.path().join("settings.json")).unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&saved).unwrap(),
                serde_json::json!({
                    "schemaVersion": 1,
                    "thumbnailDensity": public_value,
                })
            );
        }
    }

    #[test]
    fn existing_version_one_thumbnail_densities_still_load_unchanged() {
        for (public_value, expected) in [
            ("compact", ThumbnailDensity::Compact),
            ("standard", ThumbnailDensity::Standard),
            ("large", ThumbnailDensity::Large),
        ] {
            let directory = tempfile::tempdir().unwrap();
            std::fs::write(
                directory.path().join("settings.json"),
                serde_json::json!({
                    "schemaVersion": 1,
                    "thumbnailDensity": public_value,
                })
                .to_string(),
            )
            .unwrap();
            let store = JsonViewerSettingsStore::new(directory.path().to_path_buf());

            assert_eq!(store.load().thumbnail_density, expected);
        }
    }

    #[test]
    fn malformed_json_uses_standard() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("settings.json"), "not json").unwrap();
        let store = JsonViewerSettingsStore::new(directory.path().to_path_buf());

        assert_eq!(store.load().thumbnail_density, ThumbnailDensity::Standard);
    }

    #[test]
    fn settings_with_an_extra_field_use_standard() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("settings.json"),
            r#"{"schemaVersion":1,"thumbnailDensity":"large","unexpected":true}"#,
        )
        .unwrap();
        let store = JsonViewerSettingsStore::new(directory.path().to_path_buf());

        assert_eq!(store.load().thumbnail_density, ThumbnailDensity::Standard);
    }

    #[test]
    fn settings_with_an_invalid_density_use_standard() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("settings.json"),
            r#"{"schemaVersion":1,"thumbnailDensity":"huge"}"#,
        )
        .unwrap();
        let store = JsonViewerSettingsStore::new(directory.path().to_path_buf());

        assert_eq!(store.load().thumbnail_density, ThumbnailDensity::Standard);
    }

    #[test]
    fn settings_with_an_unsupported_schema_version_use_standard() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("settings.json"),
            r#"{"schemaVersion":2,"thumbnailDensity":"large"}"#,
        )
        .unwrap();
        let store = JsonViewerSettingsStore::new(directory.path().to_path_buf());

        assert_eq!(store.load().thumbnail_density, ThumbnailDensity::Standard);
    }

    #[test]
    fn saving_to_a_file_path_is_unavailable_and_leaves_a_sibling_project_unchanged() {
        let root = tempfile::tempdir().unwrap();
        let settings_path = root.path().join("settings-file");
        std::fs::write(&settings_path, "not a directory").unwrap();
        let project_directory = root.path().join("fake-project");
        std::fs::create_dir(&project_directory).unwrap();
        let sentinel = project_directory.join("sentinel");
        std::fs::write(&sentinel, "unchanged").unwrap();
        let store = JsonViewerSettingsStore::new(settings_path);

        assert_eq!(
            store.save(ViewerSettings {
                thumbnail_density: ThumbnailDensity::Large,
            }),
            Err(ViewerSettingsError::Unavailable)
        );
        assert_eq!(std::fs::read_to_string(sentinel).unwrap(), "unchanged");
    }
}
