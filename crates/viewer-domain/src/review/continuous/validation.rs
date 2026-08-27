//! Cross-snapshot identity checks, independent of repository hash verification.
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use super::{ContinuousReviewError, ContinuousReviewState, VersionedFeedback, VersionedTarget};
use crate::{ReviewSnapshotId, ReviewTargetId};

pub(super) const MAX_HISTORY_STATES: usize = 10_000;
pub(super) type TargetIndex<'a> =
    HashMap<ReviewTargetId, (&'a VersionedFeedback, &'a VersionedTarget)>;

pub(super) fn target_index(state: &ContinuousReviewState) -> TargetIndex<'_> {
    state
        .feedback
        .iter()
        .flat_map(|feedback| {
            feedback
                .targets
                .iter()
                .map(move |target| (target.id, (feedback, target)))
        })
        .collect()
}

pub(super) struct ValidatedStates<'a> {
    pub states: HashMap<ReviewSnapshotId, &'a ContinuousReviewState>,
    pub targets: HashMap<ReviewSnapshotId, TargetIndex<'a>>,
    pub bases: HashSet<ReviewSnapshotId>,
}

impl<'a> ValidatedStates<'a> {
    pub fn new(
        current: &'a ContinuousReviewState,
        bases: &'a [ContinuousReviewState],
    ) -> Result<Self, ContinuousReviewError> {
        use ContinuousReviewError::*;
        if bases.len() > MAX_HISTORY_STATES {
            return Err(LimitExceeded);
        }
        let mut result = Self {
            states: HashMap::new(),
            targets: HashMap::new(),
            bases: HashSet::new(),
        };
        for basis in bases {
            if !result.bases.insert(basis.snapshot_id) {
                return Err(DuplicateIdentity);
            }
        }
        let mut assets = HashMap::new();
        let mut feedback_origins = HashMap::new();
        let mut text_revisions = HashMap::new();
        let mut target_owners = HashMap::new();
        let mut target_revisions = HashMap::new();
        for state in std::iter::once(current).chain(bases) {
            state.validate()?;
            if state.project_id != current.project_id || state.stream_id != current.stream_id {
                return Err(InvalidData);
            }
            remember(&mut result.states, state.snapshot_id, state)?;
            for asset in &state.assets {
                remember(&mut assets, asset.id, asset)?;
            }
            for feedback in &state.feedback {
                remember(
                    &mut feedback_origins,
                    feedback.id,
                    (feedback.created_at_ms, &feedback.history_ref),
                )?;
                remember(
                    &mut text_revisions,
                    feedback.text_revision_id,
                    (feedback.id, &feedback.text),
                )?;
                for target in &feedback.targets {
                    remember(&mut target_owners, target.id, feedback.id)?;
                    remember(
                        &mut target_revisions,
                        target.revision_id,
                        (feedback.id, target),
                    )?;
                }
            }
            result
                .targets
                .insert(state.snapshot_id, target_index(state));
        }
        Ok(result)
    }
}

pub(super) fn remember<K: Eq + Hash, V: PartialEq>(
    map: &mut HashMap<K, V>,
    key: K,
    value: V,
) -> Result<(), ContinuousReviewError> {
    if map.get(&key).is_some_and(|previous| *previous != value) {
        return Err(ContinuousReviewError::DuplicateIdentity);
    }
    map.insert(key, value);
    Ok(())
}
