use super::*;
use crate::review_evidence::*;
use crate::{PreparedReviewAsset, ReviewTaskCancellation};
use std::{collections::HashMap, sync::Arc};
use viewer_domain::{
    AssetVersionId,
    review::{FeedbackAnchor, ReviewMedia, continuous::*},
};

pub(super) struct EvidencePreparation {
    pub bindings: Vec<ReviewEvidenceBinding>,
    pub files: Vec<PreparedEvidenceFile>,
    pub leases: Vec<Arc<dyn ReviewEvidenceStaging>>,
}
pub(super) async fn prepare(
    renderer: &dyn ReviewEvidencePort,
    repository: Arc<dyn ContinuousReviewRepositoryPort>,
    previous: Option<&StoredContinuousSnapshot>,
    next: &ContinuousReviewState,
    prepared_assets: &HashMap<AssetVersionId, PreparedReviewAsset>,
    cancellation: ReviewTaskCancellation,
) -> Result<EvidencePreparation, ReviewWorkspaceError> {
    let mut result = EvidencePreparation {
        bindings: vec![],
        files: vec![],
        leases: vec![],
    };
    let old_bindings: HashMap<_, _> = previous
        .into_iter()
        .flat_map(|s| &s.evidence)
        .map(|b| (b.asset_version_id, b))
        .collect();
    let mut by_asset: HashMap<AssetVersionId, Vec<NumberedTargetAnnotation>> = HashMap::new();
    for feedback in &next.feedback {
        for target in &feedback.targets {
            if matches!(
                target.anchor,
                FeedbackAnchor::ImageRect(_) | FeedbackAnchor::ImageStroke(_)
            ) {
                let annotations = by_asset.entry(target.asset_version_id).or_default();
                annotations.push(NumberedTargetAnnotation {
                    ordinal: (annotations.len() + 1) as u32,
                    key: TargetVersionKey {
                        feedback_id: feedback.id,
                        text_revision_id: feedback.text_revision_id,
                        target_id: target.id,
                        target_revision_id: target.revision_id,
                    },
                    anchor: target.anchor.clone(),
                });
            }
        }
    }
    for asset in &next.assets {
        let old = old_bindings.get(&asset.id).copied();
        if !matches!(asset.media, ReviewMedia::Image { .. }) {
            result.bindings.push(ReviewEvidenceBinding {
                asset_version_id: asset.id,
                capability: EvidenceCapability::NotImage,
            });
            continue;
        }
        let annotations = by_asset.remove(&asset.id).unwrap_or_default();
        let mapping: Vec<_> = annotations
            .iter()
            .map(|a| EvidenceAnnotation {
                ordinal: a.ordinal,
                key: a.key,
            })
            .collect();
        if let Some(binding) = old {
            if let EvidenceCapability::Image {
                annotations: old_mapping,
                base,
                annotated,
            } = &binding.capability
            {
                if old_mapping.len() == mapping.len()
                    && old_mapping.iter().zip(&mapping).all(|(a, b)| {
                        a.ordinal == b.ordinal
                            && a.key.feedback_id == b.key.feedback_id
                            && a.key.target_id == b.key.target_id
                            && a.key.target_revision_id == b.key.target_revision_id
                    })
                {
                    result.bindings.push(ReviewEvidenceBinding {
                        asset_version_id: asset.id,
                        capability: EvidenceCapability::Image {
                            base: base.clone(),
                            annotated: annotated.clone(),
                            annotations: mapping,
                        },
                    });
                    continue;
                }
            } else {
                result.bindings.push(binding.clone());
                continue;
            }
        }
        let base = if let Some(previous) = previous.filter(|_| old.is_some()) {
            let repository = repository.clone();
            let reference = previous.reference;
            let stream = next.stream_id;
            let asset_id = asset.id;
            super::service::io(move || {
                repository.load_evidence(
                    stream,
                    &HistorySelector::Snapshot(reference),
                    asset_id,
                    EvidenceRole::Base,
                )
            })
            .await?
        } else {
            let prepared = prepared_assets
                .get(&asset.id)
                .ok_or(ReviewWorkspaceError::PreviewRequired)?;
            renderer
                .capture_base(prepared.clone(), cancellation.clone())
                .await?
        };
        if base.asset() != asset {
            return Err(crate::ReviewArtifactError::SourceChanged.into());
        }
        let rendered = renderer
            .render(ReviewEvidenceRequest {
                base,
                annotations,
                cancellation: cancellation.clone(),
            })
            .await?;
        if rendered.annotations != mapping {
            return Err(crate::ReviewArtifactError::InvalidRequest.into());
        }
        result.bindings.push(ReviewEvidenceBinding {
            asset_version_id: asset.id,
            capability: EvidenceCapability::Image {
                base: rendered.base_ref,
                annotated: rendered.annotated_ref,
                annotations: rendered.annotations,
            },
        });
        result.files.extend_from_slice(rendered.staging.files());
        result.leases.push(rendered.staging);
    }
    Ok(result)
}
