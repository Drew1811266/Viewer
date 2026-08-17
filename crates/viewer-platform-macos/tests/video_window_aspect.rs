use viewer_platform_macos::video::{
    AspectRect, AspectSize, VideoDisplayGeometry, best_fit_content_size, display_aspect,
    place_frame_inside_visible,
};

#[test]
fn rotation_changes_the_installed_display_aspect() {
    assert_eq!(
        display_aspect(VideoDisplayGeometry {
            width: 1920,
            height: 1080,
            rotation_degrees: 90,
        }),
        Some(1080.0 / 1920.0)
    );
    assert_eq!(
        display_aspect(VideoDisplayGeometry {
            width: 1920,
            height: 1080,
            rotation_degrees: -90,
        }),
        Some(1080.0 / 1920.0)
    );
    assert_eq!(
        display_aspect(VideoDisplayGeometry {
            width: 1920,
            height: 1080,
            rotation_degrees: 180,
        }),
        Some(1920.0 / 1080.0)
    );
}

#[test]
fn display_aspect_rejects_zero_dimensions_and_unsupported_rotation() {
    for media in [
        VideoDisplayGeometry {
            width: 0,
            height: 1080,
            rotation_degrees: 0,
        },
        VideoDisplayGeometry {
            width: 1920,
            height: 0,
            rotation_degrees: 0,
        },
        VideoDisplayGeometry {
            width: 1920,
            height: 1080,
            rotation_degrees: 45,
        },
    ] {
        assert_eq!(display_aspect(media), None);
    }
}

#[test]
fn best_fit_stays_inside_the_visible_screen() {
    let size = best_fit_content_size(
        AspectSize::new(1600.0, 900.0),
        AspectSize::new(1200.0, 800.0),
        16.0 / 9.0,
    )
    .expect("valid fit");

    assert_eq!(size, AspectSize::new(1200.0, 675.0));
    assert!(size.width <= 1200.0);
    assert!(size.height <= 800.0);
    assert!((size.width / size.height - 16.0 / 9.0).abs() < 0.000_001);
}

#[test]
fn best_fit_does_not_enlarge_the_current_content_bounds() {
    assert_eq!(
        best_fit_content_size(
            AspectSize::new(800.0, 600.0),
            AspectSize::new(1200.0, 800.0),
            16.0 / 9.0,
        ),
        Some(AspectSize::new(800.0, 450.0))
    );
}

#[test]
fn best_fit_rejects_zero_and_non_finite_inputs() {
    for (current, visible, aspect) in [
        (
            AspectSize::new(0.0, 900.0),
            AspectSize::new(1200.0, 800.0),
            16.0 / 9.0,
        ),
        (
            AspectSize::new(1600.0, 900.0),
            AspectSize::new(f64::INFINITY, 800.0),
            16.0 / 9.0,
        ),
        (
            AspectSize::new(1600.0, 900.0),
            AspectSize::new(1200.0, 800.0),
            f64::NAN,
        ),
        (
            AspectSize::new(1600.0, 900.0),
            AspectSize::new(1200.0, 800.0),
            0.0,
        ),
    ] {
        assert_eq!(best_fit_content_size(current, visible, aspect), None);
    }
}

#[test]
fn frame_placement_clamps_to_a_screen_with_a_negative_origin() {
    assert_eq!(
        place_frame_inside_visible(
            AspectRect::new(-1700.0, 100.0, 800.0, 600.0),
            AspectRect::new(-1440.0, 23.0, 1440.0, 877.0),
        ),
        Some(AspectRect::new(-1440.0, 100.0, 800.0, 600.0))
    );
}

#[test]
fn frame_placement_moves_a_partially_offscreen_frame_inside_the_work_area() {
    assert_eq!(
        place_frame_inside_visible(
            AspectRect::new(1300.0, 700.0, 800.0, 500.0),
            AspectRect::new(0.0, 25.0, 1440.0, 875.0),
        ),
        Some(AspectRect::new(640.0, 400.0, 800.0, 500.0))
    );
}

#[test]
fn frame_placement_respects_dock_and_menu_bar_work_area_limits() {
    assert_eq!(
        place_frame_inside_visible(
            AspectRect::new(0.0, 0.0, 1200.0, 750.0),
            AspectRect::new(80.0, 25.0, 1280.0, 775.0),
        ),
        Some(AspectRect::new(80.0, 25.0, 1200.0, 750.0))
    );
}

#[test]
fn frame_placement_accepts_finite_negative_origins() {
    assert_eq!(
        place_frame_inside_visible(
            AspectRect::new(-1200.0, -800.0, 800.0, 500.0),
            AspectRect::new(-1600.0, -900.0, 1600.0, 900.0),
        ),
        Some(AspectRect::new(-1200.0, -800.0, 800.0, 500.0))
    );
}

#[test]
fn frame_placement_rejects_invalid_native_rectangles() {
    let valid_frame = AspectRect::new(100.0, 100.0, 800.0, 500.0);
    let valid_visible = AspectRect::new(0.0, 25.0, 1440.0, 875.0);
    for invalid_frame in [
        AspectRect::new(f64::NAN, 100.0, 800.0, 500.0),
        AspectRect::new(100.0, f64::INFINITY, 800.0, 500.0),
        AspectRect::new(100.0, 100.0, 0.0, 500.0),
        AspectRect::new(100.0, 100.0, -800.0, 500.0),
        AspectRect::new(100.0, 100.0, 800.0, f64::INFINITY),
    ] {
        assert_eq!(
            place_frame_inside_visible(invalid_frame, valid_visible),
            None
        );
    }
    for invalid_visible in [
        AspectRect::new(f64::NEG_INFINITY, 25.0, 1440.0, 875.0),
        AspectRect::new(0.0, f64::NAN, 1440.0, 875.0),
        AspectRect::new(0.0, 25.0, 0.0, 875.0),
        AspectRect::new(0.0, 25.0, 1440.0, -875.0),
    ] {
        assert_eq!(
            place_frame_inside_visible(valid_frame, invalid_visible),
            None
        );
    }
}

#[test]
fn frame_placement_rejects_outer_frames_larger_than_the_visible_work_area() {
    assert_eq!(
        place_frame_inside_visible(
            AspectRect::new(0.0, 0.0, 1441.0, 800.0),
            AspectRect::new(0.0, 25.0, 1440.0, 875.0),
        ),
        None
    );
}
