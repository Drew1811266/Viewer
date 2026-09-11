use super::*;
use std::{collections::HashMap, sync::MutexGuard};
use viewer_application::review_assets::{
    ContinuousReviewAssetPort, ReviewSourceLocator, SourceRelocationDecision,
};
use viewer_domain::review::continuous::{SourceCheck, SourceCheckStatus};
mod source;
use source::SourceCheckOutcome;

#[derive(Default)]
pub(super) struct CatalogState {
    assets: HashMap<AssetVersionId, AssetVersion>,
    locators: HashMap<AssetVersionId, ReviewSourceLocator>,
}
impl CatalogState {
    fn register(&mut self, asset: &AssetVersion) -> Result<(), ReviewAssetError> {
        if let Some(old) = self.assets.get(&asset.id) {
            if old != asset {
                return Err(ReviewAssetError::InvalidScope);
            }
        } else {
            if self.assets.len() >= MAX_ASSETS_PER_ROUND {
                return Err(ReviewAssetError::LimitExceeded);
            }
            self.assets.insert(asset.id, asset.clone());
        }
        Ok(())
    }
    fn reuse(
        &self,
        prepared: &PreparedReviewAsset,
    ) -> Result<Option<AssetVersion>, ReviewAssetError> {
        let mut matches = self.assets.values().filter(|old| {
            let same_entity = old.source_entity_id == Some(prepared.entity_id)
                || self
                    .locators
                    .get(&old.id)
                    .is_some_and(|l| l.entity_id == prepared.entity_id);
            same_entity
                && old.evidence.blake3.is_some()
                && old.evidence.blake3 == prepared.asset.evidence.blake3
                && old.evidence.size_bytes == prepared.asset.evidence.size_bytes
                && old.media == prepared.asset.media
        });
        let Some(old) = matches.next() else {
            return Ok(None);
        };
        if matches.next().is_some() {
            return Err(ReviewAssetError::UnconfirmedLocation);
        }
        let located = self
            .locators
            .get(&old.id)
            .map(|l| &l.relative_path)
            .unwrap_or(&old.relative_path);
        if located != &prepared.asset.relative_path {
            return Err(ReviewAssetError::UnconfirmedLocation);
        }
        Ok(Some(old.clone()))
    }
}
impl IndexedReviewAssetCatalog {
    fn continuous_state(&self) -> Result<MutexGuard<'_, CatalogState>, ReviewAssetError> {
        self.continuous
            .lock()
            .map_err(|_| ReviewAssetError::Unavailable)
    }
}
#[async_trait]
impl ContinuousReviewAssetPort for IndexedReviewAssetCatalog {
    async fn prepare_additions(
        &self,
        entity_ids: &[EntityId],
        cancellation: ReviewTaskCancellation,
    ) -> Result<Vec<PreparedReviewAsset>, ReviewAssetError> {
        if cancellation.is_cancelled() {
            return Err(ReviewAssetError::Cancelled);
        }
        let nodes = self.exact_nodes(entity_ids)?;
        self.changes
            .add_members(&nodes.iter().map(|n| n.node.entity_id).collect::<Vec<_>>())?;
        let mut prepared = Vec::with_capacity(nodes.len());
        for indexed in nodes {
            let _permit = tokio::select! {permit=self.evidence_gate.acquire()=>permit.map_err(|_|ReviewAssetError::Unavailable)?,()=wait_until_cancelled(cancellation.clone())=>return Err(ReviewAssetError::Cancelled)};
            // The continuous probe policy ignores image/video metadata caches and
            // expresses image dimensions in captured evidence's EXIF-upright space.
            let node = indexed.node.clone();
            let before = MediaFileIdentity::from_metadata(&validate_owned_metadata(
                &self.project_root,
                &node,
            )?);
            let asset = prepare_one(
                self.project_root.clone(),
                indexed,
                self.image.clone(),
                self.video.clone(),
                self.changes.clone(),
                cancellation.clone(),
                MediaProbeMode::Continuous,
            )
            .await?;
            let after = MediaFileIdentity::from_metadata(&validate_owned_metadata(
                &self.project_root,
                &node,
            )?);
            if before != after {
                return Err(ReviewAssetError::SourceChanged);
            }
            prepared.push(asset);
        }
        if cancellation.is_cancelled() {
            return Err(ReviewAssetError::Cancelled);
        }
        let mut state = self.continuous_state()?;
        // Decide all reuse before registering; no old captured record is modified.
        for item in &mut prepared {
            if let Some(previous) = state.reuse(item)? {
                item.asset = previous;
            }
        }
        let additions = prepared
            .iter()
            .filter(|p| !state.assets.contains_key(&p.asset.id))
            .count();
        if state.assets.len() + additions > MAX_ASSETS_PER_ROUND {
            return Err(ReviewAssetError::LimitExceeded);
        }
        for item in &prepared {
            state.register(&item.asset)?;
        }
        Ok(prepared)
    }

    async fn reopen_exact(
        &self,
        assets: &[AssetVersion],
        cancellation: ReviewTaskCancellation,
    ) -> Result<Vec<PreparedReviewAsset>, ReviewAssetError> {
        if assets.len() > MAX_ASSETS_PER_ROUND {
            return Err(ReviewAssetError::LimitExceeded);
        }
        let mut seen = HashSet::new();
        if assets.iter().any(|asset| !seen.insert(asset.id)) {
            return Err(ReviewAssetError::InvalidScope);
        }
        let mut reopened = Vec::with_capacity(assets.len());
        for asset in assets {
            if cancellation.is_cancelled() {
                return Err(ReviewAssetError::Cancelled);
            }
            let locator = {
                let state = self.continuous_state()?;
                if state
                    .assets
                    .get(&asset.id)
                    .is_some_and(|stored| stored != asset)
                {
                    return Err(ReviewAssetError::InvalidScope);
                }
                state.locators.get(&asset.id).cloned().or_else(|| {
                    asset.source_entity_id.map(|entity_id| ReviewSourceLocator {
                        entity_id,
                        relative_path: asset.relative_path.clone(),
                    })
                })
            }
            .ok_or(ReviewAssetError::Unavailable)?;
            let mut locator = locator;
            let outcome = source::check(
                self.project_root.clone(),
                locator.clone(),
                asset.clone(),
                cancellation.clone(),
            )
            .await?;
            if outcome.status != SourceCheckStatus::Match {
                return Err(match outcome.status {
                    SourceCheckStatus::Missing => ReviewAssetError::NotFound,
                    SourceCheckStatus::Changed => ReviewAssetError::SourceChanged,
                    _ => ReviewAssetError::Unavailable,
                });
            }
            if let Some(current) = outcome.current_entity
                && locator.entity_id != current
            {
                // Session identity drifted (e.g. volume device reassignment);
                // continue with the verified current identity.
                locator.entity_id = current;
                let mut state = self.continuous_state()?;
                state
                    .locators
                    .insert(asset.id, locator.clone());
            }
            reopened.push(PreparedReviewAsset {
                entity_id: locator.entity_id,
                asset: asset.clone(),
                failure: None,
                change_revision: self.changes.revision(locator.entity_id),
                source_path: self.project_root.join(locator.relative_path.as_str()),
            });
        }
        let mut state = self.continuous_state()?;
        for item in &reopened {
            state.register(&item.asset)?;
        }
        Ok(reopened)
    }

    async fn check_sources(
        &self,
        assets: &mut [AssetVersion],
        cancellation: ReviewTaskCancellation,
    ) -> Result<Vec<SourceCheck>, ReviewAssetError> {
        if assets.len() > MAX_ASSETS_PER_ROUND {
            return Err(ReviewAssetError::LimitExceeded);
        }
        let mut seen = HashSet::new();
        if assets.iter().any(|a| !seen.insert(a.id)) {
            return Err(ReviewAssetError::InvalidScope);
        }
        let mut result = Vec::with_capacity(assets.len());
        // Verbatim copies registered into the catalog: the persisted authoring
        // state keeps its stored identity so repeated loads stay consistent;
        // live identity drift is tracked through `locators` instead.
        let mut registered = Vec::with_capacity(assets.len());
        for asset in assets.iter_mut() {
            if cancellation.is_cancelled() {
                return Err(ReviewAssetError::Cancelled);
            }
            registered.push(asset.clone());
            let confirmed = {
                let state = self.continuous_state()?;
                if state.assets.get(&asset.id).is_some_and(|old| old != asset) {
                    return Err(ReviewAssetError::InvalidScope);
                }
                state.locators.get(&asset.id).cloned()
            };
            let outcome = if let Some(locator) = confirmed {
                source::check(
                    self.project_root.clone(),
                    locator,
                    asset.clone(),
                    cancellation.clone(),
                )
                .await?
            } else if let Some(entity_id) = asset.source_entity_id {
                let current = self
                    .index
                    .indexed_node(entity_id)
                    .map_err(|_| ReviewAssetError::IndexUnavailable)?;
                if current.is_some_and(|n| n.node.relative_path != asset.relative_path) {
                    SourceCheckOutcome::unchanged(SourceCheckStatus::Unverified)
                } else {
                    source::check(
                        self.project_root.clone(),
                        ReviewSourceLocator {
                            entity_id,
                            relative_path: asset.relative_path.clone(),
                        },
                        asset.clone(),
                        cancellation.clone(),
                    )
                    .await?
                }
            } else {
                SourceCheckOutcome::unchanged(SourceCheckStatus::Unverified)
            };
            if outcome.status == SourceCheckStatus::Match
                && let Some(current) = outcome.current_entity
                && asset.source_entity_id != Some(current)
            {
                // Refresh the caller's state and the session locator with the
                // content-verified current identity (ADR 0003 session keys).
                // Explicitly relocated assets keep their authoritative locator:
                // their stored identity intentionally lags behind.
                let mut state = self.continuous_state()?;
                let relocated = state
                    .locators
                    .get(&asset.id)
                    .is_some_and(|l| l.relative_path != asset.relative_path);
                if !relocated {
                    asset.source_entity_id = Some(current);
                    let relative_path = state
                        .locators
                        .get(&asset.id)
                        .map(|l| l.relative_path.clone())
                        .unwrap_or_else(|| asset.relative_path.clone());
                    state.locators.insert(
                        asset.id,
                        ReviewSourceLocator {
                            entity_id: current,
                            relative_path,
                        },
                    );
                }
            }
            let checked_at_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| ReviewAssetError::Unavailable)?
                .as_millis()
                .try_into()
                .map_err(|_| ReviewAssetError::Unavailable)?;
            result.push(SourceCheck {
                asset_version_id: asset.id,
                checked_at_ms,
                status: outcome.status,
            });
        }
        let mut state = self.continuous_state()?;
        for (original, check) in registered.iter().zip(&result) {
            if check.status == SourceCheckStatus::Match {
                state.register(original)?;
            }
        }
        Ok(result)
    }

    async fn confirm_relocation(
        &self,
        asset: &AssetVersion,
        decision: SourceRelocationDecision,
        cancellation: ReviewTaskCancellation,
    ) -> Result<(), ReviewAssetError> {
        if cancellation.is_cancelled() {
            return Err(ReviewAssetError::Cancelled);
        }
        let indexed = self.indexed_node(decision.candidate.entity_id)?;
        if indexed.node.relative_path != decision.candidate.relative_path {
            return Err(ReviewAssetError::UnconfirmedLocation);
        }
        if self.continuous_state()?.locators.get(&asset.id) != decision.previous.as_ref() {
            return Err(ReviewAssetError::StaleLocator);
        }
        let outcome = source::check(
            self.project_root.clone(),
            decision.candidate.clone(),
            asset.clone(),
            cancellation.clone(),
        )
        .await?;
        if outcome.status != SourceCheckStatus::Match {
            return Err(ReviewAssetError::SourceChanged);
        }
        if cancellation.is_cancelled() {
            return Err(ReviewAssetError::Cancelled);
        }
        if self
            .indexed_node(decision.candidate.entity_id)?
            .node
            .relative_path
            != decision.candidate.relative_path
        {
            return Err(ReviewAssetError::SourceChanged);
        }
        let mut state = self.continuous_state()?;
        if state.locators.get(&asset.id) != decision.previous.as_ref() {
            return Err(ReviewAssetError::StaleLocator);
        }
        self.changes.add_members(&[decision.candidate.entity_id])?;
        state.register(asset)?;
        state.locators.insert(asset.id, decision.candidate);
        Ok(())
    }
}
