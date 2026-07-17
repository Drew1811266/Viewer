use crate::{
    dto::{
        FolderTreeItemDto, FolderWorkspaceDto, ImageRepresentationDto,
        ImageRepresentationRequestDto,
    },
    error::{CommandError, ErrorCategory},
    state::DesktopRuntime,
};
use std::{str::FromStr, sync::Arc};
use tauri::State;
use viewer_domain::EntityId;

#[tauri::command]
pub async fn folder_tree(
    runtime: State<'_, Arc<DesktopRuntime>>,
) -> Result<Vec<FolderTreeItemDto>, CommandError> {
    runtime.folder_tree().await
}

#[tauri::command]
pub async fn query_folder(
    runtime: State<'_, Arc<DesktopRuntime>>,
    folder_id: Option<String>,
    aggregate: Option<bool>,
) -> Result<FolderWorkspaceDto, CommandError> {
    let folder = folder_id.map(|value| parse_entity_id(&value)).transpose()?;
    runtime
        .query_folder_projection(folder, aggregate.unwrap_or(false))
        .await
}

#[tauri::command]
pub async fn request_image_representation(
    runtime: State<'_, Arc<DesktopRuntime>>,
    entity_id: String,
    representation: ImageRepresentationRequestDto,
) -> Result<ImageRepresentationDto, CommandError> {
    runtime
        .request_image(parse_entity_id(&entity_id)?, representation.into())
        .await
}

pub(crate) fn parse_entity_id(value: &str) -> Result<EntityId, CommandError> {
    EntityId::from_str(value).map_err(|_| {
        CommandError::new(
            "invalid_entity_id",
            ErrorCategory::Validation,
            "文件标识无效，请刷新项目后重试。",
            false,
        )
    })
}
