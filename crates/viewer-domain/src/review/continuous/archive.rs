use std::collections::{HashMap, HashSet};

use super::mutation::prepare_next;
use super::validation::{MAX_HISTORY_STATES, ValidatedStates, target_index};
use super::{
    ContinuousReviewError, ContinuousReviewState, ReviewChange, ReviewChangeKind, SnapshotRef,
    TargetVersionKey, withdraw_targets,
};
use crate::review::{MAX_FEEDBACK_ITEMS_PER_ROUND, MAX_TARGETS_PER_FEEDBACK};
use crate::{ProjectId, ReviewArchiveId, ReviewSnapshotId, ReviewStreamId, ReviewUsageId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveBasis {
    Known {
        snapshot: SnapshotRef,
        source: ArchiveBasisSource,
    },
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveBasisSource {
    AgentDeclared { usage_id: ReviewUsageId },
    UserSelected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveGroup {
    pub basis: ArchiveBasis,
    pub targets: Vec<TargetVersionKey>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveSelection {
    pub expected_snapshot_id: ReviewSnapshotId,
    pub groups: Vec<ArchiveGroup>,
}

/// Resolved coverage supplied by the verified archive/restore chain, not a directory scan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveCoverage {
    pub archive_id: ReviewArchiveId,
    pub key: TargetVersionKey,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveDisposition {
    RemoveCurrent,
    RetainLaterEdit,
    AlreadyAbsent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveRetention {
    pub basis: TargetVersionKey,
    pub current: Option<TargetVersionKey>,
    pub disposition: ArchiveDisposition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchivePlan {
    pub expected_snapshot_id: ReviewSnapshotId,
    /// Only newly covered keys; duplicate active coverage never creates another fact.
    pub groups: Vec<ArchiveGroup>,
    pub removed: Vec<TargetVersionKey>,
    pub retained: Vec<ArchiveRetention>,
    pub already_covered: Vec<TargetVersionKey>,
}

impl ArchivePlan {
    pub fn is_noop(&self) -> bool {
        self.groups.is_empty() && self.removed.is_empty()
    }

    pub fn changes(&self, archive_id: ReviewArchiveId) -> Vec<ReviewChange> {
        self.removed
            .iter()
            .map(|&key| ReviewChange {
                target_id: key.target_id,
                before: Some(key),
                after: None,
                kind: ReviewChangeKind::Archived,
                archive_id: Some(archive_id),
                historical_key: Some(key),
            })
            .chain(self.retained.iter().map(|entry| ReviewChange {
                target_id: entry.basis.target_id,
                before: entry.current,
                after: entry.current,
                kind: ReviewChangeKind::Archived,
                archive_id: Some(archive_id),
                historical_key: Some(entry.basis),
            }))
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveCheckpoint {
    pub project_id: ProjectId,
    pub stream_id: ReviewStreamId,
    pub archive_id: ReviewArchiveId,
    pub created_at_ms: i64,
    /// The confirmed C, including its real digest even when the usage basis is unknown.
    pub before: SnapshotRef,
    pub groups: Vec<ArchiveGroup>,
    pub removed: Vec<TargetVersionKey>,
    pub retained: Vec<ArchiveRetention>,
}

impl ArchiveCheckpoint {
    pub fn from_plan(
        current: &ContinuousReviewState,
        before: SnapshotRef,
        plan: &ArchivePlan,
        archive_id: ReviewArchiveId,
        created_at_ms: i64,
    ) -> Result<Self, ContinuousReviewError> {
        current.validate()?;
        if before.snapshot_id != current.snapshot_id
            || plan.expected_snapshot_id != current.snapshot_id
        {
            return Err(ContinuousReviewError::StaleSnapshot);
        }
        if plan.is_noop()
            || created_at_ms < 0
            || current
                .feedback
                .iter()
                .any(|feedback| feedback.created_at_ms > created_at_ms)
        {
            return Err(ContinuousReviewError::InvalidData);
        }
        validate_results(current, &plan.groups, &plan.removed, &plan.retained)?;
        for group in &plan.groups {
            if matches!(&group.basis, ArchiveBasis::Known { snapshot, .. } if snapshot.snapshot_id == before.snapshot_id && snapshot != &before)
            {
                return Err(ContinuousReviewError::InvalidData);
            }
        }
        Ok(Self {
            project_id: current.project_id,
            stream_id: current.stream_id,
            archive_id,
            created_at_ms,
            before,
            groups: plan.groups.clone(),
            removed: plan.removed.clone(),
            retained: plan.retained.clone(),
        })
    }
}

pub fn classify_archive(
    current: Option<TargetVersionKey>,
    basis: TargetVersionKey,
) -> ArchiveDisposition {
    match current {
        None => ArchiveDisposition::AlreadyAbsent,
        Some(key) if key == basis => ArchiveDisposition::RemoveCurrent,
        Some(_) => ArchiveDisposition::RetainLaterEdit,
    }
}

pub fn plan_archive(
    current: &ContinuousReviewState,
    bases: &[ContinuousReviewState],
    selection: &ArchiveSelection,
    coverage: &[ArchiveCoverage],
) -> Result<ArchivePlan, ContinuousReviewError> {
    use ContinuousReviewError::*;
    if selection.expected_snapshot_id != current.snapshot_id {
        return Err(StaleSnapshot);
    }
    validate_groups(&selection.groups)?;
    if coverage.len() > MAX_FEEDBACK_ITEMS_PER_ROUND * MAX_TARGETS_PER_FEEDBACK {
        return Err(LimitExceeded);
    }
    let states = ValidatedStates::new(current, bases)?;
    let current_targets = &states.targets[&current.snapshot_id];
    let mut covered = HashSet::new();
    let mut coverage_ids = HashSet::new();
    for entry in coverage {
        if !coverage_ids.insert((entry.archive_id, entry.key))
            || (entry.active && !covered.insert(entry.key))
        {
            return Err(DuplicateIdentity);
        }
    }
    let mut plan = ArchivePlan {
        expected_snapshot_id: current.snapshot_id,
        groups: vec![],
        removed: vec![],
        retained: vec![],
        already_covered: vec![],
    };
    for group in &selection.groups {
        let basis_targets = match &group.basis {
            ArchiveBasis::Known { snapshot, .. } => {
                if !states.bases.contains(&snapshot.snapshot_id) {
                    return Err(MissingReference);
                }
                &states.targets[&snapshot.snapshot_id]
            }
            ArchiveBasis::Unknown => current_targets,
        };
        let mut new_group = ArchiveGroup {
            basis: group.basis.clone(),
            targets: vec![],
        };
        for &key in &group.targets {
            let basis_key = basis_targets
                .get(&key.target_id)
                .map(|(feedback, target)| feedback.key(target));
            if basis_key != Some(key) {
                return Err(if matches!(group.basis, ArchiveBasis::Unknown) {
                    SelectionConflict
                } else {
                    MissingReference
                });
            }
            if covered.contains(&key) {
                plan.already_covered.push(key);
                continue;
            }
            new_group.targets.push(key);
            let current_key = current_targets
                .get(&key.target_id)
                .map(|(feedback, target)| feedback.key(target));
            match classify_archive(current_key, key) {
                ArchiveDisposition::RemoveCurrent => plan.removed.push(key),
                disposition => plan.retained.push(ArchiveRetention {
                    basis: key,
                    current: current_key,
                    disposition,
                }),
            }
        }
        if !new_group.targets.is_empty() {
            plan.groups.push(new_group);
        }
    }
    Ok(plan)
}

/// Applies a newly recomputed plan, never trusting caller-edited removal arrays.
pub fn apply_archive(
    current: &ContinuousReviewState,
    bases: &[ContinuousReviewState],
    selection: &ArchiveSelection,
    coverage: &[ArchiveCoverage],
    snapshot_id: ReviewSnapshotId,
) -> Result<(ContinuousReviewState, ArchivePlan), ContinuousReviewError> {
    let plan = plan_archive(current, bases, selection, coverage)?;
    let next = if plan.is_noop() {
        current.clone()
    } else if plan.removed.is_empty() {
        prepare_next(current, snapshot_id)?
    } else {
        withdraw_targets(
            current,
            &plan
                .removed
                .iter()
                .map(|key| key.target_id)
                .collect::<Vec<_>>(),
            snapshot_id,
        )?
    };
    if !plan.is_noop()
        && bases
            .iter()
            .any(|basis| basis.snapshot_id == next.snapshot_id)
    {
        return Err(ContinuousReviewError::DuplicateIdentity);
    }
    next.validate()?;
    Ok((next, plan))
}

pub(super) fn validate_groups(groups: &[ArchiveGroup]) -> Result<(), ContinuousReviewError> {
    use ContinuousReviewError::*;
    if groups.len() > MAX_HISTORY_STATES {
        return Err(LimitExceeded);
    }
    let mut selected = HashSet::new();
    let mut references = HashMap::new();
    for group in groups {
        if group.targets.len() > MAX_FEEDBACK_ITEMS_PER_ROUND * MAX_TARGETS_PER_FEEDBACK {
            return Err(LimitExceeded);
        }
        if group.targets.is_empty() {
            return Err(InvalidData);
        }
        if let ArchiveBasis::Known { snapshot, .. } = &group.basis
            && references
                .insert(snapshot.snapshot_id, snapshot.blake3)
                .is_some_and(|previous| previous != snapshot.blake3)
        {
            return Err(InvalidData);
        }
        for key in &group.targets {
            if !selected.insert(key.target_id) {
                return Err(SelectionConflict);
            }
        }
    }
    Ok(())
}

pub(super) fn validate_results(
    current: &ContinuousReviewState,
    groups: &[ArchiveGroup],
    removed: &[TargetVersionKey],
    retained: &[ArchiveRetention],
) -> Result<(), ContinuousReviewError> {
    validate_groups(groups)?;
    let targets = target_index(current);
    let mut expected_removed = vec![];
    let mut expected_retained = vec![];
    for group in groups {
        for key in &group.targets {
            let current_key = targets
                .get(&key.target_id)
                .map(|(feedback, target)| feedback.key(target));
            if matches!(group.basis, ArchiveBasis::Unknown) && current_key != Some(*key) {
                return Err(ContinuousReviewError::SelectionConflict);
            }
            match classify_archive(current_key, *key) {
                ArchiveDisposition::RemoveCurrent => expected_removed.push(*key),
                disposition => expected_retained.push(ArchiveRetention {
                    basis: *key,
                    current: current_key,
                    disposition,
                }),
            }
        }
    }
    if expected_removed != removed || expected_retained != retained {
        return Err(ContinuousReviewError::InvalidData);
    }
    Ok(())
}
