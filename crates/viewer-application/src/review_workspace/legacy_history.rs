use super::*;
use std::collections::{HashMap, HashSet};
use viewer_domain::{AssetVersionId, review::continuous::*};

/// Legacy targets have no v3 revision key. Continue with an explicit legacy selector and
/// independently confirmed current assets, never by pretending an old target has a new key.
pub(super) fn continue_legacy(
    repository: &dyn ContinuousReviewRepositoryPort,
    current: &ContinuousReviewState,
    envelope: &ReviewCommandEnvelope,
    prepared: &HashMap<AssetVersionId, crate::PreparedReviewAsset>,
) -> Result<ContinuousReviewState, ReviewWorkspaceError> {
    let ReviewWorkspaceCommand::ContinueLegacy {
        history_ref,
        bindings,
    } = &envelope.command
    else {
        return Err(ContinuousReviewError::InvalidData.into());
    };
    if history_ref.project_id != current.project_id || history_ref.stream_id != current.stream_id {
        return Err(ReviewWorkspaceError::WrongContext);
    }
    let HistorySource::Legacy {
        round_id,
        record_blake3,
        targets: selected,
    } = &history_ref.source
    else {
        return Err(ContinuousReviewError::InvalidData.into());
    };
    if selected.is_empty()
        || selected.len() != bindings.len()
        || bindings.len() != envelope.generated.targets.len()
    {
        return Err(ContinuousReviewError::InvalidData.into());
    }
    let record = repository.load_legacy(current.stream_id, *round_id)?;
    if record.reference.blake3 != *record_blake3 {
        return Err(ReviewCommitError::Integrity.into());
    }
    let (feedback, old_assets) = match &record.contents {
        LegacyReviewContents::Draft(d) => (&d.feedback, &d.assets),
        LegacyReviewContents::Completed(d) => (&d.feedback, &d.assets),
    };
    let source = feedback
        .iter()
        .find(|f| f.id == selected[0].feedback_id)
        .ok_or(ContinuousReviewError::MissingReference)?;
    let selected: HashSet<_> = selected.iter().copied().collect();
    if selected.len() != bindings.len() {
        return Err(ContinuousReviewError::DuplicateIdentity.into());
    }
    let mut seen = HashSet::new();
    let mut next = current.clone();
    let mut targets = vec![];
    for (binding, &(id, revision_id)) in bindings.iter().zip(&envelope.generated.targets) {
        let key = binding.legacy_target;
        if !binding.position_confirmed
            || key.round_id != *round_id
            || key.feedback_id != source.id
            || key.target_index as usize >= source.targets.len()
            || !selected.contains(&key)
            || !seen.insert(key)
        {
            return Err(ContinuousReviewError::NeedsConfirmation.into());
        }
        if old_assets
            .iter()
            .any(|a| a.id == binding.new_asset_version_id)
        {
            return Err(ContinuousReviewError::InvalidData.into());
        }
        super::editing::add_asset(&mut next, binding.new_asset_version_id, prepared)?;
        targets.push(VersionedTarget {
            id,
            revision_id,
            asset_version_id: binding.new_asset_version_id,
            anchor: binding.anchor.clone(),
            availability: ReviewAvailability::Ready,
        });
    }
    next.feedback.push(VersionedFeedback {
        id: envelope.generated.feedback_id,
        text_revision_id: envelope.generated.text_revision_id,
        text: source.text.clone(),
        created_at_ms: envelope.generated.created_at_ms,
        targets,
        history_ref: Some(history_ref.clone()),
    });
    next.snapshot_id = envelope.generated.snapshot_id;
    next.parent = None;
    next.validate()?;
    Ok(next)
}
