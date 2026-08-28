use super::*;
use crate::review_evidence::HistorySelector;
use std::collections::{HashMap, HashSet};
use viewer_domain::{review::continuous::*, *};

impl ContinuousReviewService {
    pub async fn history(
        &self,
        selector: HistorySelector,
    ) -> Result<HistoryView, ReviewWorkspaceError> {
        let provider = self.provider.clone();
        let stream = self.context.stream_id;
        super::service::work(move || {
            let reader = provider.open_reader()?;
            let mut selections = vec![];
            let mut restore_actions = vec![];
            match selector {
                HistorySelector::Snapshot(reference) => selections.push((reference, vec![])),
                HistorySelector::Archive(id) => {
                    let archive = reader.load_archive(stream, id)?;
                    for group in &archive.groups {
                        let reference = match group.basis {
                            ArchiveBasis::Known { snapshot, .. } => snapshot,
                            ArchiveBasis::Unknown => archive.before,
                        };
                        selections.push((reference, group.targets.clone()));
                        restore_actions.extend_from_slice(&group.targets);
                    }
                }
                HistorySelector::Legacy(round) => {
                    let legacy = reader.load_legacy(stream, round)?;
                    return Ok(HistoryView {
                        selector,
                        entries: vec![],
                        legacy: Some(legacy),
                        limitations: vec![
                            ReviewHistoryLimitation::BackgroundOnly,
                            ReviewHistoryLimitation::LegacyEvidenceAbsent,
                            ReviewHistoryLimitation::UsageUnconfirmed,
                            ReviewHistoryLimitation::ExternalCopiesCannotBeRevoked,
                        ],
                        restore_actions: vec![],
                    });
                }
            }
            let mut combined = Vec::<(SnapshotRef, Vec<TargetVersionKey>)>::new();
            let mut positions = HashMap::new();
            let mut seen = HashSet::new();
            for (reference, targets) in selections {
                let position = *positions.entry(reference).or_insert_with(|| {
                    combined.push((reference, vec![]));
                    combined.len() - 1
                });
                for key in targets {
                    if seen.insert((reference, key)) {
                        combined[position].1.push(key);
                    }
                }
            }
            let mut entries = vec![];
            let mut retained = restore_actions.len().saturating_mul(128);
            for (reference, selected) in combined {
                let state = reader.load_snapshot(stream, &reference)?;
                retained = retained
                    .saturating_add(super::budget::snapshot_bytes(&state))
                    .saturating_add(selected.len().saturating_mul(128));
                if retained > super::budget::MAX_PREPARED_BYTES {
                    return Err(ReviewCommitError::LimitExceeded.into());
                }
                entries.push(HistoryEntry {
                    snapshot: reference,
                    feedback: state.state.feedback,
                    assets: state.state.assets,
                    evidence: state.evidence,
                    selected,
                });
            }
            Ok(HistoryView {
                selector,
                entries,
                legacy: None,
                limitations: vec![ReviewHistoryLimitation::BackgroundOnly],
                restore_actions,
            })
        })
        .await
    }
}

pub(super) fn continue_historical(
    repository: &dyn ContinuousReviewRepositoryPort,
    current: &ContinuousReviewState,
    envelope: &ReviewCommandEnvelope,
    prepared: &HashMap<AssetVersionId, crate::PreparedReviewAsset>,
    usages: &[super::usage::VerifiedUsage],
) -> Result<ContinuousReviewState, ReviewWorkspaceError> {
    let ReviewWorkspaceCommand::ContinueHistorical {
        history_ref,
        bindings,
    } = &envelope.command
    else {
        return Err(ContinuousReviewError::InvalidData.into());
    };
    if history_ref.project_id != current.project_id || history_ref.stream_id != current.stream_id {
        return Err(ReviewWorkspaceError::WrongContext);
    }
    let HistorySource::Snapshot { snapshot, keys } = &history_ref.source else {
        return Err(ReviewWorkspaceError::CapabilityUnavailable);
    };
    if keys.len() != bindings.len()
        || bindings.len() != envelope.generated.targets.len()
        || keys.is_empty()
    {
        return Err(ContinuousReviewError::InvalidData.into());
    }
    let history = repository.load_snapshot(current.stream_id, snapshot)?;
    let feedback = history
        .state
        .feedback
        .iter()
        .find(|f| f.id == keys[0].feedback_id)
        .ok_or(ContinuousReviewError::MissingReference)?;
    let mut next = current.clone();
    let mut targets = vec![];
    let mut seen = HashSet::new();
    for (binding, &(id, revision_id)) in bindings.iter().zip(&envelope.generated.targets) {
        if !keys.contains(&binding.target_key)
            || !seen.insert(binding.target_key)
            || binding.target_key.feedback_id != feedback.id
            || history.state.target_key(binding.target_key.target_id) != Some(binding.target_key)
        {
            return Err(ContinuousReviewError::MissingReference.into());
        }
        super::editing::add_asset(&mut next, binding.new_asset_version_id, prepared)?;
        let new = next
            .assets
            .iter()
            .find(|a| a.id == binding.new_asset_version_id)
            .ok_or(ContinuousReviewError::MissingReference)?;
        super::usage::validate_binding(&history.state, new, binding, usages)?;
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
        text: feedback.text.clone(),
        created_at_ms: envelope.generated.created_at_ms,
        targets,
        history_ref: Some(history_ref.clone()),
    });
    next.snapshot_id = envelope.generated.snapshot_id;
    next.parent = None;
    next.validate()?;
    Ok(next)
}
