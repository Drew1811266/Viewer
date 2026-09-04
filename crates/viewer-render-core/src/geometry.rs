use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalPoint {
    pub x: f64,
    pub y: f64,
}

impl LogicalPoint {
    pub fn new(x: f64, y: f64) -> Result<Self, GeometryError> {
        require_finite("logical point x", x)?;
        require_finite("logical point y", y)?;
        Ok(Self { x, y })
    }

    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalSize {
    pub width: f64,
    pub height: f64,
}

impl LogicalSize {
    pub fn new(width: f64, height: f64) -> Result<Self, GeometryError> {
        require_positive("logical width", width)?;
        require_positive("logical height", height)?;
        Ok(Self { width, height })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSize {
    pub width: u32,
    pub height: u32,
}

impl SourceSize {
    pub fn new(width: u32, height: u32) -> Result<Self, GeometryError> {
        if width == 0 || height == 0 {
            return Err(GeometryError::NonPositive("source dimensions"));
        }
        Ok(Self { width, height })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalRect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

impl LogicalRect {
    pub fn new(left: f64, top: f64, width: f64, height: f64) -> Result<Self, GeometryError> {
        require_finite("logical rect left", left)?;
        require_finite("logical rect top", top)?;
        require_positive("logical rect width", width)?;
        require_positive("logical rect height", height)?;
        Ok(Self {
            left,
            top,
            width,
            height,
        })
    }

    pub fn center(self) -> LogicalPoint {
        LogicalPoint {
            x: self.left + self.width / 2.0,
            y: self.top + self.height / 2.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalizedPoint {
    pub x: f64,
    pub y: f64,
}

impl NormalizedPoint {
    pub fn new(x: f64, y: f64) -> Result<Self, GeometryError> {
        require_unit("normalized point x", x)?;
        require_unit("normalized point y", y)?;
        Ok(Self { x, y })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rotation {
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

impl Rotation {
    pub(crate) fn orient(self, point: NormalizedPoint) -> NormalizedPoint {
        match self {
            Self::Deg0 => point,
            Self::Deg90 => NormalizedPoint {
                x: 1.0 - point.y,
                y: point.x,
            },
            Self::Deg180 => NormalizedPoint {
                x: 1.0 - point.x,
                y: 1.0 - point.y,
            },
            Self::Deg270 => NormalizedPoint {
                x: point.y,
                y: 1.0 - point.x,
            },
        }
    }

    pub(crate) fn unorient(self, point: NormalizedPoint) -> NormalizedPoint {
        match self {
            Self::Deg0 => point,
            Self::Deg90 => NormalizedPoint {
                x: point.y,
                y: 1.0 - point.x,
            },
            Self::Deg180 => NormalizedPoint {
                x: 1.0 - point.x,
                y: 1.0 - point.y,
            },
            Self::Deg270 => NormalizedPoint {
                x: 1.0 - point.y,
                y: point.x,
            },
        }
    }

    pub(crate) fn swaps_dimensions(self) -> bool {
        matches!(self, Self::Deg90 | Self::Deg270)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportLayout {
    pub logical_size: LogicalSize,
    pub scale_factor: f64,
    pub fit_inset: f64,
}

impl ViewportLayout {
    pub fn new(
        logical_size: LogicalSize,
        scale_factor: f64,
        fit_inset: f64,
    ) -> Result<Self, GeometryError> {
        require_positive("viewport scale factor", scale_factor)?;
        require_finite("viewport fit inset", fit_inset)?;
        if !(0.0 < fit_inset && fit_inset <= 1.0) {
            return Err(GeometryError::OutOfRange("viewport fit inset"));
        }
        if logical_size.width * scale_factor > u32::MAX as f64
            || logical_size.height * scale_factor > u32::MAX as f64
        {
            return Err(GeometryError::OutOfRange("physical viewport dimensions"));
        }
        Ok(Self {
            logical_size,
            scale_factor,
            fit_inset,
        })
    }

    pub fn physical_size(self) -> PhysicalSize {
        PhysicalSize {
            width: (self.logical_size.width * self.scale_factor).round() as u32,
            height: (self.logical_size.height * self.scale_factor).round() as u32,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeometryError {
    NonFinite(&'static str),
    NonPositive(&'static str),
    OutOfRange(&'static str),
}

impl fmt::Display for GeometryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite(field) => write!(formatter, "{field} must be finite"),
            Self::NonPositive(field) => write!(formatter, "{field} must be positive"),
            Self::OutOfRange(field) => write!(formatter, "{field} is out of range"),
        }
    }
}

impl Error for GeometryError {}

fn require_finite(field: &'static str, value: f64) -> Result<(), GeometryError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(GeometryError::NonFinite(field))
    }
}

fn require_positive(field: &'static str, value: f64) -> Result<(), GeometryError> {
    require_finite(field, value)?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(GeometryError::NonPositive(field))
    }
}

fn require_unit(field: &'static str, value: f64) -> Result<(), GeometryError> {
    require_finite(field, value)?;
    if (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(GeometryError::OutOfRange(field))
    }
}
