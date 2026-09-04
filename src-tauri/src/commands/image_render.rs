use std::sync::Arc;

use tauri::{State, WebviewWindow};

use crate::{
    dto::{ImageRenderAckDto, ImageRenderCommandDto},
    error::CommandError,
    image_render_runtime::ImageRenderRuntime,
};

#[tauri::command]
pub async fn image_render_command(
    command: ImageRenderCommandDto,
    window: WebviewWindow,
    runtime: State<'_, Arc<ImageRenderRuntime>>,
) -> Result<ImageRenderAckDto, CommandError> {
    runtime
        .dispatch_from_window(command, window)
        .await
        .map_err(CommandError::from)
}
