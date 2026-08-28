use super::super::{MAX_REVIEW_DOCUMENT_BYTES, v3};
use super::{
    history,
    repository::{View, protocol_error},
};
use std::collections::HashSet;
use viewer_application::review_workspace::{ReviewCommitError, ReviewUsageDeclaration};
use viewer_domain::review::continuous::TargetVersionKey;
use viewer_domain::{ReviewStreamId, ReviewUsageId};

impl From<ReviewUsageDeclaration> for v3::ReviewUsageRecord {
    fn from(value: ReviewUsageDeclaration) -> Self {
        Self {
            declaration_id: value.id,
            project_id: value.project_id,
            review_stream_id: value.stream_id,
            basis: value.basis,
            targets: value.targets,
            outputs: value
                .outputs
                .into_iter()
                .map(|v| v3::UsageOutputRecord {
                    relative_path: v.relative_path,
                    blake3: v.blake3,
                    previous_asset_version_id: v.previous_asset_version_id,
                })
                .collect(),
        }
    }
}
impl From<v3::ReviewUsageRecord> for ReviewUsageDeclaration {
    fn from(value: v3::ReviewUsageRecord) -> Self {
        Self {
            id: value.declaration_id,
            project_id: value.project_id,
            stream_id: value.review_stream_id,
            basis: value.basis,
            targets: value.targets,
            outputs: value
                .outputs
                .into_iter()
                .map(|o| viewer_application::review_workspace::UsageOutput {
                    relative_path: o.relative_path,
                    blake3: o.blake3,
                    previous_asset_version_id: o.previous_asset_version_id,
                })
                .collect(),
        }
    }
}

pub(super) fn validate(
    view: &View,
    stream: ReviewStreamId,
    record: &v3::ReviewUsageRecord,
) -> Result<(), ReviewCommitError> {
    if record.project_id != view.index.project_id || record.review_stream_id != stream {
        return Err(ReviewCommitError::Integrity);
    }
    let basis = history::reachable(view, stream, &record.basis)?;
    let keys: HashSet<_> = basis
        .state
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
        .collect();
    // The exact basis/target claim is validated here. Outputs remain producer claims,
    // not confirmed lineage; Application verifies old-target ownership and fresh new bytes
    // separately before accepting a producer-backed SourceBindingDecision.
    if record.targets.iter().any(|key| !keys.contains(key)) {
        return Err(ReviewCommitError::Integrity);
    }
    Ok(())
}

pub(super) fn read(
    view: &View,
    stream: ReviewStreamId,
    id: ReviewUsageId,
) -> Result<v3::ReviewUsageRecord, ReviewCommitError> {
    let reference = history::stream(view, stream)?
        .usage_refs
        .iter()
        .find(|v| v.declaration_id == id)
        .ok_or(ReviewCommitError::Integrity)?;
    let directory = view.directory.required_child("usage")?;
    let bytes = directory
        .read(&format!("{id}.json"), MAX_REVIEW_DOCUMENT_BYTES)?
        .ok_or(ReviewCommitError::Integrity)?;
    history::verify_digest(&bytes, &reference.blake3)?;
    let record = v3::decode_usage_v1(&bytes).map_err(protocol_error)?;
    if record.declaration_id != id {
        return Err(ReviewCommitError::Integrity);
    }
    validate(view, stream, &record)?;
    Ok(record)
}
