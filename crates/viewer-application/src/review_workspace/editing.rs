use super::*;
use std::collections::{HashMap, HashSet};
use viewer_domain::{review::continuous::*, *};

pub(super) fn save(
    current: &ContinuousReviewState,
    envelope: &ReviewCommandEnvelope,
    assets: &HashMap<AssetVersionId, crate::PreparedReviewAsset>,
) -> Result<ContinuousReviewState, ReviewWorkspaceError> {
    let ReviewWorkspaceCommand::SaveFeedback {
        feedback_id,
        text,
        targets,
    } = &envelope.command
    else {
        return Err(ContinuousReviewError::InvalidData.into());
    };
    let ids = &envelope.generated;
    let mut next = current.clone();
    let mut feedback = match feedback_id {
        Some(id) => update_feedback_text(
            current
                .feedback
                .iter()
                .find(|f| f.id == *id)
                .ok_or(ContinuousReviewError::MissingReference)?,
            ids.text_revision_id,
            text,
        )?,
        None => VersionedFeedback {
            id: ids.feedback_id,
            text_revision_id: ids.text_revision_id,
            text: text.clone(),
            created_at_ms: ids.created_at_ms,
            targets: vec![],
            history_ref: None,
        },
    };
    if targets.len() != ids.targets.len() {
        return Err(ContinuousReviewError::InvalidData.into());
    }
    let mut seen = HashSet::new();
    for (edit, &(id, revision)) in targets.iter().zip(&ids.targets) {
        match edit {
            TargetEdit::Add {
                asset_version_id,
                anchor,
            } => {
                add_asset(&mut next, *asset_version_id, assets)?;
                feedback.targets.push(VersionedTarget {
                    id,
                    revision_id: revision,
                    asset_version_id: *asset_version_id,
                    anchor: anchor.clone(),
                    availability: ReviewAvailability::Ready,
                });
            }
            TargetEdit::Redraw {
                key,
                asset_version_id,
                anchor,
            } => {
                if !seen.insert(key.target_id)
                    || Some(key.feedback_id) != *feedback_id
                    || current.target_key(key.target_id) != Some(*key)
                {
                    return Err(ContinuousReviewError::StaleSnapshot.into());
                }
                if feedback
                    .targets
                    .iter()
                    .find(|t| t.id == key.target_id)
                    .is_none_or(|t| t.asset_version_id != *asset_version_id)
                {
                    return Err(ContinuousReviewError::InvalidData.into());
                }
                feedback = replace_target(&feedback, key.target_id, revision, anchor.clone())?;
            }
        }
    }
    feedback.validate()?;
    if let Some(position) = next.feedback.iter().position(|f| f.id == feedback.id) {
        if next.feedback[position] == feedback {
            return Err(ReviewWorkspaceError::NoChanges);
        }
        next.feedback[position] = feedback;
    } else {
        next.feedback.push(feedback);
    }
    next.snapshot_id = ids.snapshot_id;
    next.parent = None;
    next.validate()?;
    Ok(next)
}

pub(super) fn add_asset(
    state: &mut ContinuousReviewState,
    id: AssetVersionId,
    assets: &HashMap<AssetVersionId, crate::PreparedReviewAsset>,
) -> Result<(), ReviewWorkspaceError> {
    if state.assets.iter().any(|a| a.id == id) {
        return Ok(());
    }
    let prepared = assets
        .get(&id)
        .ok_or(ReviewWorkspaceError::PreviewRequired)?;
    if prepared.failure.is_some() {
        return Err(crate::ReviewAssetError::Unavailable.into());
    }
    state.assets.push(prepared.asset.clone());
    Ok(())
}

pub(super) fn changes(
    before: &ContinuousReviewState,
    after: &ContinuousReviewState,
    kind: ReviewChangeKind,
) -> Vec<ReviewChange> {
    let old: HashMap<_, _> = before
        .feedback
        .iter()
        .flat_map(|f| {
            f.targets.iter().map(move |t| {
                (
                    t.id,
                    TargetVersionKey {
                        feedback_id: f.id,
                        text_revision_id: f.text_revision_id,
                        target_id: t.id,
                        target_revision_id: t.revision_id,
                    },
                )
            })
        })
        .collect();
    let new: HashMap<_, _> = after
        .feedback
        .iter()
        .flat_map(|f| {
            f.targets.iter().map(move |t| {
                (
                    t.id,
                    TargetVersionKey {
                        feedback_id: f.id,
                        text_revision_id: f.text_revision_id,
                        target_id: t.id,
                        target_revision_id: t.revision_id,
                    },
                )
            })
        })
        .collect();
    // Preserve deterministic state order, never HashMap iteration order in a committed record.
    let mut result = vec![];
    for target in before.feedback.iter().flat_map(|f| &f.targets) {
        let previous = old[&target.id];
        let next = new.get(&target.id).copied();
        if next != Some(previous) {
            result.push(ReviewChange {
                target_id: target.id,
                before: Some(previous),
                after: next,
                kind: if next.is_none() {
                    ReviewChangeKind::Withdrawn
                } else {
                    kind
                },
                archive_id: None,
                historical_key: None,
            });
        }
    }
    for target in after.feedback.iter().flat_map(|f| &f.targets) {
        if !old.contains_key(&target.id) {
            result.push(ReviewChange {
                target_id: target.id,
                before: None,
                after: new.get(&target.id).copied(),
                kind: ReviewChangeKind::Added,
                archive_id: None,
                historical_key: None,
            });
        }
    }
    result
}
