use crate::image_cache::ImageCacheKey;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Mutex,
};
use viewer_application::ImageBackend;
use viewer_domain::SessionId;

pub const SESSION_CACHE_LIMIT_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CachedImage {
    pub path: PathBuf,
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub backend: ImageBackend,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionCacheError {
    #[error("session cache I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("session cache root is not owned by Viewer")]
    UnsafeRoot,
    #[error("image artifact is outside the owned session cache")]
    ArtifactOutsideCache,
    #[error("image artifact is not a regular file")]
    ArtifactNotFile,
    #[error("image artifact exceeds the session cache budget")]
    ArtifactTooLarge,
}

struct CacheEntry {
    image: CachedImage,
    bytes: u64,
    last_access: u64,
}

#[derive(Default)]
struct ImageLru {
    entries: HashMap<ImageCacheKey, CacheEntry>,
    total_bytes: u64,
    access_epoch: u64,
}

pub struct SessionCache {
    base: PathBuf,
    root: PathBuf,
    session_id: SessionId,
    limit_bytes: u64,
    images: Mutex<ImageLru>,
}

impl SessionCache {
    pub fn create_in(base: &Path, session_id: SessionId) -> Result<Self, SessionCacheError> {
        Self::create_in_with_limit(base, session_id, SESSION_CACHE_LIMIT_BYTES)
    }

    pub fn create_in_with_limit(
        base: &Path,
        session_id: SessionId,
        limit_bytes: u64,
    ) -> Result<Self, SessionCacheError> {
        fs::create_dir_all(base)?;
        let base = fs::canonicalize(base)?;
        let root = base.join(session_id.to_string());
        fs::create_dir(&root)?;
        fs::create_dir(root.join("images"))?;
        fs::create_dir(root.join("review-artifacts"))?;
        Ok(Self {
            base,
            root,
            session_id,
            limit_bytes,
            images: Mutex::new(ImageLru::default()),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn index_path(&self) -> PathBuf {
        self.root.join("session.sqlite")
    }

    pub fn image_root(&self) -> PathBuf {
        self.root.join("images")
    }

    pub fn review_artifact_root(&self) -> PathBuf {
        self.root.join("review-artifacts")
    }

    pub fn lookup_image(&self, key: ImageCacheKey) -> Option<CachedImage> {
        let mut images = self.lock_images();
        let live = images
            .entries
            .get(&key)
            .is_some_and(|entry| entry.image.path.is_file());
        if !live {
            if let Some(removed) = images.entries.remove(&key) {
                images.total_bytes = images.total_bytes.saturating_sub(removed.bytes);
            }
            return None;
        }
        images.access_epoch = images.access_epoch.wrapping_add(1).max(1);
        let access_epoch = images.access_epoch;
        let entry = images.entries.get_mut(&key)?;
        entry.last_access = access_epoch;
        Some(entry.image.clone())
    }

    pub fn insert_image(
        &self,
        key: ImageCacheKey,
        mut image: CachedImage,
    ) -> Result<(), SessionCacheError> {
        let image_root = fs::canonicalize(self.image_root())?;
        let metadata = fs::symlink_metadata(&image.path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(SessionCacheError::ArtifactNotFile);
        }
        let canonical = fs::canonicalize(&image.path)?;
        if canonical == image_root || !canonical.starts_with(&image_root) {
            return Err(SessionCacheError::ArtifactOutsideCache);
        }
        image.path = canonical;
        let bytes = metadata.len();
        if bytes > self.limit_bytes {
            let _ = fs::remove_file(&image.path);
            return Err(SessionCacheError::ArtifactTooLarge);
        }

        let mut images = self.lock_images();
        if let Some(previous) = images.entries.remove(&key) {
            images.total_bytes = images.total_bytes.saturating_sub(previous.bytes);
            if previous.image.path != image.path {
                let _ = fs::remove_file(previous.image.path);
            }
        }
        images.access_epoch = images.access_epoch.wrapping_add(1).max(1);
        let access_epoch = images.access_epoch;
        images.total_bytes = images.total_bytes.saturating_add(bytes);
        images.entries.insert(
            key,
            CacheEntry {
                image,
                bytes,
                last_access: access_epoch,
            },
        );
        while images.total_bytes > self.limit_bytes {
            let Some(eviction_key) = images
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_access)
                .map(|(key, _)| *key)
            else {
                break;
            };
            if let Some(evicted) = images.entries.remove(&eviction_key) {
                images.total_bytes = images.total_bytes.saturating_sub(evicted.bytes);
                let _ = fs::remove_file(evicted.image.path);
            }
        }
        Ok(())
    }

    pub fn discard_owned_image_artifact(&self, artifact: &Path) -> Result<bool, SessionCacheError> {
        let image_root = fs::canonicalize(self.image_root())?;
        let metadata = match fs::symlink_metadata(artifact) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(SessionCacheError::ArtifactNotFile);
        }
        let canonical = fs::canonicalize(artifact)?;
        if canonical == image_root || !canonical.starts_with(&image_root) {
            return Err(SessionCacheError::ArtifactOutsideCache);
        }
        fs::remove_file(canonical)?;
        Ok(true)
    }

    pub fn cleanup(&self) -> Result<(), SessionCacheError> {
        if !self.root.exists() {
            return Ok(());
        }
        self.validate_owned_root()?;
        fs::remove_dir_all(&self.root)?;
        Ok(())
    }

    pub fn cleanup_stale(
        base: &Path,
        active: Option<SessionId>,
    ) -> Result<usize, SessionCacheError> {
        fs::create_dir_all(base)?;
        let base = fs::canonicalize(base)?;
        let mut removed = 0;
        for entry in fs::read_dir(&base)? {
            let entry = entry?;
            let metadata = fs::symlink_metadata(entry.path())?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(session_id) = SessionId::from_str(&name) else {
                continue;
            };
            if Some(session_id) == active {
                continue;
            }
            let canonical = fs::canonicalize(entry.path())?;
            if canonical.parent() != Some(base.as_path()) {
                continue;
            }
            fs::remove_dir_all(canonical)?;
            removed += 1;
        }
        Ok(removed)
    }

    fn validate_owned_root(&self) -> Result<(), SessionCacheError> {
        let metadata = fs::symlink_metadata(&self.root)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(SessionCacheError::UnsafeRoot);
        }
        let name = self
            .root
            .file_name()
            .and_then(|value| value.to_str())
            .and_then(|value| SessionId::from_str(value).ok());
        if name != Some(self.session_id) {
            return Err(SessionCacheError::UnsafeRoot);
        }
        let canonical = fs::canonicalize(&self.root)?;
        if canonical.parent() != Some(self.base.as_path()) {
            return Err(SessionCacheError::UnsafeRoot);
        }
        Ok(())
    }

    fn lock_images(&self) -> std::sync::MutexGuard<'_, ImageLru> {
        self.images
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::{CachedImage, SessionCache, SessionCacheError};
    use crate::image_cache::{ImageCacheKey, ImageCacheKeyInput};
    use std::{fs, str::FromStr};
    use viewer_application::ImageBackend;
    use viewer_domain::{ProjectId, RelativePath, SessionId, image::ImageRepresentationKind};

    fn fixed_session(value: u128) -> SessionId {
        SessionId::from_u128(value)
    }

    fn key(value: u64) -> ImageCacheKey {
        let path = RelativePath::parse(&format!("products/id-{value}/front.jpg")).unwrap();
        ImageCacheKey::from_request(&ImageCacheKeyInput {
            project_id: ProjectId::from_u128(1),
            relative_path: &path,
            source_size: value,
            source_mtime_ns: i128::from(value),
            kind: ImageRepresentationKind::Thumbnail {
                max_pixels: 256,
                scale_milli: 2_000,
            },
            renderer_version: 1,
        })
    }

    fn write_image(cache: &SessionCache, name: &str, size: usize) -> CachedImage {
        let path = cache.image_root().join(name);
        fs::write(&path, vec![0_u8; size]).unwrap();
        CachedImage {
            path,
            mime: "image/png".to_owned(),
            width: 64,
            height: 64,
            backend: ImageBackend::ImageIo,
        }
    }

    #[test]
    fn cleanup_removes_only_the_owned_session_directory() {
        let base = tempfile::tempdir().unwrap();
        let cache = SessionCache::create_in(base.path(), fixed_session(1)).unwrap();
        let sibling = base.path().join("keep-me");
        fs::create_dir(&sibling).unwrap();
        let owned = cache.root().to_owned();

        cache.cleanup().unwrap();

        assert!(!owned.exists());
        assert!(sibling.exists());
    }

    #[test]
    fn review_artifact_directory_is_session_scoped_and_removed_with_the_session() {
        let base = tempfile::tempdir().unwrap();
        let cache = SessionCache::create_in(base.path(), fixed_session(13)).unwrap();
        let artifact_root = cache.review_artifact_root();

        assert!(artifact_root.is_dir());
        assert_eq!(artifact_root.parent(), Some(cache.root()));
        cache.cleanup().unwrap();
        assert!(!artifact_root.exists());
    }

    #[test]
    fn cleanup_stale_ignores_non_session_directories_and_the_active_session() {
        let base = tempfile::tempdir().unwrap();
        let active_id = fixed_session(2);
        let active = SessionCache::create_in(base.path(), active_id).unwrap();
        let stale = SessionCache::create_in(base.path(), fixed_session(3)).unwrap();
        let unrelated = base.path().join("downloads");
        fs::create_dir(&unrelated).unwrap();

        let removed = SessionCache::cleanup_stale(base.path(), Some(active_id)).unwrap();

        assert_eq!(removed, 1);
        assert!(active.root().exists());
        assert!(!stale.root().exists());
        assert!(unrelated.exists());
    }

    #[test]
    fn representation_cache_reuses_a_live_artifact_and_evicts_lru_over_budget() {
        let base = tempfile::tempdir().unwrap();
        let cache = SessionCache::create_in_with_limit(base.path(), fixed_session(4), 10).unwrap();
        let first = write_image(&cache, "first.png", 6);
        let second = write_image(&cache, "second.png", 6);
        cache.insert_image(key(1), first.clone()).unwrap();
        assert_eq!(cache.lookup_image(key(1)), Some(first.clone()));

        cache.insert_image(key(2), second.clone()).unwrap();

        assert_eq!(cache.lookup_image(key(1)), None);
        assert_eq!(cache.lookup_image(key(2)), Some(second));
        assert!(!first.path.exists());
    }

    #[test]
    fn missing_artifact_is_removed_from_the_lookup() {
        let base = tempfile::tempdir().unwrap();
        let cache = SessionCache::create_in(base.path(), fixed_session(5)).unwrap();
        let image = write_image(&cache, "gone.png", 1);
        cache.insert_image(key(1), image.clone()).unwrap();
        fs::remove_file(&image.path).unwrap();

        assert_eq!(cache.lookup_image(key(1)), None);
        assert_eq!(cache.lookup_image(key(1)), None);
    }

    #[test]
    fn insert_rejects_an_artifact_outside_the_owned_image_directory() {
        let base = tempfile::tempdir().unwrap();
        let cache = SessionCache::create_in(base.path(), fixed_session(6)).unwrap();
        let outside = base.path().join("outside.png");
        fs::write(&outside, b"outside").unwrap();

        let result = cache.insert_image(
            key(1),
            CachedImage {
                path: outside,
                mime: "image/png".to_owned(),
                width: 1,
                height: 1,
                backend: ImageBackend::ImageIo,
            },
        );

        assert!(matches!(
            result,
            Err(SessionCacheError::ArtifactOutsideCache)
        ));
    }

    #[test]
    fn discard_removes_an_owned_unregistered_image_artifact() {
        let base = tempfile::tempdir().unwrap();
        let cache = SessionCache::create_in(base.path(), fixed_session(10)).unwrap();
        let image = write_image(&cache, "cancelled.png", 4);

        assert!(cache.discard_owned_image_artifact(&image.path).unwrap());
        assert!(!image.path.exists());
    }

    #[test]
    fn discard_rejects_an_external_artifact_without_deleting_it() {
        let base = tempfile::tempdir().unwrap();
        let cache = SessionCache::create_in(base.path(), fixed_session(11)).unwrap();
        let outside = base.path().join("external.png");
        fs::write(&outside, b"must survive").unwrap();

        let result = cache.discard_owned_image_artifact(&outside);

        assert!(matches!(
            result,
            Err(SessionCacheError::ArtifactOutsideCache)
        ));
        assert_eq!(fs::read(&outside).unwrap(), b"must survive");
    }

    #[cfg(unix)]
    #[test]
    fn discard_rejects_an_artifact_symlink_without_deleting_it_or_its_target() {
        let base = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let cache = SessionCache::create_in(base.path(), fixed_session(12)).unwrap();
        let target = outside.path().join("external.png");
        fs::write(&target, b"external target").unwrap();
        let artifact = cache.image_root().join("linked.png");
        std::os::unix::fs::symlink(&target, &artifact).unwrap();

        let result = cache.discard_owned_image_artifact(&artifact);

        assert!(matches!(result, Err(SessionCacheError::ArtifactNotFile)));
        assert!(
            fs::symlink_metadata(&artifact)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read(&target).unwrap(), b"external target");
    }

    #[test]
    fn insert_accepts_a_native_renderer_session_subdirectory() {
        let base = tempfile::tempdir().unwrap();
        let cache = SessionCache::create_in(base.path(), fixed_session(9)).unwrap();
        let native_session = cache.image_root().join(fixed_session(9).to_string());
        fs::create_dir(&native_session).unwrap();
        let path = native_session.join("thumbnail.png");
        fs::write(&path, b"png").unwrap();
        let image = CachedImage {
            path,
            mime: "image/png".to_owned(),
            width: 1,
            height: 1,
            backend: ImageBackend::ImageIo,
        };

        cache.insert_image(key(9), image.clone()).unwrap();

        assert_eq!(cache.lookup_image(key(9)), Some(image));
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_refuses_a_session_directory_replaced_by_a_symlink() {
        let base = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let session_id = fixed_session(7);
        let cache = SessionCache::create_in(base.path(), session_id).unwrap();
        fs::remove_dir_all(cache.root()).unwrap();
        std::os::unix::fs::symlink(outside.path(), cache.root()).unwrap();

        assert!(matches!(
            cache.cleanup(),
            Err(SessionCacheError::UnsafeRoot)
        ));
        assert!(outside.path().exists());
    }

    #[test]
    fn session_directory_names_remain_parseable_ids() {
        let base = tempfile::tempdir().unwrap();
        let cache = SessionCache::create_in(base.path(), fixed_session(8)).unwrap();
        let name = cache.root().file_name().unwrap().to_string_lossy();
        assert_eq!(SessionId::from_str(&name).unwrap(), fixed_session(8));
    }
}
