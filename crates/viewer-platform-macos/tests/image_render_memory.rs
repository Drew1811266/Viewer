use std::sync::{Arc, Barrier};
use viewer_platform_macos::image_render::{
    AuthorizedImageSource, MacImageResourceProvider, PreviewRequest,
};
use viewer_render_core::{
    AllocationClass, AssetGeneration, ImageMemoryCoordinator, ImageMemoryPolicy, LiveMemoryLimits,
    MemoryBudget,
};
use viewer_test_support::image_fixtures::image_fixture;

#[test]
fn cancellation_after_native_thumbnail_prevents_pixel_allocation_and_cache_store() {
    use viewer_render_core::{ResourcePlan, TextureStrategy, TileCoordinate};
    let root = tempfile::tempdir().unwrap();
    let source = AuthorizedImageSource::authorize_for_process(image_fixture("srgb.jpg")).unwrap();
    let memory = ImageMemoryCoordinator::new(ImageMemoryPolicy::baseline_8gb());
    let provider = MacImageResourceProvider::with_memory(
        root.path(),
        MemoryBudget::baseline_8gb(),
        memory.clone(),
    )
    .unwrap();
    let normalized_at_cancellation = std::cell::Cell::new(None);
    let result = provider.stream_tiles_cancellable(
        &source,
        AssetGeneration(1),
        &ResourcePlan {
            strategy: TextureStrategy::Tiled { tile_size: 512 },
            level: 0,
            required_tiles: vec![TileCoordinate {
                level: 0,
                x: 0,
                y: 0,
            }],
            prefetch_tiles: vec![],
        },
        &|| {
            let snapshot = memory.snapshot();
            if snapshot.bytes_for_class(AllocationClass::NativeDecode) == 0 {
                return false;
            }
            normalized_at_cancellation.set(Some(
                snapshot.bytes_for_class(AllocationClass::DecodedPixels),
            ));
            true
        },
        &mut |_| panic!("cancelled thumbnail must never reach delivery"),
    );
    assert_eq!(
        result.unwrap_err(),
        viewer_platform_macos::image_render::ImageResourceError::Cancelled
    );
    assert_eq!(
        normalized_at_cancellation.get(),
        Some(0),
        "cancel immediately after thumbnail, before a normalized tile exists"
    );
    // Compound preflight briefly RESERVES one output before Image I/O. It is
    // not a pixel allocation and must not be confused with committed storage.
    // 1024x768 fixture: at most a 4352-byte aligned row times 769 rows
    // for each native/opaque envelope, plus the 513x513 bordered tile.
    // Allow tighter accounting, but reject a duplicate full normalized mip.
    assert!(memory.snapshot().peak_combined_bytes <= 7_746_052);
    assert_eq!(memory.snapshot().combined_bytes, 0);
}

#[test]
fn two_warm_provider_clients_share_admission_and_cancel_does_not_cross_sessions() {
    let root = tempfile::tempdir().unwrap();
    let source = AuthorizedImageSource::authorize_for_process(image_fixture("srgb.jpg")).unwrap();
    let request = PreviewRequest::new(32, 32).unwrap();
    let warmer = MacImageResourceProvider::new(root.path(), MemoryBudget::baseline_8gb()).unwrap();
    drop(
        warmer
            .request_preview(&source, AssetGeneration(1), request)
            .unwrap(),
    );
    let limits = LiveMemoryLimits {
        combined_bytes: 4000,
        gpu_bytes: 2000,
    };
    let memory =
        ImageMemoryCoordinator::new(ImageMemoryPolicy::new(limits, limits, limits).unwrap());
    let providers: Vec<_> = (0..2)
        .map(|_| {
            MacImageResourceProvider::with_memory(
                root.path(),
                MemoryBudget::baseline_8gb(),
                memory.clone(),
            )
            .unwrap()
        })
        .collect();
    let barrier = Arc::new(Barrier::new(3));
    std::thread::scope(|scope| {
        let workers: Vec<_> = providers
            .iter()
            .map(|provider| {
                let source = &source;
                let barrier = barrier.clone();
                scope.spawn(move || {
                    barrier.wait();
                    let pixels = provider.request_preview(source, AssetGeneration(1), request);
                    barrier.wait();
                    barrier.wait();
                    pixels.is_ok()
                })
            })
            .collect();
        barrier.wait();
        barrier.wait();
        let snapshot = memory.snapshot();
        barrier.wait();
        let successes = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .filter(|success| *success)
            .count();
        assert_eq!(successes, 1);
        assert_eq!(snapshot.combined_bytes, 32 * 24 * 4);
        assert_eq!(snapshot.bytes_for_class(AllocationClass::NativeDecode), 0);
        assert_eq!(
            snapshot.bytes_for_class(AllocationClass::OpaqueAllowance),
            0
        );
        assert_eq!(snapshot.peak_combined_bytes, snapshot.combined_bytes);
    });
    assert_eq!(memory.snapshot().combined_bytes, 0);
    providers[0].cancel_generation(AssetGeneration(9));
    let pixels = providers[1]
        .request_preview(&source, AssetGeneration(1), request)
        .unwrap();
    let clone = pixels.clone();
    assert_eq!(clone.pixels.as_ptr(), pixels.pixels.as_ptr());
    drop(pixels);
    assert_eq!(memory.snapshot().combined_bytes, 3072);
    drop(clone);
    assert_eq!(memory.snapshot().combined_bytes, 0);
}

#[test]
fn wrong_cached_tile_dimensions_are_missed_and_canonical_bytes_restored() {
    use viewer_render_core::{ResourcePlan, TextureStrategy, TileCoordinate};
    let root = tempfile::tempdir().unwrap();
    let source = AuthorizedImageSource::authorize_for_process(image_fixture("srgb.jpg")).unwrap();
    let provider =
        MacImageResourceProvider::new(root.path(), MemoryBudget::baseline_8gb()).unwrap();
    let plan = ResourcePlan {
        strategy: TextureStrategy::Tiled { tile_size: 256 },
        level: 0,
        required_tiles: vec![TileCoordinate {
            level: 0,
            x: 0,
            y: 2,
        }],
        prefetch_tiles: vec![],
    };
    let first = provider
        .request_tiles(&source, AssetGeneration(1), &plan)
        .unwrap()
        .remove(0);
    let path = provider.cache_entry_path(&first.cache_key);
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[8..12].copy_from_slice(&(first.width / 2).to_le_bytes());
    bytes[12..16].copy_from_slice(&(first.height * 2).to_le_bytes());
    bytes[16..20].copy_from_slice(&(first.bytes_per_row / 2).to_le_bytes());
    std::fs::write(path, bytes).unwrap();
    let validation_memory = ImageMemoryCoordinator::new(ImageMemoryPolicy::baseline_8gb());
    let cache = viewer_platform_macos::image_render::MacImageTileCache::with_memory(
        root.path(),
        MemoryBudget::baseline_8gb(),
        validation_memory.clone(),
    )
    .unwrap();
    assert!(
        cache
            .load_expected(
                &first.cache_key,
                AssetGeneration(2),
                first.kind,
                viewer_render_core::ResourcePriority::Visible,
                Some((first.width, first.height))
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(
        validation_memory.snapshot().peak_combined_bytes,
        0,
        "reject wrong tile bounds before storage allocation"
    );
    let repaired = provider
        .request_tiles(&source, AssetGeneration(2), &plan)
        .unwrap()
        .remove(0);
    assert_eq!(
        (repaired.width, repaired.height),
        (first.width, first.height)
    );
    assert_eq!(repaired.pixels, first.pixels);
}

#[test]
fn sixteen_bit_png_reserves_native_depth_before_decode_and_keeps_bgra8_output() {
    use objc2_core_foundation::{CFData, CFString, CFURL};
    use objc2_core_graphics::{
        CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGDataProvider, CGImage,
        CGImageAlphaInfo, CGImageByteOrderInfo,
    };
    use objc2_image_io::CGImageDestination;
    use viewer_render_core::{ResourcePlan, TextureStrategy, TileCoordinate};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("rgba16.png");
    let rgba = [0xff, 0xff, 0, 0, 0, 0, 0xff, 0xff].repeat(256 * 128);
    let data = CFData::from_bytes(&rgba);
    let provider = CGDataProvider::with_cf_data(Some(&data)).unwrap();
    let color = CGColorSpace::new_device_rgb().unwrap();
    // SAFETY: retained provider contains exactly 256x128 RGBA16 big-endian pixels.
    let image = unsafe {
        CGImage::new(
            256,
            128,
            16,
            64,
            256 * 8,
            Some(&color),
            CGBitmapInfo::from_bits_retain(
                CGImageAlphaInfo::PremultipliedLast.0 | CGImageByteOrderInfo::Order16Big.0,
            ),
            Some(&provider),
            std::ptr::null(),
            false,
            CGColorRenderingIntent::RenderingIntentDefault,
        )
    }
    .unwrap();
    let url = CFURL::from_file_path(&path).unwrap();
    let png = CFString::from_str("public.png");
    // SAFETY: local URL, PNG type, one retained image and no untyped options.
    let destination = unsafe { CGImageDestination::with_url(&url, &png, 1, None) }.unwrap();
    unsafe {
        destination.add_image(&image, None);
        assert!(destination.finalize());
    }
    assert_eq!(
        std::fs::read(&path).unwrap()[24],
        16,
        "fixture must really encode 16-bit PNG"
    );
    let source = AuthorizedImageSource::authorize_for_process(&path).unwrap();
    let memory = ImageMemoryCoordinator::new(ImageMemoryPolicy::baseline_8gb());
    let provider = MacImageResourceProvider::with_memory(
        &root.path().join("cache"),
        MemoryBudget::baseline_8gb(),
        memory.clone(),
    )
    .unwrap();
    let plan = ResourcePlan {
        strategy: TextureStrategy::Tiled { tile_size: 256 },
        level: 0,
        required_tiles: vec![TileCoordinate {
            level: 0,
            x: 0,
            y: 0,
        }],
        prefetch_tiles: vec![],
    };
    let mut native_during_delivery = 0;
    provider
        .stream_tiles_cancellable(
            &source,
            AssetGeneration(1),
            &plan,
            &|| false,
            &mut |resource| {
                native_during_delivery = memory
                    .snapshot()
                    .bytes_for_class(AllocationClass::NativeDecode);
                assert_eq!(resource.pixels.len(), 256 * 128 * 4);
                assert_eq!(&resource.pixels[..4], &[0, 0, 255, 255]);
                Ok(())
            },
        )
        .unwrap();
    assert!(native_during_delivery >= 256 * 128 * 8);
    assert_eq!(memory.snapshot().combined_bytes, 0);
}

#[test]
fn cold_stream_keeps_native_backing_charged_until_cancel_and_warm_stream_does_not() {
    use std::cell::Cell;
    use viewer_render_core::{ResourcePlan, TextureStrategy, TileCoordinate};
    let root = tempfile::tempdir().unwrap();
    let source = AuthorizedImageSource::authorize_for_process(image_fixture("srgb.jpg")).unwrap();
    let memory = ImageMemoryCoordinator::new(ImageMemoryPolicy::baseline_8gb());
    let provider = MacImageResourceProvider::with_memory(
        root.path(),
        MemoryBudget::baseline_8gb(),
        memory.clone(),
    )
    .unwrap();
    let mut plan = ResourcePlan {
        strategy: TextureStrategy::Tiled { tile_size: 512 },
        level: 0,
        required_tiles: vec![
            TileCoordinate {
                level: 0,
                x: 0,
                y: 0,
            },
            TileCoordinate {
                level: 0,
                x: 1,
                y: 0,
            },
        ],
        prefetch_tiles: vec![],
    };
    let cancelled = Cell::new(false);
    let mut held = None;
    let result = provider.stream_tiles_cancellable(
        &source,
        AssetGeneration(1),
        &plan,
        &|| cancelled.get(),
        &mut |resource| {
            assert!(
                memory
                    .snapshot()
                    .bytes_for_class(AllocationClass::NativeDecode)
                    >= 1024 * 768 * 4
            );
            assert!(
                memory
                    .snapshot()
                    .bytes_for_class(AllocationClass::OpaqueAllowance)
                    >= 1024 * 768 * 4
            );
            held = Some(resource);
            cancelled.set(true);
            Ok(())
        },
    );
    assert_eq!(
        result.unwrap_err(),
        viewer_platform_macos::image_render::ImageResourceError::Cancelled
    );
    assert_eq!(memory.snapshot().combined_bytes, 513 * 513 * 4);
    drop(held);
    assert_eq!(memory.snapshot().combined_bytes, 0);
    plan.required_tiles.truncate(1);
    provider
        .stream_tiles_cancellable(&source, AssetGeneration(2), &plan, &|| false, &mut |_| {
            assert_eq!(
                memory
                    .snapshot()
                    .bytes_for_class(AllocationClass::NativeDecode),
                0
            );
            assert_eq!(
                memory
                    .snapshot()
                    .bytes_for_class(AllocationClass::OpaqueAllowance),
                0
            );
            Ok(())
        })
        .unwrap();
    assert_eq!(memory.snapshot().combined_bytes, 0);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        provider.stream_tiles_cancellable(
            &source,
            AssetGeneration(3),
            &plan,
            &|| false,
            &mut |_| panic!("consumer cancelled"),
        )
    }));
    assert!(unwind.is_err());
    assert_eq!(memory.snapshot().combined_bytes, 0);
}

#[test]
fn native_plus_opaque_plus_one_output_excess_is_permanent_before_decode() {
    use viewer_render_core::{MemoryAdmissionError, ResourcePlan, TextureStrategy, TileCoordinate};
    let root = tempfile::tempdir().unwrap();
    let source = AuthorizedImageSource::authorize_for_process(image_fixture("srgb.jpg")).unwrap();
    let limits = LiveMemoryLimits {
        combined_bytes: 6 << 20,
        gpu_bytes: 3 << 20,
    };
    let memory =
        ImageMemoryCoordinator::new(ImageMemoryPolicy::new(limits, limits, limits).unwrap());
    let provider = MacImageResourceProvider::with_memory(
        root.path(),
        MemoryBudget::baseline_8gb(),
        memory.clone(),
    )
    .unwrap();
    let plan = ResourcePlan {
        strategy: TextureStrategy::Tiled { tile_size: 512 },
        level: 0,
        required_tiles: vec![TileCoordinate {
            level: 0,
            x: 0,
            y: 0,
        }],
        prefetch_tiles: vec![],
    };
    assert_eq!(
        provider
            .request_tiles(&source, AssetGeneration(1), &plan)
            .unwrap_err(),
        viewer_platform_macos::image_render::ImageResourceError::MemoryAdmission(
            MemoryAdmissionError::ExceedsPolicy
        )
    );
    assert_eq!(memory.snapshot().combined_bytes, 0);
}
