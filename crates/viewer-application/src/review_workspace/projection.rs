use super::*;
use crate::ReviewTaskCancellation;
use viewer_domain::review::continuous::*;

impl ContinuousReviewService {
    pub async fn view(
        &self,
        stream: viewer_domain::ReviewStreamId,
    ) -> Result<ReviewWorkspaceView, ReviewWorkspaceError> {
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
            return Ok(ReviewWorkspaceView {
                stream_id: stream,
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
        let (current, recovery) = super::service::io(move || {
            let reader = provider.open_reader()?;
            let current = reader.load_current(stream)?;
            let recovery = reader.load_unresolved_recovery(stream)?;
            Ok((current, recovery))
        })
        .await?;
        if current.as_ref().is_some_and(|s| {
            s.state.project_id != self.context.project_id || s.production != self.context.production
        }) {
            return Err(ReviewWorkspaceError::WrongContext);
        }
        let (source_checks, projection) = if let Some(current) = &current {
            let checks = self
                .assets
                .check_sources(&current.state.assets, ReviewTaskCancellation::default())
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
        Ok(ReviewWorkspaceView {
            stream_id: stream,
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
}
