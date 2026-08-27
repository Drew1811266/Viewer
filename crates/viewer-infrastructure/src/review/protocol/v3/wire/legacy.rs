use super::*;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyFeedback {
    #[serde(deserialize_with = "canonical_id")]
    feedback_id: FeedbackId,
    text: String,
    created_at_ms: i64,
    targets: Vec<LegacyFeedbackTarget>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyFeedbackTarget {
    #[serde(deserialize_with = "canonical_id")]
    asset_version_id: AssetVersionId,
    anchor: Anchor,
}
impl From<&viewer_domain::review::Feedback> for LegacyFeedback {
    fn from(v: &viewer_domain::review::Feedback) -> Self {
        Self {
            feedback_id: v.id,
            text: v.text.clone(),
            created_at_ms: v.created_at_ms,
            targets: v
                .targets
                .iter()
                .map(|t| LegacyFeedbackTarget {
                    asset_version_id: t.asset_version_id,
                    anchor: (&t.anchor).into(),
                })
                .collect(),
        }
    }
}
impl TryFrom<LegacyFeedback> for viewer_domain::review::Feedback {
    type Error = ReviewProtocolError;
    fn try_from(v: LegacyFeedback) -> Result<Self, Self::Error> {
        Self::new(
            v.feedback_id,
            v.text,
            v.created_at_ms,
            v.targets
                .into_iter()
                .map(|t| {
                    Ok(FeedbackTarget {
                        asset_version_id: t.asset_version_id,
                        anchor: t.anchor.try_into()?,
                    })
                })
                .collect::<Result<_, ReviewProtocolError>>()?,
        )
        .map_err(|_| ReviewProtocolError::InvalidData)
    }
}
vector_adapter!(
    legacy_feedback,
    viewer_domain::review::Feedback,
    LegacyFeedback
);
