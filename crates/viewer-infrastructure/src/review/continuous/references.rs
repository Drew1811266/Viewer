use super::super::v3;
use super::{history, repository::View};
use std::collections::HashSet;
use viewer_application::review_workspace::ReviewCommitError;
use viewer_domain::review::continuous::*;

pub(super) fn feedback_origins(
    view: &View,
    record: &v3::ReviewStateRecord,
) -> Result<(), ReviewCommitError> {
    for feedback in &record.state.feedback {
        let Some(origin) = &feedback.history_ref else {
            continue;
        };
        match &origin.source {
            HistorySource::Snapshot { snapshot, keys } => {
                let basis =
                    history::reachable_from(view, origin.stream_id, record.state.parent, snapshot)?;
                let available = target_keys(&basis.state);
                if keys.iter().any(|key| !available.contains(key)) {
                    return Err(ReviewCommitError::Integrity);
                }
            }
            HistorySource::Legacy { .. } => super::legacy::validate_origin(view, origin)?,
        }
    }
    Ok(())
}

pub(super) fn target_keys(state: &ContinuousReviewState) -> HashSet<TargetVersionKey> {
    state
        .feedback
        .iter()
        .flat_map(|feedback| {
            feedback.targets.iter().map(move |target| TargetVersionKey {
                feedback_id: feedback.id,
                text_revision_id: feedback.text_revision_id,
                target_id: target.id,
                target_revision_id: target.revision_id,
            })
        })
        .collect()
}

pub(super) fn transitions(
    view: &View,
    record: &v3::ReviewStateRecord,
    checkpoints: &[ArchiveCheckpoint],
) -> Result<(), ReviewCommitError> {
    let new_archives: HashSet<_> = checkpoints.iter().map(|v| v.archive_id).collect();
    for change in &record.changes {
        if record.state.parent.is_none() && change.kind != ReviewChangeKind::Added {
            return Err(ReviewCommitError::Integrity);
        }
        match change.kind {
            ReviewChangeKind::Archived
                if !change
                    .archive_id
                    .is_some_and(|id| new_archives.contains(&id)) =>
            {
                return Err(ReviewCommitError::Integrity);
            }
            ReviewChangeKind::Restored => {
                let id = change.archive_id.ok_or(ReviewCommitError::Integrity)?;
                let key = change.historical_key.ok_or(ReviewCommitError::Integrity)?;
                let archive = history::archive(view, record.state.stream_id, id)?;
                if !archive
                    .checkpoint
                    .groups
                    .iter()
                    .any(|g| g.targets.contains(&key))
                {
                    return Err(ReviewCommitError::Integrity);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn evidence_identity(
    candidate: &v3::ReviewStateRecord,
    historical: &v3::ReviewStateRecord,
) -> Result<(), ReviewCommitError> {
    let previous: std::collections::HashMap<_, _> = historical
        .evidence
        .iter()
        .map(|v| (v.asset_version_id, &v.capability))
        .collect();
    for binding in &candidate.evidence {
        let Some(old) = previous.get(&binding.asset_version_id) else {
            continue;
        };
        let same_base = match (&binding.capability, *old) {
            (
                v3::EvidenceCapability::Image { base: left, .. },
                v3::EvidenceCapability::Image { base: right, .. },
            ) => left == right,
            (v3::EvidenceCapability::NotImage {}, v3::EvidenceCapability::NotImage {})
            | (v3::EvidenceCapability::LegacyAbsent {}, v3::EvidenceCapability::LegacyAbsent {}) => {
                true
            }
            _ => false,
        };
        if !same_base {
            return Err(ReviewCommitError::Integrity);
        }
    }
    Ok(())
}

pub(super) fn declared_usage(
    view: &View,
    checkpoint: &ArchiveCheckpoint,
    adopted: &[v3::ReviewUsageRecord],
) -> Result<(), ReviewCommitError> {
    for group in &checkpoint.groups {
        if let ArchiveBasis::Known {
            snapshot,
            source: ArchiveBasisSource::AgentDeclared { usage_id },
        } = &group.basis
        {
            let stored;
            let usage = if let Some(usage) = adopted.iter().find(|v| v.declaration_id == *usage_id)
            {
                usage
            } else {
                stored = super::usage::read(view, checkpoint.stream_id, *usage_id)?;
                &stored
            };
            super::usage::validate(view, checkpoint.stream_id, usage)?;
            let selected: HashSet<_> = usage.targets.iter().collect();
            if usage.basis != *snapshot || group.targets.iter().any(|key| !selected.contains(key)) {
                return Err(ReviewCommitError::Integrity);
            }
        }
    }
    Ok(())
}
