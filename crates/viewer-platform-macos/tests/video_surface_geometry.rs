use viewer_platform_macos::video::{
    AppKitFrame, MacVideoSurface, SurfaceError, SurfaceRect, appkit_frame, backing_pixels,
};

#[allow(dead_code)]
fn native_surface_api_is_encapsulated_by_the_tauri_window<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    surface: &MacVideoSurface,
    rect: SurfaceRect,
) -> Result<(), SurfaceError> {
    let mounted = MacVideoSurface::mount(window, rect)?;
    mounted.update_geometry(rect)?;
    surface.update_geometry(rect)
}

#[test]
fn converts_css_top_left_coordinates_to_appkit_bottom_left_coordinates() {
    let rect = SurfaceRect {
        x: 40.0,
        y: 120.0,
        width: 960.0,
        height: 540.0,
    };

    assert_eq!(
        appkit_frame(rect, 1_200.0),
        Some(AppKitFrame {
            x: 40.0,
            y: 540.0,
            width: 960.0,
            height: 540.0,
        })
    );
}

#[test]
fn converts_logical_surface_size_to_retina_backing_pixels() {
    assert_eq!(backing_pixels((960.0, 540.0), 2.0), Some((1920, 1080)));
}

#[test]
fn rejects_invalid_geometry_before_reaching_appkit_or_opengl() {
    assert_eq!(
        appkit_frame(
            SurfaceRect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 540.0,
            },
            1_200.0,
        ),
        None
    );
    assert_eq!(backing_pixels((960.0, 540.0), 0.0), None);
    assert_eq!(backing_pixels((f64::NAN, 540.0), 2.0), None);
}
