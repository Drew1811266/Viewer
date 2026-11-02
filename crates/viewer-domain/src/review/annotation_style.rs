use super::{FeedbackAnchor, ReviewValueError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnnotationTool {
    Point,
    Arrow,
    Brush,
    Rectangle,
    Ellipse,
    Text,
    Line,
    Highlighter,
    Polygon,
    Ruler,
}

impl AnnotationTool {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Point => "point",
            Self::Arrow => "arrow",
            Self::Brush => "brush",
            Self::Rectangle => "rectangle",
            Self::Ellipse => "ellipse",
            Self::Text => "text",
            Self::Line => "line",
            Self::Highlighter => "highlighter",
            Self::Polygon => "polygon",
            Self::Ruler => "ruler",
        }
    }

    pub fn matches_anchor(self, anchor: &FeedbackAnchor) -> bool {
        matches!(
            (self, anchor),
            (Self::Point, FeedbackAnchor::ImagePoint(_))
                | (Self::Arrow, FeedbackAnchor::ImageArrow(_))
                | (Self::Brush, FeedbackAnchor::ImageStroke(_))
                | (Self::Rectangle, FeedbackAnchor::ImageRect(_))
                | (Self::Ellipse, FeedbackAnchor::ImageEllipse(_))
                | (Self::Text, FeedbackAnchor::ImageText(_))
                | (Self::Line, FeedbackAnchor::ImageLine(_))
                | (Self::Highlighter, FeedbackAnchor::ImageStroke(_))
                | (Self::Polygon, FeedbackAnchor::ImagePolygon(_))
                | (Self::Ruler, FeedbackAnchor::ImageLine(_))
        )
    }

    pub fn supports_dashed(self) -> bool {
        matches!(
            self,
            Self::Rectangle | Self::Ellipse | Self::Line | Self::Polygon
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnnotationColor {
    Red,
    Orange,
    Yellow,
    Green,
    Teal,
    Blue,
    Purple,
    Black,
}

impl AnnotationColor {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Orange => "orange",
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Teal => "teal",
            Self::Blue => "blue",
            Self::Purple => "purple",
            Self::Black => "black",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnnotationWidth {
    Thin,
    Medium,
    Thick,
}

impl AnnotationWidth {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Thin => "thin",
            Self::Medium => "medium",
            Self::Thick => "thick",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnnotationTextSize {
    Small,
    Medium,
    Large,
}

impl AnnotationTextSize {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnnotationStyle {
    tool: AnnotationTool,
    color: AnnotationColor,
    width: AnnotationWidth,
    opacity_percent: u8,
    dashed: bool,
    text_size: Option<AnnotationTextSize>,
}

impl AnnotationStyle {
    pub fn new(
        tool: AnnotationTool,
        color: AnnotationColor,
        width: AnnotationWidth,
        opacity_percent: u8,
        dashed: bool,
        text_size: Option<AnnotationTextSize>,
    ) -> Result<Self, ReviewValueError> {
        if !(20..=100).contains(&opacity_percent) {
            return Err(ReviewValueError::InvalidNumber);
        }
        if dashed && !tool.supports_dashed() {
            return Err(ReviewValueError::InvalidFormat);
        }
        if tool == AnnotationTool::Highlighter && (opacity_percent != 35 || dashed) {
            return Err(ReviewValueError::InvalidFormat);
        }
        if (tool == AnnotationTool::Text) != text_size.is_some() {
            return Err(ReviewValueError::InvalidFormat);
        }
        Ok(Self {
            tool,
            color,
            width,
            opacity_percent,
            dashed,
            text_size,
        })
    }

    pub fn accepts(&self, anchor: &FeedbackAnchor) -> bool {
        self.tool.matches_anchor(anchor)
    }

    pub fn tool(&self) -> AnnotationTool {
        self.tool
    }

    pub fn color(&self) -> AnnotationColor {
        self.color
    }

    pub fn width(&self) -> AnnotationWidth {
        self.width
    }

    pub fn opacity_percent(&self) -> u8 {
        self.opacity_percent
    }

    pub fn dashed(&self) -> bool {
        self.dashed
    }

    pub fn text_size(&self) -> Option<AnnotationTextSize> {
        self.text_size
    }
}
