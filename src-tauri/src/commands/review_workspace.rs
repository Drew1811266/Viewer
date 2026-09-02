use crate::{dto::review_workspace::*, state::DesktopRuntime};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use viewer_application::review_workspace::{MigrationInspection, UsageImportPreview};
use viewer_domain::{
    review::continuous::{ArchivePlan, RestorePlan},
    search::Generation,
};

#[tauri::command]
pub async fn get_review_workspace(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ReviewWorkspaceSessionRequestDto,
) -> Result<ReviewWorkspaceViewDto, ReviewWorkspaceErrorDto> {
    runtime
        .get_review_workspace(request.session_id, Generation::new(request.generation))
        .await
        .map(Into::into)
        .and_then(|value| review_response(value, None))
}
#[tauri::command]
pub async fn prepare_review_assets(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: PrepareReviewAssetsRequestDto,
) -> Result<Vec<PreparedReviewAssetDto>, ReviewWorkspaceErrorDto> {
    runtime
        .prepare_review_assets(
            request.session_id,
            Generation::new(request.generation),
            request.entity_ids,
        )
        .await
        .and_then(|value| review_response(value, None))
}
#[tauri::command]
pub async fn prepare_review_command(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: PrepareReviewCommandRequestDto,
) -> Result<PreparedReviewCommandDto, ReviewWorkspaceErrorDto> {
    runtime
        .prepare_review_command(
            request.session_id,
            Generation::new(request.generation),
            request.command_id,
            request.expected_snapshot_id,
            request.command,
        )
        .await
        .map(Into::into)
        .and_then(|value| review_response(value, None))
}
#[tauri::command]
pub async fn apply_review_command(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ApplyReviewCommandRequestDto,
) -> Result<ReviewAuthoringApplyResultDto, ReviewWorkspaceErrorDto> {
    runtime
        .apply_review_authoring_command(
            request.session_id,
            Generation::new(request.generation),
            request.envelope,
        )
        .await
        .map(Into::into)
        .and_then(|value| review_response(value, None))
}

#[tauri::command]
pub async fn get_review_publication_status(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ReviewWorkspaceSessionRequestDto,
) -> Result<ReviewPublicationStatusDto, ReviewWorkspaceErrorDto> {
    runtime
        .get_review_publication_status(request.session_id, Generation::new(request.generation))
        .await
        .map(Into::into)
        .and_then(|value| review_response(value, None))
}
#[tauri::command]
pub async fn preview_review_archive(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: PreviewReviewArchiveRequestDto,
) -> Result<ReviewWire<ArchivePlan>, ReviewWorkspaceErrorDto> {
    runtime
        .preview_review_archive(
            request.session_id,
            Generation::new(request.generation),
            request.selection,
        )
        .await
        .map(Into::into)
        .and_then(|value| review_response(value, None))
}
#[tauri::command]
pub async fn preview_review_restore(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: PreviewReviewRestoreRequestDto,
) -> Result<ReviewWire<RestorePlan>, ReviewWorkspaceErrorDto> {
    runtime
        .preview_review_restore(
            request.session_id,
            Generation::new(request.generation),
            request.archive_id,
            request.decisions,
        )
        .await
        .map(Into::into)
        .and_then(|value| review_response(value, None))
}
#[tauri::command]
pub async fn get_review_history(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ReviewHistoryRequestDto,
) -> Result<ReviewHistoryViewDto, ReviewWorkspaceErrorDto> {
    runtime
        .get_review_history(
            request.session_id,
            Generation::new(request.generation),
            request.selector,
        )
        .await
        .map(Into::into)
        .and_then(|value| review_response(value, None))
}
#[tauri::command]
pub async fn inspect_review_usage(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: InspectReviewUsageRequestDto,
) -> Result<ReviewWire<UsageImportPreview>, ReviewWorkspaceErrorDto> {
    runtime
        .inspect_review_usage(
            request.session_id,
            Generation::new(request.generation),
            request.entity_id,
        )
        .await
        .map(Into::into)
        .and_then(|value| review_response(value, None))
}
#[tauri::command]
pub async fn inspect_review_usage_selection(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ReviewWorkspaceSessionRequestDto,
    selected_path: PathBuf,
) -> Result<ReviewWire<UsageImportPreview>, ReviewWorkspaceErrorDto> {
    runtime
        .inspect_review_usage_selection(
            request.session_id,
            Generation::new(request.generation),
            selected_path,
        )
        .await
        .map(Into::into)
        .and_then(|value| review_response(value, None))
}
#[tauri::command]
pub async fn inspect_review_migration(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ReviewWorkspaceSessionRequestDto,
) -> Result<ReviewWire<Option<MigrationInspection>>, ReviewWorkspaceErrorDto> {
    runtime
        .inspect_review_migration(request.session_id, Generation::new(request.generation))
        .await
        .map(Into::into)
        .and_then(|value| review_response(value, None))
}
#[tauri::command]
pub async fn get_review_evidence(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ReviewEvidenceRequestDto,
) -> Result<ReviewEvidenceImageDto, ReviewWorkspaceErrorDto> {
    runtime
        .get_review_evidence(
            request.session_id,
            Generation::new(request.generation),
            request.selector,
            request.asset_version_id,
            request.role,
        )
        .await
        .and_then(|value| review_response(value, None))
}
#[tauri::command]
pub async fn cancel_review_workspace_task(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ReviewWorkspaceSessionRequestDto,
) -> Result<usize, ReviewWorkspaceErrorDto> {
    runtime
        .cancel_review_workspace_task(request.session_id, Generation::new(request.generation))
        .await
        .and_then(|value| review_response(value, None))
}
