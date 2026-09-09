use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::CGImage;
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

use super::color::{normalize_to_bgra_srgb, normalize_to_bgra_srgb_in_rect};
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
    /// Number of duplicated pixels on each non-image edge of a tile texture.
    /// Preview resources always use zero. The GPU uses this metadata to keep
    /// linear filtering inside the tile's padded sample rectangle.
    pub sample_border: u32,
    pub pixels: viewer_render_core::SharedPixels,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ImageResourceError {
    #[error("image memory admission failed: {0}")]
    MemoryAdmission(#[from] viewer_render_core::MemoryAdmissionError),
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
    memory: viewer_render_core::ImageMemoryCoordinator,
    cache: MacImageTileCache,
    budget: MemoryBudget,
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
        Self::with_memory_and_schema(
            cache_root,
            budget,
            viewer_render_core::ImageMemoryCoordinator::new(
                viewer_render_core::ImageMemoryPolicy::baseline_8gb(),
            ),
            schema_version,
        )
    }

    pub fn with_memory(
        cache_root: &Path,
        budget: MemoryBudget,
        memory: viewer_render_core::ImageMemoryCoordinator,
    ) -> Result<Self, ImageResourceError> {
        Self::with_memory_and_schema(cache_root, budget, memory, super::CACHE_SCHEMA_VERSION)
    }

    fn with_memory_and_schema(
        cache_root: &Path,
        budget: MemoryBudget,
        memory: viewer_render_core::ImageMemoryCoordinator,
        schema_version: u32,
    ) -> Result<Self, ImageResourceError> {
        Ok(Self {
            cache: MacImageTileCache::with_memory(cache_root, budget, memory.clone())?,
            memory,
            budget,
            schema_version,
            cancelled_through: Arc::new(Mutex::new(None)),
            current_generation: Arc::new(AtomicU64::new(0)),
            latest_pressure_report: Arc::new(Mutex::new(None)),
        })
    }

    pub fn memory(&self) -> &viewer_render_core::ImageMemoryCoordinator {
        &self.memory
    }

    pub fn probe(&self, source: &AuthorizedImageSource) -> Result<ImageProbe, ImageResourceError> {
        source.verify_unchanged()?;
        let image_source = open_image_source(source.path())?;
        let probe = probe_image_source(&image_source)?;
        source.verify_unchanged()?;
        Ok(probe)
    }

    pub const fn budget(&self) -> MemoryBudget {
        self.budget
    }

    pub fn request_preview(
        &self,
        source: &AuthorizedImageSource,
        generation: AssetGeneration,
        request: PreviewRequest,
    ) -> Result<DecodedResource, ImageResourceError> {
        self.probe_and_request_preview(source, generation, request)
            .map(|(_, resource)| resource)
    }

    pub fn probe_and_request_preview(
        &self,
        source: &AuthorizedImageSource,
        generation: AssetGeneration,
        request: PreviewRequest,
    ) -> Result<(ImageProbe, DecodedResource), ImageResourceError> {
        self.probe_and_request_preview_cancellable(source, generation, request, &|| false)
    }

    pub fn probe_and_request_preview_cancellable(
        &self,
        source: &AuthorizedImageSource,
        generation: AssetGeneration,
        request: PreviewRequest,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(ImageProbe, DecodedResource), ImageResourceError> {
        if cancelled() {
            return Err(ImageResourceError::Cancelled);
        }
        self.begin_request(generation)?;
        let probe = self.probe(source)?;
        let (source_width, source_height) = oriented_dimensions(&probe);
        let (width, height) = fit_dimensions(
            source_width,
            source_height,
            request.max_width,
            request.max_height,
        )?;
        // Image I/O's thumbnail and our normalized BGRA buffer coexist.
        if u64::from(width)
            .saturating_mul(u64::from(height))
            .saturating_mul(8)
            > self.budget.cpu_staging_bytes
        {
            return Err(ImageResourceError::LimitExceeded);
        }
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
            if cancelled() {
                return Err(ImageResourceError::Cancelled);
            }
            self.check_cancelled(generation)?;
            return Ok((probe, resource));
        }

        if cancelled() {
            return Err(ImageResourceError::Cancelled);
        }
        self.check_cancelled(generation)?;
        let image = AccountedNativeImage::decode(
            source,
            width,
            height,
            &self.memory,
            generation,
            u64::from(width) * u64::from(height) * 4,
        )?;
        if cancelled() {
            return Err(ImageResourceError::Cancelled);
        }
        let pixels = normalize_to_bgra_srgb_in_rect(
            &image.image,
            (width, height),
            CGRect::new(
                CGPoint::ZERO,
                CGSize::new(f64::from(width), f64::from(height)),
            ),
            &self.memory,
            generation,
        )?;
        if (pixels.width, pixels.height) != (width, height) {
            return Err(ImageResourceError::InvalidResource);
        }
        source.verify_unchanged()?;
        self.check_cancelled(generation)?;
        let resource = resource_from_pixels(generation, key, kind, pixels);
        self.cache.store(&resource, ResourcePriority::Visible)?;
        if cancelled() || self.check_cancelled(generation).is_err() {
            self.cache.remove(&resource.cache_key)?;
            return Err(ImageResourceError::Cancelled);
        }
        Ok((probe, resource))
    }

    pub fn request_tiles(
        &self,
        source: &AuthorizedImageSource,
        generation: AssetGeneration,
        plan: &ResourcePlan,
    ) -> Result<Vec<DecodedResource>, ImageResourceError> {
        self.request_tiles_cancellable(source, generation, plan, &|| false)
    }

    pub fn request_tiles_cancellable(
        &self,
        source: &AuthorizedImageSource,
        generation: AssetGeneration,
        plan: &ResourcePlan,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<DecodedResource>, ImageResourceError> {
        let mut resources = Vec::new();
        self.stream_tiles_cancellable(source, generation, plan, cancelled, &mut |resource| {
            resources.push(resource);
            Ok(())
        })?;
        Ok(resources)
    }

    /// Deliver each resource before producing the next. The sink can apply
    /// byte backpressure; it must return Cancelled when superseded/shut down.
    /// Memory admission never waits or retries a native decode internally.
    pub fn stream_tiles_cancellable(
        &self,
        source: &AuthorizedImageSource,
        generation: AssetGeneration,
        plan: &ResourcePlan,
        cancelled: &dyn Fn() -> bool,
        deliver: &mut dyn FnMut(DecodedResource) -> Result<(), ImageResourceError>,
    ) -> Result<(), ImageResourceError> {
        if cancelled() {
            return Err(ImageResourceError::Cancelled);
        }
        self.begin_request(generation)?;
        validate_plan(plan)?;
        if plan.required_tiles.is_empty() && plan.prefetch_tiles.is_empty() {
            return Ok(());
        }
        let probe = self.probe(source)?;
        let (source_width, source_height) = oriented_dimensions(&probe);
        let level_width = mip_dimension(source_width, plan.level);
        let level_height = mip_dimension(source_height, plan.level);
        let tile_size = match plan.strategy {
            TextureStrategy::SingleTexture => level_width.max(level_height),
            TextureStrategy::Tiled { tile_size } if tile_size > 0 => tile_size,
            TextureStrategy::Tiled { .. } => return Err(ImageResourceError::InvalidRequest),
        };
        let mut seen = BTreeSet::new();
        let ordered: Vec<_> = plan
            .required_tiles
            .iter()
            .map(|tile| (*tile, ResourcePriority::Visible))
            .chain(
                plan.prefetch_tiles
                    .iter()
                    .map(|tile| (*tile, ResourcePriority::Prefetch)),
            )
            .filter(|(tile, _)| seen.insert(*tile))
            .collect();
        let mut largest_output = 0;
        for (tile, _) in &ordered {
            validate_tile(*tile, plan.level, level_width, level_height, tile_size)?;
            let (width, height) = tile_dimensions(
                *tile,
                level_width,
                level_height,
                tile_size,
                viewer_render_core::DEFAULT_TILE_BORDER,
            )?;
            largest_output = largest_output.max(u64::from(width) * u64::from(height) * 4);
        }
        let mut native = None;
        for (tile, priority) in ordered {
            if cancelled() {
                return Err(ImageResourceError::Cancelled);
            }
            self.check_cancelled(generation)?;
            let expected_dimensions = tile_dimensions(
                tile,
                level_width,
                level_height,
                tile_size,
                viewer_render_core::DEFAULT_TILE_BORDER,
            )?;
            let key = DerivedCacheKey::new(
                source.fingerprint.clone(),
                &probe,
                plan.level,
                DerivedRegion::Tile(tile),
                self.schema_version,
            );
            let kind = DecodedResourceKind::Tile(tile);
            let resource = if let Some(hit) = self.cache.load_expected(
                &key,
                generation,
                kind,
                priority,
                Some(expected_dimensions),
            )? {
                hit
            } else {
                if native.is_none() {
                    let native_bytes = u64::from(level_width) * u64::from(level_height) * 4;
                    if native_bytes
                        .checked_add(largest_output)
                        .is_none_or(|peak| peak > self.budget.cpu_staging_bytes)
                    {
                        return Err(ImageResourceError::LimitExceeded);
                    }
                    native = Some(AccountedNativeImage::decode(
                        source,
                        level_width,
                        level_height,
                        &self.memory,
                        generation,
                        largest_output,
                    )?);
                }
                if cancelled() || self.check_cancelled(generation).is_err() {
                    return Err(ImageResourceError::Cancelled);
                }
                let image = &native.as_ref().expect("native decode initialized").image;
                let pixels = crop_tile(
                    image,
                    (level_width, level_height),
                    tile,
                    tile_size,
                    &self.memory,
                    generation,
                )?;
                let resource = resource_from_pixels(generation, key, kind, pixels);
                self.cache.store(&resource, priority)?;
                resource
            };
            if cancelled() || self.check_cancelled(generation).is_err() {
                return Err(ImageResourceError::Cancelled);
            }
            deliver(resource)?;
        }
        Ok(())
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
        sample_border: match kind {
            DecodedResourceKind::Preview { .. } => 0,
            DecodedResourceKind::Tile(_) => viewer_render_core::DEFAULT_TILE_BORDER,
        },
        pixels: pixels.pixels,
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

fn tile_dimensions(
    tile: TileCoordinate,
    level_width: u32,
    level_height: u32,
    tile_size: u32,
    border: u32,
) -> Result<(u32, u32), ImageResourceError> {
    let x = tile
        .x
        .checked_mul(tile_size)
        .ok_or(ImageResourceError::LimitExceeded)?;
    let y = tile
        .y
        .checked_mul(tile_size)
        .ok_or(ImageResourceError::LimitExceeded)?;
    if x >= level_width || y >= level_height {
        return Err(ImageResourceError::InvalidRequest);
    }
    let width = tile_size.min(level_width - x);
    let height = tile_size.min(level_height - y);
    let left = border.min(x);
    let top = border.min(y);
    let right = border.min(level_width.saturating_sub(x + width));
    let bottom = border.min(level_height.saturating_sub(y + height));
    Ok((width + left + right, height + top + bottom))
}

fn crop_tile(
    level: &CGImage,
    level_size: (u32, u32),
    tile: TileCoordinate,
    tile_size: u32,
    memory: &viewer_render_core::ImageMemoryCoordinator,
    generation: AssetGeneration,
) -> Result<DecodedPixels, ImageResourceError> {
    let x = tile
        .x
        .checked_mul(tile_size)
        .ok_or(ImageResourceError::LimitExceeded)?;
    let y = tile
        .y
        .checked_mul(tile_size)
        .ok_or(ImageResourceError::LimitExceeded)?;
    let (level_width, level_height) = level_size;
    if x >= level_width || y >= level_height {
        return Err(ImageResourceError::InvalidRequest);
    }
    let width = tile_size.min(level_width - x);
    let height = tile_size.min(level_height - y);
    let border = viewer_render_core::DEFAULT_TILE_BORDER;
    let left = border.min(x);
    let top = border.min(y);
    let right = border.min(level_width.saturating_sub(x + width));
    let bottom = border.min(level_height.saturating_sub(y + height));
    if (CGImage::width(Some(level)), CGImage::height(Some(level)))
        != (level_width as usize, level_height as usize)
    {
        return normalize_to_bgra_srgb_in_rect(
            level,
            (width + left + right, height + top + bottom),
            CGRect::new(
                // Core Graphics draw bounds are bottom-up; tile coordinates
                // and normalized pixel rows are top-down.
                CGPoint::new(
                    -f64::from(x - left),
                    -f64::from(level_height - y - height - bottom),
                ),
                CGSize::new(f64::from(level_width), f64::from(level_height)),
            ),
            memory,
            generation,
        );
    }
    let cropped = CGImage::with_image_in_rect(
        Some(level),
        CGRect::new(
            CGPoint::new(f64::from(x - left), f64::from(y - top)),
            CGSize::new(
                f64::from(width + left + right),
                f64::from(height + top + bottom),
            ),
        ),
    )
    .ok_or(ImageResourceError::InvalidResource)?;
    normalize_to_bgra_srgb(&cropped, memory, generation)
}

// Field order releases the CGImage/source (including cropped backing) before
// their reservations. Crops never escape the synchronous normalization call.
struct AccountedNativeImage {
    image: objc2_core_foundation::CFRetained<CGImage>,
    _backing: NativeBacking,
}

struct NativeBacking {
    _source: objc2_core_foundation::CFRetained<objc2_image_io::CGImageSource>,
    _native: viewer_render_core::MemoryLease,
    _opaque: viewer_render_core::MemoryLease,
}

impl AccountedNativeImage {
    fn decode(
        source: &AuthorizedImageSource,
        width: u32,
        height: u32,
        memory: &viewer_render_core::ImageMemoryCoordinator,
        generation: AssetGeneration,
        largest_output: u64,
    ) -> Result<Self, ImageResourceError> {
        use viewer_render_core::AllocationClass;
        // Metadata-only inspection precedes pixel admission. PNG can retain
        // 16-bit components in Image I/O's thumbnail; BGRA8 is only our OUTPUT.
        let image_source = open_image_source(source.path())?;
        let component_bytes = native_component_bytes(&image_source)?;
        // Reserve a one-pixel rounding envelope before Image I/O rasterizes.
        // Only the final BGRA output is required to match our planned size.
        let row = (u64::from(width) + 1)
            .checked_mul(4 * component_bytes)
            .and_then(|row| row.checked_add(255))
            .map(|row| row / 256 * 256)
            .ok_or(ImageResourceError::LimitExceeded)?;
        let envelope = row
            .checked_mul(u64::from(height) + 1)
            .ok_or(ImageResourceError::LimitExceeded)?;
        let minimum_peak = envelope
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(largest_output))
            .ok_or(ImageResourceError::LimitExceeded)?;
        if minimum_peak > memory.normal_limits().combined_bytes {
            return Err(viewer_render_core::MemoryAdmissionError::ExceedsPolicy.into());
        }
        let native = memory.try_reserve(AllocationClass::NativeDecode, envelope, generation)?;
        // One additional aligned native output envelope is a separately labeled
        // conservative decoder allowance, NOT measured Image I/O internal RAM.
        let opaque = memory.try_reserve(AllocationClass::OpaqueAllowance, envelope, generation)?;
        // Fail before calling Image I/O if this request cannot coexist with even
        // one output. No wait ever retains a mip that prevents its own output.
        drop(memory.try_reserve(AllocationClass::DecodedPixels, largest_output, generation)?);
        // Own the metadata source and leases in drop order BEFORE rasterizing,
        // so every decoder/validation error releases retained source backing
        // before releasing its accounting (not just the successful return).
        let backing = NativeBacking {
            _source: image_source,
            _native: native,
            _opaque: opaque,
        };
        let image = thumbnail_from_image_source(&backing._source, width.max(height))?;
        let actual_width = CGImage::width(Some(&image));
        let actual_height = CGImage::height(Some(&image));
        let actual_bytes = (CGImage::bytes_per_row(Some(&image)) as u64)
            .checked_mul(actual_height as u64)
            .ok_or(ImageResourceError::LimitExceeded)?;
        if actual_width == 0
            || actual_height == 0
            || actual_width.abs_diff(width as usize) > 1
            || actual_height.abs_diff(height as usize) > 1
            || actual_bytes > envelope
        {
            return Err(ImageResourceError::InvalidResource);
        }
        backing._native.commit()?;
        backing._opaque.commit()?;
        source.verify_unchanged()?;
        Ok(Self {
            image,
            _backing: backing,
        })
    }
}

fn native_component_bytes(
    source: &objc2_image_io::CGImageSource,
) -> Result<u64, ImageResourceError> {
    use objc2_core_foundation::{CFDictionary, CFNumber, CFString, CFType};
    // SAFETY: the validated source has index zero; properties are metadata only.
    let properties = unsafe { source.properties_at_index(0, None) }
        .ok_or(ImageResourceError::InvalidResource)?;
    // SAFETY: Image I/O property dictionaries have CFString keys and CF objects.
    let properties: &CFDictionary<CFString, CFType> = unsafe { properties.cast_unchecked() };
    // SAFETY: immutable documented Image I/O property key.
    let depth = properties
        .get(unsafe { objc2_image_io::kCGImagePropertyDepth })
        .and_then(|value| value.downcast::<CFNumber>().ok())
        .and_then(|value| value.as_i64());
    Ok(match depth {
        Some(1..=8) => 1,
        Some(9..=16) | None => 2,
        _ => 4,
    })
}
