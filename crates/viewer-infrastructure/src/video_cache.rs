use std::{
    collections::HashMap,
    ffi::{CStr, CString, OsStr, OsString},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::{OsStrExt, OsStringExt},
            fs::MetadataExt,
        },
    },
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, OnceLock, Weak},
    time::SystemTime,
};

const CACHE_KEY_SCHEMA: &[u8] = b"viewer-video-cache-key\0v1";
const CACHE_DIRECTORY_NAME: &str = "video";
const CACHE_MARKER_NAME: &str = ".viewer-video-cache-v1";
const CACHE_MARKER_CONTENTS: &[u8] = b"Viewer owned video cache\n";
const ARTIFACT_NAME: &str = "artifact.png";
const ACCESS_STAMP_NAME: &str = "access";
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

type OperationLock = Mutex<()>;
static ROOT_OPERATION_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Weak<OperationLock>>>> =
    OnceLock::new();

pub const VIDEO_CACHE_BUDGET_BYTES: u64 = 1_073_741_824;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VideoSourceIdentity {
    canonical_path: PathBuf,
    size: u64,
    modified_ns: i128,
}

impl VideoSourceIdentity {
    pub fn new(canonical_path: &Path, size: u64, modified_ns: i128) -> Self {
        Self {
            canonical_path: canonical_path.to_path_buf(),
            size,
            modified_ns,
        }
    }

    pub fn canonical_path(&self) -> &Path {
        &self.canonical_path
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub const fn modified_ns(&self) -> i128 {
        self.modified_ns
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct VideoCacheKey([u8; 32]);

impl VideoCacheKey {
    pub fn cover(source: VideoSourceIdentity, algorithm_version: u16) -> Self {
        Self::from_parts(&source, 0, None, algorithm_version)
    }

    pub fn timeline(source: VideoSourceIdentity, bucket_us: u64, algorithm_version: u16) -> Self {
        Self::from_parts(&source, 1, Some(bucket_us), algorithm_version)
    }

    fn from_parts(
        source: &VideoSourceIdentity,
        kind: u8,
        bucket_us: Option<u64>,
        algorithm_version: u16,
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(CACHE_KEY_SCHEMA);
        let path = source.canonical_path.as_os_str().as_encoded_bytes();
        hasher.update(&(path.len() as u64).to_le_bytes());
        hasher.update(path);
        hasher.update(&source.size.to_le_bytes());
        hasher.update(&source.modified_ns.to_le_bytes());
        hasher.update(&[kind]);
        if let Some(bucket_us) = bucket_us {
            hasher.update(&bucket_us.to_le_bytes());
        }
        hasher.update(&algorithm_version.to_le_bytes());
        Self(*hasher.finalize().as_bytes())
    }

    pub fn to_hex(self) -> String {
        blake3::Hash::from_bytes(self.0).to_hex().to_string()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VideoCacheStats {
    pub bytes_used: u64,
    pub entry_count: u64,
    pub budget_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CacheError {
    #[error("video cache I/O failed")]
    Io,
    #[error("video cache root is not verified")]
    UnverifiedRoot,
    #[error("video cache entry is unsafe")]
    UnsafeEntry,
    #[error("video cache artifact is not a PNG")]
    InvalidPng,
    #[error("video cache artifact exceeds the fixed budget")]
    ArtifactTooLarge,
}

#[derive(Debug)]
pub enum VideoCacheCommitError<E> {
    Cache(CacheError),
    Rejected(E),
}

#[derive(Debug)]
pub struct VideoCacheStagedPng {
    root_directory: Arc<File>,
    root_identity: DirectoryIdentity,
    entry_directory: File,
    entry_name: String,
    temporary_name: Option<OsString>,
    artifact_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct VerifiedVideoPng {
    path: PathBuf,
    bytes: Arc<[u8]>,
}

impl VerifiedVideoPng {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn bytes(&self) -> Arc<[u8]> {
        Arc::clone(&self.bytes)
    }
}

impl Drop for VideoCacheStagedPng {
    fn drop(&mut self) {
        let Some(temporary_name) = self.temporary_name.take() else {
            return;
        };
        let _ = unlink_file_at(&self.entry_directory, &temporary_name);
        if directory_at_matches(
            &self.root_directory,
            OsStr::new(&self.entry_name),
            &self.entry_directory,
        )
        .unwrap_or(false)
        {
            let _ = unlink_directory_at(&self.root_directory, OsStr::new(&self.entry_name));
        }
    }
}

#[derive(Clone, Debug)]
pub struct VideoCache {
    root: PathBuf,
    root_directory: Arc<File>,
    root_identity: DirectoryIdentity,
    operations: Arc<OperationLock>,
}

impl VideoCache {
    pub fn initialize(app_cache_root: &Path) -> Result<Self, CacheError> {
        if !app_cache_root.is_absolute() {
            return Err(CacheError::UnverifiedRoot);
        }
        fs::create_dir_all(app_cache_root).map_err(|_| CacheError::Io)?;
        let base_metadata = fs::symlink_metadata(app_cache_root).map_err(|_| CacheError::Io)?;
        if base_metadata.file_type().is_symlink() || !base_metadata.is_dir() {
            return Err(CacheError::UnverifiedRoot);
        }
        let base = fs::canonicalize(app_cache_root).map_err(|_| CacheError::Io)?;
        let base_directory = open_directory(&base).map_err(|_| CacheError::UnverifiedRoot)?;
        let root = base.join(CACHE_DIRECTORY_NAME);
        match open_directory_at(&base_directory, OsStr::new(CACHE_DIRECTORY_NAME)) {
            Ok(_) => {}
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => {
                mkdir_at(&base_directory, OsStr::new(CACHE_DIRECTORY_NAME))?;
            }
            Err(_) => return Err(CacheError::UnverifiedRoot),
        }
        let root_directory = open_directory_at(&base_directory, OsStr::new(CACHE_DIRECTORY_NAME))
            .map_err(|_| CacheError::UnverifiedRoot)?;
        match open_regular_at(
            &root_directory,
            OsStr::new(CACHE_MARKER_NAME),
            libc::O_RDONLY,
            0,
        ) {
            Ok(_) => {}
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => {
                let mut file = open_regular_at(
                    &root_directory,
                    OsStr::new(CACHE_MARKER_NAME),
                    libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
                    0o600,
                )
                .map_err(|_| CacheError::Io)?;
                file.write_all(CACHE_MARKER_CONTENTS)
                    .and_then(|()| file.sync_all())
                    .map_err(|_| CacheError::Io)?;
            }
            Err(_) => return Err(CacheError::Io),
        }
        Self::open(&root)
    }

    pub fn open(root: &Path) -> Result<Self, CacheError> {
        if !root.is_absolute() {
            return Err(CacheError::UnverifiedRoot);
        }
        let root_metadata = fs::symlink_metadata(root).map_err(|_| CacheError::UnverifiedRoot)?;
        if root_metadata.file_type().is_symlink()
            || !root_metadata.is_dir()
            || root.file_name().and_then(|name| name.to_str()) != Some(CACHE_DIRECTORY_NAME)
        {
            return Err(CacheError::UnverifiedRoot);
        }
        let root = fs::canonicalize(root).map_err(|_| CacheError::UnverifiedRoot)?;
        let root_directory = open_directory(&root).map_err(|_| CacheError::UnverifiedRoot)?;
        let root_identity = DirectoryIdentity::of(&root_directory)?;
        let cache = Self {
            operations: shared_operation_lock(&root),
            root,
            root_directory: Arc::new(root_directory),
            root_identity,
        };
        cache.verify_root()?;
        Ok(cache)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn put_atomic(&self, key: &VideoCacheKey, png: &[u8]) -> Result<PathBuf, CacheError> {
        self.put_atomic_inner(key, png, |_| {})
    }

    pub async fn get_async(&self, key: VideoCacheKey) -> Result<Option<PathBuf>, CacheError> {
        let cache = self.clone();
        tokio::task::spawn_blocking(move || cache.get(&key))
            .await
            .map_err(|_| CacheError::Io)?
    }

    pub async fn stage_atomic_async(
        &self,
        key: VideoCacheKey,
        png: Vec<u8>,
    ) -> Result<VideoCacheStagedPng, CacheError> {
        let cache = self.clone();
        tokio::task::spawn_blocking(move || cache.stage_atomic(&key, &png))
            .await
            .map_err(|_| CacheError::Io)?
    }

    pub fn commit_staged_if<E>(
        &self,
        mut staged: VideoCacheStagedPng,
        validate: impl FnOnce() -> Result<(), E>,
    ) -> Result<PathBuf, VideoCacheCommitError<E>> {
        let _guard = self.lock_operations();
        self.verify_root().map_err(VideoCacheCommitError::Cache)?;
        if staged.root_identity != self.root_identity
            || !directory_at_matches(
                &self.root_directory,
                OsStr::new(&staged.entry_name),
                &staged.entry_directory,
            )
            .map_err(VideoCacheCommitError::Cache)?
        {
            return Err(VideoCacheCommitError::Cache(CacheError::UnsafeEntry));
        }
        verify_optional_regular_at(&staged.entry_directory, OsStr::new(ARTIFACT_NAME))
            .map_err(VideoCacheCommitError::Cache)?;
        validate().map_err(VideoCacheCommitError::Rejected)?;
        let temporary_name = staged
            .temporary_name
            .take()
            .ok_or(VideoCacheCommitError::Cache(CacheError::UnsafeEntry))?;
        rename_at(
            &staged.entry_directory,
            &temporary_name,
            &staged.entry_directory,
            OsStr::new(ARTIFACT_NAME),
        )
        .map_err(VideoCacheCommitError::Cache)?;
        touch_access_stamp(&staged.entry_directory).map_err(VideoCacheCommitError::Cache)?;
        self.verify_root().map_err(VideoCacheCommitError::Cache)?;
        Ok(staged.artifact_path.clone())
    }

    pub async fn evict_to_budget_async(&self) -> Result<(), CacheError> {
        let cache = self.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = cache.lock_operations();
            cache.verify_root()?;
            cache.evict_to_budget()
        })
        .await
        .map_err(|_| CacheError::Io)?
    }

    fn stage_atomic(
        &self,
        key: &VideoCacheKey,
        png: &[u8],
    ) -> Result<VideoCacheStagedPng, CacheError> {
        if !png.starts_with(PNG_SIGNATURE) {
            return Err(CacheError::InvalidPng);
        }
        if png.len() as u64 > VIDEO_CACHE_BUDGET_BYTES {
            return Err(CacheError::ArtifactTooLarge);
        }
        let _guard = self.lock_operations();
        self.verify_root()?;
        let entry_name = key.to_hex();
        let entry_directory = self.ensure_entry_directory(OsStr::new(&entry_name))?;
        verify_optional_regular_at(&entry_directory, OsStr::new(ARTIFACT_NAME))?;
        let temporary_name = OsString::from(format!(".artifact.tmp-{}", random_hex()?));
        let mut file = open_regular_at(
            &entry_directory,
            &temporary_name,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
            0o600,
        )
        .map_err(|_| CacheError::Io)?;
        if file.write_all(png).and_then(|()| file.sync_all()).is_err() {
            let _ = unlink_file_at(&entry_directory, &temporary_name);
            return Err(CacheError::Io);
        }
        drop(file);
        Ok(VideoCacheStagedPng {
            root_directory: Arc::clone(&self.root_directory),
            root_identity: self.root_identity,
            entry_directory,
            entry_name: entry_name.clone(),
            temporary_name: Some(temporary_name),
            artifact_path: self.root.join(entry_name).join(ARTIFACT_NAME),
        })
    }

    fn put_atomic_inner(
        &self,
        key: &VideoCacheKey,
        png: &[u8],
        before_commit: impl FnOnce(&Path),
    ) -> Result<PathBuf, CacheError> {
        if !png.starts_with(PNG_SIGNATURE) {
            return Err(CacheError::InvalidPng);
        }
        if png.len() as u64 > VIDEO_CACHE_BUDGET_BYTES {
            return Err(CacheError::ArtifactTooLarge);
        }
        let _guard = self.lock_operations();
        self.verify_root()?;
        let entry_name = key.to_hex();
        let entry = self.entry_directory(key);
        let entry_directory = self.ensure_entry_directory(OsStr::new(&entry_name))?;
        verify_optional_regular_at(&entry_directory, OsStr::new(ARTIFACT_NAME))?;
        let temporary_name = format!(".artifact.tmp-{}", random_hex()?);
        let mut file = open_regular_at(
            &entry_directory,
            OsStr::new(&temporary_name),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
            0o600,
        )
        .map_err(|_| CacheError::Io)?;
        let write_result = file.write_all(png).and_then(|()| file.sync_all());
        if write_result.is_err() {
            let _ = unlink_file_at(&entry_directory, OsStr::new(&temporary_name));
            return Err(CacheError::Io);
        }
        drop(file);
        before_commit(&entry);
        if !directory_at_matches(
            &self.root_directory,
            OsStr::new(&entry_name),
            &entry_directory,
        )? {
            let _ = unlink_file_at(&entry_directory, OsStr::new(&temporary_name));
            return Err(CacheError::UnsafeEntry);
        }
        rename_at(
            &entry_directory,
            OsStr::new(&temporary_name),
            &entry_directory,
            OsStr::new(ARTIFACT_NAME),
        )?;
        touch_access_stamp(&entry_directory)?;
        self.evict_to_budget()?;
        self.verify_root()?;
        Ok(entry.join(ARTIFACT_NAME))
    }

    pub fn get(&self, key: &VideoCacheKey) -> Result<Option<PathBuf>, CacheError> {
        let _guard = self.lock_operations();
        self.verify_root()?;
        let entry_name = key.to_hex();
        let entry = self.entry_directory(key);
        let entry_directory = match open_directory_at(&self.root_directory, OsStr::new(&entry_name))
        {
            Ok(directory) => directory,
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => return Ok(None),
            Err(error)
                if error.raw_os_error() == Some(libc::ELOOP)
                    || error.raw_os_error() == Some(libc::ENOTDIR) =>
            {
                return Err(CacheError::UnsafeEntry);
            }
            Err(_) => return Err(CacheError::Io),
        };
        let artifact = match open_regular_at(
            &entry_directory,
            OsStr::new(ARTIFACT_NAME),
            libc::O_RDONLY,
            0,
        ) {
            Ok(file) => file,
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => return Ok(None),
            Err(error) if error.raw_os_error() == Some(libc::ELOOP) => {
                return Err(CacheError::UnsafeEntry);
            }
            Err(_) => return Err(CacheError::Io),
        };
        if !artifact.metadata().map_err(|_| CacheError::Io)?.is_file() {
            return Err(CacheError::UnsafeEntry);
        }
        if !directory_at_matches(
            &self.root_directory,
            OsStr::new(&entry_name),
            &entry_directory,
        )? {
            return Err(CacheError::UnsafeEntry);
        }
        touch_access_stamp(&entry_directory)?;
        self.verify_root()?;
        Ok(Some(entry.join(ARTIFACT_NAME)))
    }

    pub fn stats(&self) -> Result<VideoCacheStats, CacheError> {
        let _guard = self.lock_operations();
        self.verify_root()?;
        self.stats_unlocked()
    }

    pub fn bind_png_artifact(&self, path: &Path) -> Result<VerifiedVideoPng, CacheError> {
        let _guard = self.lock_operations();
        self.verify_root()?;
        if path.file_name().and_then(|name| name.to_str()) != Some(ARTIFACT_NAME) {
            return Err(CacheError::UnsafeEntry);
        }
        let entry = path.parent().ok_or(CacheError::UnsafeEntry)?;
        let name = entry
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(CacheError::UnsafeEntry)?;
        if !is_cache_key_name(name) || entry.parent() != Some(self.root.as_path()) {
            return Err(CacheError::UnsafeEntry);
        }
        let entry_directory = open_directory_at(&self.root_directory, OsStr::new(name))
            .map_err(|_| CacheError::UnsafeEntry)?;
        let file = open_regular_at(
            &entry_directory,
            OsStr::new(ARTIFACT_NAME),
            libc::O_RDONLY,
            0,
        )
        .map_err(|_| CacheError::UnsafeEntry)?;
        if !file
            .metadata()
            .map_err(|_| CacheError::UnsafeEntry)?
            .is_file()
            || !directory_at_matches(&self.root_directory, OsStr::new(name), &entry_directory)?
        {
            return Err(CacheError::UnsafeEntry);
        }
        let metadata = file.metadata().map_err(|_| CacheError::InvalidPng)?;
        if metadata.len() > VIDEO_CACHE_BUDGET_BYTES {
            return Err(CacheError::ArtifactTooLarge);
        }
        let length = usize::try_from(metadata.len()).map_err(|_| CacheError::ArtifactTooLarge)?;
        let mut bytes = vec![0_u8; length];
        read_exact_at(&file, &mut bytes).map_err(|_| CacheError::InvalidPng)?;
        if !bytes.starts_with(PNG_SIGNATURE) {
            return Err(CacheError::InvalidPng);
        }
        self.verify_root()?;
        Ok(VerifiedVideoPng {
            path: path.to_path_buf(),
            bytes: Arc::from(bytes),
        })
    }

    pub async fn clear(&self) -> Result<VideoCacheStats, CacheError> {
        let cache = self.clone();
        tokio::task::spawn_blocking(move || cache.clear_sync())
            .await
            .map_err(|_| CacheError::Io)?
    }

    fn clear_sync(&self) -> Result<VideoCacheStats, CacheError> {
        self.clear_sync_inner(|_| {})
    }

    fn clear_sync_inner(
        &self,
        mut before_remove: impl FnMut(&Path),
    ) -> Result<VideoCacheStats, CacheError> {
        let _guard = self.lock_operations();
        self.verify_root()?;
        for entry in self.verified_entries()? {
            before_remove(&self.root.join(&entry.name));
            self.remove_entry(&entry)?;
        }
        self.verify_root()?;
        self.stats_unlocked()
    }

    fn verify_root(&self) -> Result<(), CacheError> {
        let current = open_directory(&self.root).map_err(|_| CacheError::UnverifiedRoot)?;
        if DirectoryIdentity::of(&current)? != self.root_identity {
            return Err(CacheError::UnverifiedRoot);
        }
        let mut marker = open_regular_at(
            &self.root_directory,
            OsStr::new(CACHE_MARKER_NAME),
            libc::O_RDONLY,
            0,
        )
        .map_err(|_| CacheError::UnverifiedRoot)?;
        if !marker
            .metadata()
            .map_err(|_| CacheError::UnverifiedRoot)?
            .is_file()
        {
            return Err(CacheError::UnverifiedRoot);
        }
        let mut contents = Vec::new();
        marker
            .read_to_end(&mut contents)
            .map_err(|_| CacheError::UnverifiedRoot)?;
        if contents != CACHE_MARKER_CONTENTS {
            return Err(CacheError::UnverifiedRoot);
        }
        Ok(())
    }

    fn entry_directory(&self, key: &VideoCacheKey) -> PathBuf {
        self.root.join(key.to_hex())
    }

    fn verified_entries(&self) -> Result<Vec<CacheEntry>, CacheError> {
        let mut entries = Vec::new();
        for name in directory_names(&self.root_directory)? {
            let Some(name) = name.to_str() else {
                continue;
            };
            if !is_cache_key_name(name) {
                continue;
            }
            let directory = open_directory_at(&self.root_directory, OsStr::new(name))
                .map_err(|_| CacheError::UnsafeEntry)?;
            let mut bytes = 0_u64;
            let mut accessed = SystemTime::UNIX_EPOCH;
            let mut owned_files = Vec::new();
            for file_name in directory_names(&directory)? {
                let Some(file_name_str) = file_name.to_str() else {
                    return Err(CacheError::UnsafeEntry);
                };
                if !is_owned_entry_file(file_name_str) {
                    return Err(CacheError::UnsafeEntry);
                }
                let file = open_regular_at(&directory, &file_name, libc::O_RDONLY, 0)
                    .map_err(|_| CacheError::UnsafeEntry)?;
                let metadata = file.metadata().map_err(|_| CacheError::Io)?;
                if !metadata.is_file() {
                    return Err(CacheError::UnsafeEntry);
                }
                bytes = bytes.saturating_add(metadata.len());
                if file_name_str == ACCESS_STAMP_NAME {
                    accessed = metadata.modified().map_err(|_| CacheError::Io)?;
                }
                owned_files.push(file_name);
            }
            entries.push(CacheEntry {
                directory,
                owned_files,
                bytes,
                accessed,
                name: name.to_owned(),
            });
        }
        Ok(entries)
    }

    fn stats_unlocked(&self) -> Result<VideoCacheStats, CacheError> {
        let entries = self.verified_entries()?;
        Ok(VideoCacheStats {
            bytes_used: entries
                .iter()
                .fold(0_u64, |total, entry| total.saturating_add(entry.bytes)),
            entry_count: entries.len() as u64,
            budget_bytes: VIDEO_CACHE_BUDGET_BYTES,
        })
    }

    fn evict_to_budget(&self) -> Result<(), CacheError> {
        let mut entries = self.verified_entries()?;
        let mut bytes_used = entries
            .iter()
            .fold(0_u64, |total, entry| total.saturating_add(entry.bytes));
        entries.sort_by(|left, right| {
            left.accessed
                .cmp(&right.accessed)
                .then_with(|| left.name.cmp(&right.name))
        });
        for entry in entries {
            if bytes_used <= VIDEO_CACHE_BUDGET_BYTES {
                break;
            }
            self.remove_entry(&entry)?;
            bytes_used = bytes_used.saturating_sub(entry.bytes);
        }
        Ok(())
    }

    fn lock_operations(&self) -> MutexGuard<'_, ()> {
        self.operations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn ensure_entry_directory(&self, name: &OsStr) -> Result<File, CacheError> {
        match open_directory_at(&self.root_directory, name) {
            Ok(directory) => Ok(directory),
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => {
                mkdir_at(&self.root_directory, name)?;
                open_directory_at(&self.root_directory, name).map_err(|_| CacheError::Io)
            }
            Err(error)
                if error.raw_os_error() == Some(libc::ELOOP)
                    || error.raw_os_error() == Some(libc::ENOTDIR) =>
            {
                Err(CacheError::UnsafeEntry)
            }
            Err(_) => Err(CacheError::Io),
        }
    }

    fn remove_entry(&self, entry: &CacheEntry) -> Result<(), CacheError> {
        if !directory_at_matches(
            &self.root_directory,
            OsStr::new(&entry.name),
            &entry.directory,
        )? {
            return Err(CacheError::UnsafeEntry);
        }
        for name in &entry.owned_files {
            unlink_file_at(&entry.directory, name)?;
        }
        unlink_directory_at(&self.root_directory, OsStr::new(&entry.name))
    }

    #[cfg(test)]
    fn put_atomic_with_before_commit(
        &self,
        key: &VideoCacheKey,
        png: &[u8],
        before_commit: impl FnOnce(&Path),
    ) -> Result<PathBuf, CacheError> {
        self.put_atomic_inner(key, png, before_commit)
    }

    #[cfg(test)]
    fn clear_sync_with_before_remove(
        &self,
        before_remove: impl FnMut(&Path),
    ) -> Result<VideoCacheStats, CacheError> {
        self.clear_sync_inner(before_remove)
    }
}

struct CacheEntry {
    directory: File,
    owned_files: Vec<OsString>,
    bytes: u64,
    accessed: SystemTime,
    name: String,
}

fn touch_access_stamp(entry: &File) -> Result<(), CacheError> {
    verify_optional_regular_at(entry, OsStr::new(ACCESS_STAMP_NAME))?;
    let mut file = open_regular_at(
        entry,
        OsStr::new(ACCESS_STAMP_NAME),
        libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
        0o600,
    )
    .map_err(|_| CacheError::Io)?;
    file.write_all(b"1").map_err(|_| CacheError::Io)
}

fn read_exact_at(file: &File, bytes: &mut [u8]) -> std::io::Result<()> {
    use std::os::unix::fs::FileExt;

    let mut offset = 0_usize;
    while offset < bytes.len() {
        let read = file.read_at(&mut bytes[offset..], offset as u64)?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "video cache artifact changed while reading",
            ));
        }
        offset += read;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectoryIdentity {
    device: u64,
    inode: u64,
}

impl DirectoryIdentity {
    fn of(directory: &File) -> Result<Self, CacheError> {
        let metadata = directory.metadata().map_err(|_| CacheError::Io)?;
        if !metadata.is_dir() {
            return Err(CacheError::UnsafeEntry);
        }
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
}

fn c_name(name: &OsStr) -> Result<CString, CacheError> {
    if name.as_bytes().contains(&0) || name.as_bytes().contains(&b'/') {
        return Err(CacheError::UnsafeEntry);
    }
    CString::new(name.as_bytes()).map_err(|_| CacheError::UnsafeEntry)
}

fn open_directory(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
}

fn open_directory_at(parent: &File, name: &OsStr) -> std::io::Result<File> {
    let name = c_name(name).map_err(|_| std::io::Error::from_raw_os_error(libc::EINVAL))?;
    let descriptor = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if descriptor < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_fd(descriptor) })
    }
}

fn open_regular_at(
    parent: &File,
    name: &OsStr,
    flags: libc::c_int,
    mode: libc::mode_t,
) -> std::io::Result<File> {
    let name = c_name(name).map_err(|_| std::io::Error::from_raw_os_error(libc::EINVAL))?;
    let descriptor = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            libc::c_uint::from(mode),
        )
    };
    if descriptor < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_fd(descriptor) })
    }
}

fn mkdir_at(parent: &File, name: &OsStr) -> Result<(), CacheError> {
    let name = c_name(name)?;
    let result = unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) };
    if result == 0 {
        Ok(())
    } else {
        Err(CacheError::Io)
    }
}

fn rename_at(
    old_parent: &File,
    old_name: &OsStr,
    new_parent: &File,
    new_name: &OsStr,
) -> Result<(), CacheError> {
    let old_name = c_name(old_name)?;
    let new_name = c_name(new_name)?;
    let result = unsafe {
        libc::renameat(
            old_parent.as_raw_fd(),
            old_name.as_ptr(),
            new_parent.as_raw_fd(),
            new_name.as_ptr(),
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(CacheError::Io)
    }
}

fn unlink_file_at(parent: &File, name: &OsStr) -> Result<(), CacheError> {
    unlink_at(parent, name, 0)
}

fn unlink_directory_at(parent: &File, name: &OsStr) -> Result<(), CacheError> {
    unlink_at(parent, name, libc::AT_REMOVEDIR)
}

fn unlink_at(parent: &File, name: &OsStr, flags: libc::c_int) -> Result<(), CacheError> {
    let name = c_name(name)?;
    let result = unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), flags) };
    if result == 0 {
        Ok(())
    } else {
        let error = std::io::Error::last_os_error();
        if matches!(
            error.raw_os_error(),
            Some(libc::ELOOP) | Some(libc::ENOTDIR)
        ) {
            Err(CacheError::UnsafeEntry)
        } else {
            Err(CacheError::Io)
        }
    }
}

fn verify_optional_regular_at(parent: &File, name: &OsStr) -> Result<(), CacheError> {
    match open_regular_at(parent, name, libc::O_RDONLY, 0) {
        Ok(file) if file.metadata().map_err(|_| CacheError::Io)?.is_file() => Ok(()),
        Ok(_) => Err(CacheError::UnsafeEntry),
        Err(error) if error.raw_os_error() == Some(libc::ENOENT) => Ok(()),
        Err(error) if error.raw_os_error() == Some(libc::ELOOP) => Err(CacheError::UnsafeEntry),
        Err(_) => Err(CacheError::Io),
    }
}

fn directory_at_matches(parent: &File, name: &OsStr, expected: &File) -> Result<bool, CacheError> {
    let current = match open_directory_at(parent, name) {
        Ok(directory) => directory,
        Err(error)
            if error.raw_os_error() == Some(libc::ENOENT)
                || error.raw_os_error() == Some(libc::ELOOP)
                || error.raw_os_error() == Some(libc::ENOTDIR) =>
        {
            return Ok(false);
        }
        Err(_) => return Err(CacheError::Io),
    };
    Ok(DirectoryIdentity::of(&current)? == DirectoryIdentity::of(expected)?)
}

fn directory_names(directory: &File) -> Result<Vec<OsString>, CacheError> {
    let descriptor = unsafe { libc::fcntl(directory.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    if descriptor < 0 {
        return Err(CacheError::Io);
    }
    let stream = unsafe { libc::fdopendir(descriptor) };
    if stream.is_null() {
        unsafe { libc::close(descriptor) };
        return Err(CacheError::Io);
    }
    unsafe { libc::rewinddir(stream) };
    let mut names = Vec::new();
    loop {
        let entry = unsafe { libc::readdir(stream) };
        if entry.is_null() {
            break;
        }
        let bytes = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
        if bytes != b"." && bytes != b".." {
            names.push(OsString::from_vec(bytes.to_vec()));
        }
    }
    if unsafe { libc::closedir(stream) } != 0 {
        return Err(CacheError::Io);
    }
    Ok(names)
}

fn is_owned_entry_file(name: &str) -> bool {
    name == ARTIFACT_NAME
        || name == ACCESS_STAMP_NAME
        || name.strip_prefix(".artifact.tmp-").is_some_and(|suffix| {
            suffix.len() == 32
                && suffix
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
}

fn is_cache_key_name(name: &str) -> bool {
    name.len() == 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn random_hex() -> Result<String, CacheError> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random).map_err(|_| CacheError::Io)?;
    let mut encoded = String::with_capacity(32);
    for byte in random {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    Ok(encoded)
}

fn shared_operation_lock(root: &Path) -> Arc<OperationLock> {
    let locks = ROOT_OPERATION_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut locks = locks
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(root).and_then(Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(Mutex::new(()));
    locks.insert(root.to_path_buf(), Arc::downgrade(&lock));
    lock
}

#[cfg(test)]
mod adversarial_tests {
    use super::{CacheError, VideoCache, VideoCacheKey, VideoSourceIdentity};
    use std::{fs, path::Path};

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfixture";

    fn key() -> VideoCacheKey {
        VideoCacheKey::cover(
            VideoSourceIdentity::new(Path::new("/project/a.mp4"), 1, 1),
            1,
        )
    }

    #[cfg(unix)]
    #[test]
    fn publication_does_not_follow_an_entry_swapped_after_open() {
        let app_cache = tempfile::tempdir().unwrap();
        let cache = VideoCache::initialize(app_cache.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let key = key();

        let result = cache.put_atomic_with_before_commit(&key, PNG, |entry| {
            fs::rename(entry, entry.with_extension("moved")).unwrap();
            std::os::unix::fs::symlink(outside.path(), entry).unwrap();
        });

        assert!(
            matches!(result, Err(CacheError::UnsafeEntry)),
            "unexpected clear result: {result:?}"
        );
        assert!(!outside.path().join("artifact.png").exists());
    }

    #[cfg(unix)]
    #[test]
    fn clear_does_not_follow_a_root_entry_swapped_after_open() {
        let app_cache = tempfile::tempdir().unwrap();
        let cache = VideoCache::initialize(app_cache.path()).unwrap();
        let artifact = cache.put_atomic(&key(), PNG).unwrap();
        let entry = artifact.parent().unwrap().to_path_buf();
        let outside = tempfile::tempdir().unwrap();
        let sentinel = outside.path().join("sentinel");
        fs::write(&sentinel, b"keep").unwrap();

        let result = cache.clear_sync_with_before_remove(|_| {
            fs::rename(&entry, entry.with_extension("moved")).unwrap();
            std::os::unix::fs::symlink(outside.path(), &entry).unwrap();
        });

        assert!(
            matches!(result, Err(CacheError::UnsafeEntry)),
            "unexpected clear result: {result:?}"
        );
        assert_eq!(fs::read(&sentinel).unwrap(), b"keep");
    }

    #[cfg(unix)]
    #[test]
    fn publication_does_not_follow_a_cache_root_swapped_after_open() {
        let app_cache = tempfile::tempdir().unwrap();
        let cache = VideoCache::initialize(app_cache.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = cache.root().to_path_buf();

        let result = cache.put_atomic_with_before_commit(&key(), PNG, |_| {
            fs::rename(&root, root.with_extension("moved")).unwrap();
            std::os::unix::fs::symlink(outside.path(), &root).unwrap();
        });

        assert!(matches!(result, Err(CacheError::UnverifiedRoot)));
        assert!(!outside.path().join("artifact.png").exists());
    }

    #[cfg(unix)]
    #[test]
    fn clear_does_not_follow_a_cache_root_swapped_after_open() {
        let app_cache = tempfile::tempdir().unwrap();
        let cache = VideoCache::initialize(app_cache.path()).unwrap();
        cache.put_atomic(&key(), PNG).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let sentinel = outside.path().join("sentinel");
        fs::write(&sentinel, b"keep").unwrap();
        let root = cache.root().to_path_buf();

        let result = cache.clear_sync_with_before_remove(|_| {
            fs::rename(&root, root.with_extension("moved")).unwrap();
            std::os::unix::fs::symlink(outside.path(), &root).unwrap();
        });

        assert!(matches!(result, Err(CacheError::UnverifiedRoot)));
        assert_eq!(fs::read(&sentinel).unwrap(), b"keep");
    }
}
