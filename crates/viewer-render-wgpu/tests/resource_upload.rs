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
            pixels: vec![0, 0, 255, 255],
        })
        .unwrap();
    let replacement_handle = renderer
        .upload_resource(DecodedResource::WholeImage {
            generation: AssetGeneration(1),
            level: 0,
            source_size,
            width: 1,
            height: 1,
            pixels: vec![0, 255, 0, 255],
        })
        .unwrap();
    assert_eq!(replacement_handle, first_handle);
    assert_eq!(renderer.gpu_resource_bytes(), 4);

    renderer
        .upload_resource(DecodedResource::WholeImage {
            generation: AssetGeneration(2),
            level: 0,
            source_size,
            width: 1,
            height: 1,
            pixels: vec![255, 0, 0, 255],
        })
        .unwrap();
    assert!(matches!(
        renderer.upload_resource(DecodedResource::WholeImage {
            generation: AssetGeneration(1),
            level: 0,
            source_size,
            width: 1,
            height: 1,
            pixels: vec![0, 0, 255, 255],
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
