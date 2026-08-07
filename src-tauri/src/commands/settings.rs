use std::sync::Arc;

use tauri::State;
use viewer_application::ViewerSettingsService;

use crate::{
    dto::{ViewerSettingsDto, ViewerSettingsUpdateDto},
    error::{CommandError, ErrorCategory},
};

#[tauri::command]
pub fn get_viewer_settings(service: State<'_, Arc<ViewerSettingsService>>) -> ViewerSettingsDto {
    service.load().into()
}

#[tauri::command]
pub fn update_viewer_settings(
    settings: ViewerSettingsUpdateDto,
    service: State<'_, Arc<ViewerSettingsService>>,
) -> Result<ViewerSettingsDto, CommandError> {
    let settings = settings
        .try_into()
        .map_err(|()| invalid_viewer_settings())?;
    service.update(settings).map(Into::into).map_err(Into::into)
}

fn invalid_viewer_settings() -> CommandError {
    CommandError::new(
        "invalid_viewer_settings",
        ErrorCategory::Validation,
        "设置值无效，请重新选择。",
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::invalid_viewer_settings;
    use crate::error::{CommandError, ErrorCategory};

    #[test]
    fn invalid_settings_use_the_public_validation_error() {
        assert_eq!(
            invalid_viewer_settings(),
            CommandError::new(
                "invalid_viewer_settings",
                ErrorCategory::Validation,
                "设置值无效，请重新选择。",
                false,
            )
        );
    }
}
