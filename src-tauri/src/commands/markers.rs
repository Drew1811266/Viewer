use std::sync::Arc;

use tauri::State;
use viewer_domain::{EntityId, search::Generation};

use crate::{
    commands::{browse::parse_entity_id, search::parse_session_id},
    dto::{
        MarkerBatchResultDto, SelectionInfoDto, SelectionInfoRequestDto, SetReviewStateRequestDto,
        ToggleFavoriteRequestDto,
    },
    error::{CommandError, ErrorCategory},
    state::DesktopRuntime,
};

const MAX_MARKER_TARGETS: usize = 10_000;

#[tauri::command]
pub async fn set_review_state(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: SetReviewStateRequestDto,
) -> Result<MarkerBatchResultDto, CommandError> {
    let entity_ids = parse_entity_ids(&request.entity_ids)?;
    runtime
        .set_review_state(
            parse_session_id(&request.session_id)?,
            Generation::new(request.generation),
            &entity_ids,
            request.review_state,
        )
        .await
}

#[tauri::command]
pub async fn toggle_favorite(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: ToggleFavoriteRequestDto,
) -> Result<MarkerBatchResultDto, CommandError> {
    let entity_ids = parse_entity_ids(&request.entity_ids)?;
    runtime
        .toggle_favorite(
            parse_session_id(&request.session_id)?,
            Generation::new(request.generation),
            &entity_ids,
        )
        .await
}

#[tauri::command]
pub async fn selection_info(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: SelectionInfoRequestDto,
) -> Result<SelectionInfoDto, CommandError> {
    let entity_ids = parse_entity_ids(&request.entity_ids)?;
    runtime
        .selection_info(
            parse_session_id(&request.session_id)?,
            Generation::new(request.generation),
            &entity_ids,
        )
        .await
}

fn parse_entity_ids(values: &[String]) -> Result<Vec<EntityId>, CommandError> {
    if values.len() > MAX_MARKER_TARGETS {
        return Err(CommandError::new(
            "too_many_marker_targets",
            ErrorCategory::Validation,
            "一次选择的文件数量过多，请分批操作。",
            false,
        ));
    }
    values.iter().map(|value| parse_entity_id(value)).collect()
}
