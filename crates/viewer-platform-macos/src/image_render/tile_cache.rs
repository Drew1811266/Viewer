use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use viewer_domain::image::ImageProbe;
use viewer_render_core::{
    AssetGeneration, BudgetedLru, CacheEntry, CacheTier, MemoryBudget, PressureLevel,
    ResourcePriority, TileCoordinate,
};

use super::{
    DecodedResource, DecodedResourceKind, ImageResourceError, PixelFormat, SourceFingerprint,
};

// Version 2 fixes the canonical scanline orientation. Keeping it in the cache
// key prevents version-1 bottom-up previews from surviving the decoder change.
pub const CACHE_SCHEMA_VERSION: u32 = 2;
const CACHE_MAGIC: &[u8; 8] = b"VWBGRA01";
const CACHE_HEADER_BYTES: usize = 28;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DerivedRegion {
    Preview { width: u32, height: u32 },
    Tile(TileCoordinate),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DerivedCacheKey {
    source: SourceFingerprint,
    orientation: u8,
    source_color_profile: Option<String>,
    level: u32,
    region: DerivedRegion,
    schema_version: u32,
}

impl DerivedCacheKey {
    pub fn new(
        source: SourceFingerprint,
        probe: &ImageProbe,
        level: u32,
        region: DerivedRegion,
        schema_version: u32,
    ) -> Self {
        Self {
            source,
            orientation: probe.orientation,
            source_color_profile: probe.icc_profile_name.clone(),
            level,
            region,
            schema_version,
        }
    }

    pub const fn level(&self) -> u32 {
        self.level
    }

    pub fn region(&self) -> &DerivedRegion {
        &self.region
    }

    fn digest_hex(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"viewer-image-render-cache-key\0");
        hasher.update(&self.schema_version.to_le_bytes());
        hasher.update(&self.source.blake3);
        hasher.update(&self.source.size_bytes.to_le_bytes());
        hasher.update(&self.source.modified_ns.to_le_bytes());
        hasher.update(&self.source.device.to_le_bytes());
        hasher.update(&self.source.inode.to_le_bytes());
        hasher.update(&self.orientation.to_le_bytes());
        let profile = self.source_color_profile.as_deref().unwrap_or("");
        hasher.update(&(profile.len() as u64).to_le_bytes());
        hasher.update(profile.as_bytes());
        hasher.update(&self.level.to_le_bytes());
        match self.region {
            DerivedRegion::Preview { width, height } => {
                hasher.update(&[0]);
                hasher.update(&width.to_le_bytes());
                hasher.update(&height.to_le_bytes());
            }
            DerivedRegion::Tile(tile) => {
                hasher.update(&[1]);
                hasher.update(&tile.level.to_le_bytes());
                hasher.update(&tile.x.to_le_bytes());
                hasher.update(&tile.y.to_le_bytes());
            }
        }
        hasher.finalize().to_hex().to_string()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CacheUsage {
    pub cpu_bytes: u64,
    pub gpu_bytes: u64,
    pub disk_bytes: u64,
}

impl CacheUsage {
    pub const fn total(self) -> u64 {
        self.cpu_bytes
            .saturating_add(self.gpu_bytes)
            .saturating_add(self.disk_bytes)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheReclaimReport {
    pub pressure: PressureLevel,
    pub before: CacheUsage,
    pub after: CacheUsage,
    pub reclaimed_entries: usize,
}

#[derive(Clone)]
pub struct MacImageTileCache {
    root: Arc<PathBuf>,
    budget: MemoryBudget,
    ledger: Arc<Mutex<BudgetedLru<DerivedCacheKey>>>,
}

impl MacImageTileCache {
    pub fn new(root: &Path, budget: MemoryBudget) -> Result<Self, ImageResourceError> {
        if root.exists() {
            let metadata = fs::symlink_metadata(root).map_err(ImageResourceError::io)?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(ImageResourceError::UnsafeCacheRoot);
            }
        } else {
            fs::create_dir_all(root).map_err(ImageResourceError::io)?;
        }
        let root = fs::canonicalize(root).map_err(ImageResourceError::io)?;
        Ok(Self {
            root: Arc::new(root),
            budget,
            ledger: Arc::new(Mutex::new(BudgetedLru::new(budget))),
        })
    }

    pub fn entry_path(&self, key: &DerivedCacheKey) -> PathBuf {
        self.root.join(format!("{}.bgra", key.digest_hex()))
    }

    pub fn load(
        &self,
        key: &DerivedCacheKey,
        generation: AssetGeneration,
        kind: DecodedResourceKind,
        priority: ResourcePriority,
    ) -> Result<Option<DecodedResource>, ImageResourceError> {
        let path = self.entry_path(key);
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.remove_ledger_entry(key)?;
                return Ok(None);
            }
            Err(error) => return Err(ImageResourceError::io(error)),
        };
        if metadata.len() > self.budget.disk_cache_bytes {
            let _ = fs::remove_file(&path);
            self.remove_ledger_entry(key)?;
            return Ok(None);
        }
        let decoded = match read_entry(&path, key.clone(), generation, kind) {
            Ok(decoded) => decoded,
            Err(_) => {
                let _ = fs::remove_file(&path);
                self.remove_ledger_entry(key)?;
                return Ok(None);
            }
        };
        let victims = self.insert_ledger(key, metadata.len(), generation, priority)?;
        self.remove_files(victims, Some(key));
        Ok(Some(decoded))
    }

    pub fn store(
        &self,
        resource: &DecodedResource,
        priority: ResourcePriority,
    ) -> Result<(), ImageResourceError> {
        validate_resource(resource)?;
        let path = self.entry_path(&resource.cache_key);
        let entry_bytes = u64::try_from(CACHE_HEADER_BYTES)
            .ok()
            .and_then(|header| header.checked_add(resource.pixels.len() as u64))
            .ok_or(ImageResourceError::LimitExceeded)?;
        if entry_bytes > self.budget.disk_cache_bytes {
            return Err(ImageResourceError::LimitExceeded);
        }
        let created = !path.exists();
        if created {
            write_entry_atomically(&path, resource)?;
        }
        let victims = match self.insert_ledger(
            &resource.cache_key,
            entry_bytes,
            resource.generation,
            priority,
        ) {
            Ok(victims) => victims,
            Err(error) => {
                if created {
                    let _ = fs::remove_file(&path);
                }
                return Err(error);
            }
        };
        self.remove_files(victims, Some(&resource.cache_key));
        Ok(())
    }

    pub fn remove(&self, key: &DerivedCacheKey) -> Result<bool, ImageResourceError> {
        let removed = self
            .ledger
            .lock()
            .map_err(|_| ImageResourceError::CacheUnavailable)?
            .remove(key)
            .is_some();
        match fs::remove_file(self.entry_path(key)) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(removed),
            Err(error) => Err(ImageResourceError::io(error)),
        }
    }

    pub fn apply_pressure(
        &self,
        pressure: PressureLevel,
        current_generation: AssetGeneration,
    ) -> Result<CacheReclaimReport, ImageResourceError> {
        let (before, victims, after) = {
            let mut ledger = self
                .ledger
                .lock()
                .map_err(|_| ImageResourceError::CacheUnavailable)?;
            let before = usage(&ledger);
            let victims = ledger.apply_pressure(pressure, current_generation);
            let after = usage(&ledger);
            (before, victims, after)
        };
        let reclaimed_entries = victims.len();
        self.remove_files(victims, None);
        Ok(CacheReclaimReport {
            pressure,
            before,
            after,
            reclaimed_entries,
        })
    }

    fn insert_ledger(
        &self,
        key: &DerivedCacheKey,
        bytes: u64,
        generation: AssetGeneration,
        priority: ResourcePriority,
    ) -> Result<Vec<DerivedCacheKey>, ImageResourceError> {
        self.ledger
            .lock()
            .map_err(|_| ImageResourceError::CacheUnavailable)?
            .insert(CacheEntry {
                key: key.clone(),
                tier: CacheTier::DiskDerived,
                bytes,
                generation,
                priority,
                rebuildable: true,
            })
            .map_err(|error| ImageResourceError::Cache(error.to_string()))
    }

    fn remove_ledger_entry(&self, key: &DerivedCacheKey) -> Result<(), ImageResourceError> {
        self.ledger
            .lock()
            .map_err(|_| ImageResourceError::CacheUnavailable)?
            .remove(key);
        Ok(())
    }

    fn remove_files(&self, victims: Vec<DerivedCacheKey>, except: Option<&DerivedCacheKey>) {
        for victim in victims {
            if except == Some(&victim) {
                continue;
            }
            let _ = fs::remove_file(self.entry_path(&victim));
        }
    }
}

fn usage(ledger: &BudgetedLru<DerivedCacheKey>) -> CacheUsage {
    CacheUsage {
        cpu_bytes: ledger.usage(CacheTier::CpuStaging),
        gpu_bytes: ledger.usage(CacheTier::GpuTexture),
        disk_bytes: ledger.usage(CacheTier::DiskDerived),
    }
}

fn validate_resource(resource: &DecodedResource) -> Result<(), ImageResourceError> {
    if resource.pixel_format != PixelFormat::Bgra8PremultipliedSrgb
        || resource.width == 0
        || resource.height == 0
        || resource.bytes_per_row != resource.width.saturating_mul(4)
        || u64::try_from(resource.pixels.len()).ok()
            != Some(u64::from(resource.bytes_per_row) * u64::from(resource.height))
    {
        return Err(ImageResourceError::InvalidResource);
    }
    Ok(())
}

fn write_entry_atomically(
    destination: &Path,
    resource: &DecodedResource,
) -> Result<(), ImageResourceError> {
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(ImageResourceError::UnsafeCacheRoot)?;
    let temporary =
        destination.with_file_name(format!("{file_name}.tmp-{}", uuid::Uuid::new_v4().simple()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(ImageResourceError::io)?;
        file.write_all(CACHE_MAGIC)
            .and_then(|_| file.write_all(&resource.width.to_le_bytes()))
            .and_then(|_| file.write_all(&resource.height.to_le_bytes()))
            .and_then(|_| file.write_all(&resource.bytes_per_row.to_le_bytes()))
            .and_then(|_| file.write_all(&(resource.pixels.len() as u64).to_le_bytes()))
            .and_then(|_| file.write_all(&resource.pixels))
            .and_then(|_| file.sync_all())
            .map_err(ImageResourceError::io)?;
        fs::rename(&temporary, destination).map_err(ImageResourceError::io)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn read_entry(
    path: &Path,
    key: DerivedCacheKey,
    generation: AssetGeneration,
    kind: DecodedResourceKind,
) -> Result<DecodedResource, ImageResourceError> {
    let mut file = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(ImageResourceError::io)?;
    let mut header = [0_u8; CACHE_HEADER_BYTES];
    file.read_exact(&mut header)
        .map_err(ImageResourceError::io)?;
    if &header[..8] != CACHE_MAGIC {
        return Err(ImageResourceError::InvalidResource);
    }
    let width = u32::from_le_bytes(header[8..12].try_into().unwrap());
    let height = u32::from_le_bytes(header[12..16].try_into().unwrap());
    let bytes_per_row = u32::from_le_bytes(header[16..20].try_into().unwrap());
    let payload_len = u64::from_le_bytes(header[20..28].try_into().unwrap());
    let expected = u64::from(bytes_per_row)
        .checked_mul(u64::from(height))
        .ok_or(ImageResourceError::InvalidResource)?;
    if width == 0
        || height == 0
        || bytes_per_row != width.saturating_mul(4)
        || payload_len != expected
    {
        return Err(ImageResourceError::InvalidResource);
    }
    let mut pixels =
        vec![0; usize::try_from(payload_len).map_err(|_| ImageResourceError::LimitExceeded)?];
    file.read_exact(&mut pixels)
        .map_err(ImageResourceError::io)?;
    let mut trailing = [0_u8; 1];
    if file.read(&mut trailing).map_err(ImageResourceError::io)? != 0 {
        return Err(ImageResourceError::InvalidResource);
    }
    Ok(DecodedResource {
        generation,
        cache_key: key,
        kind,
        width,
        height,
        bytes_per_row,
        pixel_format: PixelFormat::Bgra8PremultipliedSrgb,
        pixels: pixels.into(),
    })
}
