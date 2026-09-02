use super::*;
use crate::ReviewTaskCancellation;
use viewer_domain::review::continuous::*;

impl ContinuousReviewService {
    /// Loads only the authoritative logical head. This is the reconciliation source for patch
    /// tests and, after activation, for UI patch mismatches; it never performs source checks.
    pub async fn view_authoring_current(
        &self,
        stream: viewer_domain::ReviewStreamId,
    ) -> Result<Option<ReviewWorkspaceCurrent>, ReviewWorkspaceError> {
        if stream != self.context.stream_id {
            return Err(ReviewWorkspaceError::WrongContext);
        }
        let provider = self.provider.clone();
        let lookup_provider = provider.clone();
        let loaded = super::service::io(move || {
            let store = lookup_provider.open_authoring_reader()?;
            let heads = store.load_heads(stream)?;
            let Some(authoring) = store.load_current(stream)? else {
                return Ok(None);
            };
            if heads.authoring != Some(authoring.head) {
                return Err(ReviewCommitError::Integrity);
            }
            Ok(Some((authoring, heads)))
        })
        .await?;
        let Some((authoring, heads)) = loaded else {
            return Ok(None);
        };
        let (published_ref, evidence) = if let Some(published_head) = heads.published {
            let provider = provider.clone();
            super::service::io(move || {
                let current = provider
                    .open_reader()?
                    .load_current_for_materialization(stream)?
                    .ok_or(ReviewCommitError::Integrity)?;
                if current.state.snapshot_id != published_head.snapshot_id {
                    return Err(ReviewCommitError::Integrity);
                }
                Ok((Some(current.reference), current.evidence))
            })
            .await?
        } else {
            (None, vec![])
        };
        Ok(Some(ReviewWorkspaceCurrent {
            authoring,
            published_ref,
            evidence,
        }))
    }

    /// UI projection for the authoring/outbox pipeline. It reads the logical head even while the
    /// external v3 reader is deliberately gated as `publication_pending`.
    pub async fn view_authoring_with_cancellation(
        &self,
        stream: viewer_domain::ReviewStreamId,
        cancellation: ReviewTaskCancellation,
    ) -> Result<ReviewWorkspaceView, ReviewWorkspaceError> {
        super::preview::check_cancelled(&cancellation)?;
        if self.inspect_migration().await?.is_some() {
            return self.view_with_cancellation(stream, cancellation).await;
        }
        let current = self.view_authoring_current(stream).await?;
        let provider = self.provider.clone();
        let (history_selectors, recovery) = super::service::io(move || {
            let reader = provider.open_reader()?;
            Ok((
                reader.load_history_selectors(stream)?,
                reader.load_unresolved_recovery(stream)?,
            ))
        })
        .await?;
        let (source_checks, projection) = if let Some(current) = &current {
            let checks = self
                .assets
                .check_sources(&current.authoring.state.assets, cancellation.clone())
                .await?;
            let projection = project_current(&current.authoring.state, &checks)?;
            (checks, projection)
        } else {
            (
                vec![],
                CurrentReviewProjection {
                    actionable: vec![],
                    needs_confirmation: vec![],
                },
            )
        };
        super::preview::check_cancelled(&cancellation)?;
        Ok(ReviewWorkspaceView {
            stream_id: stream,
            history_selectors,
            current,
            source_checks,
            projection,
            recovery,
            migration: None,
            capabilities: ReviewWorkspaceCapabilities {
                continuous_editing: true,
                usage_import: self.usage_importer.is_some(),
                migration: false,
            },
        })
    }

    pub async fn view(
        &self,
        stream: viewer_domain::ReviewStreamId,
    ) -> Result<ReviewWorkspaceView, ReviewWorkspaceError> {
        self.view_with_cancellation(stream, ReviewTaskCancellation::default())
            .await
    }

    pub async fn view_with_cancellation(
        &self,
        stream: viewer_domain::ReviewStreamId,
        cancellation: ReviewTaskCancellation,
    ) -> Result<ReviewWorkspaceView, ReviewWorkspaceError> {
        super::preview::check_cancelled(&cancellation)?;
        if stream != self.context.stream_id {
            return Err(ReviewWorkspaceError::WrongContext);
        }
        if let Some(migration) = self.inspect_migration().await? {
            let provider = self.provider.clone();
            let recovery = super::service::io(move || provider.load_migration_recovery())
                .await?
                .into_iter()
                .filter(|d| d.stream_id == stream)
                .collect();
            super::preview::check_cancelled(&cancellation)?;
            return Ok(ReviewWorkspaceView {
                stream_id: stream,
                history_selectors: vec![],
                current: None,
                source_checks: vec![],
                projection: CurrentReviewProjection {
                    actionable: vec![],
                    needs_confirmation: vec![],
                },
                recovery,
                migration: Some(migration),
                capabilities: ReviewWorkspaceCapabilities {
                    continuous_editing: false,
                    usage_import: false,
                    migration: true,
                },
            });
        }
        let provider = self.provider.clone();
        let (current, history_selectors, recovery) = super::service::io(move || {
            let reader = provider.open_reader()?;
            let current = reader.load_current(stream)?;
            let history_selectors = reader.load_history_selectors(stream)?;
            let recovery = reader.load_unresolved_recovery(stream)?;
            Ok((current, history_selectors, recovery))
        })
        .await?;
        super::preview::check_cancelled(&cancellation)?;
        if current.as_ref().is_some_and(|s| {
            s.state.project_id != self.context.project_id || s.production != self.context.production
        }) {
            return Err(ReviewWorkspaceError::WrongContext);
        }
        let (source_checks, projection) = if let Some(current) = &current {
            let checks = self
                .assets
                .check_sources(&current.state.assets, cancellation.clone())
                .await?;
            let projection = project_current(&current.state, &checks)?;
            (checks, projection)
        } else {
            (
                vec![],
                CurrentReviewProjection {
                    actionable: vec![],
                    needs_confirmation: vec![],
                },
            )
        };
        super::preview::check_cancelled(&cancellation)?;
        Ok(ReviewWorkspaceView {
            stream_id: stream,
            history_selectors,
            current: current.map(ReviewWorkspaceCurrent::from_published),
            source_checks,
            projection,
            recovery,
            migration: None,
            capabilities: ReviewWorkspaceCapabilities {
                continuous_editing: true,
                usage_import: self.usage_importer.is_some(),
                migration: false,
            },
        })
    }
}
