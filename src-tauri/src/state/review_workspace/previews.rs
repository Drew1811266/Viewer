use super::*;
use crate::dto::review_workspace::{PreparedReviewAssetDto, ReviewEvidenceImageDto};
use viewer_application::review_evidence::{BoundReviewImage, EvidenceRole, HistorySelector};
use viewer_domain::{
    AssetVersionId, ReviewArchiveId,
    review::continuous::{ArchivePlan, ArchiveSelection, RestoreDecision, RestorePlan},
};
use viewer_infrastructure::image_cache::register_review_png;

impl DesktopRuntime {
    pub async fn prepare_review_assets(
        &self,
        id: SessionId,
        generation: Generation,
        entities: Vec<EntityId>,
    ) -> Result<Vec<PreparedReviewAssetDto>, Error> {
        let session = self.continuous_session(id, generation, false).await?;
        let previews = session
            .run(move |b, cancel| async move {
                b.service
                    .prepare_asset_previews(&entities, cancel)
                    .await
                    .map_err(Into::into)
            })
            .await?;
        // Registration shares the session lock with close's invalidation. It cannot recreate a
        // token after close removed this session's registry entries.
        let guard = self.session.lock().await;
        let current = guard
            .as_ref()
            .ok_or_else(|| Error::new(Code::StaleSession))?;
        if current.active.session_id != id || current.active.generation != generation {
            return Err(Error::new(Code::StaleSession));
        }
        previews
            .into_iter()
            .map(|p| {
                let preview = p
                    .image
                    .as_ref()
                    .map(|image| self.register_review_image(id, image))
                    .transpose()?;
                Ok(PreparedReviewAssetDto {
                    asset: p.asset.into(),
                    preview,
                })
            })
            .collect()
    }

    pub async fn get_review_evidence(
        &self,
        id: SessionId,
        generation: Generation,
        selector: HistorySelector,
        asset: AssetVersionId,
        role: EvidenceRole,
    ) -> Result<ReviewEvidenceImageDto, Error> {
        let session = self.continuous_session(id, generation, false).await?;
        let image = session
            .run(move |b, cancel| async move {
                let image = tokio::task::spawn_blocking(move || {
                    b.provider.continuous_reader()?.load_evidence(
                        b.context.stream_id,
                        &selector,
                        asset,
                        role,
                    )
                })
                .await
                .map_err(|_| Error::new(Code::Internal))?
                .map_err(ReviewWorkspaceError::from)?;
                check_cancelled(&cancel)?;
                Ok(image)
            })
            .await?;
        let guard = self.session.lock().await;
        let current = guard
            .as_ref()
            .ok_or_else(|| Error::new(Code::StaleSession))?;
        if current.active.session_id != id || current.active.generation != generation {
            return Err(Error::new(Code::StaleSession));
        }
        self.register_review_image(id, &image)
    }

    fn register_review_image(
        &self,
        id: SessionId,
        image: &BoundReviewImage,
    ) -> Result<ReviewEvidenceImageDto, Error> {
        let entity = image.asset().source_entity_id.unwrap_or_default();
        let token = register_review_png(&self.image_registry, id, entity, image)
            .map_err(|_| Error::new(Code::Integrity))?;
        let mapping = image.mapping();
        Ok(ReviewEvidenceImageDto {
            asset_version_id: image.asset().id,
            role: image.role(),
            url: format!("viewer-image://localhost/{id}/{}", token.as_str()),
            width: mapping.png_width,
            height: mapping.png_height,
            source_width: mapping.oriented_source_width,
            source_height: mapping.oriented_source_height,
        })
    }

    pub async fn get_review_history(
        &self,
        id: SessionId,
        generation: Generation,
        selector: HistorySelector,
    ) -> Result<HistoryView, Error> {
        let session = self.continuous_session(id, generation, false).await?;
        let result = session
            .run(move |b, cancel| async move {
                let value = b.service.history(selector).await?;
                check_cancelled(&cancel)?;
                Ok(value)
            })
            .await;
        self.finish_review(id, generation, result, None).await
    }

    pub async fn preview_review_archive(
        &self,
        id: SessionId,
        generation: Generation,
        selection: ArchiveSelection,
    ) -> Result<ArchivePlan, Error> {
        let session = self.continuous_session(id, generation, false).await?;
        let result = session
            .run(move |b, cancel| async move {
                let value = b.service.preview_archive(selection).await?;
                check_cancelled(&cancel)?;
                Ok(value)
            })
            .await;
        self.finish_review(id, generation, result, None).await
    }

    pub async fn preview_review_restore(
        &self,
        id: SessionId,
        generation: Generation,
        archive: ReviewArchiveId,
        decisions: Vec<RestoreDecision>,
    ) -> Result<RestorePlan, Error> {
        let session = self.continuous_session(id, generation, false).await?;
        let result = session
            .run(move |b, cancel| async move {
                let value = b.service.preview_restore(archive, decisions).await?;
                check_cancelled(&cancel)?;
                Ok(value)
            })
            .await;
        self.finish_review(id, generation, result, None).await
    }

    pub async fn inspect_review_migration(
        &self,
        id: SessionId,
        generation: Generation,
    ) -> Result<Option<MigrationInspection>, Error> {
        let session = self.continuous_session(id, generation, false).await?;
        let result = session
            .run(move |b, cancel| async move {
                let value = b.service.inspect_migration().await?;
                check_cancelled(&cancel)?;
                Ok(value)
            })
            .await;
        self.finish_review(id, generation, result, None).await
    }

    pub async fn inspect_review_usage(
        &self,
        id: SessionId,
        generation: Generation,
        entity: EntityId,
    ) -> Result<UsageImportPreview, Error> {
        let session = self.continuous_session(id, generation, false).await?;
        let index = session.config.index.clone();
        let result = session
            .run(move |b, cancel| async move {
                let node = tokio::task::spawn_blocking(move || index.indexed_node(entity))
                    .await
                    .map_err(|_| Error::new(Code::Internal))?
                    .map_err(|_| Error::new(Code::AssetUnavailable))?
                    .ok_or_else(|| Error::new(Code::AssetUnavailable))?;
                check_cancelled(&cancel)?;
                let value = b.service.inspect_usage(node.node.relative_path).await?;
                check_cancelled(&cancel)?;
                Ok(value)
            })
            .await;
        self.finish_review(id, generation, result, None).await
    }
}
