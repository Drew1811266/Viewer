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
        let provider = self.provider.clone();
        let (current, recovery) = super::service::io(move || {
            let reader = provider.open_reader()?;
            let current = reader.load_current(stream)?;
            let mut recovery = vec![];
            for draft in reader
                .load_recovery()?
                .into_iter()
                .filter(|r| r.stream_id == stream)
            {
                if !matches!(
                    reader.resolve_recovery(draft.command_id)?,
                    CommandLookup::Found(_)
                ) {
                    recovery.push(draft);
                }
            }
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
                usage_import: false,
                migration: false,
            },
        })
    }
}
