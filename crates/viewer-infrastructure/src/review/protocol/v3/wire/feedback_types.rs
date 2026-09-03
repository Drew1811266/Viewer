//! Feedback, anchor and provenance DTOs share one validated Domain conversion.
use super::*;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Feedback {
    #[serde(deserialize_with = "canonical_id")]
    feedback_id: FeedbackId,
    #[serde(deserialize_with = "canonical_id")]
    text_revision_id: ReviewTextRevisionId,
    text: String,
    created_at_ms: i64,
    #[serde(deserialize_with = "required_option")]
    history_ref: Option<History>,
    targets: Vec<Target>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Target {
    #[serde(deserialize_with = "canonical_id")]
    target_id: ReviewTargetId,
    #[serde(deserialize_with = "canonical_id")]
    target_revision_id: ReviewTargetRevisionId,
    #[serde(deserialize_with = "canonical_id")]
    asset_version_id: AssetVersionId,
    anchor: Anchor,
    availability: Availability,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum Availability {
    Ready {},
    NeedsConfirmation { reasons: Vec<Reason> },
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Reason {
    SourceChanged,
    SourceMissing,
    SourceUnreadable,
    SourceUnverified,
    LegacyUsageUnknown,
    LegacyEvidenceAbsent,
    ApplicabilityUnconfirmed,
}
impl From<ReviewPendingReason> for Reason {
    fn from(v: ReviewPendingReason) -> Self {
        match v {
            ReviewPendingReason::SourceChanged => Self::SourceChanged,
            ReviewPendingReason::SourceMissing => Self::SourceMissing,
            ReviewPendingReason::SourceUnreadable => Self::SourceUnreadable,
            ReviewPendingReason::SourceUnverified => Self::SourceUnverified,
            ReviewPendingReason::LegacyUsageUnknown => Self::LegacyUsageUnknown,
            ReviewPendingReason::LegacyEvidenceAbsent => Self::LegacyEvidenceAbsent,
            ReviewPendingReason::ApplicabilityUnconfirmed => Self::ApplicabilityUnconfirmed,
        }
    }
}
impl From<Reason> for ReviewPendingReason {
    fn from(v: Reason) -> Self {
        match v {
            Reason::SourceChanged => Self::SourceChanged,
            Reason::SourceMissing => Self::SourceMissing,
            Reason::SourceUnreadable => Self::SourceUnreadable,
            Reason::SourceUnverified => Self::SourceUnverified,
            Reason::LegacyUsageUnknown => Self::LegacyUsageUnknown,
            Reason::LegacyEvidenceAbsent => Self::LegacyEvidenceAbsent,
            Reason::ApplicabilityUnconfirmed => Self::ApplicabilityUnconfirmed,
        }
    }
}
impl From<&VersionedFeedback> for Feedback {
    fn from(v: &VersionedFeedback) -> Self {
        Self {
            feedback_id: v.id,
            text_revision_id: v.text_revision_id,
            text: v.text.clone(),
            created_at_ms: v.created_at_ms,
            history_ref: v.history_ref.as_ref().map(History::from),
            targets: v.targets.iter().map(Target::from).collect(),
        }
    }
}
impl TryFrom<Feedback> for VersionedFeedback {
    type Error = ReviewProtocolError;
    fn try_from(v: Feedback) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.feedback_id,
            text_revision_id: v.text_revision_id,
            text: v.text,
            created_at_ms: v.created_at_ms,
            history_ref: v.history_ref.map(Into::into),
            targets: v
                .targets
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        })
    }
}
impl From<&VersionedTarget> for Target {
    fn from(v: &VersionedTarget) -> Self {
        Self {
            target_id: v.id,
            target_revision_id: v.revision_id,
            asset_version_id: v.asset_version_id,
            anchor: (&v.anchor).into(),
            availability: match &v.availability {
                ReviewAvailability::Ready => Availability::Ready {},
                ReviewAvailability::NeedsConfirmation(reasons) => Availability::NeedsConfirmation {
                    reasons: reasons.iter().copied().map(Into::into).collect(),
                },
            },
        }
    }
}
impl TryFrom<Target> for VersionedTarget {
    type Error = ReviewProtocolError;
    fn try_from(v: Target) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.target_id,
            revision_id: v.target_revision_id,
            asset_version_id: v.asset_version_id,
            anchor: v.anchor.try_into()?,
            availability: match v.availability {
                Availability::Ready {} => ReviewAvailability::Ready,
                Availability::NeedsConfirmation { reasons } => {
                    ReviewAvailability::NeedsConfirmation(
                        reasons.into_iter().map(Into::into).collect(),
                    )
                }
            },
        })
    }
}
vector_adapter!(feedback, VersionedFeedback, Feedback);
vector_adapter!(targets, VersionedTarget, Target);

#[derive(Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(super) enum Anchor {
    Asset {},
    ImagePoint {
        x: f64,
        y: f64,
    },
    ImageArrow {
        tail: Point,
        head: Point,
    },
    ImageRect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    ImageStroke {
        points: Vec<Point>,
    },
    ImageEllipse {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    VideoPoint {
        position_us: u64,
    },
    VideoRange {
        start_us: u64,
        end_us: u64,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Point {
    x: f64,
    y: f64,
}
impl From<&FeedbackAnchor> for Anchor {
    fn from(v: &FeedbackAnchor) -> Self {
        match v {
            FeedbackAnchor::Asset => Self::Asset {},
            FeedbackAnchor::ImagePoint(p) => Self::ImagePoint { x: p.x(), y: p.y() },
            FeedbackAnchor::ImageArrow(a) => Self::ImageArrow {
                tail: Point {
                    x: a.tail().x(),
                    y: a.tail().y(),
                },
                head: Point {
                    x: a.head().x(),
                    y: a.head().y(),
                },
            },
            FeedbackAnchor::ImageRect(r) => Self::ImageRect {
                x: r.x(),
                y: r.y(),
                width: r.width(),
                height: r.height(),
            },
            FeedbackAnchor::ImageStroke(s) => Self::ImageStroke {
                points: s
                    .points()
                    .iter()
                    .map(|p| Point { x: p.x(), y: p.y() })
                    .collect(),
            },
            FeedbackAnchor::ImageEllipse(r) => Self::ImageEllipse {
                x: r.x(),
                y: r.y(),
                width: r.width(),
                height: r.height(),
            },
            FeedbackAnchor::VideoPoint { position_us } => Self::VideoPoint {
                position_us: *position_us,
            },
            FeedbackAnchor::VideoRange { start_us, end_us } => Self::VideoRange {
                start_us: *start_us,
                end_us: *end_us,
            },
        }
    }
}
impl TryFrom<Anchor> for FeedbackAnchor {
    type Error = ReviewProtocolError;
    fn try_from(v: Anchor) -> Result<Self, Self::Error> {
        Ok(match v {
            Anchor::Asset {} => Self::Asset,
            Anchor::ImagePoint { x, y } => Self::ImagePoint(
                NormalizedPoint::new(x, y).map_err(|_| ReviewProtocolError::InvalidData)?,
            ),
            Anchor::ImageArrow { tail, head } => Self::ImageArrow(
                NormalizedArrow::new(
                    NormalizedPoint::new(tail.x, tail.y)
                        .map_err(|_| ReviewProtocolError::InvalidData)?,
                    NormalizedPoint::new(head.x, head.y)
                        .map_err(|_| ReviewProtocolError::InvalidData)?,
                )
                .map_err(|_| ReviewProtocolError::InvalidData)?,
            ),
            Anchor::ImageRect {
                x,
                y,
                width,
                height,
            } => Self::ImageRect(
                NormalizedRect::new(x, y, width, height)
                    .map_err(|_| ReviewProtocolError::InvalidData)?,
            ),
            Anchor::ImageStroke { points } => Self::ImageStroke(
                ImageStroke::new(
                    points
                        .into_iter()
                        .map(|p| NormalizedPoint::new(p.x, p.y))
                        .collect::<Result<_, _>>()
                        .map_err(|_| ReviewProtocolError::InvalidData)?,
                )
                .map_err(|_| ReviewProtocolError::InvalidData)?,
            ),
            Anchor::ImageEllipse {
                x,
                y,
                width,
                height,
            } => Self::ImageEllipse(
                NormalizedRect::new(x, y, width, height)
                    .map_err(|_| ReviewProtocolError::InvalidData)?,
            ),
            Anchor::VideoPoint { position_us } => Self::VideoPoint { position_us },
            Anchor::VideoRange { start_us, end_us } => Self::VideoRange { start_us, end_us },
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct History {
    #[serde(deserialize_with = "canonical_id")]
    project_id: ProjectId,
    #[serde(deserialize_with = "canonical_id")]
    review_stream_id: ReviewStreamId,
    source: HistoryOrigin,
}
#[derive(Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum HistoryOrigin {
    Snapshot {
        snapshot: Snapshot,
        keys: Vec<Key>,
    },
    Legacy {
        #[serde(deserialize_with = "canonical_id")]
        round_id: ReviewRoundId,
        #[serde(with = "digest")]
        record_blake3: [u8; 32],
        targets: Vec<LegacyTarget>,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyTarget {
    #[serde(deserialize_with = "canonical_id")]
    round_id: ReviewRoundId,
    #[serde(deserialize_with = "canonical_id")]
    feedback_id: FeedbackId,
    target_index: u32,
}
impl From<&HistoryRef> for History {
    fn from(v: &HistoryRef) -> Self {
        Self {
            project_id: v.project_id,
            review_stream_id: v.stream_id,
            source: match &v.source {
                HistorySource::Snapshot { snapshot, keys } => HistoryOrigin::Snapshot {
                    snapshot: snapshot.into(),
                    keys: keys.iter().map(Into::into).collect(),
                },
                HistorySource::Legacy {
                    round_id,
                    record_blake3,
                    targets,
                } => HistoryOrigin::Legacy {
                    round_id: *round_id,
                    record_blake3: *record_blake3,
                    targets: targets
                        .iter()
                        .map(|t| LegacyTarget {
                            round_id: t.round_id,
                            feedback_id: t.feedback_id,
                            target_index: t.target_index,
                        })
                        .collect(),
                },
            },
        }
    }
}
impl From<History> for HistoryRef {
    fn from(v: History) -> Self {
        Self {
            project_id: v.project_id,
            stream_id: v.review_stream_id,
            source: match v.source {
                HistoryOrigin::Snapshot { snapshot, keys } => HistorySource::Snapshot {
                    snapshot: snapshot.into(),
                    keys: keys.into_iter().map(Into::into).collect(),
                },
                HistoryOrigin::Legacy {
                    round_id,
                    record_blake3,
                    targets,
                } => HistorySource::Legacy {
                    round_id,
                    record_blake3,
                    targets: targets
                        .into_iter()
                        .map(|t| LegacyTargetRef {
                            round_id: t.round_id,
                            feedback_id: t.feedback_id,
                            target_index: t.target_index,
                        })
                        .collect(),
                },
            },
        }
    }
}
vector_adapter!(histories, HistoryRef, History);

pub(in crate::review::protocol::v3) mod optional_history {
    use super::*;
    pub fn serialize<S: Serializer>(
        value: &Option<HistoryRef>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.as_ref().map(History::from).serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<HistoryRef>, D::Error> {
        Ok(Option::<History>::deserialize(deserializer)?.map(Into::into))
    }
}
