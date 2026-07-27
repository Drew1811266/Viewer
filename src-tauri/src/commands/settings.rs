use std::sync::Arc;

use tauri::State;
use viewer_application::ViewerSettingsService;

use crate::{
    dto::{ThumbnailDensityDto, ViewerSettingsDto},
    error::CommandError,
};

#[tauri::command]
pub fn get_viewer_settings(service: State<'_, Arc<ViewerSettingsService>>) -> ViewerSettingsDto {
    service.load().into()
}

#[tauri::command]
pub fn update_thumbnail_density(
    density: ThumbnailDensityDto,
    service: State<'_, Arc<ViewerSettingsService>>,
) -> Result<ViewerSettingsDto, CommandError> {
    service
        .update_thumbnail_density(density.into())
        .map(Into::into)
        .map_err(Into::into)
}
