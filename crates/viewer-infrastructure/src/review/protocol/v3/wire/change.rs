use super::*;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Change {
    #[serde(deserialize_with = "canonical_id")]
    target_id: ReviewTargetId,
    #[serde(deserialize_with = "required_option")]
    before: Option<Key>,
    #[serde(deserialize_with = "required_option")]
    after: Option<Key>,
    kind: ChangeKind,
    #[serde(deserialize_with = "optional_id")]
    archive_id: Option<ReviewArchiveId>,
    #[serde(deserialize_with = "required_option")]
    historical_key: Option<Key>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ChangeKind {
    Added,
    Edited,
    Withdrawn,
    Archived,
    Restored,
    Rebound,
    AvailabilityChanged,
}
impl From<&ReviewChange> for Change {
    fn from(v: &ReviewChange) -> Self {
        Self {
            target_id: v.target_id,
            before: v.before.as_ref().map(Into::into),
            after: v.after.as_ref().map(Into::into),
            archive_id: v.archive_id,
            historical_key: v.historical_key.as_ref().map(Into::into),
            kind: match v.kind {
                ReviewChangeKind::Added => ChangeKind::Added,
                ReviewChangeKind::Edited => ChangeKind::Edited,
                ReviewChangeKind::Withdrawn => ChangeKind::Withdrawn,
                ReviewChangeKind::Archived => ChangeKind::Archived,
                ReviewChangeKind::Restored => ChangeKind::Restored,
                ReviewChangeKind::Rebound => ChangeKind::Rebound,
                ReviewChangeKind::AvailabilityChanged => ChangeKind::AvailabilityChanged,
            },
        }
    }
}
impl From<Change> for ReviewChange {
    fn from(v: Change) -> Self {
        Self {
            target_id: v.target_id,
            before: v.before.map(Into::into),
            after: v.after.map(Into::into),
            archive_id: v.archive_id,
            historical_key: v.historical_key.map(Into::into),
            kind: match v.kind {
                ChangeKind::Added => ReviewChangeKind::Added,
                ChangeKind::Edited => ReviewChangeKind::Edited,
                ChangeKind::Withdrawn => ReviewChangeKind::Withdrawn,
                ChangeKind::Archived => ReviewChangeKind::Archived,
                ChangeKind::Restored => ReviewChangeKind::Restored,
                ChangeKind::Rebound => ReviewChangeKind::Rebound,
                ChangeKind::AvailabilityChanged => ReviewChangeKind::AvailabilityChanged,
            },
        }
    }
}
vector_adapter!(changes, ReviewChange, Change);

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Delta {
    #[serde(deserialize_with = "canonical_id")]
    target_id: ReviewTargetId,
    #[serde(deserialize_with = "required_option")]
    before: Option<Key>,
    #[serde(deserialize_with = "required_option")]
    after: Option<Key>,
    text_changed: bool,
    anchor_changed: bool,
    binding_changed: bool,
    availability_changed: bool,
    #[serde(deserialize_with = "required_option")]
    removal_reason: Option<ChangeKind>,
}
impl From<&TargetDelta> for Delta {
    fn from(v: &TargetDelta) -> Self {
        Self {
            target_id: v.target_id,
            before: v.before.as_ref().map(Into::into),
            after: v.after.as_ref().map(Into::into),
            text_changed: v.text_changed,
            anchor_changed: v.anchor_changed,
            binding_changed: v.binding_changed,
            availability_changed: v.availability_changed,
            removal_reason: v.removal_reason.map(|kind| {
                Change::from(&ReviewChange {
                    target_id: v.target_id,
                    before: v.before,
                    after: v.after,
                    kind,
                    archive_id: None,
                    historical_key: None,
                })
                .kind
            }),
        }
    }
}
impl From<Delta> for TargetDelta {
    fn from(v: Delta) -> Self {
        Self {
            target_id: v.target_id,
            before: v.before.map(Into::into),
            after: v.after.map(Into::into),
            text_changed: v.text_changed,
            anchor_changed: v.anchor_changed,
            binding_changed: v.binding_changed,
            availability_changed: v.availability_changed,
            removal_reason: v.removal_reason.map(|kind| {
                ReviewChange::from(Change {
                    target_id: v.target_id,
                    before: None,
                    after: None,
                    kind,
                    archive_id: None,
                    historical_key: None,
                })
                .kind
            }),
        }
    }
}
vector_adapter!(deltas, TargetDelta, Delta);
