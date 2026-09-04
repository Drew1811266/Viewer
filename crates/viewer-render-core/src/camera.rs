use crate::{
    GeometryError, LogicalPoint, LogicalRect, NormalizedPoint, PhysicalSize, Rotation, SourceSize,
    ViewportLayout,
};

pub const MIN_PREVIEW_ZOOM: f64 = 0.1;
pub const MAX_PREVIEW_ZOOM: f64 = 8.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CameraMode {
    Fit,
    Free,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraState {
    pub mode: CameraMode,
    pub zoom: f64,
    pub rotation: Rotation,
    pub offset: LogicalPoint,
}

impl CameraState {
    pub const fn fit(rotation: Rotation) -> Self {
        Self {
            mode: CameraMode::Fit,
            zoom: 1.0,
            rotation,
            offset: LogicalPoint::ZERO,
        }
    }

    pub const fn effective_zoom(self) -> f64 {
        match self.mode {
            CameraMode::Fit => 1.0,
            CameraMode::Free => self.zoom,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransformSnapshot {
    source: SourceSize,
    viewport: ViewportLayout,
    camera: CameraState,
    fitted_rect: LogicalRect,
}

impl TransformSnapshot {
    pub fn new(
        source: SourceSize,
        viewport: ViewportLayout,
        camera: CameraState,
    ) -> Result<Self, GeometryError> {
        let zoom = camera.effective_zoom();
        if !zoom.is_finite() || !(MIN_PREVIEW_ZOOM..=MAX_PREVIEW_ZOOM).contains(&zoom) {
            return Err(GeometryError::OutOfRange("camera zoom"));
        }
        if !camera.offset.x.is_finite() || !camera.offset.y.is_finite() {
            return Err(GeometryError::NonFinite("camera offset"));
        }

        let (oriented_width, oriented_height) = if camera.rotation.swaps_dimensions() {
            (source.height as f64, source.width as f64)
        } else {
            (source.width as f64, source.height as f64)
        };
        let aspect = oriented_width / oriented_height;
        let available_width = viewport.logical_size.width * viewport.fit_inset;
        let available_height = viewport.logical_size.height * viewport.fit_inset;
        let width = available_width.min(available_height * aspect);
        let height = width / aspect;
        let fitted_rect = LogicalRect::new(
            (viewport.logical_size.width - width) / 2.0,
            (viewport.logical_size.height - height) / 2.0,
            width,
            height,
        )?;

        Ok(Self {
            source,
            viewport,
            camera,
            fitted_rect,
        })
    }

    pub const fn source(self) -> SourceSize {
        self.source
    }

    pub const fn viewport(self) -> ViewportLayout {
        self.viewport
    }

    pub const fn camera(self) -> CameraState {
        self.camera
    }

    pub const fn fitted_rect(self) -> LogicalRect {
        self.fitted_rect
    }

    pub fn physical_viewport(self) -> PhysicalSize {
        self.viewport.physical_size()
    }

    pub fn displayed_rect(self) -> LogicalRect {
        let zoom = self.camera.effective_zoom();
        let width = self.fitted_rect.width * zoom;
        let height = self.fitted_rect.height * zoom;
        LogicalRect {
            left: (self.viewport.logical_size.width - width) / 2.0 + self.camera.offset.x,
            top: (self.viewport.logical_size.height - height) / 2.0 + self.camera.offset.y,
            width,
            height,
        }
    }

    pub fn image_to_view(self, point: NormalizedPoint) -> LogicalPoint {
        let oriented = self.camera.rotation.orient(point);
        let display = self.displayed_rect();
        LogicalPoint {
            x: display.left + oriented.x * display.width,
            y: display.top + oriented.y * display.height,
        }
    }

    pub fn view_to_image(self, point: LogicalPoint) -> Option<NormalizedPoint> {
        let oriented = self.view_to_oriented_unclamped(point);
        if !(0.0..=1.0).contains(&oriented.x) || !(0.0..=1.0).contains(&oriented.y) {
            return None;
        }
        Some(self.camera.rotation.unorient(oriented))
    }

    pub fn view_to_image_clamped(self, point: LogicalPoint) -> NormalizedPoint {
        let oriented = self.view_to_oriented_unclamped(point);
        self.camera.rotation.unorient(NormalizedPoint {
            x: oriented.x.clamp(0.0, 1.0),
            y: oriented.y.clamp(0.0, 1.0),
        })
    }

    pub fn zoom_at(self, factor: f64, anchor: LogicalPoint) -> Result<CameraState, GeometryError> {
        if !factor.is_finite() || factor <= 0.0 {
            return Err(GeometryError::NonPositive("zoom factor"));
        }
        let oriented = self.view_to_oriented_unclamped(anchor);
        let zoom =
            (self.camera.effective_zoom() * factor).clamp(MIN_PREVIEW_ZOOM, MAX_PREVIEW_ZOOM);
        let width = self.fitted_rect.width * zoom;
        let height = self.fitted_rect.height * zoom;
        let center = LogicalPoint {
            x: self.viewport.logical_size.width / 2.0,
            y: self.viewport.logical_size.height / 2.0,
        };
        let desired = LogicalPoint {
            x: anchor.x - center.x - (oriented.x - 0.5) * width,
            y: anchor.y - center.y - (oriented.y - 0.5) * height,
        };
        Ok(CameraState {
            mode: CameraMode::Free,
            zoom,
            rotation: self.camera.rotation,
            offset: self.clamp_offset(desired, width, height),
        })
    }

    pub fn pan_by(self, delta: LogicalPoint) -> Result<CameraState, GeometryError> {
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return Err(GeometryError::NonFinite("pan delta"));
        }
        let display = self.displayed_rect();
        Ok(CameraState {
            mode: CameraMode::Free,
            zoom: self.camera.effective_zoom(),
            rotation: self.camera.rotation,
            offset: self.clamp_offset(
                LogicalPoint {
                    x: self.camera.offset.x + delta.x,
                    y: self.camera.offset.y + delta.y,
                },
                display.width,
                display.height,
            ),
        })
    }

    fn view_to_oriented_unclamped(self, point: LogicalPoint) -> NormalizedPoint {
        let display = self.displayed_rect();
        NormalizedPoint {
            x: (point.x - display.left) / display.width,
            y: (point.y - display.top) / display.height,
        }
    }

    fn clamp_offset(self, offset: LogicalPoint, width: f64, height: f64) -> LogicalPoint {
        let horizontal = ((width - self.viewport.logical_size.width) / 2.0).max(0.0);
        let vertical = ((height - self.viewport.logical_size.height) / 2.0).max(0.0);
        LogicalPoint {
            x: offset.x.clamp(-horizontal, horizontal),
            y: offset.y.clamp(-vertical, vertical),
        }
    }
}
