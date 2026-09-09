use std::fs;

use viewer_platform_macos::image_render::{
    AuthorizedImageSource, CACHE_SCHEMA_VERSION, MacImageResourceProvider, MacImageTileCache,
    PreviewRequest, pressure_level_for_flags,
};
use viewer_render_core::{AssetGeneration, MemoryBudget, PressureLevel, ResourcePriority};
use viewer_test_support::image_fixtures::image_fixture;

fn source(name: &str) -> AuthorizedImageSource {
    AuthorizedImageSource::authorize_for_process(image_fixture(name)).unwrap()
}

#[test]
fn cache_key_changes_with_source_color_orientation_level_and_schema() {
    let root = tempfile::tempdir().unwrap();
    let standard = MacImageResourceProvider::with_cache_schema(
        root.path(),
        MemoryBudget::baseline_8gb(),
        CACHE_SCHEMA_VERSION,
    )
    .unwrap();
    let next_schema = MacImageResourceProvider::with_cache_schema(
        root.path(),
        MemoryBudget::baseline_8gb(),
        CACHE_SCHEMA_VERSION + 1,
    )
    .unwrap();
    let request = PreviewRequest::new(300, 300).unwrap();

    let srgb = standard
        .request_preview(&source("srgb.jpg"), AssetGeneration(1), request)
        .unwrap();
    let p3 = standard
        .request_preview(&source("p3.jpg"), AssetGeneration(2), request)
        .unwrap();
    let rotated = standard
        .request_preview(&source("rotated-6.jpg"), AssetGeneration(3), request)
        .unwrap();
    let schema_two = next_schema
        .request_preview(&source("srgb.jpg"), AssetGeneration(4), request)
        .unwrap();

    assert_ne!(srgb.cache_key, p3.cache_key);
    assert_ne!(srgb.cache_key, rotated.cache_key);
    assert_ne!(srgb.cache_key, schema_two.cache_key);
}

#[test]
fn corrupt_cache_entry_is_a_miss_and_is_replaced_atomically() {
    let root = tempfile::tempdir().unwrap();
    let provider =
        MacImageResourceProvider::new(root.path(), MemoryBudget::baseline_8gb()).unwrap();
    let source = source("srgb.jpg");
    let request = PreviewRequest::new(300, 300).unwrap();
    let first = provider
        .request_preview(&source, AssetGeneration(1), request)
        .unwrap();
    let path = provider.cache_entry_path(&first.cache_key);
    fs::write(&path, b"truncated-cache-entry").unwrap();

    let repaired = provider
        .request_preview(&source, AssetGeneration(2), request)
        .unwrap();
    assert_eq!(repaired.pixels, first.pixels);
    assert!(fs::metadata(&path).unwrap().len() > b"truncated-cache-entry".len() as u64);
    assert!(fs::read_dir(root.path()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".tmp-")
    }));
}

#[test]
fn cancellation_does_not_leave_cache_artifacts() {
    let root = tempfile::tempdir().unwrap();
    let provider =
        MacImageResourceProvider::new(root.path(), MemoryBudget::baseline_8gb()).unwrap();
    provider.cancel_generation(AssetGeneration(5));
    let result = provider.request_preview(
        &source("alpha.png"),
        AssetGeneration(5),
        PreviewRequest::new(200, 200).unwrap(),
    );

    assert!(result.is_err());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn tiny_cache_with_impossible_payload_is_a_miss_without_allocation_panic() {
    let root = tempfile::tempdir().unwrap();
    let provider =
        MacImageResourceProvider::new(root.path(), MemoryBudget::baseline_8gb()).unwrap();
    let source = source("srgb.jpg");
    let request = PreviewRequest::new(300, 300).unwrap();
    let first = provider
        .request_preview(&source, AssetGeneration(1), request)
        .unwrap();
    let mut header = b"VWBGRA01".to_vec();
    let width = u32::MAX / 4;
    let row = width * 4;
    for value in [width, u32::MAX, row] {
        header.extend_from_slice(&value.to_le_bytes());
    }
    // Larger than isize::MAX: the old reader panics at capacity validation BEFORE
    // any allocation. This regression must never try a plausible multi-GB Vec.
    header.extend_from_slice(&(u64::from(row) * u64::from(u32::MAX)).to_le_bytes());
    fs::write(provider.cache_entry_path(&first.cache_key), header).unwrap();
    let repaired = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        provider.request_preview(&source, AssetGeneration(2), request)
    }));
    assert!(repaired.is_ok(), "untrusted cache header must not panic");
    assert_eq!(repaired.unwrap().unwrap().pixels, first.pixels);
}

#[test]
fn cache_preview_dimensions_must_match_content_key() {
    let root = tempfile::tempdir().unwrap();
    let provider =
        MacImageResourceProvider::new(root.path(), MemoryBudget::baseline_8gb()).unwrap();
    let source = source("srgb.jpg");
    let request = PreviewRequest::new(300, 300).unwrap();
    let first = provider
        .request_preview(&source, AssetGeneration(1), request)
        .unwrap();
    let path = provider.cache_entry_path(&first.cache_key);
    let mut entry = fs::read(&path).unwrap();
    entry[8..12].copy_from_slice(&(first.width / 2).to_le_bytes());
    entry[12..16].copy_from_slice(&(first.height * 2).to_le_bytes());
    entry[16..20].copy_from_slice(&(first.bytes_per_row / 2).to_le_bytes());
    fs::write(path, entry).unwrap();
    let repaired = provider
        .request_preview(&source, AssetGeneration(2), request)
        .unwrap();
    assert_eq!(
        (repaired.width, repaired.height),
        (first.width, first.height)
    );
    assert_eq!(repaired.pixels, first.pixels);
}

#[test]
fn cache_read_is_bounded_by_configured_cpu_capacity_before_allocation() {
    let root = tempfile::tempdir().unwrap();
    let provider =
        MacImageResourceProvider::new(root.path(), MemoryBudget::baseline_8gb()).unwrap();
    let first = provider
        .request_preview(
            &source("srgb.jpg"),
            AssetGeneration(1),
            PreviewRequest::new(300, 300).unwrap(),
        )
        .unwrap();
    let cache = MacImageTileCache::new(
        root.path(),
        MemoryBudget::new(1024, 1024, 1024 * 1024).unwrap(),
    )
    .unwrap();
    assert!(
        cache
            .load(
                &first.cache_key,
                AssetGeneration(2),
                first.kind,
                ResourcePriority::Visible
            )
            .unwrap()
            .is_none()
    );
}

#[test]
fn cache_exact_bound_is_readable_but_truncated_trailing_and_nonregular_entries_miss() {
    let root = tempfile::tempdir().unwrap();
    let provider =
        MacImageResourceProvider::new(root.path(), MemoryBudget::baseline_8gb()).unwrap();
    let first = provider
        .request_preview(
            &source("alpha.png"),
            AssetGeneration(1),
            PreviewRequest::new(32, 32).unwrap(),
        )
        .unwrap();
    let path = provider.cache_entry_path(&first.cache_key);
    let original = fs::read(&path).unwrap();
    let cache = MacImageTileCache::new(
        root.path(),
        MemoryBudget::new(first.pixels.len() as u64, 1, original.len() as u64 + 1).unwrap(),
    )
    .unwrap();
    let load = || {
        cache
            .load(
                &first.cache_key,
                AssetGeneration(2),
                first.kind,
                ResourcePriority::Visible,
            )
            .unwrap()
    };
    assert_eq!(load().unwrap().pixels, first.pixels);
    fs::write(&path, &original[..original.len() - 1]).unwrap();
    assert!(load().is_none());
    let mut trailing = original.clone();
    trailing.push(0);
    fs::write(&path, trailing).unwrap();
    assert!(load().is_none());
    fs::create_dir(&path).unwrap();
    assert!(load().is_none());
}

#[test]
fn pressure_reclaims_rebuildable_old_generation_files_and_reports_bytes() {
    let root = tempfile::tempdir().unwrap();
    let provider =
        MacImageResourceProvider::new(root.path(), MemoryBudget::baseline_8gb()).unwrap();
    let preview = provider
        .request_preview(
            &source("srgb.jpg"),
            AssetGeneration(1),
            PreviewRequest::new(300, 300).unwrap(),
        )
        .unwrap();
    let path = provider.cache_entry_path(&preview.cache_key);
    assert!(path.exists());

    let report = provider
        .apply_memory_pressure(PressureLevel::Warning, AssetGeneration(2))
        .unwrap();
    assert!(report.before.disk_bytes > 0);
    assert_eq!(report.after.disk_bytes, 0);
    assert_eq!(report.reclaimed_entries, 1);
    assert_eq!(
        provider.latest_memory_pressure_report().unwrap(),
        Some(report)
    );
    assert!(!path.exists());
}

#[test]
fn macos_pressure_flags_map_critical_before_warning_and_normal() {
    assert_eq!(pressure_level_for_flags(0x1), Some(PressureLevel::Normal));
    assert_eq!(pressure_level_for_flags(0x2), Some(PressureLevel::Warning));
    assert_eq!(pressure_level_for_flags(0x4), Some(PressureLevel::Critical));
    assert_eq!(pressure_level_for_flags(0x6), Some(PressureLevel::Critical));
    // OR-coalesced flags are observations, never an ordered transition history.
    assert_eq!(pressure_level_for_flags(0x5), Some(PressureLevel::Critical));
    assert_eq!(pressure_level_for_flags(0x3), Some(PressureLevel::Warning));
    assert_eq!(pressure_level_for_flags(0), None);
    assert_eq!(pressure_level_for_flags(0x8), None);
    assert_eq!(pressure_level_for_flags(0x9), None);
}
