#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TheaterBounds {
    pub width: f64,
    pub height: f64,
    pub scale_factor: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TheaterFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoDisplayGeometry {
    pub width: u32,
    pub height: u32,
    pub rotation_degrees: i32,
}

/// Returns the complete native AppKit theater viewport. Compact player chrome
/// overlays the theater from the webview, so it does not reserve native video
/// space or require browser measurement/IPC during window resizing.
pub fn theater_viewport_frame(bounds: TheaterBounds) -> Option<TheaterFrame> {
    if !valid_positive(bounds.width)
        || !valid_positive(bounds.height)
        || !valid_positive(bounds.scale_factor)
    {
        return None;
    }

    Some(TheaterFrame {
        x: 0.0,
        y: 0.0,
        width: align_to_backing_pixel(bounds.width, bounds.scale_factor),
        height: align_to_backing_pixel(bounds.height, bounds.scale_factor),
    })
}

pub fn contain_fit_frame(
    bounds: TheaterBounds,
    media: VideoDisplayGeometry,
) -> Option<TheaterFrame> {
    if !valid_positive(bounds.width)
        || !valid_positive(bounds.height)
        || !valid_positive(bounds.scale_factor)
        || media.width == 0
        || media.height == 0
    {
        return None;
    }

    let rotation = media.rotation_degrees.rem_euclid(360);
    let (media_width, media_height) = match rotation {
        0 | 180 => (f64::from(media.width), f64::from(media.height)),
        90 | 270 => (f64::from(media.height), f64::from(media.width)),
        _ => return None,
    };
    let fit_scale = (bounds.width / media_width).min(bounds.height / media_height);
    let width = align_to_backing_pixel(media_width * fit_scale, bounds.scale_factor);
    let height = align_to_backing_pixel(media_height * fit_scale, bounds.scale_factor);
    let x = align_to_backing_pixel((bounds.width - width) / 2.0, bounds.scale_factor);
    let y = align_to_backing_pixel((bounds.height - height) / 2.0, bounds.scale_factor);

    Some(TheaterFrame {
        x,
        y,
        width,
        height,
    })
}

fn valid_positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn align_to_backing_pixel(value: f64, scale_factor: f64) -> f64 {
    (value * scale_factor).round() / scale_factor
}
