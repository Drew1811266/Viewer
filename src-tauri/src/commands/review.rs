use crate::dto::{
    ReviewAddFeedbackRequestDto, ReviewCompleteRequestDto, ReviewCompletionProposalDto,
    ReviewDeleteFeedbackRequestDto, ReviewGuardRequestDto, ReviewPreviewStartRequestDto,
    ReviewReplaceFeedbackAnchorRequestDto, ReviewRestoreDeletedFeedbackRequestDto,
    ReviewScopeProposalDto, ReviewSessionSnapshotDto, ReviewStartRequestDto,
    ReviewStartWithFeedbackRequestDto, ReviewUpdateFeedbackRequestDto,
    ReviewUpdateFeedbackTextRequestDto, SessionGenerationRequestDto,
};
use crate::error::CommandError;
use crate::state::DesktopRuntime;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn review_status(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: SessionGenerationRequestDto,
) -> Result<ReviewSessionSnapshotDto, CommandError> {
    let context = request.try_into_context()?;
    runtime
        .review_status(context.session_id, context.generation)
        .await
}

#[tauri::command]
pub async fn review_preview_start(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ReviewPreviewStartRequestDto,
) -> Result<ReviewScopeProposalDto, CommandError> {
    let (context, scope) = request.try_into_parts()?;
    runtime
        .review_preview_start(context.session_id, context.generation, scope)
        .await
}

#[tauri::command]
pub async fn review_start(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ReviewStartRequestDto,
) -> Result<ReviewSessionSnapshotDto, CommandError> {
    let (context, proposal_id) = request.try_into_parts()?;
    runtime
        .review_start(context.session_id, context.generation, proposal_id)
        .await
}

#[tauri::command]
pub async fn review_start_with_feedback(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ReviewStartWithFeedbackRequestDto,
) -> Result<ReviewSessionSnapshotDto, CommandError> {
    let (context, command) = request.try_into_parts()?;
    runtime
        .review_start_with_feedback(context.session_id, context.generation, command)
        .await
}

#[tauri::command]
pub async fn review_resume(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: SessionGenerationRequestDto,
) -> Result<ReviewSessionSnapshotDto, CommandError> {
    let context = request.try_into_context()?;
    runtime
        .review_resume(context.session_id, context.generation)
        .await
}

#[tauri::command]
pub async fn review_add_feedback(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ReviewAddFeedbackRequestDto,
) -> Result<ReviewSessionSnapshotDto, CommandError> {
    let (context, command) = request.try_into_parts()?;
    runtime
        .review_add_feedback(context.session_id, context.generation, command)
        .await
}

#[tauri::command]
pub async fn review_update_feedback(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ReviewUpdateFeedbackRequestDto,
) -> Result<ReviewSessionSnapshotDto, CommandError> {
    let (context, command) = request.try_into_parts()?;
    runtime
        .review_update_feedback(context.session_id, context.generation, command)
        .await
}

#[tauri::command]
pub async fn review_update_feedback_text(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ReviewUpdateFeedbackTextRequestDto,
) -> Result<ReviewSessionSnapshotDto, CommandError> {
    let (context, command) = request.try_into_parts()?;
    runtime
        .review_update_feedback_text(context.session_id, context.generation, command)
        .await
}

#[tauri::command]
pub async fn review_replace_feedback_anchor(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ReviewReplaceFeedbackAnchorRequestDto,
) -> Result<ReviewSessionSnapshotDto, CommandError> {
    let (context, command) = request.try_into_parts()?;
    runtime
        .review_replace_feedback_anchor(context.session_id, context.generation, command)
        .await
}

#[tauri::command]
pub async fn review_delete_feedback(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ReviewDeleteFeedbackRequestDto,
) -> Result<ReviewSessionSnapshotDto, CommandError> {
    let (context, command) = request.try_into_parts()?;
    runtime
        .review_delete_feedback(context.session_id, context.generation, command)
        .await
}

#[tauri::command]
pub async fn review_restore_deleted_feedback(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ReviewRestoreDeletedFeedbackRequestDto,
) -> Result<ReviewSessionSnapshotDto, CommandError> {
    let (context, command) = request.try_into_parts()?;
    runtime
        .review_restore_deleted_feedback(
            context.session_id,
            context.generation,
            command.guard,
            command.feedback_id,
        )
        .await
}

#[tauri::command]
pub async fn review_completion_summary(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ReviewGuardRequestDto,
) -> Result<ReviewCompletionProposalDto, CommandError> {
    let (context, guard) = request.try_into_parts()?;
    runtime
        .review_completion_summary(context.session_id, context.generation, guard)
        .await
}

#[tauri::command]
pub async fn review_complete(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ReviewCompleteRequestDto,
) -> Result<ReviewSessionSnapshotDto, CommandError> {
    let (context, proposal_id, guard) = request.try_into_parts()?;
    runtime
        .review_complete(context.session_id, context.generation, proposal_id, guard)
        .await
}

#[tauri::command]
pub async fn review_abandon(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ReviewGuardRequestDto,
) -> Result<ReviewSessionSnapshotDto, CommandError> {
    let (context, guard) = request.try_into_parts()?;
    runtime
        .review_abandon(context.session_id, context.generation, guard)
        .await
}

#[tauri::command]
pub async fn review_cancel_task(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: SessionGenerationRequestDto,
) -> Result<bool, CommandError> {
    let context = request.try_into_context()?;
    runtime
        .review_cancel_task(context.session_id, context.generation)
        .await
}
