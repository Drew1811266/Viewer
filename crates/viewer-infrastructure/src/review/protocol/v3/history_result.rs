//! History retains each basis' identity, including legacy identities without invented v3 revisions.
use super::super::common::ReviewProtocolError;
use super::read_result::{HistoryLimitation, HistoryRole, OkStatus};
use super::{
    EvidenceBinding, EvidenceRef, LegacyRecordRef, ReviewIndexV3, ReviewStateRecord,
    ReviewStreamV3, validate, wire,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use viewer_domain::review::continuous::{
    ContinuousReviewState, SnapshotRef, TargetVersionKey, VersionedFeedback,
};
use viewer_domain::review::{AssetVersion, Feedback, FeedbackAnchor, ReviewDraft, ReviewMedia};
use viewer_domain::{
    AssetVersionId, FeedbackId, ProjectId, ReviewArchiveId, ReviewCommandId, ReviewRoundId,
    ReviewStreamId,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryReadResult {
    protocol_version: wire::Protocol,
    status: OkStatus,
    role: HistoryRole,
    #[serde(deserialize_with = "wire::canonical_id")]
    pub project_id: ProjectId,
    #[serde(deserialize_with = "wire::canonical_id")]
    pub review_stream_id: ReviewStreamId,
    pub selector: ReadHistorySelector,
    pub entries: Vec<HistoryEntry>,
    pub limitations: Vec<HistoryLimitation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ReadHistorySelector {
    Snapshot {
        #[serde(with = "wire::snapshot")]
        snapshot: SnapshotRef,
    },
    Archive {
        #[serde(deserialize_with = "wire::canonical_id")]
        archive_id: ReviewArchiveId,
    },
    Legacy {
        #[serde(deserialize_with = "wire::canonical_id")]
        round_id: ReviewRoundId,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum HistoryEntry {
    Snapshot {
        #[serde(with = "wire::snapshot")]
        snapshot_ref: SnapshotRef,
        #[serde(with = "wire::assets")]
        assets: Vec<AssetVersion>,
        #[serde(with = "wire::feedback")]
        feedback: Vec<VersionedFeedback>,
        evidence: Vec<EvidenceBinding>,
        #[serde(with = "wire::keys")]
        selected_targets: Vec<TargetVersionKey>,
    },
    Legacy {
        reference: LegacyRecordRef,
        #[serde(with = "wire::assets")]
        assets: Vec<AssetVersion>,
        #[serde(with = "wire::legacy_feedback")]
        feedback: Vec<Feedback>,
        evidence: Vec<LegacyEvidence>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegacyEvidence {
    #[serde(deserialize_with = "wire::canonical_id")]
    pub asset_version_id: AssetVersionId,
    pub relative_path: String,
    pub image: EvidenceRef,
    pub annotations: Vec<LegacyAnnotation>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegacyAnnotation {
    pub ordinal: u32,
    #[serde(deserialize_with = "wire::canonical_id")]
    pub feedback_id: FeedbackId,
}

impl HistoryReadResult {
    pub(super) fn validate(&self) -> Result<(), ReviewProtocolError> {
        use ReviewProtocolError::*;
        if self.entries.is_empty() {
            return Err(InvalidData);
        }
        if self.entries.len() > 10_000 {
            return Err(LimitExceeded);
        }
        let unique: HashSet<_> = self.limitations.iter().collect();
        if unique.len() != self.limitations.len() || self.limitations.len() > 2 {
            return Err(InvalidData);
        }
        let mut snapshots = HashSet::new();
        for entry in &self.entries {
            match entry {
                HistoryEntry::Snapshot {
                    snapshot_ref,
                    assets,
                    feedback,
                    evidence,
                    selected_targets,
                } => {
                    if !snapshots.insert(snapshot_ref.snapshot_id) {
                        return Err(InvalidData);
                    }
                    match self.selector {
                        ReadHistorySelector::Snapshot { snapshot }
                            if snapshot == *snapshot_ref && self.entries.len() == 1 => {}
                        ReadHistorySelector::Archive { .. } if !selected_targets.is_empty() => {}
                        _ => return Err(InvalidData),
                    }
                    let state = ContinuousReviewState {
                        project_id: self.project_id,
                        stream_id: self.review_stream_id,
                        snapshot_id: snapshot_ref.snapshot_id,
                        parent: None,
                        assets: assets.clone(),
                        feedback: feedback.clone(),
                    };
                    let available: HashSet<_> = feedback
                        .iter()
                        .flat_map(|f| {
                            f.targets.iter().map(|t| TargetVersionKey {
                                feedback_id: f.id,
                                text_revision_id: f.text_revision_id,
                                target_id: t.id,
                                target_revision_id: t.revision_id,
                            })
                        })
                        .collect();
                    let selected: HashSet<_> = selected_targets.iter().copied().collect();
                    if selected.len() != selected_targets.len() || !selected.is_subset(&available) {
                        return Err(InvalidData);
                    }
                    if matches!(self.selector, ReadHistorySelector::Snapshot { .. })
                        && selected != available
                    {
                        return Err(InvalidData);
                    }
                    validate::state(&ReviewStateRecord {
                        state,
                        command_id: ReviewCommandId::from_u128(0),
                        payload_digest: [0; 32],
                        changes: vec![],
                        evidence: evidence.clone(),
                    })?;
                    if evidence
                        .iter()
                        .any(|e| matches!(e.capability, super::EvidenceCapability::LegacyAbsent {}))
                        && !self
                            .limitations
                            .contains(&HistoryLimitation::LegacyEvidenceAbsent)
                    {
                        return Err(InvalidData);
                    }
                }
                HistoryEntry::Legacy {
                    reference,
                    assets,
                    feedback,
                    evidence,
                } => {
                    validate::portable_assets(assets)?;
                    if feedback
                        .iter()
                        .any(|item| item.created_at_ms as u64 > validate::MAX_SAFE_INTEGER)
                    {
                        return Err(LimitExceeded);
                    }
                    if !matches!(self.selector,ReadHistorySelector::Legacy { round_id } if round_id == reference.round_id)
                        || self.entries.len() != 1
                    {
                        return Err(InvalidData);
                    }
                    if !self
                        .limitations
                        .contains(&HistoryLimitation::LegacyUsageUnknown)
                    {
                        return Err(InvalidData);
                    }
                    if assets
                        .iter()
                        .any(|a| matches!(a.media, ReviewMedia::Image { .. }))
                        && !self
                            .limitations
                            .contains(&HistoryLimitation::LegacyEvidenceAbsent)
                    {
                        return Err(InvalidData);
                    }
                    validate::index(&ReviewIndexV3 {
                        project_id: self.project_id,
                        streams: vec![ReviewStreamV3 {
                            review_stream_id: self.review_stream_id,
                            task_id: None,
                            batch_id: None,
                            current_ref: None,
                            archive_refs: vec![],
                            legacy_refs: vec![reference.clone()],
                            usage_refs: vec![],
                        }],
                    })?;
                    let mut draft = ReviewDraft::new(
                        self.project_id,
                        self.review_stream_id,
                        reference.round_id,
                        None,
                        None,
                        0,
                        assets.clone(),
                    )
                    .map_err(|_| InvalidData)?;
                    let mut ids = HashSet::new();
                    for item in feedback {
                        if !ids.insert(item.id) {
                            return Err(InvalidData);
                        }
                        draft
                            .upsert_feedback(item.clone())
                            .map_err(|_| InvalidData)?;
                    }
                    validate_legacy_evidence(reference, &draft, evidence)?;
                }
            }
        }
        Ok(())
    }
}

fn validate_legacy_evidence(
    reference: &LegacyRecordRef,
    draft: &ReviewDraft,
    evidence: &[LegacyEvidence],
) -> Result<(), ReviewProtocolError> {
    use ReviewProtocolError::*;
    if evidence.len() > 50_000 {
        return Err(LimitExceeded);
    }
    if reference.protocol_version == "viewer.review/1" {
        return if evidence.is_empty() {
            Ok(())
        } else {
            Err(InvalidData)
        };
    }
    let mut expected = std::collections::HashMap::<_, Vec<_>>::new();
    for item in &draft.feedback {
        for target in &item.targets {
            if matches!(
                target.anchor,
                FeedbackAnchor::ImageRect(_) | FeedbackAnchor::ImageStroke(_)
            ) {
                expected
                    .entry(target.asset_version_id)
                    .or_default()
                    .push(item.id);
            }
        }
    }
    let mut bytes = 0;
    for image in evidence {
        validate::evidence_ref(&image.image)?;
        bytes += image.image.size_bytes;
        if bytes > 4 * 1024 * 1024 * 1024 {
            return Err(LimitExceeded);
        }
        if image.relative_path != format!("artifacts/{}-annotation.png", image.asset_version_id) {
            return Err(InvalidData);
        }
        let Some(ids) = expected.remove(&image.asset_version_id) else {
            return Err(InvalidData);
        };
        if ids.len() != image.annotations.len()
            || image
                .annotations
                .iter()
                .zip(ids)
                .enumerate()
                .any(|(i, (a, id))| a.ordinal as usize != i + 1 || a.feedback_id != id)
        {
            return Err(InvalidData);
        }
    }
    if !expected.is_empty() {
        return Err(InvalidData);
    }
    Ok(())
}
