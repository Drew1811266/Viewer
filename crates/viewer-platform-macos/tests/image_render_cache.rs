use std::fs;

use viewer_platform_macos::image_render::{
    AuthorizedImageSource, CACHE_SCHEMA_VERSION, MacImageResourceProvider, PreviewRequest,
    pressure_level_for_flags,
};
use viewer_render_core::{AssetGeneration, MemoryBudget, PressureLevel};
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
    assert_eq!(pressure_level_for_flags(0x1), PressureLevel::Normal);
    assert_eq!(pressure_level_for_flags(0x2), PressureLevel::Warning);
    assert_eq!(pressure_level_for_flags(0x4), PressureLevel::Critical);
    assert_eq!(pressure_level_for_flags(0x6), PressureLevel::Critical);
}
