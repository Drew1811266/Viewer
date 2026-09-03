use super::*;
use crate::review_evidence::*;
use crate::{PreparedReviewAsset, ReviewTaskCancellation};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use viewer_domain::{
    AssetVersionId,
    review::{FeedbackAnchor, ReviewMedia, continuous::*},
};

pub(super) struct EvidencePreparation {
    pub bindings: Vec<ReviewEvidenceBinding>,
    pub files: Vec<PreparedEvidenceFile>,
    pub leases: Vec<Arc<dyn ReviewEvidenceStaging>>,
    pub cache_actions: Vec<CachedReviewEvidenceAction>,
}

pub(super) struct EvidenceActionCacheContext {
    pub port: Arc<dyn ReviewEvidenceActionCachePort>,
    pub policy: ReviewEvidenceActionPolicy,
}
pub(super) async fn prepare(
    renderer: &dyn ReviewEvidencePort,
    repository: Arc<dyn ContinuousReviewRepositoryPort>,
    previous: Option<&StoredContinuousSnapshot>,
    next: &ContinuousReviewState,
    prepared_assets: &HashMap<AssetVersionId, PreparedReviewAsset>,
    cancellation: ReviewTaskCancellation,
) -> Result<EvidencePreparation, ReviewWorkspaceError> {
    prepare_inner(
        renderer,
        Some(repository),
        previous,
        next,
        prepared_assets,
        cancellation,
        None,
    )
    .await
}

pub(super) async fn prepare_materialized(
    renderer: &dyn ReviewEvidencePort,
    cache: EvidenceActionCacheContext,
    repository: Arc<dyn ContinuousReviewRepositoryPort>,
    previous: Option<&StoredContinuousSnapshot>,
    target: &StoredAuthoringState,
    prepared_assets: &HashMap<AssetVersionId, PreparedReviewAsset>,
    cancellation: ReviewTaskCancellation,
) -> Result<EvidencePreparation, ReviewWorkspaceError> {
    let previous_authoring = previous
        .cloned()
        .map(ReviewWorkspaceCurrent::from_published)
        .map(|current| current.authoring);
    let dirty = dirty_evidence_assets(previous_authoring.as_ref(), target, cache.policy)?
        .into_iter()
        .collect();
    prepare_inner(
        renderer,
        Some(repository),
        previous,
        &target.state,
        prepared_assets,
        cancellation,
        Some(CachePreparation {
            cache: cache.port,
            policy: cache.policy,
            dirty,
        }),
    )
    .await
}

pub(super) async fn prepare_new(
    renderer: &dyn ReviewEvidencePort,
    next: &ContinuousReviewState,
    prepared_assets: &HashMap<AssetVersionId, PreparedReviewAsset>,
    cancellation: ReviewTaskCancellation,
) -> Result<EvidencePreparation, ReviewWorkspaceError> {
    prepare_inner(
        renderer,
        None,
        None,
        next,
        prepared_assets,
        cancellation,
        None,
    )
    .await
}

struct CachePreparation {
    cache: Arc<dyn ReviewEvidenceActionCachePort>,
    policy: ReviewEvidenceActionPolicy,
    dirty: HashSet<AssetVersionId>,
}

async fn prepare_inner(
    renderer: &dyn ReviewEvidencePort,
    repository: Option<Arc<dyn ContinuousReviewRepositoryPort>>,
    previous: Option<&StoredContinuousSnapshot>,
    next: &ContinuousReviewState,
    prepared_assets: &HashMap<AssetVersionId, PreparedReviewAsset>,
    cancellation: ReviewTaskCancellation,
    cache: Option<CachePreparation>,
) -> Result<EvidencePreparation, ReviewWorkspaceError> {
    let mut result = EvidencePreparation {
        bindings: vec![],
        files: vec![],
        leases: vec![],
        cache_actions: vec![],
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
                FeedbackAnchor::ImagePoint(_)
                    | FeedbackAnchor::ImageArrow(_)
                    | FeedbackAnchor::ImageStroke(_)
                    | FeedbackAnchor::ImageRect(_)
                    | FeedbackAnchor::ImageEllipse(_)
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
        if let Some(cache) = &cache {
            if !cache.dirty.contains(&asset.id) {
                let binding = old.ok_or(ReviewCommitError::Integrity)?;
                let EvidenceCapability::Image {
                    base, annotated, ..
                } = &binding.capability
                else {
                    return Err(ReviewCommitError::Integrity.into());
                };
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
            let action_key = evidence_action_key(next, asset.id, cache.policy)?;
            let action_cache = cache.cache.clone();
            let cached = super::service::io(move || action_cache.load_verified(action_key)).await?;
            if let Some(cached) = cached {
                let usable = cached.renderer_version == cache.policy.renderer_version
                    && cached.output_policy_version == cache.policy.output_policy_version
                    && cached.annotated.is_some() == !mapping.is_empty();
                if usable {
                    result.bindings.push(ReviewEvidenceBinding {
                        asset_version_id: asset.id,
                        capability: EvidenceCapability::Image {
                            base: cached.base,
                            annotated: cached.annotated,
                            annotations: mapping,
                        },
                    });
                    continue;
                }
                let action_cache = cache.cache.clone();
                super::service::io(move || action_cache.remove(action_key)).await?;
            }
        }
        if let Some(binding) = old {
            if cache.is_none()
                && let EvidenceCapability::Image {
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
            } else if cache.is_none() {
                result.bindings.push(binding.clone());
                continue;
            }
        }
        let base = if let Some(previous) = previous.filter(|_| old.is_some()) {
            let repository = repository.clone().ok_or(ReviewCommitError::Integrity)?;
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
            if cache.is_some() {
                renderer
                    .capture_prewarmed_base(prepared.clone(), cancellation.clone())
                    .await?
            } else {
                renderer
                    .capture_base(prepared.clone(), cancellation.clone())
                    .await?
            }
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
                base: rendered.base_ref.clone(),
                annotated: rendered.annotated_ref.clone(),
                annotations: rendered.annotations,
            },
        });
        result.files.extend_from_slice(rendered.staging.files());
        result.leases.push(rendered.staging);
        if let Some(cache) = &cache {
            result.cache_actions.push(CachedReviewEvidenceAction {
                action_key: evidence_action_key(next, asset.id, cache.policy)?,
                base: rendered.base_ref,
                annotated: rendered.annotated_ref,
                renderer_version: cache.policy.renderer_version,
                output_policy_version: cache.policy.output_policy_version,
            });
        }
    }
    Ok(result)
}
