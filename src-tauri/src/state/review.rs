use super::*;
use crate::dto::{
    ReviewCompletionProposalDto, ReviewProgressDto, ReviewScopeProposalDto,
    ReviewSessionSnapshotDto, ReviewTaskKindDto,
};
use viewer_application::{
    AddReviewFeedback, DeleteReviewFeedback, ReplaceReviewFeedbackAnchor,
    ReviewCompletionProposalId, ReviewMutationGuard, ReviewProgressPort, ReviewProposalId,
    ReviewScope, ReviewTaskProgress, StartReviewWithFeedback, UpdateReviewFeedback,
    UpdateReviewFeedbackText,
};
use viewer_domain::FeedbackId;

struct DesktopReviewProgress {
    events: Arc<dyn DesktopEventSink>,
    session_id: SessionId,
    generation: Generation,
    task_kind: ReviewTaskKindDto,
}

impl ReviewProgressPort for DesktopReviewProgress {
    fn report(&self, progress: ReviewTaskProgress) {
        self.events.emit_review(ReviewProgressDto {
            session_id: self.session_id.to_string(),
            generation: self.generation.get(),
            task_kind: self.task_kind,
            completed: progress.completed,
            total: progress.total,
            cancellable: true,
        });
    }
}

impl DesktopRuntime {
    async fn review_service(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
    ) -> Result<Arc<viewer_application::ReviewSessionService>, CommandError> {
        let session = self.session.lock().await;
        let Some(session) = session.as_ref() else {
            return Err(review_stale_session());
        };
        if session.active.session_id != expected_session
            || session.active.generation != expected_generation
        {
            return Err(review_stale_session());
        }
        Ok(Arc::clone(&session.review))
    }

    fn review_progress(
        &self,
        session_id: SessionId,
        generation: Generation,
        task_kind: ReviewTaskKindDto,
    ) -> Arc<dyn ReviewProgressPort> {
        Arc::new(DesktopReviewProgress {
            events: Arc::clone(&self.events),
            session_id,
            generation,
            task_kind,
        })
    }

    pub async fn review_status(
        &self,
        session_id: SessionId,
        generation: Generation,
    ) -> Result<ReviewSessionSnapshotDto, CommandError> {
        let service = self.review_service(session_id, generation).await?;
        Ok(service.snapshot().await.into())
    }

    pub async fn review_preview_start(
        &self,
        session_id: SessionId,
        generation: Generation,
        scope: ReviewScope,
    ) -> Result<ReviewScopeProposalDto, CommandError> {
        let service = self.review_service(session_id, generation).await?;
        service
            .preview_start(scope)
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub async fn review_start(
        &self,
        session_id: SessionId,
        generation: Generation,
        proposal_id: ReviewProposalId,
    ) -> Result<ReviewSessionSnapshotDto, CommandError> {
        let service = self.review_service(session_id, generation).await?;
        service
            .start(
                proposal_id,
                self.review_progress(session_id, generation, ReviewTaskKindDto::Start),
            )
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub async fn review_start_with_feedback(
        &self,
        session_id: SessionId,
        generation: Generation,
        command: StartReviewWithFeedback,
    ) -> Result<ReviewSessionSnapshotDto, CommandError> {
        let service = self.review_service(session_id, generation).await?;
        service
            .start_with_feedback(
                command,
                self.review_progress(session_id, generation, ReviewTaskKindDto::Start),
            )
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub async fn review_resume(
        &self,
        session_id: SessionId,
        generation: Generation,
    ) -> Result<ReviewSessionSnapshotDto, CommandError> {
        let service = self.review_service(session_id, generation).await?;
        service
            .resume(self.review_progress(session_id, generation, ReviewTaskKindDto::Resume))
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub async fn review_add_feedback(
        &self,
        session_id: SessionId,
        generation: Generation,
        command: AddReviewFeedback,
    ) -> Result<ReviewSessionSnapshotDto, CommandError> {
        let service = self.review_service(session_id, generation).await?;
        service
            .add_feedback(command)
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub async fn review_update_feedback(
        &self,
        session_id: SessionId,
        generation: Generation,
        command: UpdateReviewFeedback,
    ) -> Result<ReviewSessionSnapshotDto, CommandError> {
        let service = self.review_service(session_id, generation).await?;
        service
            .update_feedback(command)
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub async fn review_update_feedback_text(
        &self,
        session_id: SessionId,
        generation: Generation,
        command: UpdateReviewFeedbackText,
    ) -> Result<ReviewSessionSnapshotDto, CommandError> {
        let service = self.review_service(session_id, generation).await?;
        service
            .update_feedback_text(command)
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub async fn review_replace_feedback_anchor(
        &self,
        session_id: SessionId,
        generation: Generation,
        command: ReplaceReviewFeedbackAnchor,
    ) -> Result<ReviewSessionSnapshotDto, CommandError> {
        let service = self.review_service(session_id, generation).await?;
        service
            .replace_feedback_anchor(command)
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub async fn review_delete_feedback(
        &self,
        session_id: SessionId,
        generation: Generation,
        command: DeleteReviewFeedback,
    ) -> Result<ReviewSessionSnapshotDto, CommandError> {
        let service = self.review_service(session_id, generation).await?;
        service
            .delete_feedback(command)
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub async fn review_restore_deleted_feedback(
        &self,
        session_id: SessionId,
        generation: Generation,
        guard: ReviewMutationGuard,
        feedback_id: FeedbackId,
    ) -> Result<ReviewSessionSnapshotDto, CommandError> {
        let service = self.review_service(session_id, generation).await?;
        service
            .restore_deleted_feedback(guard, feedback_id)
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub async fn review_completion_summary(
        &self,
        session_id: SessionId,
        generation: Generation,
        guard: ReviewMutationGuard,
    ) -> Result<ReviewCompletionProposalDto, CommandError> {
        let service = self.review_service(session_id, generation).await?;
        service
            .completion_summary(guard)
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub async fn review_complete(
        &self,
        session_id: SessionId,
        generation: Generation,
        proposal_id: ReviewCompletionProposalId,
        guard: ReviewMutationGuard,
    ) -> Result<ReviewSessionSnapshotDto, CommandError> {
        let service = self.review_service(session_id, generation).await?;
        service
            .complete(
                proposal_id,
                guard,
                self.review_progress(session_id, generation, ReviewTaskKindDto::Complete),
            )
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub async fn review_abandon(
        &self,
        session_id: SessionId,
        generation: Generation,
        guard: ReviewMutationGuard,
    ) -> Result<ReviewSessionSnapshotDto, CommandError> {
        let service = self.review_service(session_id, generation).await?;
        service
            .abandon(guard)
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub async fn review_cancel_task(
        &self,
        session_id: SessionId,
        generation: Generation,
    ) -> Result<bool, CommandError> {
        let service = self.review_service(session_id, generation).await?;
        Ok(service.cancel_task().await)
    }
}

fn review_stale_session() -> CommandError {
    CommandError::new(
        "review_stale_session",
        ErrorCategory::Conflict,
        "评审请求已不属于当前项目会话，请刷新后重试。",
        true,
    )
}
