mod previews;
mod session;
mod tasks;
use super::*;
use crate::dto::review_workspace::{
    ReviewWorkspaceErrorCode as Code, ReviewWorkspaceErrorDto as Error,
};
pub(super) use session::{ReviewWorkspaceConfig, ReviewWorkspaceSession};
use viewer_application::{ReviewTaskCancellation, review_workspace::*};
use viewer_domain::{ReviewCommandId, ReviewSnapshotId};

fn check_cancelled(cancel: &ReviewTaskCancellation) -> Result<(), Error> {
    if cancel.is_cancelled() {
        Err(Error::new(Code::Cancelled))
    } else {
        Ok(())
    }
}

impl DesktopRuntime {
    async fn continuous_session(
        &self,
        id: SessionId,
        generation: Generation,
        writing: bool,
    ) -> Result<Arc<ReviewWorkspaceSession>, Error> {
        let session = self.session.lock().await;
        let session = session
            .as_ref()
            .ok_or_else(|| Error::new(Code::StaleSession))?;
        if session.active.session_id != id || session.active.generation != generation {
            return Err(Error::new(Code::StaleSession));
        }
        if writing && session.active.access != ProjectAccess::ReadWrite {
            return Err(Error::new(Code::ReadOnly));
        }
        Ok(session.review_workspace.clone())
    }

    async fn finish_review<T>(
        &self,
        id: SessionId,
        generation: Generation,
        result: Result<T, Error>,
        receipt: Option<ReviewCommitReceipt>,
    ) -> Result<T, Error> {
        if self.ensure_project_current(id, generation).await.is_err() {
            return match result {
                Ok(_) => Err(Error::stale_with_receipt(receipt)),
                Err(error) => Err(error), // Never turn OutcomeUnknown or a known receipt into "not saved".
            };
        }
        result
    }

    pub async fn get_review_workspace(
        &self,
        id: SessionId,
        generation: Generation,
    ) -> Result<ReviewWorkspaceView, Error> {
        let session = self.continuous_session(id, generation, false).await?;
        let writable = session.config.access == ProjectAccess::ReadWrite;
        let result = session
            .run(move |b, cancel| async move {
                let mut view = b
                    .service
                    .view_with_cancellation(b.context.stream_id, cancel)
                    .await?;
                view.capabilities.continuous_editing &= writable;
                view.capabilities.migration &= writable;
                view.capabilities.usage_import &= writable;
                Ok(view)
            })
            .await;
        self.finish_review(id, generation, result, None).await
    }
    pub async fn prepare_review_command(
        &self,
        id: SessionId,
        generation: Generation,
        command_id: ReviewCommandId,
        expected: Option<ReviewSnapshotId>,
        command: ReviewWorkspaceCommand,
    ) -> Result<ReviewCommandEnvelope, Error> {
        let session = self.continuous_session(id, generation, true).await?;
        let result = session
            .run(move |b, cancel| async move {
                let value = b.service.prepare(command_id, expected, command).await?;
                check_cancelled(&cancel)?;
                Ok(value)
            })
            .await;
        self.finish_review(id, generation, result, None).await
    }
    pub async fn apply_review_command(
        &self,
        id: SessionId,
        generation: Generation,
        envelope: ReviewCommandEnvelope,
    ) -> Result<ReviewApplyResult, Error> {
        let session = self.continuous_session(id, generation, true).await?;
        let result = session
            .run(move |b, cancel| async move {
                b.service
                    .apply_with_cancellation(envelope, cancel)
                    .await
                    .map_err(Into::into)
            })
            .await;
        let receipt = result.as_ref().ok().map(|r| r.receipt);
        self.finish_review(id, generation, result, receipt).await
    }
    pub async fn cancel_review_workspace_task(
        &self,
        id: SessionId,
        generation: Generation,
    ) -> Result<usize, Error> {
        // Keep the session guard through cancellation so a close/reopen cannot retarget it.
        let session = self.session.lock().await;
        let session = session
            .as_ref()
            .ok_or_else(|| Error::new(Code::StaleSession))?;
        if session.active.session_id != id || session.active.generation != generation {
            return Err(Error::new(Code::StaleSession));
        }
        Ok(session.review_workspace.tasks.cancel())
    }
}
