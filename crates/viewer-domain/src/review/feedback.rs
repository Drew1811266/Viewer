use super::{
    MAX_FEEDBACK_TEXT_BYTES, MAX_IMAGE_STROKE_POINTS, MAX_TARGETS_PER_FEEDBACK, ReviewValueError,
};
use crate::{AssetVersionId, FeedbackId};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalizedRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl NormalizedRect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Result<Self, ReviewValueError> {
        let values_are_finite = [x, y, width, height].iter().all(|value| value.is_finite());
        let valid = values_are_finite
            && x >= 0.0
            && y >= 0.0
            && width > 0.0
            && height > 0.0
            && x + width <= 1.0
            && y + height <= 1.0;
        valid
            .then_some(Self {
                x,
                y,
                width,
                height,
            })
            .ok_or(ReviewValueError::InvalidNumber)
    }

    pub fn x(&self) -> f64 {
        self.x
    }

    pub fn y(&self) -> f64 {
        self.y
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn height(&self) -> f64 {
        self.height
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalizedPoint {
    x: f64,
    y: f64,
}

impl NormalizedPoint {
    pub fn new(x: f64, y: f64) -> Result<Self, ReviewValueError> {
        let valid =
            x.is_finite() && y.is_finite() && (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y);
        valid
            .then_some(Self { x, y })
            .ok_or(ReviewValueError::InvalidNumber)
    }

    pub fn x(&self) -> f64 {
        self.x
    }

    pub fn y(&self) -> f64 {
        self.y
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImageStroke {
    points: Vec<NormalizedPoint>,
}

impl ImageStroke {
    pub fn new(points: Vec<NormalizedPoint>) -> Result<Self, ReviewValueError> {
        if points.len() > MAX_IMAGE_STROKE_POINTS {
            return Err(ReviewValueError::LimitExceeded);
        }
        if points.len() < 2 {
            return Err(ReviewValueError::InvalidNumber);
        }
        let (min_x, max_x, min_y, max_y) = normalized_bounds(&points);
        if max_x <= min_x || max_y <= min_y {
            return Err(ReviewValueError::InvalidNumber);
        }
        Ok(Self { points })
    }

    pub fn points(&self) -> &[NormalizedPoint] {
        &self.points
    }
}

fn normalized_bounds(points: &[NormalizedPoint]) -> (f64, f64, f64, f64) {
    points.iter().fold(
        (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ),
        |(min_x, max_x, min_y, max_y), point| {
            (
                min_x.min(point.x()),
                max_x.max(point.x()),
                min_y.min(point.y()),
                max_y.max(point.y()),
            )
        },
    )
}

#[derive(Clone, Debug, PartialEq)]
pub enum FeedbackAnchor {
    Asset,
    ImageRect(NormalizedRect),
    ImageStroke(ImageStroke),
    VideoPoint { position_us: u64 },
    VideoRange { start_us: u64, end_us: u64 },
}

impl FeedbackAnchor {
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Asset => "asset",
            Self::ImageRect(_) => "imageRect",
            Self::ImageStroke(_) => "imageStroke",
            Self::VideoPoint { .. } => "videoPoint",
            Self::VideoRange { .. } => "videoRange",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FeedbackTarget {
    pub asset_version_id: AssetVersionId,
    pub anchor: FeedbackAnchor,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Feedback {
    pub id: FeedbackId,
    pub text: String,
    pub created_at_ms: i64,
    pub targets: Vec<FeedbackTarget>,
}

impl Feedback {
    pub fn new(
        id: FeedbackId,
        text: String,
        created_at_ms: i64,
        targets: Vec<FeedbackTarget>,
    ) -> Result<Self, ReviewValueError> {
        if created_at_ms < 0 {
            return Err(ReviewValueError::InvalidNumber);
        }
        if text.trim().is_empty() || targets.is_empty() {
            return Err(ReviewValueError::Empty);
        }
        if text.len() > MAX_FEEDBACK_TEXT_BYTES || targets.len() > MAX_TARGETS_PER_FEEDBACK {
            return Err(ReviewValueError::LimitExceeded);
        }
        if targets.iter().any(|target| {
            matches!(target.anchor, FeedbackAnchor::VideoRange { start_us, end_us } if start_us >= end_us)
        }) {
            return Err(ReviewValueError::InvalidNumber);
        }
        for (index, target) in targets.iter().enumerate() {
            if targets[..index].iter().any(|candidate| candidate == target) {
                return Err(ReviewValueError::DuplicateTarget);
            }
        }

        Ok(Self {
            id,
            text,
            created_at_ms,
            targets,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset_target(id: u128) -> FeedbackTarget {
        FeedbackTarget {
            asset_version_id: AssetVersionId::from_u128(id),
            anchor: FeedbackAnchor::Asset,
        }
    }

    #[test]
    fn normalized_regions_never_escape_the_oriented_image() {
        assert!(NormalizedRect::new(0.1, 0.2, 0.3, 0.4).is_ok());
        for rect in [
            (-0.1, 0.0, 0.2, 0.2),
            (0.0, 0.0, 0.0, 0.2),
            (0.8, 0.0, 0.3, 0.2),
            (0.0, f64::NAN, 0.2, 0.2),
        ] {
            assert!(NormalizedRect::new(rect.0, rect.1, rect.2, rect.3).is_err());
        }
    }

    #[test]
    fn feedback_keeps_natural_language_and_rejects_duplicate_targets() {
        let asset = AssetVersionId::from_u128(1);
        let text = "人物手部需要修正，整体光线保持不变。";
        let feedback = Feedback::new(
            FeedbackId::from_u128(2),
            text.to_owned(),
            10,
            vec![FeedbackTarget {
                asset_version_id: asset,
                anchor: FeedbackAnchor::Asset,
            }],
        )
        .unwrap();
        assert_eq!(feedback.text, text);
        assert!(
            Feedback::new(
                FeedbackId::from_u128(3),
                text.to_owned(),
                10,
                vec![
                    FeedbackTarget {
                        asset_version_id: asset,
                        anchor: FeedbackAnchor::Asset,
                    },
                    FeedbackTarget {
                        asset_version_id: asset,
                        anchor: FeedbackAnchor::Asset,
                    },
                ],
            )
            .is_err()
        );
    }

    #[test]
    fn feedback_requires_nonblank_text_a_timestamp_and_a_target() {
        assert_eq!(
            Feedback::new(
                FeedbackId::from_u128(1),
                "  ".to_owned(),
                1,
                vec![asset_target(1)]
            ),
            Err(ReviewValueError::Empty)
        );
        assert_eq!(
            Feedback::new(
                FeedbackId::from_u128(1),
                "修正".to_owned(),
                -1,
                vec![asset_target(1)]
            ),
            Err(ReviewValueError::InvalidNumber)
        );
        assert_eq!(
            Feedback::new(FeedbackId::from_u128(1), "修正".to_owned(), 1, vec![]),
            Err(ReviewValueError::Empty)
        );
    }

    #[test]
    fn feedback_enforces_text_and_target_limits_before_duplicate_detection() {
        assert_eq!(
            Feedback::new(
                FeedbackId::from_u128(1),
                "x".repeat(MAX_FEEDBACK_TEXT_BYTES + 1),
                1,
                vec![asset_target(1)]
            ),
            Err(ReviewValueError::LimitExceeded)
        );
        assert_eq!(
            Feedback::new(
                FeedbackId::from_u128(1),
                "修正".to_owned(),
                1,
                vec![asset_target(1); MAX_TARGETS_PER_FEEDBACK + 1]
            ),
            Err(ReviewValueError::LimitExceeded)
        );
    }

    #[test]
    fn feedback_rejects_non_increasing_video_ranges() {
        for (start_us, end_us) in [(5, 5), (6, 5)] {
            assert_eq!(
                Feedback::new(
                    FeedbackId::from_u128(1),
                    "修正片段".to_owned(),
                    1,
                    vec![FeedbackTarget {
                        asset_version_id: AssetVersionId::from_u128(1),
                        anchor: FeedbackAnchor::VideoRange { start_us, end_us },
                    }]
                ),
                Err(ReviewValueError::InvalidNumber)
            );
        }
    }
}
