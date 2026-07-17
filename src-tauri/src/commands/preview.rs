use crate::{
    commands::browse::parse_entity_id, dto::TextPreviewDto, error::CommandError,
    state::DesktopRuntime,
};
use std::sync::Arc;
use tauri::State;
use viewer_application::TextEncoding;

#[tauri::command]
pub async fn preview_text(
    runtime: State<'_, Arc<DesktopRuntime>>,
    entity_id: String,
    encoding: Option<TextEncoding>,
) -> Result<TextPreviewDto, CommandError> {
    runtime
        .preview_text(parse_entity_id(&entity_id)?, encoding)
        .await
}

#[tauri::command]
pub async fn open_external_link(
    runtime: State<'_, Arc<DesktopRuntime>>,
    url: String,
) -> Result<(), CommandError> {
    runtime.open_external_link(&url).await
}
