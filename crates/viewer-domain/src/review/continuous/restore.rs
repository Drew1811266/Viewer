use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::mutation::prepare_next;
use super::validation::ValidatedStates;
use super::*;
use crate::review::{AssetVersion, FeedbackAnchor, round::validate_anchor};
use crate::{
    AssetVersionId, FeedbackId, ReviewSnapshotId, ReviewTargetId, ReviewTargetRevisionId,
    ReviewTextRevisionId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestoreDisposition {
    Restore,
    ConfirmConflict,
    AlreadyCurrent,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RestoreDecision {
    pub historical_key: TargetVersionKey,
    pub choice: RestoreChoice,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RestoreChoice {
    PreserveCurrent,
    UseHistorical,
    ContinueAsNew {
        feedback_id: FeedbackId,
        text_revision_id: ReviewTextRevisionId,
        target_id: ReviewTargetId,
        target_revision_id: ReviewTargetRevisionId,
        target_asset_version_id: AssetVersionId,
        /// None retains a provisional historical anchor, never executable without confirmation.
        confirmed_anchor: Option<FeedbackAnchor>,
        created_at_ms: i64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct RestoreFeedbackContent {
    pub id: FeedbackId,
    pub text_revision_id: ReviewTextRevisionId,
    pub text: Arc<str>,
    pub created_at_ms: i64,
    pub history_ref: Option<HistoryRef>,
}

impl RestoreFeedbackContent {
    fn from_feedback(feedback: &VersionedFeedback) -> Self {
        Self {
            id: feedback.id,
            text_revision_id: feedback.text_revision_id,
            text: Arc::from(feedback.text.as_str()),
            created_at_ms: feedback.created_at_ms,
            history_ref: feedback.history_ref.clone(),
        }
    }

    fn matches(&self, feedback: &VersionedFeedback, target: &VersionedTarget) -> bool {
        self.id == feedback.id
            && self.text_revision_id == feedback.text_revision_id
            && self.text.as_ref() == feedback.text
            && self.created_at_ms == feedback.created_at_ms
            && self.history_ref == feedback.history_ref
            && feedback.targets.as_slice() == std::slice::from_ref(target)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RestoredFeedback {
    pub historical_key: TargetVersionKey,
    /// Shared per feedback version, not a full feedback clone for every selected target.
    pub feedback: Arc<RestoreFeedbackContent>,
    pub target: VersionedTarget,
    pub asset: Arc<AssetVersion>,
    pub continued_as_new: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RestorePlan {
    pub expected_snapshot_id: ReviewSnapshotId,
    pub restored: Vec<RestoredFeedback>,
    pub conflicts: Vec<TargetVersionKey>,
    pub coverage_reversals: Vec<ArchiveCoverage>,
    /// Restoring saved availability does not authorize execution. Readers must check the source.
    pub requires_source_check: Vec<ReviewTargetId>,
}

pub fn classify_restore(
    current: Option<TargetVersionKey>,
    historical: TargetVersionKey,
) -> RestoreDisposition {
    match current {
        None => RestoreDisposition::Restore,
        Some(key) if key == historical => RestoreDisposition::AlreadyCurrent,
        Some(_) => RestoreDisposition::ConfirmConflict,
    }
}

/// Decisions are the explicit selection. An empty selection never resurrects deleted work.
/// The application implements immediate undo by selecting precisely the checkpoint's removed keys.
pub fn plan_restore(
    current: &ContinuousReviewState,
    archive: &ArchiveCheckpoint,
    bases: &[ContinuousReviewState],
    decisions: &[RestoreDecision],
) -> Result<RestorePlan, ContinuousReviewError> {
    use ContinuousReviewError::*;
    let states = ValidatedStates::new(current, bases)?;
    validate_checkpoint(current, archive, bases, &states)?;
    let historical: HashMap<_, _> = archive
        .groups
        .iter()
        .flat_map(|group| group.targets.iter().map(move |key| (*key, group)))
        .collect();
    if decisions.len() > historical.len() {
        return Err(
            if decisions.len() > crate::review::MAX_TARGETS_PER_FEEDBACK {
                LimitExceeded
            } else {
                DuplicateIdentity
            },
        );
    }
    let mut selected = HashSet::new();
    let mut selected_targets = HashSet::new();
    let mut selected_texts = HashMap::new();
    for decision in decisions {
        if !historical.contains_key(&decision.historical_key) {
            return Err(MissingReference);
        }
        if !selected.insert(decision.historical_key) {
            return Err(DuplicateIdentity);
        }
        if matches!(decision.choice, RestoreChoice::UseHistorical) {
            selected_targets.insert(decision.historical_key.target_id);
            if selected_texts
                .insert(
                    decision.historical_key.feedback_id,
                    decision.historical_key.text_revision_id,
                )
                .is_some_and(|previous| previous != decision.historical_key.text_revision_id)
            {
                return Err(SelectionConflict);
            }
        }
    }
    let current_feedback: HashMap<_, _> = current
        .feedback
        .iter()
        .map(|feedback| (feedback.id, feedback))
        .collect();
    let current_targets = &states.targets[&current.snapshot_id];
    let current_assets: HashMap<_, _> = current
        .assets
        .iter()
        .map(|asset| (asset.id, asset))
        .collect();
    let assets: HashMap<_, _> = bases
        .iter()
        .flat_map(|state| &state.assets)
        .map(|asset| (asset.id, asset))
        .collect();
    let mut identities = RestoreIdentities::new(&states);
    let mut shared_feedback = HashMap::new();
    let mut shared_assets = HashMap::new();
    let shared_conflicts: HashSet<_> = current
        .feedback
        .iter()
        .filter(|feedback| {
            selected_texts
                .get(&feedback.id)
                .is_some_and(|revision| *revision != feedback.text_revision_id)
                && feedback
                    .targets
                    .iter()
                    .any(|target| !selected_targets.contains(&target.id))
        })
        .map(|feedback| feedback.id)
        .collect();
    let mut plan = RestorePlan {
        expected_snapshot_id: current.snapshot_id,
        restored: vec![],
        conflicts: vec![],
        coverage_reversals: vec![],
        requires_source_check: vec![],
    };
    for decision in decisions {
        let key = decision.historical_key;
        let group = historical[&key];
        let reference = match group.basis {
            ArchiveBasis::Known { snapshot, .. } => snapshot,
            ArchiveBasis::Unknown => archive.before,
        };
        let (old_feedback, old_target) = states.targets[&reference.snapshot_id][&key.target_id];
        if matches!(decision.choice, RestoreChoice::PreserveCurrent) {
            continue;
        }
        let mut feedback = Arc::clone(
            shared_feedback
                .entry((old_feedback.id, old_feedback.text_revision_id))
                .or_insert_with(|| Arc::new(RestoreFeedbackContent::from_feedback(old_feedback))),
        );
        let mut target = old_target.clone();
        let continued_as_new = matches!(decision.choice, RestoreChoice::ContinueAsNew { .. });
        let asset = match &decision.choice {
            RestoreChoice::UseHistorical => {
                if current_targets
                    .get(&key.target_id)
                    .is_some_and(|(feedback, target)| feedback.key(target) == key)
                {
                    continue;
                }
                if shared_conflicts.contains(&key.feedback_id) {
                    plan.conflicts.push(key);
                    continue;
                }
                *assets
                    .get(&old_target.asset_version_id)
                    .ok_or(MissingReference)?
            }
            RestoreChoice::ContinueAsNew {
                feedback_id,
                text_revision_id,
                target_id,
                target_revision_id,
                target_asset_version_id,
                confirmed_anchor,
                created_at_ms,
            } => {
                if *created_at_ms < archive.created_at_ms {
                    return Err(InvalidData);
                }
                feedback = Arc::new(RestoreFeedbackContent {
                    id: *feedback_id,
                    text_revision_id: *text_revision_id,
                    text: Arc::clone(&feedback.text),
                    created_at_ms: *created_at_ms,
                    history_ref: Some(HistoryRef {
                        project_id: current.project_id,
                        stream_id: current.stream_id,
                        source: HistorySource::Snapshot {
                            snapshot: reference,
                            keys: vec![key],
                        },
                    }),
                });
                target = VersionedTarget {
                    id: *target_id,
                    revision_id: *target_revision_id,
                    asset_version_id: *target_asset_version_id,
                    anchor: confirmed_anchor
                        .clone()
                        .unwrap_or_else(|| old_target.anchor.clone()),
                    availability: if confirmed_anchor.is_some() {
                        ReviewAvailability::Ready
                    } else {
                        ReviewAvailability::NeedsConfirmation(vec![
                            ReviewPendingReason::ApplicabilityUnconfirmed,
                        ])
                    },
                };
                if current_feedback
                    .get(feedback_id)
                    .is_some_and(|existing| feedback.matches(existing, &target))
                {
                    continue;
                }
                identities.claim(&feedback, &target)?;
                *current_assets
                    .get(target_asset_version_id)
                    .ok_or(MissingReference)?
            }
            RestoreChoice::PreserveCurrent => {
                unreachable!("handled before preparing a restoration")
            }
        };
        // Text/origin came from a validated historical record; new timestamps and identities
        // were checked above. Newly supplied video ranges still need their intrinsic bounds.
        if matches!(target.anchor, FeedbackAnchor::VideoRange { start_us, end_us } if start_us >= end_us)
        {
            return Err(InvalidData);
        }
        validate_anchor(asset, &target.anchor).map_err(|_| InvalidData)?;
        let asset = Arc::clone(
            shared_assets
                .entry(asset.id)
                .or_insert_with(|| Arc::new(asset.clone())),
        );
        if !continued_as_new {
            plan.coverage_reversals.push(ArchiveCoverage {
                archive_id: archive.archive_id,
                key,
                active: false,
            });
        }
        plan.requires_source_check.push(target.id);
        plan.restored.push(RestoredFeedback {
            historical_key: key,
            feedback,
            target,
            asset,
            continued_as_new,
        });
    }
    Ok(plan)
}

pub fn apply_restore(
    current: &ContinuousReviewState,
    archive: &ArchiveCheckpoint,
    bases: &[ContinuousReviewState],
    decisions: &[RestoreDecision],
    snapshot_id: ReviewSnapshotId,
) -> Result<(ContinuousReviewState, RestorePlan), ContinuousReviewError> {
    let plan = plan_restore(current, archive, bases, decisions)?;
    if !plan.conflicts.is_empty() {
        return Err(ContinuousReviewError::NeedsConfirmation);
    }
    if plan.restored.is_empty() {
        return Ok((current.clone(), plan));
    }
    if bases.iter().any(|basis| basis.snapshot_id == snapshot_id) {
        return Err(ContinuousReviewError::DuplicateIdentity);
    }
    let mut next = prepare_next(current, snapshot_id)?;
    let mut asset_ids: HashSet<_> = next.assets.iter().map(|asset| asset.id).collect();
    let mut feedback_positions: HashMap<_, _> = next
        .feedback
        .iter()
        .enumerate()
        .map(|(i, feedback)| (feedback.id, i))
        .collect();
    let mut target_positions: HashMap<_, _> = next
        .feedback
        .iter()
        .flat_map(|feedback| {
            feedback
                .targets
                .iter()
                .enumerate()
                .map(|(i, target)| (target.id, i))
        })
        .collect();
    for item in &plan.restored {
        if asset_ids.insert(item.asset.id) {
            next.assets.push((*item.asset).clone());
        }
        let incoming = &item.feedback;
        if let Some(&position) = feedback_positions.get(&incoming.id) {
            let feedback = &mut next.feedback[position];
            if feedback.text_revision_id != incoming.text_revision_id {
                feedback.text = incoming.text.to_string();
                feedback.text_revision_id = incoming.text_revision_id;
            }
            if let Some(&position) = target_positions.get(&item.target.id) {
                feedback.targets[position] = item.target.clone();
            } else {
                target_positions.insert(item.target.id, feedback.targets.len());
                feedback.targets.push(item.target.clone());
            }
        } else {
            feedback_positions.insert(incoming.id, next.feedback.len());
            target_positions.insert(item.target.id, 0);
            next.feedback.push(VersionedFeedback {
                id: incoming.id,
                text_revision_id: incoming.text_revision_id,
                text: incoming.text.to_string(),
                created_at_ms: incoming.created_at_ms,
                targets: vec![item.target.clone()],
                history_ref: incoming.history_ref.clone(),
            });
        }
    }
    next.validate()?;
    Ok((next, plan))
}

fn validate_checkpoint(
    current: &ContinuousReviewState,
    archive: &ArchiveCheckpoint,
    bases: &[ContinuousReviewState],
    states: &ValidatedStates<'_>,
) -> Result<(), ContinuousReviewError> {
    use ContinuousReviewError::*;
    if archive.project_id != current.project_id
        || archive.stream_id != current.stream_id
        || archive.created_at_ms < 0
        || archive.groups.is_empty()
    {
        return Err(InvalidData);
    }
    let before = states
        .states
        .get(&archive.before.snapshot_id)
        .filter(|_| states.bases.contains(&archive.before.snapshot_id))
        .ok_or(MissingReference)?;
    let expected = plan_archive(
        before,
        bases,
        &ArchiveSelection {
            expected_snapshot_id: archive.before.snapshot_id,
            groups: archive.groups.clone(),
        },
        &[],
    )?;
    if expected.removed != archive.removed || expected.retained != archive.retained {
        return Err(InvalidData);
    }
    ArchiveCheckpoint::from_plan(
        before,
        archive.before,
        &expected,
        archive.archive_id,
        archive.created_at_ms,
    )?;
    Ok(())
}

struct RestoreIdentities {
    feedback: HashSet<FeedbackId>,
    text: HashSet<ReviewTextRevisionId>,
    targets: HashSet<ReviewTargetId>,
    revisions: HashSet<ReviewTargetRevisionId>,
}

impl RestoreIdentities {
    fn new(states: &ValidatedStates<'_>) -> Self {
        let mut ids = Self {
            feedback: HashSet::new(),
            text: HashSet::new(),
            targets: HashSet::new(),
            revisions: HashSet::new(),
        };
        for state in states.states.values() {
            for feedback in &state.feedback {
                ids.feedback.insert(feedback.id);
                ids.text.insert(feedback.text_revision_id);
                for target in &feedback.targets {
                    ids.targets.insert(target.id);
                    ids.revisions.insert(target.revision_id);
                }
            }
        }
        ids
    }

    fn claim(
        &mut self,
        feedback: &RestoreFeedbackContent,
        target: &VersionedTarget,
    ) -> Result<(), ContinuousReviewError> {
        if !self.feedback.insert(feedback.id)
            || !self.text.insert(feedback.text_revision_id)
            || !self.targets.insert(target.id)
            || !self.revisions.insert(target.revision_id)
        {
            return Err(ContinuousReviewError::DuplicateIdentity);
        }
        Ok(())
    }
}
