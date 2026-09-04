use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use thiserror::Error;
use viewer_application::ImageError;
use viewer_domain::image::ImageProbe;
use viewer_render_core::{
    AssetGeneration, MemoryBudget, PressureLevel, ResourcePlan, ResourcePriority, TextureStrategy,
    TileCoordinate,
};

use crate::image::image_io::{
    open_image_source, oriented_dimensions, probe_image_source, thumbnail_from_image_source,
};

use super::color::normalize_to_bgra_srgb;
use super::{
    CacheReclaimReport, DecodedPixels, DerivedCacheKey, DerivedRegion, MacImageTileCache,
    MemoryPressureSink, PixelFormat,
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SourceFingerprint {
    pub(crate) blake3: [u8; 32],
    pub(crate) size_bytes: u64,
    pub(crate) modified_ns: i128,
    pub(crate) device: u64,
    pub(crate) inode: u64,
}

#[derive(Clone, Debug)]
pub struct AuthorizedImageSource {
    path: Arc<PathBuf>,
    fingerprint: SourceFingerprint,
}

impl AuthorizedImageSource {
    /// Constructs a process-local capability after the caller has authorized the
    /// path. This type is deliberately neither serializable nor constructible
    /// from an untrusted frontend string.
    pub fn authorize_for_process(path: impl AsRef<Path>) -> Result<Self, ImageResourceError> {
        let supplied_path = path.as_ref();
        let link_metadata = fs::symlink_metadata(supplied_path).map_err(ImageResourceError::io)?;
        if link_metadata.file_type().is_symlink() || !link_metadata.is_file() {
            return Err(ImageResourceError::UnsafeSource);
        }

        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
            .open(supplied_path)
            .map_err(ImageResourceError::io)?;
        let before = file.metadata().map_err(ImageResourceError::io)?;
        if !before.is_file() {
            return Err(ImageResourceError::UnsafeSource);
        }
        let canonical = fs::canonicalize(supplied_path).map_err(ImageResourceError::io)?;
        let mut hasher = blake3::Hasher::new();
        hasher
            .update_reader(&mut file)
            .map_err(ImageResourceError::io)?;
        let after = file.metadata().map_err(ImageResourceError::io)?;
        if metadata_identity(&before) != metadata_identity(&after) {
            return Err(ImageResourceError::SourceChanged);
        }
        let path_metadata = fs::metadata(&canonical).map_err(ImageResourceError::io)?;
        if metadata_identity(&after) != metadata_identity(&path_metadata) {
            return Err(ImageResourceError::SourceChanged);
        }

        Ok(Self {
            path: Arc::new(canonical),
            fingerprint: SourceFingerprint {
                blake3: *hasher.finalize().as_bytes(),
                size_bytes: after.len(),
                modified_ns: modified_ns(&after),
                device: after.dev(),
                inode: after.ino(),
            },
        })
    }

    pub fn fingerprint(&self) -> &SourceFingerprint {
        &self.fingerprint
    }

    pub(crate) fn path(&self) -> &Path {
        self.path.as_ref()
    }

    fn verify_unchanged(&self) -> Result<(), ImageResourceError> {
        let metadata = fs::metadata(self.path()).map_err(ImageResourceError::io)?;
        let current = (
            metadata.len(),
            modified_ns(&metadata),
            metadata.dev(),
            metadata.ino(),
        );
        let expected = (
            self.fingerprint.size_bytes,
            self.fingerprint.modified_ns,
            self.fingerprint.device,
            self.fingerprint.inode,
        );
        if current != expected {
            return Err(ImageResourceError::SourceChanged);
        }
        Ok(())
    }
}

fn metadata_identity(metadata: &fs::Metadata) -> (u64, i128, u64, u64) {
    (
        metadata.len(),
        modified_ns(metadata),
        metadata.dev(),
        metadata.ino(),
    )
}

fn modified_ns(metadata: &fs::Metadata) -> i128 {
    i128::from(metadata.mtime()) * 1_000_000_000_i128 + i128::from(metadata.mtime_nsec())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreviewRequest {
    pub max_width: u32,
    pub max_height: u32,
}

impl PreviewRequest {
    pub fn new(max_width: u32, max_height: u32) -> Result<Self, ImageResourceError> {
        if max_width == 0 || max_height == 0 {
            return Err(ImageResourceError::InvalidRequest);
        }
        Ok(Self {
            max_width,
            max_height,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedResourceKind {
    Preview { level: u32 },
    Tile(TileCoordinate),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedResource {
    pub generation: AssetGeneration,
    pub cache_key: DerivedCacheKey,
    pub kind: DecodedResourceKind,
    pub width: u32,
    pub height: u32,
    pub bytes_per_row: u32,
    pub pixel_format: PixelFormat,
    pub pixels: Arc<[u8]>,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ImageResourceError {
    #[error("image decode failed: {0}")]
    Image(String),
    #[error("image resource request was cancelled")]
    Cancelled,
    #[error("image resource request is invalid")]
    InvalidRequest,
    #[error("decoded image resource is invalid")]
    InvalidResource,
    #[error("image resource exceeds a safe size limit")]
    LimitExceeded,
    #[error("image color conversion failed")]
    ColorConversion,
    #[error("image source is not a safe regular file")]
    UnsafeSource,
    #[error("image source changed after authorization")]
    SourceChanged,
    #[error("image cache root is unsafe")]
    UnsafeCacheRoot,
    #[error("image cache is unavailable")]
    CacheUnavailable,
    #[error("image cache failed: {0}")]
    Cache(String),
    #[error("image resource I/O failed: {0}")]
    Io(String),
}

impl ImageResourceError {
    pub(crate) fn io(error: io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<ImageError> for ImageResourceError {
    fn from(error: ImageError) -> Self {
        match error {
            ImageError::Cancelled => Self::Cancelled,
            ImageError::BudgetExceeded => Self::LimitExceeded,
            other => Self::Image(other.to_string()),
        }
    }
}

#[derive(Clone)]
pub struct MacImageResourceProvider {
    cache: MacImageTileCache,
    schema_version: u32,
    cancelled_through: Arc<Mutex<Option<AssetGeneration>>>,
    current_generation: Arc<AtomicU64>,
    latest_pressure_report: Arc<Mutex<Option<CacheReclaimReport>>>,
}

impl MacImageResourceProvider {
    pub fn new(cache_root: &Path, budget: MemoryBudget) -> Result<Self, ImageResourceError> {
        Self::with_cache_schema(cache_root, budget, super::CACHE_SCHEMA_VERSION)
    }

    pub fn with_cache_schema(
        cache_root: &Path,
        budget: MemoryBudget,
        schema_version: u32,
    ) -> Result<Self, ImageResourceError> {
        Ok(Self {
            cache: MacImageTileCache::new(cache_root, budget)?,
            schema_version,
            cancelled_through: Arc::new(Mutex::new(None)),
            current_generation: Arc::new(AtomicU64::new(0)),
            latest_pressure_report: Arc::new(Mutex::new(None)),
        })
    }

    pub fn probe(&self, source: &AuthorizedImageSource) -> Result<ImageProbe, ImageResourceError> {
        source.verify_unchanged()?;
        let image_source = open_image_source(source.path())?;
        let probe = probe_image_source(&image_source)?;
        source.verify_unchanged()?;
        Ok(probe)
    }

    pub fn request_preview(
        &self,
        source: &AuthorizedImageSource,
        generation: AssetGeneration,
        request: PreviewRequest,
    ) -> Result<DecodedResource, ImageResourceError> {
        self.begin_request(generation)?;
        let probe = self.probe(source)?;
        let (source_width, source_height) = oriented_dimensions(&probe);
        let (width, height) = fit_dimensions(
            source_width,
            source_height,
            request.max_width,
            request.max_height,
        )?;
        let level = level_for_dimensions(source_width, source_height, width, height);
        let key = DerivedCacheKey::new(
            source.fingerprint.clone(),
            &probe,
            level,
            DerivedRegion::Preview { width, height },
            self.schema_version,
        );
        let kind = DecodedResourceKind::Preview { level };
        if let Some(resource) =
            self.cache
                .load(&key, generation, kind, ResourcePriority::Visible)?
        {
            self.check_cancelled(generation)?;
            return Ok(resource);
        }

        self.check_cancelled(generation)?;
        let image_source = open_image_source(source.path())?;
        let image = thumbnail_from_image_source(&image_source, width.max(height))?;
        let pixels = normalize_to_bgra_srgb(&image)?;
        if (pixels.width, pixels.height) != (width, height) {
            return Err(ImageResourceError::InvalidResource);
        }
        source.verify_unchanged()?;
        self.check_cancelled(generation)?;
        let resource = resource_from_pixels(generation, key, kind, pixels);
        self.cache.store(&resource, ResourcePriority::Visible)?;
        if self.check_cancelled(generation).is_err() {
            self.cache.remove(&resource.cache_key)?;
            return Err(ImageResourceError::Cancelled);
        }
        Ok(resource)
    }

    pub fn request_tiles(
        &self,
        source: &AuthorizedImageSource,
        generation: AssetGeneration,
        plan: &ResourcePlan,
    ) -> Result<Vec<DecodedResource>, ImageResourceError> {
        self.begin_request(generation)?;
        validate_plan(plan)?;
        let probe = self.probe(source)?;
        let (source_width, source_height) = oriented_dimensions(&probe);
        let level_width = mip_dimension(source_width, plan.level);
        let level_height = mip_dimension(source_height, plan.level);
        let tile_size = match plan.strategy {
            TextureStrategy::SingleTexture => level_width.max(level_height),
            TextureStrategy::Tiled { tile_size } if tile_size > 0 => tile_size,
            TextureStrategy::Tiled { .. } => return Err(ImageResourceError::InvalidRequest),
        };

        let mut ordered = Vec::new();
        let mut seen = BTreeSet::new();
        for tile in plan.required_tiles.iter().copied() {
            if seen.insert(tile) {
                ordered.push((tile, ResourcePriority::Visible));
            }
        }
        for tile in plan.prefetch_tiles.iter().copied() {
            if seen.insert(tile) {
                ordered.push((tile, ResourcePriority::Prefetch));
            }
        }

        let mut resources = BTreeMap::new();
        let mut missing = Vec::new();
        for (tile, priority) in &ordered {
            validate_tile(*tile, plan.level, level_width, level_height, tile_size)?;
            let key = DerivedCacheKey::new(
                source.fingerprint.clone(),
                &probe,
                plan.level,
                DerivedRegion::Tile(*tile),
                self.schema_version,
            );
            let kind = DecodedResourceKind::Tile(*tile);
            if let Some(resource) = self.cache.load(&key, generation, kind, *priority)? {
                resources.insert(*tile, resource);
            } else {
                missing.push((*tile, *priority, key));
            }
        }

        if !missing.is_empty() {
            self.check_cancelled(generation)?;
            let image_source = open_image_source(source.path())?;
            let image = thumbnail_from_image_source(&image_source, level_width.max(level_height))?;
            let level_pixels = normalize_to_bgra_srgb(&image)?;
            if (level_pixels.width, level_pixels.height) != (level_width, level_height) {
                return Err(ImageResourceError::InvalidResource);
            }
            source.verify_unchanged()?;

            for (tile, priority, key) in missing {
                self.check_cancelled(generation)?;
                let pixels = crop_tile(&level_pixels, tile, tile_size)?;
                let resource =
                    resource_from_pixels(generation, key, DecodedResourceKind::Tile(tile), pixels);
                self.cache.store(&resource, priority)?;
                if self.check_cancelled(generation).is_err() {
                    self.cache.remove(&resource.cache_key)?;
                    return Err(ImageResourceError::Cancelled);
                }
                resources.insert(tile, resource);
            }
        }

        ordered
            .into_iter()
            .map(|(tile, _)| {
                resources
                    .remove(&tile)
                    .ok_or(ImageResourceError::InvalidResource)
            })
            .collect()
    }

    pub fn cancel_generation(&self, generation: AssetGeneration) {
        let mut cancelled = self
            .cancelled_through
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *cancelled = Some(cancelled.map_or(generation, |current| current.max(generation)));
    }

    pub fn cache_entry_path(&self, key: &DerivedCacheKey) -> PathBuf {
        self.cache.entry_path(key)
    }

    pub fn apply_memory_pressure(
        &self,
        pressure: PressureLevel,
        current_generation: AssetGeneration,
    ) -> Result<CacheReclaimReport, ImageResourceError> {
        let report = self.cache.apply_pressure(pressure, current_generation)?;
        *self
            .latest_pressure_report
            .lock()
            .map_err(|_| ImageResourceError::CacheUnavailable)? = Some(report);
        Ok(report)
    }

    pub fn memory_pressure_sink(&self) -> MemoryPressureSink {
        let cache = self.cache.clone();
        let current_generation = Arc::clone(&self.current_generation);
        let latest_report = Arc::clone(&self.latest_pressure_report);
        Arc::new(move |pressure| {
            let generation = AssetGeneration(current_generation.load(Ordering::Relaxed));
            if let Ok(report) = cache.apply_pressure(pressure, generation) {
                *latest_report
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(report);
            }
        })
    }

    pub fn latest_memory_pressure_report(
        &self,
    ) -> Result<Option<CacheReclaimReport>, ImageResourceError> {
        self.latest_pressure_report
            .lock()
            .map(|report| *report)
            .map_err(|_| ImageResourceError::CacheUnavailable)
    }

    fn begin_request(&self, generation: AssetGeneration) -> Result<(), ImageResourceError> {
        self.current_generation
            .fetch_max(generation.0, Ordering::Relaxed);
        self.check_cancelled(generation)
    }

    fn check_cancelled(&self, generation: AssetGeneration) -> Result<(), ImageResourceError> {
        let cancelled = self
            .cancelled_through
            .lock()
            .map_err(|_| ImageResourceError::CacheUnavailable)?;
        if cancelled.is_some_and(|cutoff| generation <= cutoff) {
            Err(ImageResourceError::Cancelled)
        } else {
            Ok(())
        }
    }
}

fn resource_from_pixels(
    generation: AssetGeneration,
    cache_key: DerivedCacheKey,
    kind: DecodedResourceKind,
    pixels: DecodedPixels,
) -> DecodedResource {
    DecodedResource {
        generation,
        cache_key,
        kind,
        width: pixels.width,
        height: pixels.height,
        bytes_per_row: pixels.bytes_per_row,
        pixel_format: pixels.pixel_format,
        pixels: pixels.pixels.into(),
    }
}

fn fit_dimensions(
    width: u32,
    height: u32,
    max_width: u32,
    max_height: u32,
) -> Result<(u32, u32), ImageResourceError> {
    if width == 0 || height == 0 || max_width == 0 || max_height == 0 {
        return Err(ImageResourceError::InvalidRequest);
    }
    if width <= max_width && height <= max_height {
        return Ok((width, height));
    }
    let (numerator, denominator) =
        if u64::from(max_width) * u64::from(height) <= u64::from(max_height) * u64::from(width) {
            (max_width, width)
        } else {
            (max_height, height)
        };
    let target_width = (u64::from(width) * u64::from(numerator) / u64::from(denominator)).max(1);
    let target_height = (u64::from(height) * u64::from(numerator) / u64::from(denominator)).max(1);
    Ok((
        u32::try_from(target_width).map_err(|_| ImageResourceError::LimitExceeded)?,
        u32::try_from(target_height).map_err(|_| ImageResourceError::LimitExceeded)?,
    ))
}

fn level_for_dimensions(
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
) -> u32 {
    let source_max = source_width.max(source_height);
    let target_max = target_width.max(target_height).max(1);
    let ratio = source_max / target_max;
    if ratio <= 1 {
        0
    } else {
        u32::BITS - 1 - ratio.leading_zeros()
    }
}

fn mip_dimension(dimension: u32, level: u32) -> u32 {
    if level >= u32::BITS {
        1
    } else {
        dimension.saturating_add((1_u32 << level).saturating_sub(1)) >> level
    }
    .max(1)
}

fn validate_plan(plan: &ResourcePlan) -> Result<(), ImageResourceError> {
    if plan
        .required_tiles
        .iter()
        .chain(&plan.prefetch_tiles)
        .any(|tile| tile.level != plan.level)
    {
        return Err(ImageResourceError::InvalidRequest);
    }
    Ok(())
}

fn validate_tile(
    tile: TileCoordinate,
    level: u32,
    width: u32,
    height: u32,
    tile_size: u32,
) -> Result<(), ImageResourceError> {
    if tile.level != level
        || tile.x.checked_mul(tile_size).is_none_or(|x| x >= width)
        || tile.y.checked_mul(tile_size).is_none_or(|y| y >= height)
    {
        return Err(ImageResourceError::InvalidRequest);
    }
    Ok(())
}

fn crop_tile(
    level: &DecodedPixels,
    tile: TileCoordinate,
    tile_size: u32,
) -> Result<DecodedPixels, ImageResourceError> {
    let x = tile
        .x
        .checked_mul(tile_size)
        .ok_or(ImageResourceError::LimitExceeded)?;
    let y = tile
        .y
        .checked_mul(tile_size)
        .ok_or(ImageResourceError::LimitExceeded)?;
    if x >= level.width || y >= level.height {
        return Err(ImageResourceError::InvalidRequest);
    }
    let width = tile_size.min(level.width - x);
    let height = tile_size.min(level.height - y);
    let bytes_per_row = width
        .checked_mul(4)
        .ok_or(ImageResourceError::LimitExceeded)?;
    let length = usize::try_from(u64::from(bytes_per_row) * u64::from(height))
        .map_err(|_| ImageResourceError::LimitExceeded)?;
    let mut pixels = vec![0_u8; length];
    let source_stride =
        usize::try_from(level.bytes_per_row).map_err(|_| ImageResourceError::LimitExceeded)?;
    let destination_stride =
        usize::try_from(bytes_per_row).map_err(|_| ImageResourceError::LimitExceeded)?;
    let source_x = usize::try_from(x)
        .map_err(|_| ImageResourceError::LimitExceeded)?
        .checked_mul(4)
        .ok_or(ImageResourceError::LimitExceeded)?;
    let source_y = usize::try_from(y).map_err(|_| ImageResourceError::LimitExceeded)?;
    for row in 0..usize::try_from(height).map_err(|_| ImageResourceError::LimitExceeded)? {
        let source_start = (source_y + row)
            .checked_mul(source_stride)
            .and_then(|offset| offset.checked_add(source_x))
            .ok_or(ImageResourceError::LimitExceeded)?;
        let destination_start = row * destination_stride;
        pixels[destination_start..destination_start + destination_stride]
            .copy_from_slice(&level.pixels[source_start..source_start + destination_stride]);
    }
    Ok(DecodedPixels {
        width,
        height,
        bytes_per_row,
        pixel_format: level.pixel_format,
        pixels,
    })
}
