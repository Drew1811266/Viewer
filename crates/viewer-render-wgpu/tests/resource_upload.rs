use viewer_render_core::{
    AssetGeneration, CameraState, LogicalSize, PhysicalSize, Rotation, SceneRevision, SourceSize,
    TransformSnapshot, ViewportLayout,
};
use viewer_render_wgpu::{
    DecodedResource, RendererDescriptor, ResourceGenerationGate, UploadDisposition, UploadError,
    UploadLayout, WgpuImageRenderer,
};

#[test]
fn upload_rows_are_padded_to_wgpu_copy_alignment() {
    let width = 65_u32;
    let height = 2_u32;
    let source = (0..width as usize * height as usize * 4)
        .map(|value| (value % 251) as u8)
        .collect::<Vec<_>>();

    let upload = UploadLayout::bgra8(width, height, &source).unwrap();

    assert_eq!(upload.unpadded_bytes_per_row(), 260);
    assert_eq!(upload.padded_bytes_per_row(), 512);
    assert_eq!(upload.bytes().len(), 1_024);
    assert_eq!(&upload.bytes()[..260], &source[..260]);
    assert!(upload.bytes()[260..512].iter().all(|byte| *byte == 0));
    assert_eq!(&upload.bytes()[512..772], &source[260..]);
}

#[test]
fn upload_rejects_incomplete_bgra_pixels() {
    assert_eq!(
        UploadLayout::bgra8(4, 4, &[0; 63]),
        Err(UploadError::PixelLength {
            expected: 64,
            actual: 63,
        })
    );
}

#[test]
fn refinement_preserves_alpha_and_retains_only_requested_resources() {
    if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
        return;
    }
    let logical = LogicalSize::new(16.0, 16.0).unwrap();
    let mut renderer = WgpuImageRenderer::new(
        RendererDescriptor::headless(
            logical,
            PhysicalSize {
                width: 16,
                height: 16,
            },
            1.0,
        )
        .unwrap(),
    )
    .unwrap();
    let source_size = SourceSize::new(2, 2).unwrap();
    renderer
        .set_transform(
            TransformSnapshot::new(
                source_size,
                ViewportLayout::new(logical, 1.0, 1.0).unwrap(),
                CameraState::fit(Rotation::Deg0),
            )
            .unwrap(),
        )
        .unwrap();
    renderer
        .upload_resource(DecodedResource::WholeImage {
            generation: AssetGeneration(1),
            level: 1,
            source_size,
            width: 1,
            height: 1,
            pixels: viewer_render_core::SharedPixels::try_copy_from_slice(
                &viewer_render_core::ImageMemoryCoordinator::new(
                    viewer_render_core::ImageMemoryPolicy::baseline_8gb(),
                ),
                AssetGeneration(1),
                &[0, 0, 128, 128],
            )
            .unwrap(),
        })
        .unwrap();
    let request = renderer.on_display_tick(1).unwrap();
    let (_, before) = renderer.render_headless_capture(request).unwrap();
    renderer
        .upload_resource(DecodedResource::Tile {
            generation: AssetGeneration(1),
            tile: viewer_render_core::TileCoordinate {
                level: 0,
                x: 0,
                y: 0,
            },
            tile_size: 512,
            sample_border: 0,
            source_size,
            width: 2,
            height: 2,
            pixels: viewer_render_core::SharedPixels::try_copy_from_slice(
                &viewer_render_core::ImageMemoryCoordinator::new(
                    viewer_render_core::ImageMemoryPolicy::baseline_8gb(),
                ),
                AssetGeneration(1),
                &[0, 0, 128, 128].repeat(4),
            )
            .unwrap(),
        })
        .unwrap();
    let request = renderer.on_display_tick(2).unwrap();
    let (_, refined) = renderer.render_headless_capture(request).unwrap();
    assert_eq!(
        before, refined,
        "a translucent refinement must replace its coverage without blending the same image twice"
    );
    renderer.retain_resources(&[viewer_render_wgpu::ResourceKey::WholeImage { level: 1 }]);
    assert_eq!(renderer.retained_scene_resources().image_handles().len(), 1);
    assert_eq!(renderer.gpu_resource_bytes(), 4);
}

#[test]
fn old_generation_completion_is_discarded_without_reopening_it() {
    let mut gate = ResourceGenerationGate::new(AssetGeneration(7));

    assert_eq!(
        gate.admit(AssetGeneration(6)),
        UploadDisposition::DiscardedStale
    );
    assert_eq!(gate.current(), AssetGeneration(7));
    assert_eq!(
        gate.admit(AssetGeneration(8)),
        UploadDisposition::Advanced {
            previous: AssetGeneration(7)
        }
    );
    assert_eq!(
        gate.admit(AssetGeneration(7)),
        UploadDisposition::DiscardedStale
    );
}

#[test]
fn beginning_a_generation_drops_previous_image_resources_before_decode_completes() {
    if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() || !cfg!(target_os = "macos") {
        return;
    }
    let logical = LogicalSize::new(64.0, 64.0).unwrap();
    let physical = PhysicalSize {
        width: 64,
        height: 64,
    };
    let mut renderer =
        WgpuImageRenderer::new(RendererDescriptor::headless(logical, physical, 1.0).unwrap())
            .unwrap();
    let source_size = SourceSize::new(1, 1).unwrap();
    renderer
        .upload_resource(DecodedResource::WholeImage {
            generation: AssetGeneration(1),
            level: 0,
            source_size,
            width: 1,
            height: 1,
            pixels: viewer_render_core::SharedPixels::try_copy_from_slice(
                &viewer_render_core::ImageMemoryCoordinator::new(
                    viewer_render_core::ImageMemoryPolicy::baseline_8gb(),
                ),
                AssetGeneration(1),
                &[0, 0, 255, 255],
            )
            .unwrap(),
        })
        .unwrap();

    renderer.begin_asset_generation(AssetGeneration(2)).unwrap();

    assert_eq!(renderer.gpu_resource_bytes(), 0);
    assert!(
        renderer
            .retained_scene_resources()
            .image_handles()
            .is_empty()
    );
}

#[test]
fn metal_upload_and_headless_draw_smoke_test_is_explicitly_opt_in() {
    if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() || !cfg!(target_os = "macos") {
        return;
    }
    let logical = LogicalSize::new(64.0, 64.0).unwrap();
    let physical = PhysicalSize {
        width: 64,
        height: 64,
    };
    let mut renderer =
        WgpuImageRenderer::new(RendererDescriptor::headless(logical, physical, 1.0).unwrap())
            .unwrap();
    let source_size = SourceSize::new(1, 1).unwrap();
    renderer
        .set_transform(
            TransformSnapshot::new(
                source_size,
                ViewportLayout::new(logical, 1.0, 1.0).unwrap(),
                CameraState::fit(Rotation::Deg0),
            )
            .unwrap(),
        )
        .unwrap();
    let first_handle = renderer
        .upload_resource(DecodedResource::WholeImage {
            generation: AssetGeneration(1),
            level: 0,
            source_size,
            width: 1,
            height: 1,
            pixels: viewer_render_core::SharedPixels::try_copy_from_slice(
                &viewer_render_core::ImageMemoryCoordinator::new(
                    viewer_render_core::ImageMemoryPolicy::baseline_8gb(),
                ),
                AssetGeneration(1),
                &[0, 0, 255, 255],
            )
            .unwrap(),
        })
        .unwrap();
    let replacement_handle = renderer
        .upload_resource(DecodedResource::WholeImage {
            generation: AssetGeneration(1),
            level: 0,
            source_size,
            width: 1,
            height: 1,
            pixels: viewer_render_core::SharedPixels::try_copy_from_slice(
                &viewer_render_core::ImageMemoryCoordinator::new(
                    viewer_render_core::ImageMemoryPolicy::baseline_8gb(),
                ),
                AssetGeneration(1),
                &[0, 255, 0, 255],
            )
            .unwrap(),
        })
        .unwrap();
    assert_ne!(
        replacement_handle, first_handle,
        "replacement tickets must not report old content ready"
    );
    assert_eq!(renderer.gpu_resource_bytes(), 4);

    renderer
        .upload_resource(DecodedResource::WholeImage {
            generation: AssetGeneration(2),
            level: 0,
            source_size,
            width: 1,
            height: 1,
            pixels: viewer_render_core::SharedPixels::try_copy_from_slice(
                &viewer_render_core::ImageMemoryCoordinator::new(
                    viewer_render_core::ImageMemoryPolicy::baseline_8gb(),
                ),
                AssetGeneration(1),
                &[255, 0, 0, 255],
            )
            .unwrap(),
        })
        .unwrap();
    assert!(matches!(
        renderer.upload_resource(DecodedResource::WholeImage {
            generation: AssetGeneration(1),
            level: 0,
            source_size,
            width: 1,
            height: 1,
            pixels: viewer_render_core::SharedPixels::try_copy_from_slice(
                &viewer_render_core::ImageMemoryCoordinator::new(
                    viewer_render_core::ImageMemoryPolicy::baseline_8gb()
                ),
                AssetGeneration(1),
                &[0, 0, 255, 255]
            )
            .unwrap(),
        }),
        Err(viewer_render_wgpu::RenderError::Upload(
            UploadError::StaleGeneration
        ))
    ));

    let request = renderer
        .frame_state()
        .take_request()
        .expect("camera and upload must dirty a frame");
    let receipt = renderer.render(request).unwrap();

    assert_eq!(receipt.scene_revision, SceneRevision(0));
    assert_eq!(receipt.gpu_resource_bytes, 4);
    assert!(!receipt.presented);
}

#[test]
fn metal_draw_preserves_top_to_bottom_pixel_order() {
    if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() || !cfg!(target_os = "macos") {
        return;
    }
    let logical = LogicalSize::new(64.0, 64.0).unwrap();
    let source_size = SourceSize::new(1, 2).unwrap();
    let mut renderer = WgpuImageRenderer::new(
        RendererDescriptor::headless(
            logical,
            PhysicalSize {
                width: 64,
                height: 64,
            },
            1.0,
        )
        .unwrap(),
    )
    .unwrap();
    renderer
        .set_transform(
            TransformSnapshot::new(
                source_size,
                ViewportLayout::new(logical, 1.0, 1.0).unwrap(),
                CameraState::fit(Rotation::Deg0),
            )
            .unwrap(),
        )
        .unwrap();
    renderer
        .upload_resource(DecodedResource::WholeImage {
            generation: AssetGeneration(1),
            level: 0,
            source_size,
            width: 1,
            height: 2,
            // Decoded resources are top-row first: red above blue.
            pixels: viewer_render_core::SharedPixels::try_copy_from_slice(
                &viewer_render_core::ImageMemoryCoordinator::new(
                    viewer_render_core::ImageMemoryPolicy::baseline_8gb(),
                ),
                AssetGeneration(1),
                &[0, 0, 255, 255, 255, 0, 0, 255],
            )
            .unwrap(),
        })
        .unwrap();

    let request = renderer.frame_state().take_request().unwrap();
    let (_, pixels) = renderer.render_headless_capture(request).unwrap();
    let pixel = |x: usize, y: usize| &pixels[((y * 64 + x) * 4)..((y * 64 + x) * 4 + 4)];
    let top = pixel(32, 8);
    let bottom = pixel(32, 56);

    assert!(
        top[2] > 200 && top[0] < 40,
        "expected red at the top, got BGRA {top:?}"
    );
    assert!(
        bottom[0] > 200 && bottom[2] < 40,
        "expected blue at the bottom, got BGRA {bottom:?}"
    );
}
