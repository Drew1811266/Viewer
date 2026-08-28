//! Pure migration semantics, shared by application preparation and repository revalidation.
use super::*;
use std::collections::{HashMap, HashSet};
use viewer_domain::{
    review::{AssetVersion, Feedback, ReviewMedia, continuous::*},
    *,
};

struct Input<'a> {
    round: ReviewRoundId,
    draft: bool,
    feedback: &'a Feedback,
    assets: &'a [AssetVersion],
    indices: Vec<u32>,
    digest: [u8; 32],
}

fn inputs<'a>(
    inspection: &'a MigrationInspection,
    context: &ReviewWorkspaceContext,
    plan: &MigrationPlan,
) -> Result<Vec<Input<'a>>, ReviewWorkspaceError> {
    if inspection.inspection_digest != plan.inspection_digest {
        return Err(ReviewCommitError::StaleSnapshot.into());
    }
    let selected: &[LegacyTargetRef] = match &plan.choice {
        MigrationChoice::KeepHistoryOnly => &[],
        MigrationChoice::ContinueSelected { legacy_targets, .. } => legacy_targets,
    };
    let mut remaining: HashSet<_> = selected.iter().copied().collect();
    if remaining.len() != selected.len() {
        return Err(ContinuousReviewError::DuplicateIdentity.into());
    }
    let mut out = vec![];
    let mut add = |round,
                   draft,
                   feedback: &'a [Feedback],
                   assets: &'a [AssetVersion]|
     -> Result<(), ReviewWorkspaceError> {
        let reference = inspection
            .legacy_records
            .iter()
            .find(|r| {
                r.round_id == round && r.stream_id == context.stream_id && r.is_draft == draft
            })
            .ok_or(ContinuousReviewError::MissingReference)?;
        for f in feedback {
            let indices: Vec<_> = (0..f.targets.len())
                .filter(|index| {
                    let key = LegacyTargetRef {
                        round_id: round,
                        feedback_id: f.id,
                        target_index: *index as u32,
                    };
                    let was_selected = remaining.remove(&key);
                    draft || was_selected
                })
                .map(|i| i as u32)
                .collect();
            if !indices.is_empty() {
                out.push(Input {
                    round,
                    draft,
                    feedback: f,
                    assets,
                    indices,
                    digest: reference.blake3,
                });
            }
        }
        Ok(())
    };
    if let Some(active) = &inspection.active_draft {
        let d = &active.draft;
        if d.project_id != context.project_id
            || d.review_stream_id != context.stream_id
            || d.production != context.production
        {
            return Err(ReviewWorkspaceError::WrongContext);
        }
        add(d.review_round_id, true, &d.feedback, &d.assets)?;
    }
    for d in &inspection.completed_candidates {
        if d.project_id != context.project_id {
            return Err(ReviewWorkspaceError::WrongContext);
        }
        if d.review_stream_id == context.stream_id {
            if d.production != context.production {
                return Err(ReviewWorkspaceError::WrongContext);
            }
            add(d.review_round_id, false, &d.feedback, &d.assets)?;
        }
    }
    if !remaining.is_empty() {
        return Err(ContinuousReviewError::MissingReference.into());
    }
    if out.len() > 10_000 || out.iter().map(|v| v.indices.len()).sum::<usize>() > 100_000 {
        return Err(ContinuousReviewError::LimitExceeded.into());
    }
    Ok(out)
}

pub(super) fn generate(
    inspection: &MigrationInspection,
    context: &ReviewWorkspaceContext,
    plan: &MigrationPlan,
) -> Result<Vec<MigrationFeedbackIds>, ReviewWorkspaceError> {
    Ok(inputs(inspection, context, plan)?
        .into_iter()
        .map(|i| MigrationFeedbackIds {
            round_id: i.round,
            legacy_feedback_id: i.feedback.id,
            feedback_id: if i.draft {
                i.feedback.id
            } else {
                FeedbackId::new()
            },
            text_revision_id: ReviewTextRevisionId::new(),
            targets: i
                .indices
                .into_iter()
                .map(|index| (index, ReviewTargetId::new(), ReviewTargetRevisionId::new()))
                .collect(),
        })
        .collect())
}

/// This does not read files, capture images, assign random IDs, or publish anything.
pub fn prepare_migration_state(
    inspection: &MigrationInspection,
    envelope: &ReviewCommandEnvelope,
    prepared_assets: &[AssetVersion],
) -> Result<ContinuousReviewState, ReviewWorkspaceError> {
    let ReviewWorkspaceCommand::Migrate(plan) = &envelope.command else {
        return Err(ContinuousReviewError::InvalidData.into());
    };
    if envelope.expected_snapshot_id.is_some() {
        return Err(ReviewCommitError::StaleSnapshot.into());
    }
    let inputs = inputs(inspection, &envelope.context, plan)?;
    if inputs.len() != envelope.generated.migration.len() {
        return Err(ContinuousReviewError::InvalidData.into());
    }
    let bindings: &[MigrationBinding] = match &plan.choice {
        MigrationChoice::KeepHistoryOnly => &[],
        MigrationChoice::ContinueSelected { bindings, .. } => bindings,
    };
    let bindings: HashMap<_, _> = {
        let mut map = HashMap::new();
        for b in bindings {
            if !b.position_confirmed || map.insert(b.legacy_target, b).is_some() {
                return Err(ContinuousReviewError::NeedsConfirmation.into());
            }
        }
        map
    };
    let mut used_bindings = HashSet::new();
    let old_feedback: HashSet<_> = inspection
        .completed_candidates
        .iter()
        .flat_map(|r| &r.feedback)
        .map(|f| f.id)
        .chain(
            inspection
                .active_draft
                .iter()
                .flat_map(|d| &d.draft.feedback)
                .map(|f| f.id),
        )
        .collect();
    let old_assets: HashSet<_> = inspection
        .completed_candidates
        .iter()
        .flat_map(|r| &r.assets)
        .map(|a| a.id)
        .chain(
            inspection
                .active_draft
                .iter()
                .flat_map(|d| &d.draft.assets)
                .map(|a| a.id),
        )
        .collect();
    let prepared: HashMap<_, _> = prepared_assets.iter().map(|a| (a.id, a)).collect();
    if prepared.len() != prepared_assets.len() {
        return Err(ContinuousReviewError::DuplicateIdentity.into());
    }
    let mut next = ContinuousReviewState::empty(
        envelope.context.project_id,
        envelope.context.stream_id,
        envelope.generated.snapshot_id,
    );
    let mut assets = HashMap::<AssetVersionId, usize>::new();
    for (input, ids) in inputs.into_iter().zip(&envelope.generated.migration) {
        if ids.round_id != input.round
            || ids.legacy_feedback_id != input.feedback.id
            || ids.targets.iter().map(|(i, _, _)| *i).collect::<Vec<_>>() != input.indices
            || (input.draft && ids.feedback_id != input.feedback.id)
            || (!input.draft && old_feedback.contains(&ids.feedback_id))
        {
            return Err(ContinuousReviewError::InvalidData.into());
        }
        let mut targets = vec![];
        let mut provenance = vec![];
        for &(index, id, revision_id) in &ids.targets {
            let key = LegacyTargetRef {
                round_id: input.round,
                feedback_id: input.feedback.id,
                target_index: index,
            };
            let old = &input.feedback.targets[index as usize];
            let (asset, anchor, availability) = if let Some(binding) = bindings.get(&key) {
                used_bindings.insert(key);
                let asset = prepared
                    .get(&binding.new_asset_version_id)
                    .ok_or(ReviewWorkspaceError::PreviewRequired)?;
                if old_assets.contains(&asset.id) {
                    return Err(ContinuousReviewError::InvalidData.into());
                }
                (
                    (*asset).clone(),
                    binding.anchor.clone(),
                    ReviewAvailability::Ready,
                )
            } else {
                let asset = input
                    .assets
                    .iter()
                    .find(|a| a.id == old.asset_version_id)
                    .ok_or(ContinuousReviewError::MissingReference)?
                    .clone();
                let mut reasons = vec![];
                if matches!(asset.media, ReviewMedia::Image { .. }) {
                    reasons.push(ReviewPendingReason::LegacyEvidenceAbsent);
                }
                if !input.draft {
                    reasons.extend([
                        ReviewPendingReason::LegacyUsageUnknown,
                        ReviewPendingReason::ApplicabilityUnconfirmed,
                    ]);
                }
                let availability = if reasons.is_empty() {
                    ReviewAvailability::Ready
                } else {
                    ReviewAvailability::NeedsConfirmation(reasons)
                };
                (asset, old.anchor.clone(), availability)
            };
            let asset_id = asset.id;
            if let Some(&position) = assets.get(&asset_id) {
                if next.assets[position] != asset {
                    return Err(ContinuousReviewError::DuplicateIdentity.into());
                }
            } else {
                assets.insert(asset_id, next.assets.len());
                next.assets.push(asset);
            }
            targets.push(VersionedTarget {
                id,
                revision_id,
                asset_version_id: asset_id,
                anchor,
                availability,
            });
            provenance.push(key);
        }
        next.feedback.push(VersionedFeedback {
            id: ids.feedback_id,
            text_revision_id: ids.text_revision_id,
            text: input.feedback.text.clone(),
            created_at_ms: if input.draft {
                input.feedback.created_at_ms
            } else {
                envelope.generated.created_at_ms
            },
            targets,
            history_ref: Some(HistoryRef {
                project_id: next.project_id,
                stream_id: next.stream_id,
                source: HistorySource::Legacy {
                    round_id: input.round,
                    record_blake3: input.digest,
                    targets: provenance,
                },
            }),
        });
    }
    if used_bindings.len() != bindings.len() {
        return Err(ContinuousReviewError::MissingReference.into());
    }
    next.validate()?;
    Ok(next)
}
