use crate::{
    commands::browse::parse_entity_id,
    dto::{SaveTextResultDto, TextPreviewDto},
    error::CommandError,
    state::DesktopRuntime,
};
use std::sync::Arc;
use tauri::State;
use viewer_application::{TextEncoding, TextNewlineStyle};

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
pub async fn save_text(
    runtime: State<'_, Arc<DesktopRuntime>>,
    entity_id: String,
    content: String,
    expected_fingerprint: String,
    newline_style: TextNewlineStyle,
    acknowledge_encoding_conversion: bool,
    save_as_copy: Option<bool>,
) -> Result<SaveTextResultDto, CommandError> {
    runtime
        .save_text(
            parse_entity_id(&entity_id)?,
            content,
            expected_fingerprint,
            newline_style,
            acknowledge_encoding_conversion,
            save_as_copy.unwrap_or(false),
        )
        .await
}

#[tauri::command]
pub async fn render_markdown(content: String) -> Result<String, CommandError> {
    if content.len() > viewer_application::MAX_TEXT_PREVIEW_BYTES {
        return Err(CommandError::new(
            "text_render_too_large",
            crate::error::ErrorCategory::Validation,
            "内容超出可渲染的大小限制。",
            false,
        ));
    }
    Ok(crate::markdown::render_safe_markdown(
        &content,
        &std::collections::HashMap::new(),
    ))
}

#[tauri::command]
pub async fn open_external_link(
    runtime: State<'_, Arc<DesktopRuntime>>,
    url: String,
) -> Result<(), CommandError> {
    runtime.open_external_link(&url).await
}
