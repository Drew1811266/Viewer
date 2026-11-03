use sha2::{Digest, Sha256};
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
    refs_root: PathBuf,
}

impl AssetStore {
    pub fn open(project_root: &Path) -> Result<Self, AssetError> {
        let refs_root = project_root.join(".viewer").join("refs");
        std::fs::create_dir_all(&refs_root).map_err(|error| AssetError::Io(error.to_string()))?;
        Ok(Self { refs_root })
    }

    pub fn import_bytes(&self, bytes: &[u8], extension: &str) -> Result<ImportedAsset, AssetError> {
        let extension = extension.trim_start_matches('.').to_ascii_lowercase();
        if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
            return Err(AssetError::UnsupportedFormat);
        }
        if bytes.is_empty() {
            return Err(AssetError::UnsupportedFormat);
        }
        if bytes.len() as u64 > MAX_ASSET_BYTES {
            return Err(AssetError::TooLarge);
        }
        let digest = Sha256::digest(bytes);
        let mut id_bytes = [0u8; 32];
        id_bytes.copy_from_slice(&digest);
        let asset_id = AssetId::from_bytes(id_bytes);
        let path = self.path_for(&asset_id, &extension);
        let deduped = path.exists();
        if !deduped {
            let temporary = path.with_extension(format!("{extension}.tmp"));
            std::fs::write(&temporary, bytes).map_err(|error| AssetError::Io(error.to_string()))?;
            std::fs::rename(&temporary, &path)
                .map_err(|error| AssetError::Io(error.to_string()))?;
        }
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
        if metadata.len() > MAX_ASSET_BYTES {
            return Err(AssetError::TooLarge);
        }
        let bytes = std::fs::read(source).map_err(|error| AssetError::Io(error.to_string()))?;
        self.import_bytes(&bytes, extension)
    }

    pub fn contains(&self, asset_id: &AssetId) -> bool {
        let hex = asset_id.as_hex();
        SUPPORTED_EXTENSIONS
            .iter()
            .any(|extension| self.refs_root.join(format!("{hex}.{extension}")).is_file())
    }

    pub fn path_for(&self, asset_id: &AssetId, extension: &str) -> PathBuf {
        self.refs_root
            .join(format!("{}.{extension}", asset_id.as_hex()))
    }
}

impl AssetIndexView for AssetStore {
    fn contains(&self, asset_id: &AssetId) -> bool {
        AssetStore::contains(self, asset_id)
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

        let entries: Vec<_> = std::fs::read_dir(dir.path().join(".viewer/refs"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(entries.len(), 1, "{entries:?}");
    }

    #[test]
    fn rejects_unsupported_extensions_and_empty_payloads() {
        let (_dir, store) = store();
        assert!(matches!(
            store.import_bytes(PNG, "exe"),
            Err(AssetError::UnsupportedFormat)
        ));
        assert!(matches!(
            store.import_bytes(b"", "png"),
            Err(AssetError::UnsupportedFormat)
        ));
    }

    #[test]
    fn import_path_uses_the_source_extension() {
        let (dir, store) = store();
        let source = dir.path().join("hero.PNG");
        std::fs::write(&source, PNG).unwrap();

        let imported = store.import_path(&source).unwrap();
        assert!(
            imported
                .path
                .ends_with(format!("{}.png", imported.asset_id.as_hex()))
        );
        assert!(store.contains(&imported.asset_id));
    }

    #[test]
    fn rejects_oversized_payloads() {
        let (_dir, store) = store();
        let oversized = vec![0u8; (MAX_ASSET_BYTES + 1) as usize];
        assert!(matches!(
            store.import_bytes(&oversized, "png"),
            Err(AssetError::TooLarge)
        ));
    }
}
