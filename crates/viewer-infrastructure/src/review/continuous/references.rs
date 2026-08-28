use super::super::v3;
use super::{history, repository::View};
use std::collections::HashSet;
use viewer_application::review_workspace::ReviewCommitError;
use viewer_domain::review::continuous::*;

pub(super) fn feedback_origins(
    view: &View,
    record: &v3::ReviewStateRecord,
) -> Result<(), ReviewCommitError> {
    let mut snapshots = std::collections::HashMap::new();
    let mut legacy = std::collections::HashMap::new();
    for origin in record
        .state
        .feedback
        .iter()
        .filter_map(|f| f.history_ref.as_ref())
    {
        if origin.project_id != view.index.project_id || origin.stream_id != record.state.stream_id
        {
            return Err(ReviewCommitError::Integrity);
        }
        match &origin.source {
            HistorySource::Snapshot { snapshot, keys } => snapshots
                .entry((origin.stream_id, *snapshot))
                .or_insert_with(Vec::new)
                .extend(keys.iter().copied()),
            HistorySource::Legacy {
                round_id,
                record_blake3,
                targets,
            } => legacy
                .entry((origin.stream_id, *round_id, *record_blake3))
                .or_insert_with(Vec::new)
                .extend(targets.iter().copied()),
        }
    }
    for ((stream, snapshot), keys) in snapshots {
        let basis = history::reachable_from(view, stream, record.state.parent, &snapshot)?;
        let available = target_keys(&basis.state);
        if keys.iter().any(|k| !available.contains(k)) {
            return Err(ReviewCommitError::Integrity);
        }
    }
    for ((stream, round, digest), targets) in legacy {
        let source = super::legacy::load(view, stream, round)?;
        if source.reference.blake3 != digest {
            return Err(ReviewCommitError::Integrity);
        }
        let feedback = match &source.contents {
            viewer_application::review_workspace::LegacyReviewContents::Draft(d) => &d.feedback,
            viewer_application::review_workspace::LegacyReviewContents::Completed(d) => &d.feedback,
        };
        let available: std::collections::HashMap<_, _> =
            feedback.iter().map(|f| (f.id, f.targets.len())).collect();
        if targets.iter().any(|t| {
            t.round_id != round
                || !available
                    .get(&t.feedback_id)
                    .is_some_and(|len| (t.target_index as usize) < *len)
        }) {
            return Err(ReviewCommitError::Integrity);
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
