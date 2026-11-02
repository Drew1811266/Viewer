use super::encode::encode_png;
use super::image_io::{
    open_image_bytes, oriented_dimensions, probe_image_source, thumbnail_from_image_source,
};
use async_trait::async_trait;
use objc2::{ClassType, runtime::AnyObject};
use objc2_app_kit::{
    NSColor, NSFont, NSFontAttributeName, NSForegroundColorAttributeName, NSGraphicsContext,
    NSStringDrawing,
};
use objc2_core_foundation::{CFRetained, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextCreateImage, CGBlendMode, CGColorSpace, CGContext,
    CGImage, CGImageAlphaInfo, CGLineCap, CGLineJoin, CGMutablePath, CGPath,
};
use objc2_foundation::{NSDictionary, NSString};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use viewer_application::{
    MAX_REVIEW_ARTIFACT_BYTES, MAX_REVIEW_ARTIFACT_PIXELS, NumberedImageAnnotation,
    REVIEW_ANNOTATION_MAX_EDGE, ReviewArtifactAnnotation, ReviewArtifactError, ReviewArtifactPort,
    ReviewArtifactRenderRequest, ReviewRenderedArtifact,
};
use viewer_domain::review::{
    AnnotationColor, AnnotationStyle, AnnotationTool, AnnotationWidth, FeedbackAnchor,
    NormalizedPoint, NormalizedRect, ReviewMedia,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReviewArtifactRenderCheckpoint {
    Decode,
    Drawing,
    Encode,
    Return,
}

type CheckpointHook = Arc<dyn Fn(ReviewArtifactRenderCheckpoint) + Send + Sync>;

pub struct MacReviewArtifactRenderer {
    root: PathBuf,
    max_output_bytes: u64,
    checkpoint: CheckpointHook,
}

impl MacReviewArtifactRenderer {
    pub fn new(root: &Path) -> Result<Self, ReviewArtifactError> {
        Self::new_with_configuration(root, MAX_REVIEW_ARTIFACT_BYTES, Arc::new(|_| {}))
    }

    fn new_with_configuration(
        root: &Path,
        max_output_bytes: u64,
        checkpoint: CheckpointHook,
    ) -> Result<Self, ReviewArtifactError> {
        let metadata = fs::symlink_metadata(root).map_err(|_| ReviewArtifactError::Unavailable)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(ReviewArtifactError::UnsafeSource);
        }
        let root = fs::canonicalize(root).map_err(|_| ReviewArtifactError::Unavailable)?;
        if max_output_bytes == 0 || max_output_bytes > MAX_REVIEW_ARTIFACT_BYTES {
            return Err(ReviewArtifactError::LimitExceeded);
        }
        Ok(Self {
            root,
            max_output_bytes,
            checkpoint,
        })
    }

    #[cfg(test)]
    fn new_with_checkpoint_for_test(
        root: &Path,
        checkpoint: CheckpointHook,
    ) -> Result<Self, ReviewArtifactError> {
        Self::new_with_configuration(root, MAX_REVIEW_ARTIFACT_BYTES, checkpoint)
    }

    #[cfg(test)]
    fn new_with_output_limit_for_test(
        root: &Path,
        max_output_bytes: u64,
    ) -> Result<Self, ReviewArtifactError> {
        Self::new_with_configuration(root, max_output_bytes, Arc::new(|_| {}))
    }

    fn render_sync(
        root: PathBuf,
        max_output_bytes: u64,
        checkpoint: CheckpointHook,
        request: ReviewArtifactRenderRequest,
    ) -> Result<ReviewRenderedArtifact, ReviewArtifactError> {
        request.validate()?;
        let (expected_width, expected_height) = match request.expected_asset.media {
            ReviewMedia::Image {
                width: Some(width),
                height: Some(height),
            } if width > 0 && height > 0 => (width, height),
            _ => return Err(ReviewArtifactError::InvalidRequest),
        };
        check(
            &request,
            &checkpoint,
            ReviewArtifactRenderCheckpoint::Decode,
        )?;

        let mut source = open_verified_source(&request)?;
        let source_bytes = read_and_verify_source(&mut source, &request)?;
        let image_source = open_image_bytes(&source_bytes).map_err(map_image_error)?;
        let probe = probe_image_source(&image_source).map_err(map_image_error)?;
        if oriented_dimensions(&probe) != (expected_width, expected_height) {
            return Err(ReviewArtifactError::SourceChanged);
        }
        let thumbnail = thumbnail_from_image_source(&image_source, REVIEW_ANNOTATION_MAX_EDGE)
            .map_err(map_image_error)?;
        if oriented_dimensions(&probe_image_source(&image_source).map_err(map_image_error)?)
            != (expected_width, expected_height)
        {
            return Err(ReviewArtifactError::SourceChanged);
        }
        verify_source_after_decode(&source, &request)?;
        check(
            &request,
            &checkpoint,
            ReviewArtifactRenderCheckpoint::Drawing,
        )?;

        let width = u32::try_from(CGImage::width(Some(&thumbnail)))
            .map_err(|_| ReviewArtifactError::LimitExceeded)?;
        let height = u32::try_from(CGImage::height(Some(&thumbnail)))
            .map_err(|_| ReviewArtifactError::LimitExceeded)?;
        if width == 0
            || height == 0
            || width.max(height) > REVIEW_ANNOTATION_MAX_EDGE
            || u64::from(width)
                .checked_mul(u64::from(height))
                .is_none_or(|pixels| pixels > MAX_REVIEW_ARTIFACT_PIXELS)
        {
            return Err(ReviewArtifactError::LimitExceeded);
        }
        let rendered = draw_annotations(&thumbnail, width, height, &request.annotations)?;

        check(
            &request,
            &checkpoint,
            ReviewArtifactRenderCheckpoint::Encode,
        )?;
        let destination = root.join(format!(
            ".{}-{}.annotation.png",
            request.expected_asset.id,
            uuid::Uuid::new_v4()
        ));
        let mut owned = OwnedArtifact::new(destination);
        encode_png(&rendered, owned.path()).map_err(map_image_error)?;
        let (size_bytes, blake3) = inspect_output(owned.path(), max_output_bytes)?;
        check(
            &request,
            &checkpoint,
            ReviewArtifactRenderCheckpoint::Return,
        )?;

        Ok(ReviewRenderedArtifact {
            asset_version_id: request.expected_asset.id,
            temporary_path: owned.commit(),
            media_type: "image/png".to_owned(),
            width,
            height,
            size_bytes,
            blake3,
            annotations: request
                .annotations
                .into_iter()
                .map(|annotation| ReviewArtifactAnnotation {
                    ordinal: annotation.ordinal,
                    feedback_id: annotation.feedback_id,
                })
                .collect(),
        })
    }
}

#[async_trait]
impl ReviewArtifactPort for MacReviewArtifactRenderer {
    async fn render(
        &self,
        request: ReviewArtifactRenderRequest,
    ) -> Result<ReviewRenderedArtifact, ReviewArtifactError> {
        let root = self.root.clone();
        let max_output_bytes = self.max_output_bytes;
        let checkpoint = Arc::clone(&self.checkpoint);
        tokio::task::spawn_blocking(move || {
            Self::render_sync(root, max_output_bytes, checkpoint, request)
        })
        .await
        .map_err(|_| ReviewArtifactError::Unavailable)?
    }
}

fn check(
    request: &ReviewArtifactRenderRequest,
    hook: &CheckpointHook,
    checkpoint: ReviewArtifactRenderCheckpoint,
) -> Result<(), ReviewArtifactError> {
    hook(checkpoint);
    if request.cancellation.is_cancelled() {
        Err(ReviewArtifactError::Cancelled)
    } else {
        Ok(())
    }
}

fn open_verified_source(
    request: &ReviewArtifactRenderRequest,
) -> Result<File, ReviewArtifactError> {
    let path_metadata = fs::symlink_metadata(&request.source_path)
        .map_err(|_| ReviewArtifactError::UnsafeSource)?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err(ReviewArtifactError::UnsafeSource);
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(&request.source_path)
        .map_err(|_| ReviewArtifactError::UnsafeSource)?;
    verify_metadata(
        &file
            .metadata()
            .map_err(|_| ReviewArtifactError::Unavailable)?,
        request,
    )?;
    Ok(file)
}

fn read_and_verify_source(
    source: &mut File,
    request: &ReviewArtifactRenderRequest,
) -> Result<Vec<u8>, ReviewArtifactError> {
    let expected_len = usize::try_from(request.expected_asset.evidence.size_bytes)
        .map_err(|_| ReviewArtifactError::LimitExceeded)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected_len)
        .map_err(|_| ReviewArtifactError::LimitExceeded)?;
    source
        .take(request.expected_asset.evidence.size_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| ReviewArtifactError::Unavailable)?;
    let expected_digest = request
        .expected_asset
        .evidence
        .blake3
        .ok_or(ReviewArtifactError::InvalidRequest)?;
    if bytes.len() != expected_len || *blake3::hash(&bytes).as_bytes() != expected_digest {
        return Err(ReviewArtifactError::SourceChanged);
    }
    verify_source_after_decode(source, request)?;
    Ok(bytes)
}

fn verify_source_after_decode(
    source: &File,
    request: &ReviewArtifactRenderRequest,
) -> Result<(), ReviewArtifactError> {
    let handle_metadata = source
        .metadata()
        .map_err(|_| ReviewArtifactError::SourceChanged)?;
    verify_metadata(&handle_metadata, request)?;
    let path_metadata = fs::symlink_metadata(&request.source_path)
        .map_err(|_| ReviewArtifactError::SourceChanged)?;
    if path_metadata.file_type().is_symlink()
        || !path_metadata.is_file()
        || path_metadata.dev() != handle_metadata.dev()
        || path_metadata.ino() != handle_metadata.ino()
    {
        return Err(ReviewArtifactError::SourceChanged);
    }
    Ok(())
}

fn verify_metadata(
    metadata: &Metadata,
    request: &ReviewArtifactRenderRequest,
) -> Result<(), ReviewArtifactError> {
    if !metadata.is_file()
        || metadata.len() != request.expected_asset.evidence.size_bytes
        || modified_ns(metadata) != request.expected_asset.evidence.modified_ns
    {
        return Err(ReviewArtifactError::SourceChanged);
    }
    Ok(())
}

fn modified_ns(metadata: &Metadata) -> i128 {
    i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec())
}

pub(super) fn draw_annotations(
    source: &CGImage,
    width: u32,
    height: u32,
    annotations: &[NumberedImageAnnotation],
) -> Result<CFRetained<CGImage>, ReviewArtifactError> {
    let color_space = CGColorSpace::new_device_rgb().ok_or(ReviewArtifactError::Unavailable)?;
    let bytes_per_row = usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(4))
        .ok_or(ReviewArtifactError::LimitExceeded)?;
    // SAFETY: Passing a null data pointer asks Core Graphics to allocate the
    // bitmap. Dimensions and row bytes were checked above and the color space
    // and bitmap layout are compatible RGBA values.
    let context = unsafe {
        CGBitmapContextCreate(
            std::ptr::null_mut(),
            width as usize,
            height as usize,
            8,
            bytes_per_row,
            Some(&color_space),
            CGImageAlphaInfo::PremultipliedLast.0,
        )
    }
    .ok_or(ReviewArtifactError::Unavailable)?;
    let bounds = CGRect::new(
        CGPoint::ZERO,
        CGSize::new(f64::from(width), f64::from(height)),
    );
    // A plain bitmap context preserves CGImage row orientation. Only normalized
    // top-origin annotation coordinates need the y conversion below.
    CGContext::draw_image(Some(&context), bounds, Some(source));
    CGContext::set_line_cap(Some(&context), CGLineCap::Round);
    CGContext::set_line_join(Some(&context), CGLineJoin::Round);

    let short_edge = f64::from(width.min(height));
    let base_width = (short_edge * 0.006).clamp(3.0, 12.0);
    for annotation in annotations {
        let paint = EvidencePaint::from_style(annotation.style, base_width);
        let marker = match &annotation.anchor {
            FeedbackAnchor::ImageRect(rect) => {
                let region = source_rect(*rect, width, height);
                let radius = paint.corner_radius();
                let path =
                    unsafe { CGPath::with_rounded_rect(region, radius, radius, std::ptr::null()) };
                paint_path(&context, &path, &paint, true);
                source_point(
                    NormalizedPoint::new(rect.x(), rect.y())
                        .map_err(|_| ReviewArtifactError::InvalidRequest)?,
                    width,
                    height,
                )
            }
            FeedbackAnchor::ImageEllipse(rect) => {
                let region = source_rect(*rect, width, height);
                let path = unsafe { CGPath::with_ellipse_in_rect(region, std::ptr::null()) };
                paint_path(&context, &path, &paint, true);
                source_point(
                    NormalizedPoint::new(rect.x(), rect.y())
                        .map_err(|_| ReviewArtifactError::InvalidRequest)?,
                    width,
                    height,
                )
            }
            FeedbackAnchor::ImageStroke(stroke) => {
                let points = stroke
                    .points()
                    .iter()
                    .map(|point| source_point(*point, width, height))
                    .collect::<Vec<_>>();
                if points.len() < 2 {
                    return Err(ReviewArtifactError::InvalidRequest);
                }
                let path = polyline_path(&points, false);
                paint_path(&context, &path, &paint, false);
                points[0]
            }
            FeedbackAnchor::ImageLine(line) => {
                let tail = source_point(line.tail(), width, height);
                let head = source_point(line.head(), width, height);
                let path = polyline_path(&[tail, head], false);
                paint_path(&context, &path, &paint, false);
                tail
            }
            FeedbackAnchor::ImagePolygon(polygon) => {
                let points = polygon
                    .points()
                    .iter()
                    .map(|point| source_point(*point, width, height))
                    .collect::<Vec<_>>();
                if points.len() < 3 {
                    return Err(ReviewArtifactError::InvalidRequest);
                }
                let path = polyline_path(&points, true);
                paint_path(&context, &path, &paint, true);
                points[0]
            }
            FeedbackAnchor::ImageArrow(arrow) => {
                let tail = source_point(arrow.tail(), width, height);
                let head = source_point(arrow.head(), width, height);
                let path = arrow_path(tail, head, width, height);
                paint_path(&context, &path, &paint, false);
                tail
            }
            FeedbackAnchor::ImagePoint(point) => source_point(*point, width, height),
            FeedbackAnchor::ImageText(point) => source_point(*point, width, height),
            FeedbackAnchor::Asset
            | FeedbackAnchor::VideoPoint { .. }
            | FeedbackAnchor::VideoRange { .. } => {
                return Err(ReviewArtifactError::InvalidRequest);
            }
        };
        draw_marker(&context, marker, annotation.ordinal, width, height, &paint);
    }
    CGBitmapContextCreateImage(Some(&context)).ok_or(ReviewArtifactError::Unavailable)
}

struct EvidencePaint {
    color: (f64, f64, f64),
    alpha: f64,
    width: f64,
    halo: f64,
    dashed: bool,
    multiply: bool,
}

impl EvidencePaint {
    fn from_style(style: Option<AnnotationStyle>, base_width: f64) -> Self {
        let fallback = AnnotationStyle::new(
            AnnotationTool::Rectangle,
            AnnotationColor::Red,
            AnnotationWidth::Medium,
            100,
            false,
            None,
        )
        .expect("hard-coded fallback style is valid");
        let style = style.unwrap_or(fallback);
        let factor = match style.width() {
            AnnotationWidth::Thin => 0.75,
            AnnotationWidth::Medium => 1.0,
            AnnotationWidth::Thick => 2.0,
        };
        let width = (base_width * factor).clamp(2.0, 24.0);
        let highlighter = style.tool() == AnnotationTool::Highlighter;
        Self {
            color: annotation_rgb(style.color()),
            alpha: if highlighter {
                0.35
            } else {
                f64::from(style.opacity_percent()) / 100.0
            },
            width,
            halo: if highlighter {
                0.0
            } else {
                (base_width * 0.3).clamp(1.5, 5.0)
            },
            dashed: style.dashed(),
            multiply: highlighter,
        }
    }

    fn corner_radius(&self) -> f64 {
        (self.width * 1.2).clamp(3.5, 12.0)
    }
}

fn annotation_rgb(color: AnnotationColor) -> (f64, f64, f64) {
    match color {
        AnnotationColor::Red => (0.776, 0.157, 0.157),
        AnnotationColor::Orange => (0.937, 0.424, 0.0),
        AnnotationColor::Yellow => (0.976, 0.659, 0.145),
        AnnotationColor::Green => (0.18, 0.49, 0.196),
        AnnotationColor::Teal => (0.0, 0.412, 0.361),
        AnnotationColor::Blue => (0.082, 0.4, 0.753),
        AnnotationColor::Purple => (0.416, 0.106, 0.604),
        AnnotationColor::Black => (0.129, 0.129, 0.129),
    }
}

fn polyline_path(points: &[CGPoint], closed: bool) -> CFRetained<CGPath> {
    let path = CGMutablePath::new();
    if let Some(first) = points.first() {
        // SAFETY: `path` is a fresh mutable path and the transform is null.
        unsafe {
            CGMutablePath::move_to_point(Some(&path), std::ptr::null(), first.x, first.y);
            for point in &points[1..] {
                CGMutablePath::add_line_to_point(Some(&path), std::ptr::null(), point.x, point.y);
            }
            if closed {
                CGMutablePath::close_subpath(Some(&path));
            }
        }
    }
    // SAFETY: The mutable path is complete and no caller retains a mutable
    // reference while it is drawn.
    unsafe { CFRetained::cast_unchecked(path) }
}

fn arrow_path(tail: CGPoint, head: CGPoint, width: u32, height: u32) -> CFRetained<CGPath> {
    let dx = head.x - tail.x;
    let dy = head.y - tail.y;
    let distance = dx.hypot(dy);
    if distance <= f64::EPSILON {
        return polyline_path(&[tail, head], false);
    }
    let unit_x = dx / distance;
    let unit_y = dy / distance;
    let head_length = (f64::from(width.min(height)) * 0.035)
        .clamp(12.0, 36.0)
        .min(distance * 0.45);
    let wing = head_length * 0.55;
    let base_x = head.x - unit_x * head_length;
    let base_y = head.y - unit_y * head_length;
    let perpendicular_x = -unit_y;
    let perpendicular_y = unit_x;
    let points = [
        tail,
        head,
        CGPoint::new(
            base_x + perpendicular_x * wing,
            base_y + perpendicular_y * wing,
        ),
        head,
        CGPoint::new(
            base_x - perpendicular_x * wing,
            base_y - perpendicular_y * wing,
        ),
    ];
    let path = CGMutablePath::new();
    // SAFETY: `path` is fresh mutable storage and the transform is null.
    unsafe {
        CGMutablePath::move_to_point(Some(&path), std::ptr::null(), points[0].x, points[0].y);
        for point in &points[1..] {
            CGMutablePath::add_line_to_point(Some(&path), std::ptr::null(), point.x, point.y);
        }
    }
    // SAFETY: The mutable path is complete and not aliased.
    unsafe { CFRetained::cast_unchecked(path) }
}

fn paint_path(context: &CGContext, path: &CGPath, paint: &EvidencePaint, fill: bool) {
    CGContext::set_blend_mode(
        Some(context),
        if paint.multiply {
            CGBlendMode::Multiply
        } else {
            CGBlendMode::Normal
        },
    );
    if fill {
        CGContext::begin_path(Some(context));
        CGContext::add_path(Some(context), Some(path));
        CGContext::set_rgb_fill_color(
            Some(context),
            paint.color.0,
            paint.color.1,
            paint.color.2,
            0.1,
        );
        CGContext::fill_path(Some(context));
    }
    if paint.halo > 0.0 {
        CGContext::begin_path(Some(context));
        CGContext::add_path(Some(context), Some(path));
        CGContext::set_rgb_stroke_color(Some(context), 1.0, 1.0, 1.0, 0.9);
        CGContext::set_line_width(Some(context), paint.width + paint.halo * 2.0);
        set_dash(context, &[]);
        CGContext::stroke_path(Some(context));
    }
    CGContext::begin_path(Some(context));
    CGContext::add_path(Some(context), Some(path));
    CGContext::set_rgb_stroke_color(
        Some(context),
        paint.color.0,
        paint.color.1,
        paint.color.2,
        paint.alpha,
    );
    CGContext::set_line_width(Some(context), paint.width);
    let dashed = [paint.width * 3.0, paint.width * 2.0];
    set_dash(context, if paint.dashed { &dashed } else { &[] });
    CGContext::stroke_path(Some(context));
    CGContext::set_blend_mode(Some(context), CGBlendMode::Normal);
}

fn set_dash(context: &CGContext, lengths: &[f64]) {
    // SAFETY: `lengths` points to a valid slice for the duration of the call;
    // an empty slice is explicitly permitted as "solid".
    unsafe {
        CGContext::set_line_dash(
            Some(context),
            0.0,
            if lengths.is_empty() {
                std::ptr::null()
            } else {
                lengths.as_ptr()
            },
            lengths.len(),
        );
    }
}

fn source_point(point: NormalizedPoint, width: u32, height: u32) -> CGPoint {
    CGPoint::new(
        point.x() * f64::from(width),
        (1.0 - point.y()) * f64::from(height),
    )
}

fn source_rect(rect: NormalizedRect, width: u32, height: u32) -> CGRect {
    CGRect::new(
        CGPoint::new(
            rect.x() * f64::from(width),
            (1.0 - rect.y() - rect.height()) * f64::from(height),
        ),
        CGSize::new(
            rect.width() * f64::from(width),
            rect.height() * f64::from(height),
        ),
    )
}

fn draw_marker(
    context: &CGContext,
    center: CGPoint,
    ordinal: u32,
    width: u32,
    height: u32,
    paint: &EvidencePaint,
) {
    let graphics_context = NSGraphicsContext::graphicsContextWithCGContext_flipped(context, false);
    let _guard = GraphicsContextGuard::install(context, &graphics_context);
    let short_edge = f64::from(width.min(height));
    let radius = (short_edge * 0.025).clamp(14.0, 30.0).min(short_edge / 2.0);
    let center = CGPoint::new(
        center.x.clamp(radius, f64::from(width) - radius),
        center.y.clamp(radius, f64::from(height) - radius),
    );
    let marker = CGRect::new(
        CGPoint::new(center.x - radius, center.y - radius),
        CGSize::new(radius * 2.0, radius * 2.0),
    );
    CGContext::set_rgb_fill_color(Some(context), 1.0, 1.0, 1.0, 0.97);
    CGContext::fill_ellipse_in_rect(Some(context), marker);
    CGContext::set_rgb_stroke_color(
        Some(context),
        paint.color.0,
        paint.color.1,
        paint.color.2,
        1.0,
    );
    CGContext::set_line_width(Some(context), (radius * 0.08).max(1.5));
    CGContext::stroke_ellipse_in_rect(Some(context), marker);

    let font = NSFont::boldSystemFontOfSize(radius * 0.9);
    let color = NSColor::colorWithSRGBRed_green_blue_alpha(
        paint.color.0,
        paint.color.1,
        paint.color.2,
        1.0,
    );
    let values: [&AnyObject; 2] = [font.as_super().as_super(), color.as_super().as_super()];
    let attributes = NSDictionary::from_slices(
        &[unsafe { NSFontAttributeName }, unsafe {
            NSForegroundColorAttributeName
        }],
        &values,
    );
    let label = NSString::from_str(&ordinal.to_string());
    // SAFETY: The dictionary keys are AppKit attributed-string keys and both
    // values are the documented NSFont and NSColor object types.
    let size = unsafe { label.sizeWithAttributes(Some(&attributes)) };
    let origin = CGPoint::new(center.x - size.width / 2.0, center.y - size.height / 2.0);
    // SAFETY: The same correctly typed attribute dictionary remains alive for
    // the duration of the synchronous draw call.
    unsafe { label.drawAtPoint_withAttributes(origin, Some(&attributes)) };
}

struct GraphicsContextGuard<'a> {
    context: &'a CGContext,
    previous: Option<objc2::rc::Retained<NSGraphicsContext>>,
}

impl<'a> GraphicsContextGuard<'a> {
    fn install(context: &'a CGContext, graphics_context: &NSGraphicsContext) -> Self {
        let previous = NSGraphicsContext::currentContext();
        // AppKit text drawing also changes the CGContext's stroke color. Restoring
        // only the current NSGraphicsContext would leak that state into the next anchor.
        CGContext::save_g_state(Some(context));
        NSGraphicsContext::setCurrentContext(Some(graphics_context));
        Self { context, previous }
    }
}

impl Drop for GraphicsContextGuard<'_> {
    fn drop(&mut self) {
        CGContext::restore_g_state(Some(self.context));
        NSGraphicsContext::setCurrentContext(self.previous.as_deref());
    }
}

fn inspect_output(
    path: &Path,
    max_output_bytes: u64,
) -> Result<(u64, [u8; 32]), ReviewArtifactError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| ReviewArtifactError::Unavailable)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ReviewArtifactError::Unavailable);
    }
    if metadata.len() > max_output_bytes {
        return Err(ReviewArtifactError::LimitExceeded);
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| ReviewArtifactError::Unavailable)?;
    let mut hasher = blake3::Hasher::new();
    let mut bytes = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let read = file
            .read(&mut bytes)
            .map_err(|_| ReviewArtifactError::Unavailable)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or(ReviewArtifactError::LimitExceeded)?;
        if total > max_output_bytes {
            return Err(ReviewArtifactError::LimitExceeded);
        }
        hasher.update(&bytes[..read]);
    }
    if total != metadata.len() {
        return Err(ReviewArtifactError::Unavailable);
    }
    Ok((total, *hasher.finalize().as_bytes()))
}

pub(super) fn map_image_error(error: viewer_application::ImageError) -> ReviewArtifactError {
    match error {
        viewer_application::ImageError::Unsupported | viewer_application::ImageError::Corrupt => {
            ReviewArtifactError::DecodeFailed
        }
        viewer_application::ImageError::BudgetExceeded => ReviewArtifactError::LimitExceeded,
        viewer_application::ImageError::Cancelled => ReviewArtifactError::Cancelled,
        viewer_application::ImageError::Io(_) => ReviewArtifactError::Unavailable,
    }
}

struct OwnedArtifact {
    path: Option<PathBuf>,
}

impl OwnedArtifact {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    fn path(&self) -> &Path {
        self.path.as_deref().expect("owned artifact path")
    }

    fn commit(&mut self) -> PathBuf {
        self.path.take().expect("owned artifact path")
    }
}

impl Drop for OwnedArtifact {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::{EvidencePaint, MacReviewArtifactRenderer, ReviewArtifactRenderCheckpoint};
    use objc2_core_foundation::{
        CFDictionary, CFNumber, CFString, CFType, CFURL, CGPoint, CGRect, CGSize,
    };
    use objc2_core_graphics::{
        CGBitmapContextCreate, CGBitmapContextCreateImage, CGColorSpace, CGContext, CGImage,
        CGImageAlphaInfo,
    };
    use objc2_image_io::{CGImageDestination, kCGImagePropertyOrientation};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use viewer_application::{
        MAX_REVIEW_ARTIFACT_BYTES, NumberedImageAnnotation, REVIEW_ANNOTATION_MAX_EDGE,
        ReviewArtifactError, ReviewArtifactPort, ReviewArtifactRenderRequest,
        ReviewTaskCancellation,
    };
    use viewer_domain::review::{
        AnnotationColor, AnnotationStyle, AnnotationTool, AnnotationWidth, AssetEvidence,
        AssetVersion, FeedbackAnchor, ImageStroke, MAX_IMAGE_STROKE_POINTS_PER_ROUND,
        NormalizedArrow, NormalizedPoint, NormalizedRect, ReviewMedia,
    };
    use viewer_domain::{AssetVersionId, FeedbackId, RelativePath};

    #[test]
    fn highlighter_evidence_paint_multiplies_without_halo() {
        let style = AnnotationStyle::new(
            AnnotationTool::Highlighter,
            AnnotationColor::Yellow,
            AnnotationWidth::Thick,
            35,
            false,
            None,
        )
        .unwrap();
        let paint = EvidencePaint::from_style(Some(style), 8.0);
        assert!(paint.multiply);
        assert_eq!(paint.alpha, 0.35);
        assert_eq!(paint.halo, 0.0);

        let rectangle = AnnotationStyle::new(
            AnnotationTool::Rectangle,
            AnnotationColor::Red,
            AnnotationWidth::Medium,
            100,
            false,
            None,
        )
        .unwrap();
        let paint = EvidencePaint::from_style(Some(rectangle), 8.0);
        assert!(!paint.multiply);
        assert!(paint.halo > 0.0);
    }

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/images")
            .join(name)
    }

    fn review_asset_for_source(path: &Path) -> AssetVersion {
        let bytes = fs::read(path).unwrap();
        let metadata = fs::metadata(path).unwrap();
        let modified_ns = metadata
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i128;
        AssetVersion {
            id: AssetVersionId::from_u128(4),
            source_entity_id: None,
            relative_path: RelativePath::parse("rotated-6.jpg").unwrap(),
            evidence: AssetEvidence {
                size_bytes: bytes.len() as u64,
                modified_ns,
                blake3: Some(*blake3::hash(&bytes).as_bytes()),
            },
            media: ReviewMedia::Image {
                width: Some(600),
                height: Some(800),
            },
            producer_asset_id: None,
            parent_asset_version_id: None,
        }
    }

    fn annotations(rect_x: f64) -> Vec<NumberedImageAnnotation> {
        vec![
            NumberedImageAnnotation {
                style: None,
                ordinal: 1,
                feedback_id: FeedbackId::from_u128(11),
                anchor: FeedbackAnchor::ImageRect(
                    NormalizedRect::new(rect_x, 0.2, 0.2, 0.3).unwrap(),
                ),
            },
            NumberedImageAnnotation {
                style: None,
                ordinal: 2,
                feedback_id: FeedbackId::from_u128(12),
                anchor: FeedbackAnchor::ImageStroke(
                    ImageStroke::new(vec![
                        NormalizedPoint::new(0.2, 0.2).unwrap(),
                        NormalizedPoint::new(0.8, 0.7).unwrap(),
                    ])
                    .unwrap(),
                ),
            },
        ]
    }

    fn request(
        source_path: PathBuf,
        cancellation: ReviewTaskCancellation,
        annotations: Vec<NumberedImageAnnotation>,
    ) -> ReviewArtifactRenderRequest {
        ReviewArtifactRenderRequest {
            expected_asset: review_asset_for_source(&source_path),
            source_path,
            cancellation,
            annotations,
        }
    }

    // Literal two-colour source: orientation 6 rotates left/right into top/bottom.
    pub(in crate::image) fn asymmetric_source(
        path: &Path,
        width: u32,
        height: u32,
        orientation: i64,
    ) {
        let mut rgba = Vec::<u8>::new();
        for y in 0..height {
            for x in 0..width {
                let red = if orientation == 6 {
                    x < width / 2
                } else {
                    y < height / 2
                };
                rgba.extend_from_slice(if red {
                    &[255, 0, 0, 255]
                } else {
                    &[0, 0, 255, 255]
                });
            }
        }
        let space = CGColorSpace::new_device_rgb().unwrap();
        // SAFETY: Owned RGBA storage outlives the context and its copied image.
        let context = unsafe {
            CGBitmapContextCreate(
                rgba.as_mut_ptr().cast(),
                width as usize,
                height as usize,
                8,
                width as usize * 4,
                Some(&space),
                CGImageAlphaInfo::PremultipliedLast.0,
            )
        }
        .unwrap();
        let image = CGBitmapContextCreateImage(Some(&context)).unwrap();
        let url = CFURL::from_file_path(path).unwrap();
        let kind = CFString::from_str(if orientation == 6 {
            "public.jpeg"
        } else {
            "public.png"
        });
        let value = CFNumber::new_i64(orientation);
        let properties = CFDictionary::<CFString, CFType>::from_slices(
            &[unsafe { kCGImagePropertyOrientation }],
            &[value.as_ref()],
        );
        // SAFETY: A single image and the documented integer orientation property.
        unsafe {
            let destination = CGImageDestination::with_url(&url, &kind, 1, None).unwrap();
            destination.add_image(&image, Some(properties.as_opaque()));
            assert!(destination.finalize());
        }
    }

    pub(in crate::image) fn decoded_rgba(path: &Path, width: u32, height: u32) -> Vec<u8> {
        let source = super::open_image_bytes(&fs::read(path).unwrap()).unwrap();
        let image = super::thumbnail_from_image_source(&source, width.max(height)).unwrap();
        assert_eq!(
            (CGImage::width(Some(&image)), CGImage::height(Some(&image))),
            (width as usize, height as usize)
        );
        let mut pixels = vec![0; width as usize * height as usize * 4];
        let space = CGColorSpace::new_device_rgb().unwrap();
        // SAFETY: The checked RGBA buffer is alive until the context is dropped.
        let context = unsafe {
            CGBitmapContextCreate(
                pixels.as_mut_ptr().cast(),
                width as usize,
                height as usize,
                8,
                width as usize * 4,
                Some(&space),
                CGImageAlphaInfo::PremultipliedLast.0,
            )
        }
        .unwrap();
        CGContext::draw_image(
            Some(&context),
            CGRect::new(CGPoint::ZERO, CGSize::new(width.into(), height.into())),
            Some(&image),
        );
        drop(context);
        pixels
    }

    #[tokio::test]
    async fn exported_pixels_keep_source_top_bottom_and_top_anchor_including_exif_orientation() {
        for orientation in [1, 6] {
            let source_root = tempfile::tempdir().unwrap();
            let source = source_root.path().join(if orientation == 6 {
                "source.jpg"
            } else {
                "source.png"
            });
            let (raw_width, raw_height) = if orientation == 6 {
                (200, 120)
            } else {
                (120, 200)
            };
            asymmetric_source(&source, raw_width, raw_height, orientation);
            let original = fs::read(&source).unwrap();
            let cache = tempfile::tempdir().unwrap();
            let renderer = MacReviewArtifactRenderer::new(cache.path()).unwrap();
            let mut input = request(
                source.clone(),
                ReviewTaskCancellation::default(),
                vec![NumberedImageAnnotation {
                    style: None,
                    ordinal: 1,
                    feedback_id: FeedbackId::from_u128(1),
                    anchor: FeedbackAnchor::ImageRect(
                        NormalizedRect::new(0.3, 0.1, 0.4, 0.2).unwrap(),
                    ),
                }],
            );
            input.expected_asset.media = ReviewMedia::Image {
                width: Some(120),
                height: Some(200),
            };
            let artifact = renderer.render(input).await.unwrap();
            let pixels = decoded_rgba(&artifact.temporary_path, 120, 200);
            let pixel = |x: usize, y: usize| &pixels[(y * 120 + x) * 4..(y * 120 + x) * 4 + 3];
            assert!(
                pixel(110, 10)[0] > 240 && pixel(110, 10)[2] < 15,
                "top must remain red, orientation {orientation}: {:?}",
                pixel(110, 10)
            );
            assert!(
                pixel(110, 190)[2] > 240 && pixel(110, 190)[0] < 15,
                "bottom must remain blue"
            );
            // Right edge of the top rectangle uses the annotation red, not
            // the source image's red/blue.
            assert!(
                (pixel(84, 40)[0] as i32 - 198).abs() < 8
                    && (pixel(84, 40)[1] as i32 - 40).abs() < 8
            );
            assert_eq!(fs::read(&source).unwrap(), original);
        }
    }

    #[tokio::test]
    async fn renderer_honors_stored_color_and_width() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("styled.png");
        asymmetric_source(&source, 600, 800, 0);
        let renderer = MacReviewArtifactRenderer::new(root.path()).unwrap();
        let styled = |ordinal, x, color, width| NumberedImageAnnotation {
            ordinal,
            feedback_id: FeedbackId::from_u128(100 + ordinal as u128),
            anchor: FeedbackAnchor::ImageRect(NormalizedRect::new(x, 0.2, 0.2, 0.3).unwrap()),
            style: Some(
                AnnotationStyle::new(AnnotationTool::Rectangle, color, width, 100, false, None)
                    .unwrap(),
            ),
        };
        let artifact = renderer
            .render(request(
                source,
                ReviewTaskCancellation::default(),
                vec![
                    styled(1, 0.1, AnnotationColor::Blue, AnnotationWidth::Thick),
                    styled(2, 0.6, AnnotationColor::Green, AnnotationWidth::Thin),
                ],
            ))
            .await
            .unwrap();
        let pixels = decoded_rgba(&artifact.temporary_path, 600, 800);
        let pixel = |x: usize, y: usize| &pixels[(y * 600 + x) * 4..(y * 600 + x) * 4 + 3];
        for (channel, expected) in pixel(60, 300).iter().zip([21_i16, 101, 192]) {
            assert!((i16::from(*channel) - expected).abs() < 12);
        }
        for (channel, expected) in pixel(360, 300).iter().zip([46_i16, 125, 50]) {
            assert!((i16::from(*channel) - expected).abs() < 12);
        }
    }

    #[tokio::test]
    async fn renderer_accepts_tiny_and_thin_sources_without_mutating_them() {
        for (width, height) in [(1, 1), (16, 16), (1, 200), (200, 1)] {
            let source_root = tempfile::tempdir().unwrap();
            let source = source_root.path().join("tiny.png");
            asymmetric_source(&source, width, height, 1);
            let original = fs::read(&source).unwrap();
            let cache = tempfile::tempdir().unwrap();
            let renderer = MacReviewArtifactRenderer::new(cache.path()).unwrap();
            let mut input = request(
                source.clone(),
                ReviewTaskCancellation::default(),
                annotations(0.1),
            );
            input.expected_asset.media = ReviewMedia::Image {
                width: Some(width),
                height: Some(height),
            };
            let artifact = renderer.render(input).await.unwrap();
            assert_eq!((artifact.width, artifact.height), (width, height));
            assert_eq!(
                decoded_rgba(&artifact.temporary_path, width, height).len(),
                width as usize * height as usize * 4
            );
            assert_eq!(fs::read(&source).unwrap(), original);
        }
    }

    #[tokio::test]
    async fn exported_outline_color_survives_each_numbered_marker() {
        let source_path = fixture("rotated-6.jpg");
        let original = fs::read(&source_path).unwrap();
        let anchors = [
            FeedbackAnchor::ImageStroke(
                ImageStroke::new(vec![
                    NormalizedPoint::new(0.1, 0.2).unwrap(),
                    NormalizedPoint::new(0.2, 0.4).unwrap(),
                ])
                .unwrap(),
            ),
            FeedbackAnchor::ImageStroke(
                ImageStroke::new(vec![
                    NormalizedPoint::new(0.3, 0.2).unwrap(),
                    NormalizedPoint::new(0.4, 0.4).unwrap(),
                ])
                .unwrap(),
            ),
            FeedbackAnchor::ImageRect(NormalizedRect::new(0.5, 0.2, 0.1, 0.2).unwrap()),
            FeedbackAnchor::ImageRect(NormalizedRect::new(0.75, 0.2, 0.1, 0.2).unwrap()),
        ];

        // Both shape orders must retain visible outlines after drawing white numbers.
        for order in [[0, 1, 2, 3], [2, 3, 0, 1]] {
            let cache = tempfile::tempdir().unwrap();
            let renderer = MacReviewArtifactRenderer::new(cache.path()).unwrap();
            let numbered = order
                .iter()
                .enumerate()
                .map(|(index, &anchor)| NumberedImageAnnotation {
                    style: None,
                    ordinal: index as u32 + 1,
                    feedback_id: FeedbackId::from_u128(index as u128 + 11),
                    anchor: anchors[anchor].clone(),
                })
                .collect();
            let artifact = renderer
                .render(request(
                    source_path.clone(),
                    ReviewTaskCancellation::default(),
                    numbered,
                ))
                .await
                .unwrap();
            let pixels = decoded_rgba(&artifact.temporary_path, 600, 800);
            let pixel = |x: usize, y: usize| &pixels[(y * 600 + x) * 4..(y * 600 + x) * 4 + 3];

            // Literal midpoints on both paths and both rectangle edges, away from badges.
            for (outline_x, marker_x) in [(90, 60), (210, 180), (300, 300), (450, 450)] {
                for (channel, expected) in pixel(outline_x, 240).iter().zip([198_i16, 40, 40]) {
                    assert!(
                        (i16::from(*channel) - expected).abs() < 8,
                        "outline at ({outline_x},240) lost its color for order {order:?}: {:?}",
                        pixel(outline_x, 240)
                    );
                }
                // Sample strictly inside the badge: changing the text color to hide the
                // leak must not pass by making the white ordinal disappear.
                let mut white_pixels = 0;
                for y in 153..167 {
                    for badge_x in (marker_x - 7)..(marker_x + 7) {
                        if pixel(badge_x, y).iter().all(|channel| *channel > 245) {
                            white_pixels += 1;
                        }
                    }
                }
                assert!(
                    white_pixels >= 3,
                    "ordinal must remain white at ({marker_x},160)"
                );
            }
        }
        assert_eq!(fs::read(&source_path).unwrap(), original);
    }

    #[tokio::test]
    async fn renderer_orients_source_and_draws_numbered_rect_and_stroke() {
        let cache = tempfile::tempdir().unwrap();
        let renderer = MacReviewArtifactRenderer::new(cache.path()).unwrap();
        let source_path = fixture("rotated-6.jpg");

        let first = renderer
            .render(request(
                source_path.clone(),
                ReviewTaskCancellation::default(),
                annotations(0.1),
            ))
            .await
            .unwrap();
        let first_bytes = fs::read(&first.temporary_path).unwrap();
        let second = renderer
            .render(request(
                source_path.clone(),
                ReviewTaskCancellation::default(),
                annotations(0.6),
            ))
            .await
            .unwrap();
        let repeated = renderer
            .render(request(
                source_path,
                ReviewTaskCancellation::default(),
                annotations(0.1),
            ))
            .await
            .unwrap();

        assert!(first.width < first.height);
        assert!(first.width.max(first.height) <= REVIEW_ANNOTATION_MAX_EDGE);
        assert_eq!(*blake3::hash(&first_bytes).as_bytes(), first.blake3);
        assert_eq!(&first_bytes[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(first.annotations.len(), 2);
        assert_ne!(first.blake3, second.blake3);
        assert_eq!(first.blake3, repeated.blake3);
    }

    #[tokio::test]
    async fn renderer_draws_every_extended_anchor_at_its_normalized_target() {
        let source_root = tempfile::tempdir().unwrap();
        let source_path = source_root.path().join("source.png");
        asymmetric_source(&source_path, 600, 800, 1);
        let original = fs::read(&source_path).unwrap();
        let cache = tempfile::tempdir().unwrap();
        let renderer = MacReviewArtifactRenderer::new(cache.path()).unwrap();
        let annotations = vec![
            NumberedImageAnnotation {
                style: None,
                ordinal: 1,
                feedback_id: FeedbackId::from_u128(11),
                anchor: FeedbackAnchor::ImagePoint(NormalizedPoint::new(0.20, 0.25).unwrap()),
            },
            NumberedImageAnnotation {
                style: None,
                ordinal: 2,
                feedback_id: FeedbackId::from_u128(12),
                anchor: FeedbackAnchor::ImageArrow(
                    NormalizedArrow::new(
                        NormalizedPoint::new(0.50, 0.30).unwrap(),
                        NormalizedPoint::new(0.75, 0.30).unwrap(),
                    )
                    .unwrap(),
                ),
            },
            NumberedImageAnnotation {
                style: None,
                ordinal: 3,
                feedback_id: FeedbackId::from_u128(13),
                anchor: FeedbackAnchor::ImageEllipse(
                    NormalizedRect::new(0.20, 0.70, 0.20, 0.15).unwrap(),
                ),
            },
            NumberedImageAnnotation {
                style: None,
                ordinal: 4,
                feedback_id: FeedbackId::from_u128(14),
                anchor: FeedbackAnchor::ImageRect(
                    NormalizedRect::new(0.55, 0.55, 0.10, 0.10).unwrap(),
                ),
            },
            NumberedImageAnnotation {
                style: None,
                ordinal: 5,
                feedback_id: FeedbackId::from_u128(15),
                anchor: FeedbackAnchor::ImageStroke(
                    ImageStroke::new(vec![
                        NormalizedPoint::new(0.70, 0.65).unwrap(),
                        NormalizedPoint::new(0.85, 0.75).unwrap(),
                    ])
                    .unwrap(),
                ),
            },
        ];

        let rendered = renderer
            .render(request(
                source_path.clone(),
                ReviewTaskCancellation::default(),
                annotations,
            ))
            .await
            .unwrap();
        assert_eq!(rendered.annotations.len(), 5);
        assert_eq!(rendered.annotations[0].ordinal, 1);
        let source_pixels = decoded_rgba(&source_path, 600, 800);
        let rendered_pixels = decoded_rgba(&rendered.temporary_path, 600, 800);
        let pixel = |bytes: &[u8], (x, y): (usize, usize)| {
            let offset = (y * 600 + x) * 4;
            [bytes[offset], bytes[offset + 1], bytes[offset + 2]]
        };
        let normalized_pixel =
            |x: f64, y: f64| ((x * 599.0).round() as usize, (y * 799.0).round() as usize);
        let changed_near = |center: (usize, usize), radius: usize| {
            let (cx, cy) = center;
            (cy - radius..=cy + radius).any(|y| {
                (cx - radius..=cx + radius)
                    .any(|x| pixel(&rendered_pixels, (x, y)) != pixel(&source_pixels, (x, y)))
            })
        };
        for target in [
            normalized_pixel(0.20, 0.25),
            normalized_pixel(0.75, 0.30),
            normalized_pixel(0.30, 0.70),
        ] {
            assert!(changed_near(target, 6));
        }
        assert_eq!(fs::read(&source_path).unwrap(), original);
    }

    #[tokio::test]
    async fn renderer_rejects_changed_symlinked_and_non_image_sources() {
        let cache = tempfile::tempdir().unwrap();
        let renderer = MacReviewArtifactRenderer::new(cache.path()).unwrap();
        let source_path = fixture("rotated-6.jpg");

        let changed_root = tempfile::tempdir().unwrap();
        let changed = changed_root.path().join("changed.jpg");
        fs::copy(&source_path, &changed).unwrap();
        let changed_request = request(
            changed.clone(),
            ReviewTaskCancellation::default(),
            annotations(0.1),
        );
        fs::write(&changed, b"changed").unwrap();
        assert_eq!(
            renderer.render(changed_request).await,
            Err(ReviewArtifactError::SourceChanged),
        );

        let symlink = changed_root.path().join("linked.jpg");
        std::os::unix::fs::symlink(&source_path, &symlink).unwrap();
        assert_eq!(
            renderer
                .render(request(
                    symlink,
                    ReviewTaskCancellation::default(),
                    annotations(0.1),
                ))
                .await,
            Err(ReviewArtifactError::UnsafeSource),
        );

        let mut video_request = request(
            source_path,
            ReviewTaskCancellation::default(),
            annotations(0.1),
        );
        video_request.expected_asset.media = ReviewMedia::Video {
            duration_us: Some(1),
            display_width: Some(600),
            display_height: Some(800),
        };
        assert_eq!(
            renderer.render(video_request).await,
            Err(ReviewArtifactError::InvalidRequest),
        );
    }

    #[tokio::test]
    async fn renderer_cancels_before_decode_and_before_encode_without_leaking_files() {
        let cache = tempfile::tempdir().unwrap();
        let renderer = MacReviewArtifactRenderer::new(cache.path()).unwrap();
        let source_path = fixture("rotated-6.jpg");
        let cancellation = ReviewTaskCancellation::default();
        cancellation.cancel();
        assert_eq!(
            renderer
                .render(request(source_path.clone(), cancellation, annotations(0.1),))
                .await,
            Err(ReviewArtifactError::Cancelled),
        );

        let cancellation = ReviewTaskCancellation::default();
        let observer = cancellation.clone();
        let renderer = MacReviewArtifactRenderer::new_with_checkpoint_for_test(
            cache.path(),
            Arc::new(move |checkpoint| {
                if checkpoint == ReviewArtifactRenderCheckpoint::Encode {
                    observer.cancel();
                }
            }),
        )
        .unwrap();
        assert_eq!(
            renderer
                .render(request(source_path, cancellation, annotations(0.1)))
                .await,
            Err(ReviewArtifactError::Cancelled),
        );
        assert_eq!(fs::read_dir(cache.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn renderer_rejects_unsupported_anchors_aggregate_points_and_oversize_output() {
        let cache = tempfile::tempdir().unwrap();
        let source_path = fixture("rotated-6.jpg");
        let renderer = MacReviewArtifactRenderer::new(cache.path()).unwrap();
        assert_eq!(
            renderer
                .render(request(
                    source_path.clone(),
                    ReviewTaskCancellation::default(),
                    vec![NumberedImageAnnotation {
                        style: None,
                        ordinal: 1,
                        feedback_id: FeedbackId::from_u128(1),
                        anchor: FeedbackAnchor::Asset,
                    }],
                ))
                .await,
            Err(ReviewArtifactError::InvalidRequest),
        );

        let points_per_stroke = viewer_domain::review::MAX_IMAGE_STROKE_POINTS;
        let stroke_count = MAX_IMAGE_STROKE_POINTS_PER_ROUND / points_per_stroke + 1;
        let points = (0..points_per_stroke)
            .map(|index| {
                let value = if index % 2 == 0 { 0.1 } else { 0.9 };
                NormalizedPoint::new(value, value).unwrap()
            })
            .collect::<Vec<_>>();
        let too_many_points = (0..stroke_count)
            .map(|index| NumberedImageAnnotation {
                style: None,
                ordinal: u32::try_from(index + 1).unwrap(),
                feedback_id: FeedbackId::from_u128(index as u128 + 1),
                anchor: FeedbackAnchor::ImageStroke(ImageStroke::new(points.clone()).unwrap()),
            })
            .collect();
        assert_eq!(
            renderer
                .render(request(
                    source_path.clone(),
                    ReviewTaskCancellation::default(),
                    too_many_points,
                ))
                .await,
            Err(ReviewArtifactError::LimitExceeded),
        );

        let renderer = MacReviewArtifactRenderer::new_with_output_limit_for_test(
            cache.path(),
            MAX_REVIEW_ARTIFACT_BYTES.min(7),
        )
        .unwrap();
        assert_eq!(
            renderer
                .render(request(
                    source_path,
                    ReviewTaskCancellation::default(),
                    annotations(0.1),
                ))
                .await,
            Err(ReviewArtifactError::LimitExceeded),
        );
        assert_eq!(fs::read_dir(cache.path()).unwrap().count(), 0);
    }
}
