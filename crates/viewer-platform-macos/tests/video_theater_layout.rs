use viewer_platform_macos::video::{
    THEATER_BOTTOM_INSPECTOR_HEIGHT, THEATER_TOP_COMMAND_BAR_HEIGHT, TheaterBounds, TheaterFrame,
    VideoDisplayGeometry, contain_fit_frame, theater_viewport_frame,
};

#[test]
fn reserves_native_top_and_bottom_chrome_before_video_aspect_fit() {
    assert_eq!(THEATER_TOP_COMMAND_BAR_HEIGHT, 50.0);
    assert_eq!(THEATER_BOTTOM_INSPECTOR_HEIGHT, 88.0);
    assert_eq!(
        theater_viewport_frame(TheaterBounds {
            width: 1_200.0,
            height: 800.0,
            scale_factor: 2.0,
        }),
        Some(TheaterFrame {
            x: 0.0,
            y: 88.0,
            width: 1_200.0,
            height: 662.0,
        })
    );
}

#[test]
fn contains_landscape_video_inside_a_taller_theater() {
    assert_eq!(
        contain_fit_frame(
            TheaterBounds {
                width: 1_200.0,
                height: 800.0,
                scale_factor: 2.0,
            },
            VideoDisplayGeometry {
                width: 1_920,
                height: 1_080,
                rotation_degrees: 0,
            },
        ),
        Some(TheaterFrame {
            x: 0.0,
            y: 62.5,
            width: 1_200.0,
            height: 675.0,
        })
    );
}

#[test]
fn centers_portrait_and_rotated_video_without_cropping() {
    let expected = Some(TheaterFrame {
        x: 375.0,
        y: 0.0,
        width: 450.0,
        height: 800.0,
    });
    let bounds = TheaterBounds {
        width: 1_200.0,
        height: 800.0,
        scale_factor: 2.0,
    };

    assert_eq!(
        contain_fit_frame(
            bounds,
            VideoDisplayGeometry {
                width: 1_080,
                height: 1_920,
                rotation_degrees: 0,
            },
        ),
        expected
    );
    assert_eq!(
        contain_fit_frame(
            bounds,
            VideoDisplayGeometry {
                width: 1_920,
                height: 1_080,
                rotation_degrees: 90,
            },
        ),
        expected
    );
    assert_eq!(
        contain_fit_frame(
            bounds,
            VideoDisplayGeometry {
                width: 1_920,
                height: 1_080,
                rotation_degrees: 270,
            },
        ),
        expected
    );
}

#[test]
fn aligns_fractional_fit_to_backing_pixels() {
    assert_eq!(
        contain_fit_frame(
            TheaterBounds {
                width: 1_001.0,
                height: 701.0,
                scale_factor: 2.0,
            },
            VideoDisplayGeometry {
                width: 1_920,
                height: 1_080,
                rotation_degrees: 0,
            },
        ),
        Some(TheaterFrame {
            x: 0.0,
            y: 69.0,
            width: 1_001.0,
            height: 563.0,
        })
    );
}

#[test]
fn rejects_missing_media_or_invalid_theater_geometry() {
    let media = VideoDisplayGeometry {
        width: 1_920,
        height: 1_080,
        rotation_degrees: 0,
    };
    let bounds = TheaterBounds {
        width: 1_200.0,
        height: 800.0,
        scale_factor: 2.0,
    };

    assert_eq!(
        contain_fit_frame(bounds, VideoDisplayGeometry { width: 0, ..media },),
        None
    );
    assert_eq!(
        contain_fit_frame(
            TheaterBounds {
                width: f64::NAN,
                ..bounds
            },
            media,
        ),
        None
    );
    assert_eq!(
        contain_fit_frame(
            TheaterBounds {
                scale_factor: 0.0,
                ..bounds
            },
            media,
        ),
        None
    );
}
