use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use viewer_application::plan::AssetIndexView;
use viewer_domain::plan::AssetId;

pub const MAX_ASSET_BYTES: u64 = 32 * 1024 * 1024;

const SUPPORTED_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "tif", "tiff", "gif", "bmp", "heic", "heif", "avif",
];

#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    #[error("reference asset format is not supported")]
    UnsupportedFormat,
    #[error("reference asset exceeds the supported size")]
    TooLarge,
    #[error("reference asset source is not a regular file")]
    NotRegularFile,
    #[error("project root cannot be used for the reference store")]
    RootUnavailable,
    #[error("reference store io failed: {0}")]
    Io(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedAsset {
    pub asset_id: AssetId,
    pub path: PathBuf,
    pub deduped: bool,
}

pub struct AssetStore {
    project_root: PathBuf,
    refs_root: PathBuf,
}

impl AssetStore {
    /// Opens the store without creating anything on disk: a pure validation must not mutate the project.
    pub fn open(project_root: &Path) -> Result<Self, AssetError> {
        let project_root =
            std::fs::canonicalize(project_root).map_err(|_| AssetError::RootUnavailable)?;
        if !project_root.is_dir() {
            return Err(AssetError::RootUnavailable);
        }
        let refs_root = project_root.join(".viewer").join("refs");
        Ok(Self {
            project_root,
            refs_root,
        })
    }

    pub fn import_bytes(&self, bytes: &[u8], extension: &str) -> Result<ImportedAsset, AssetError> {
        let extension = normalize_extension(extension)?;
        if !matches_signature(bytes, extension) {
            return Err(AssetError::UnsupportedFormat);
        }
        if bytes.len() as u64 > MAX_ASSET_BYTES {
            return Err(AssetError::TooLarge);
        }

        let digest = Sha256::digest(bytes);
        let mut id_bytes = [0u8; 32];
        id_bytes.copy_from_slice(&digest);
        let asset_id = AssetId::from_bytes(id_bytes);
        let path = self.path_for(&asset_id);
        let deduped = path
            .metadata()
            .map(|metadata| metadata.is_file() && metadata.len() == bytes.len() as u64)
            .unwrap_or(false);

        if !deduped {
            self.ensure_refs_root()?;
            let temporary = self.refs_root.join(format!("{}.tmp", asset_id.as_hex()));
            std::fs::write(&temporary, bytes).map_err(|error| AssetError::Io(error.to_string()))?;
            std::fs::rename(&temporary, &path)
                .map_err(|error| AssetError::Io(error.to_string()))?;
        }
        self.record_index(asset_id, extension, bytes.len() as u64)?;

        Ok(ImportedAsset {
            asset_id,
            path,
            deduped,
        })
    }

    pub fn import_path(&self, source: &Path) -> Result<ImportedAsset, AssetError> {
        let extension = source
            .extension()
            .and_then(|value| value.to_str())
            .ok_or(AssetError::UnsupportedFormat)?;
        let metadata =
            std::fs::metadata(source).map_err(|error| AssetError::Io(error.to_string()))?;
        if !metadata.is_file() {
            return Err(AssetError::NotRegularFile);
        }
        if metadata.len() > MAX_ASSET_BYTES {
            return Err(AssetError::TooLarge);
        }
        let file =
            std::fs::File::open(source).map_err(|error| AssetError::Io(error.to_string()))?;
        let mut bytes = Vec::new();
        file.take(MAX_ASSET_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| AssetError::Io(error.to_string()))?;
        self.import_bytes(&bytes, extension)
    }

    pub fn contains(&self, asset_id: &AssetId) -> bool {
        self.path_for(asset_id).is_file()
    }

    pub fn path_for(&self, asset_id: &AssetId) -> PathBuf {
        self.refs_root.join(asset_id.as_hex())
    }

    fn ensure_refs_root(&self) -> Result<(), AssetError> {
        let viewer = self.project_root.join(".viewer");
        std::fs::create_dir_all(&viewer).map_err(|error| AssetError::Io(error.to_string()))?;
        let viewer =
            std::fs::canonicalize(&viewer).map_err(|error| AssetError::Io(error.to_string()))?;
        if !viewer.starts_with(&self.project_root) {
            return Err(AssetError::RootUnavailable);
        }
        std::fs::create_dir_all(viewer.join("refs"))
            .map_err(|error| AssetError::Io(error.to_string()))?;
        Ok(())
    }

    fn record_index(
        &self,
        asset_id: AssetId,
        extension: &str,
        bytes: u64,
    ) -> Result<(), AssetError> {
        let path = self.refs_root.join("index.json");
        let mut index: serde_json::Map<String, serde_json::Value> = std::fs::read(&path)
            .ok()
            .and_then(|raw| serde_json::from_slice::<serde_json::Value>(&raw).ok())
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        index.insert(
            asset_id.as_hex(),
            serde_json::json!({ "ext": extension, "bytes": bytes }),
        );
        let bytes = serde_json::to_vec_pretty(&serde_json::Value::Object(index))
            .map_err(|error| AssetError::Io(error.to_string()))?;
        let temporary = self.refs_root.join("index.json.tmp");
        std::fs::write(&temporary, bytes).map_err(|error| AssetError::Io(error.to_string()))?;
        std::fs::rename(&temporary, path).map_err(|error| AssetError::Io(error.to_string()))?;
        Ok(())
    }
}

impl AssetIndexView for AssetStore {
    fn contains(&self, asset_id: &AssetId) -> bool {
        AssetStore::contains(self, asset_id)
    }
}

fn normalize_extension(extension: &str) -> Result<&'static str, AssetError> {
    let lowered = extension.trim_start_matches('.').to_ascii_lowercase();
    SUPPORTED_EXTENSIONS
        .iter()
        .find(|candidate| **candidate == lowered)
        .copied()
        .ok_or(AssetError::UnsupportedFormat)
}

fn matches_signature(bytes: &[u8], extension: &str) -> bool {
    match extension {
        "png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "jpg" | "jpeg" => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        "gif" => bytes.starts_with(b"GIF8"),
        "bmp" => bytes.starts_with(b"BM"),
        "tif" | "tiff" => bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*"),
        "webp" => bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        "heic" | "heif" | "avif" => bytes.len() > 12 && &bytes[4..8] == b"ftyp",
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake-image-bytes";

    fn store() -> (tempfile::TempDir, AssetStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = AssetStore::open(dir.path()).unwrap();
        (dir, store)
    }

    #[test]
    fn same_bytes_under_different_names_collapse_to_one_asset() {
        let (dir, store) = store();
        let first = store.import_bytes(PNG, "png").unwrap();
        let second = store.import_bytes(PNG, "png").unwrap();

        assert_eq!(first.asset_id, second.asset_id);
        assert!(!first.deduped);
        assert!(second.deduped);

        let files: Vec<_> = std::fs::read_dir(dir.path().join(".viewer/refs"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name != "index.json")
            .collect();
        assert_eq!(files.len(), 1, "{files:?}");
    }

    #[test]
    fn rejects_unsupported_extensions_and_non_images() {
        let (_dir, store) = store();
        assert!(matches!(
            store.import_bytes(PNG, "exe"),
            Err(AssetError::UnsupportedFormat)
        ));
        assert!(matches!(
            store.import_bytes(b"<html>not an image</html>", "png"),
            Err(AssetError::UnsupportedFormat)
        ));
    }

    #[test]
    fn open_does_not_create_the_reference_directory() {
        let dir = tempfile::tempdir().unwrap();
        let store = AssetStore::open(dir.path()).unwrap();
        assert!(!store.contains(&AssetId::from_bytes([0; 32])));
        assert!(!dir.path().join(".viewer/refs").exists());
    }

    #[test]
    fn import_path_uses_the_source_extension() {
        let (dir, store) = store();
        let source = dir.path().join("hero.PNG");
        std::fs::write(&source, PNG).unwrap();

        let imported = store.import_path(&source).unwrap();
        assert!(imported.path.ends_with(imported.asset_id.as_hex()));
        assert!(store.contains(&imported.asset_id));
    }

    #[test]
    fn rejects_oversized_payloads() {
        let (_dir, store) = store();
        let mut oversized = b"\x89PNG\r\n\x1a\n".to_vec();
        oversized.resize((MAX_ASSET_BYTES + 1) as usize, 0);
        assert!(matches!(
            store.import_bytes(&oversized, "png"),
            Err(AssetError::TooLarge)
        ));
    }

    #[test]
    fn rejects_non_regular_files() {
        let (dir, store) = store();
        let source = dir.path().join("pipe.png");
        let status = std::process::Command::new("mkfifo")
            .arg(&source)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(matches!(
            store.import_path(&source),
            Err(AssetError::NotRegularFile)
        ));
    }
}
