use std::collections::HashSet;

use super::*;
use crate::review_evidence::HistorySelector;
use viewer_domain::{
    AssetVersionId, FeedbackId, ProjectId, ReviewSnapshotId, ReviewStreamId,
    review::{
        AssetVersion,
        continuous::{CurrentReviewProjection, ReviewAvailability, SnapshotRef, VersionedFeedback},
    },
};

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewWorkspacePatch {
    pub basis_snapshot_id: Option<ReviewSnapshotId>,
    pub head: ReviewAuthoringHead,
    pub upsert_assets: Vec<AssetVersion>,
    pub remove_asset_version_ids: Vec<AssetVersionId>,
    pub upsert_feedback: Vec<VersionedFeedback>,
    pub remove_feedback_ids: Vec<FeedbackId>,
    pub projection: CurrentReviewProjection,
    pub history_selectors: Option<Vec<HistorySelector>>,
    target: StoredAuthoringState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ReviewPatchError {
    #[error("review patch basis does not match the current authoring head")]
    StaleBasis,
    #[error("review patch contains conflicting identities")]
    InvalidPatch,
}

impl ReviewWorkspacePatch {
    /// Project identity required by presentation reducers when the first authoring patch creates
    /// a current state from an empty workspace.
    pub fn project_id(&self) -> ProjectId {
        self.target.state.project_id
    }

    /// Stream identity lets presentation reducers reject a patch routed to the wrong workspace.
    pub fn stream_id(&self) -> ReviewStreamId {
        self.target.state.stream_id
    }

    /// Logical parent fingerprint for the next authoring state. This is not a published v3 ref.
    pub fn parent(&self) -> Option<SnapshotRef> {
        self.target.state.parent
    }

    pub fn between(
        before: Option<&ReviewWorkspaceCurrent>,
        after: &ReviewWorkspaceCurrent,
        history_selectors: Option<Vec<HistorySelector>>,
    ) -> Self {
        let before_assets = before
            .map(|current| current.authoring.state.assets.as_slice())
            .unwrap_or_default();
        let before_feedback = before
            .map(|current| current.authoring.state.feedback.as_slice())
            .unwrap_or_default();
        let after_asset_ids: HashSet<_> = after
            .authoring
            .state
            .assets
            .iter()
            .map(|asset| asset.id)
            .collect();
        let after_feedback_ids: HashSet<_> = after
            .authoring
            .state
            .feedback
            .iter()
            .map(|feedback| feedback.id)
            .collect();

        let mut remove_asset_version_ids: Vec<_> = before_assets
            .iter()
            .filter(|asset| !after_asset_ids.contains(&asset.id))
            .map(|asset| asset.id)
            .collect();
        remove_asset_version_ids.sort_by_key(ToString::to_string);
        let mut remove_feedback_ids: Vec<_> = before_feedback
            .iter()
            .filter(|feedback| !after_feedback_ids.contains(&feedback.id))
            .map(|feedback| feedback.id)
            .collect();
        remove_feedback_ids.sort_by_key(ToString::to_string);

        Self {
            basis_snapshot_id: before.map(|current| current.authoring.head.snapshot_id),
            head: after.authoring.head,
            upsert_assets: after
                .authoring
                .state
                .assets
                .iter()
                .filter(|candidate| {
                    before_assets.iter().find(|asset| asset.id == candidate.id) != Some(candidate)
                })
                .cloned()
                .collect(),
            remove_asset_version_ids,
            upsert_feedback: after
                .authoring
                .state
                .feedback
                .iter()
                .filter(|candidate| {
                    before_feedback
                        .iter()
                        .find(|feedback| feedback.id == candidate.id)
                        != Some(candidate)
                })
                .cloned()
                .collect(),
            remove_feedback_ids,
            projection: logical_projection(&after.authoring.state.feedback),
            history_selectors,
            target: after.authoring.clone(),
        }
    }

    pub fn apply(
        &self,
        current: Option<ReviewWorkspaceCurrent>,
    ) -> Result<ReviewWorkspaceCurrent, ReviewPatchError> {
        if self.target.head.snapshot_id != self.target.state.snapshot_id
            || self.target.head.snapshot_id != self.target.generated.snapshot_id
            || self.target.head.sequence == 0
            || current
                .as_ref()
                .is_some_and(|value| value.authoring.head.sequence >= self.target.head.sequence)
        {
            return Err(ReviewPatchError::InvalidPatch);
        }
        if current
            .as_ref()
            .map(|value| value.authoring.head.snapshot_id)
            != self.basis_snapshot_id
        {
            return Err(ReviewPatchError::StaleBasis);
        }
        validate_id_sets(
            &self
                .upsert_assets
                .iter()
                .map(|asset| asset.id)
                .collect::<Vec<_>>(),
            &self.remove_asset_version_ids,
        )?;
        validate_id_sets(
            &self
                .upsert_feedback
                .iter()
                .map(|feedback| feedback.id)
                .collect::<Vec<_>>(),
            &self.remove_feedback_ids,
        )?;

        let (published_ref, evidence, mut assets, mut feedback) = match current {
            Some(current) => (
                current.published_ref,
                current.evidence,
                current.authoring.state.assets,
                current.authoring.state.feedback,
            ),
            None => (None, vec![], vec![], vec![]),
        };
        assets.retain(|asset| !self.remove_asset_version_ids.contains(&asset.id));
        feedback.retain(|item| !self.remove_feedback_ids.contains(&item.id));
        apply_upserts(&mut assets, &self.upsert_assets, |asset| asset.id);
        apply_upserts(&mut feedback, &self.upsert_feedback, |item| item.id);

        if assets != self.target.state.assets
            || feedback != self.target.state.feedback
            || self.head != self.target.head
            || self.projection != logical_projection(&self.target.state.feedback)
        {
            return Err(ReviewPatchError::InvalidPatch);
        }

        let mut authoring = self.target.clone();
        authoring.state.assets = assets;
        authoring.state.feedback = feedback;
        Ok(ReviewWorkspaceCurrent {
            authoring,
            published_ref,
            evidence,
        })
    }
}

fn validate_id_sets<T: Copy + Eq + std::hash::Hash>(
    upserts: &[T],
    removals: &[T],
) -> Result<(), ReviewPatchError> {
    let upsert_ids: HashSet<_> = upserts.iter().copied().collect();
    let removal_ids: HashSet<_> = removals.iter().copied().collect();
    if upsert_ids.len() != upserts.len()
        || removal_ids.len() != removals.len()
        || upsert_ids.iter().any(|id| removal_ids.contains(id))
    {
        return Err(ReviewPatchError::InvalidPatch);
    }
    Ok(())
}

fn apply_upserts<T: Clone, I: Copy + Eq>(values: &mut Vec<T>, upserts: &[T], id: impl Fn(&T) -> I) {
    for upsert in upserts {
        if let Some(existing) = values.iter_mut().find(|value| id(value) == id(upsert)) {
            *existing = upsert.clone();
        } else {
            values.push(upsert.clone());
        }
    }
}

fn logical_projection(feedback: &[VersionedFeedback]) -> CurrentReviewProjection {
    let mut projection = CurrentReviewProjection {
        actionable: vec![],
        needs_confirmation: vec![],
    };
    for target in feedback.iter().flat_map(|item| &item.targets) {
        if matches!(target.availability, ReviewAvailability::Ready) {
            projection.actionable.push(target.id);
        } else {
            projection.needs_confirmation.push(target.id);
        }
    }
    projection
}
