use viewer_platform_macos::video::{
    AspectSize, VideoDisplayGeometry, best_fit_content_size, display_aspect,
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
