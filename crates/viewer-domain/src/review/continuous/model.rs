use std::collections::{HashMap, HashSet};

use thiserror::Error;

use crate::review::{
    AssetVersion, FeedbackAnchor, MAX_ASSETS_PER_ROUND, MAX_FEEDBACK_ITEMS_PER_ROUND,
    MAX_FEEDBACK_TEXT_BYTES, MAX_IMAGE_STROKE_POINTS_PER_ROUND, MAX_TARGETS_PER_FEEDBACK,
    round::{asset_is_valid, validate_anchor},
};
use crate::{
    AssetVersionId, FeedbackId, ProjectId, ReviewRoundId, ReviewSnapshotId, ReviewStreamId,
    ReviewTargetId, ReviewTargetRevisionId, ReviewTextRevisionId,
};

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ContinuousReviewError {
    #[error("continuous review data is invalid")]
    InvalidData,
    #[error("continuous review exceeds a resource limit")]
    LimitExceeded,
    #[error("continuous review references missing data")]
    MissingReference,
    #[error("continuous review contains a duplicate identity")]
    DuplicateIdentity,
    #[error("the current review snapshot has changed")]
    StaleSnapshot,
    #[error("review selections conflict")]
    SelectionConflict,
    #[error("review changes require confirmation")]
    NeedsConfirmation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SnapshotRef {
    pub snapshot_id: ReviewSnapshotId,
    pub blake3: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct TargetVersionKey {
    pub feedback_id: FeedbackId,
    pub text_revision_id: ReviewTextRevisionId,
    pub target_id: ReviewTargetId,
    pub target_revision_id: ReviewTargetRevisionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryRef {
    pub project_id: ProjectId,
    pub stream_id: ReviewStreamId,
    pub source: HistorySource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistorySource {
    Snapshot {
        snapshot: SnapshotRef,
        keys: Vec<TargetVersionKey>,
    },
    Legacy {
        round_id: ReviewRoundId,
        record_blake3: [u8; 32],
        targets: Vec<LegacyTargetRef>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct LegacyTargetRef {
    pub round_id: ReviewRoundId,
    pub feedback_id: FeedbackId,
    pub target_index: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReviewAvailability {
    Ready,
    NeedsConfirmation(Vec<ReviewPendingReason>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ReviewPendingReason {
    SourceChanged,
    SourceMissing,
    SourceUnreadable,
    SourceUnverified,
    LegacyUsageUnknown,
    LegacyEvidenceAbsent,
    ApplicabilityUnconfirmed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VersionedTarget {
    pub id: ReviewTargetId,
    pub revision_id: ReviewTargetRevisionId,
    pub asset_version_id: AssetVersionId,
    pub anchor: FeedbackAnchor,
    pub availability: ReviewAvailability,
}

impl VersionedTarget {
    pub fn asset(
        id: ReviewTargetId,
        revision: ReviewTargetRevisionId,
        asset: AssetVersionId,
    ) -> Self {
        Self {
            id,
            revision_id: revision,
            asset_version_id: asset,
            anchor: FeedbackAnchor::Asset,
            availability: ReviewAvailability::Ready,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VersionedFeedback {
    pub id: FeedbackId,
    pub text_revision_id: ReviewTextRevisionId,
    pub text: String,
    pub created_at_ms: i64,
    pub targets: Vec<VersionedTarget>,
    pub history_ref: Option<HistoryRef>,
}

impl VersionedFeedback {
    pub fn validate(&self) -> Result<(), ContinuousReviewError> {
        use ContinuousReviewError::*;
        if self.text.len() > MAX_FEEDBACK_TEXT_BYTES
            || self.targets.len() > MAX_TARGETS_PER_FEEDBACK
        {
            return Err(LimitExceeded);
        }
        if self.text.trim().is_empty() || self.targets.is_empty() || self.created_at_ms < 0 {
            return Err(InvalidData);
        }
        let mut ids = HashSet::new();
        let mut revisions = HashSet::new();
        let mut points = 0;
        for target in &self.targets {
            if !ids.insert(target.id) || !revisions.insert(target.revision_id) {
                return Err(DuplicateIdentity);
            }
            if matches!(target.anchor, FeedbackAnchor::VideoRange { start_us, end_us } if start_us >= end_us)
            {
                return Err(InvalidData);
            }
            if let ReviewAvailability::NeedsConfirmation(reasons) = &target.availability {
                if reasons.is_empty() {
                    return Err(InvalidData);
                }
                if reasons.len() > 7 {
                    return Err(LimitExceeded);
                }
                let mut seen = HashSet::new();
                if reasons.iter().any(|reason| !seen.insert(reason)) {
                    return Err(DuplicateIdentity);
                }
            }
            points += stroke_points(target);
            if points > MAX_IMAGE_STROKE_POINTS_PER_ROUND {
                return Err(LimitExceeded);
            }
        }
        if let Some(history) = &self.history_ref {
            history.validate()?;
        }
        Ok(())
    }

    pub(super) fn key(&self, target: &VersionedTarget) -> TargetVersionKey {
        TargetVersionKey {
            feedback_id: self.id,
            text_revision_id: self.text_revision_id,
            target_id: target.id,
            target_revision_id: target.revision_id,
        }
    }
}

impl HistoryRef {
    fn validate(&self) -> Result<(), ContinuousReviewError> {
        use ContinuousReviewError::*;
        match &self.source {
            HistorySource::Snapshot { keys, .. } => {
                if keys.len() > MAX_TARGETS_PER_FEEDBACK {
                    return Err(LimitExceeded);
                }
                if keys.is_empty() {
                    return Err(InvalidData);
                }
                let mut ids = HashSet::new();
                if keys.iter().any(|key| !ids.insert(key.target_id)) {
                    return Err(DuplicateIdentity);
                }
            }
            HistorySource::Legacy {
                round_id, targets, ..
            } => {
                if targets.len() > MAX_TARGETS_PER_FEEDBACK {
                    return Err(LimitExceeded);
                }
                if targets.is_empty() {
                    return Err(InvalidData);
                }
                let mut seen = HashSet::new();
                for target in targets {
                    if target.round_id != *round_id
                        || target.target_index as usize >= MAX_TARGETS_PER_FEEDBACK
                    {
                        return Err(InvalidData);
                    }
                    if !seen.insert(target) {
                        return Err(DuplicateIdentity);
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContinuousReviewState {
    pub project_id: ProjectId,
    pub stream_id: ReviewStreamId,
    pub snapshot_id: ReviewSnapshotId,
    pub parent: Option<SnapshotRef>,
    pub assets: Vec<AssetVersion>,
    pub feedback: Vec<VersionedFeedback>,
}

impl ContinuousReviewState {
    pub fn empty(
        project_id: ProjectId,
        stream_id: ReviewStreamId,
        snapshot_id: ReviewSnapshotId,
    ) -> Self {
        Self {
            project_id,
            stream_id,
            snapshot_id,
            parent: None,
            assets: vec![],
            feedback: vec![],
        }
    }

    pub fn target_key(&self, target_id: ReviewTargetId) -> Option<TargetVersionKey> {
        self.feedback.iter().find_map(|feedback| {
            feedback
                .targets
                .iter()
                .find(|target| target.id == target_id)
                .map(|target| feedback.key(target))
        })
    }

    pub fn validate(&self) -> Result<(), ContinuousReviewError> {
        use ContinuousReviewError::*;
        if self.assets.len() > MAX_ASSETS_PER_ROUND
            || self.feedback.len() > MAX_FEEDBACK_ITEMS_PER_ROUND
        {
            return Err(LimitExceeded);
        }
        if self
            .parent
            .is_some_and(|parent| parent.snapshot_id == self.snapshot_id)
        {
            return Err(InvalidData);
        }
        let mut assets = HashMap::new();
        for asset in &self.assets {
            if !asset_is_valid(asset) {
                return Err(InvalidData);
            }
            if assets.insert(asset.id, asset).is_some() {
                return Err(DuplicateIdentity);
            }
        }
        let mut feedback_ids = HashSet::new();
        let mut text_revisions = HashSet::new();
        let mut target_ids = HashSet::new();
        let mut target_revisions = HashSet::new();
        let mut points = 0;
        for feedback in &self.feedback {
            feedback.validate()?;
            if !feedback_ids.insert(feedback.id)
                || !text_revisions.insert(feedback.text_revision_id)
            {
                return Err(DuplicateIdentity);
            }
            if let Some(history) = &feedback.history_ref {
                if history.project_id != self.project_id || history.stream_id != self.stream_id {
                    return Err(InvalidData);
                }
                if matches!(&history.source, HistorySource::Snapshot { snapshot, .. } if snapshot.snapshot_id == self.snapshot_id)
                {
                    return Err(InvalidData);
                }
            }
            for target in &feedback.targets {
                if !target_ids.insert(target.id) || !target_revisions.insert(target.revision_id) {
                    return Err(DuplicateIdentity);
                }
                let asset = assets
                    .get(&target.asset_version_id)
                    .ok_or(MissingReference)?;
                validate_anchor(asset, &target.anchor).map_err(|_| InvalidData)?;
                points += stroke_points(target);
                if points > MAX_IMAGE_STROKE_POINTS_PER_ROUND {
                    return Err(LimitExceeded);
                }
            }
        }
        Ok(())
    }
}

fn stroke_points(target: &VersionedTarget) -> usize {
    match &target.anchor {
        FeedbackAnchor::ImageStroke(stroke) => stroke.points().len(),
        _ => 0,
    }
}
