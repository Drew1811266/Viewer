use viewer_platform_macos::image_render::{
    AppKitFrame, SurfaceLayout, appkit_frame, backing_pixels,
};

#[test]
fn converts_dom_top_origin_layout_to_appkit_bottom_origin_frame() {
    let layout = SurfaceLayout {
        left: 72.0,
        top: 118.0,
        width: 960.0,
        height: 540.0,
        scale_factor: 2.0,
    };

    assert_eq!(
        appkit_frame(layout, 1_200.0),
        Some(AppKitFrame {
            x: 72.0,
            y: 542.0,
            width: 960.0,
            height: 540.0,
        })
    );
    assert_eq!(backing_pixels(layout), Some((1_920, 1_080)));
}

#[test]
fn live_resize_preserves_requested_geometry_while_appkit_clips_it() {
    let layout = SurfaceLayout {
        left: 10.0,
        top: 780.0,
        width: 640.0,
        height: 360.0,
        scale_factor: 1.0,
    };

    assert_eq!(
        appkit_frame(layout, 900.0),
        Some(AppKitFrame {
            x: 10.0,
            y: -240.0,
            width: 640.0,
            height: 360.0,
        })
    );
}

#[test]
fn display_scale_switch_only_changes_backing_pixels_not_logical_frame() {
    let standard = SurfaceLayout {
        left: 24.0,
        top: 40.0,
        width: 800.0,
        height: 600.0,
        scale_factor: 1.0,
    };
    let retina = SurfaceLayout {
        scale_factor: 2.0,
        ..standard
    };

    assert_eq!(appkit_frame(standard, 900.0), appkit_frame(retina, 900.0));
    assert_eq!(backing_pixels(standard), Some((800, 600)));
    assert_eq!(backing_pixels(retina), Some((1_600, 1_200)));
}

#[test]
fn invalid_layout_is_rejected_before_appkit_or_metal() {
    let invalid = SurfaceLayout {
        left: 0.0,
        top: 0.0,
        width: f64::NAN,
        height: 600.0,
        scale_factor: 2.0,
    };
    assert_eq!(appkit_frame(invalid, 900.0), None);
    assert_eq!(backing_pixels(invalid), None);

    let zero_scale = SurfaceLayout {
        width: 800.0,
        scale_factor: 0.0,
        ..invalid
    };
    assert_eq!(backing_pixels(zero_scale), None);
}
