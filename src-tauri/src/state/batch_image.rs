//! Batch image processing at the desktop boundary (0.2.4, ADR 0008).

use super::*;
use viewer_application::{BatchImageIoPort, BatchImageService, TrashPort};
use viewer_domain::batch_image::BatchImageSettings;

pub(super) struct DesktopBatchImageIo {
    root: PathBuf,
}

impl DesktopBatchImageIo {
    pub(super) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn ensure_contained(&self, path: &Path) -> Result<(), String> {
        if path.starts_with(&self.root) {
            Ok(())
        } else {
            Err(format!(
                "batch path is outside the project: {}",
                path.display()
            ))
        }
    }
}

#[async_trait::async_trait]
impl BatchImageIoPort for DesktopBatchImageIo {
    async fn encode(
        &self,
        source: &Path,
        settings: &BatchImageSettings,
        temp: &Path,
    ) -> Result<(u32, u32, u64), ImageError> {
        self.ensure_contained(source).map_err(ImageError::Io)?;
        self.ensure_contained(temp).map_err(ImageError::Io)?;
        let source = source.to_path_buf();
        let settings = settings.clone();
        let temp = temp.to_path_buf();
        tokio::task::spawn_blocking(move || {
            viewer_platform_macos::image::transform::encode_batch_image(
                source.as_path(),
                &settings,
                temp.as_path(),
            )
        })
        .await
        .map_err(|_| ImageError::Io("batch encode task failed".into()))?
    }

    async fn trash(&self, path: &Path) -> Result<(), String> {
        self.ensure_contained(path)?;
        let port = viewer_platform_macos::files::MacTrashPort::new(&self.root)
            .map_err(|error| format!("open trash port: {error:?}"))?;
        port.trash(path)
            .await
            .map_err(|error| format!("trash batch source: {error:?}"))
    }

    fn path_exists(&self, path: &Path) -> bool {
        path.starts_with(&self.root) && std::fs::symlink_metadata(path).is_ok()
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<(), String> {
        self.ensure_contained(from)?;
        self.ensure_contained(to)?;
        std::fs::rename(from, to).map_err(|error| format!("rename batch output: {error}"))
    }

    fn create_directory(&self, path: &Path) -> Result<(), String> {
        self.ensure_contained(path)?;
        std::fs::create_dir_all(path).map_err(|error| format!("create batch folder: {error}"))
    }

    fn available_formats(&self) -> Vec<viewer_domain::batch_image::BatchImageTargetFormat> {
        viewer_platform_macos::image::transform::available_batch_formats()
    }
}

impl DesktopRuntime {
    pub async fn preview_batch_image(
        &self,
        entity_ids: Vec<EntityId>,
        settings: BatchImageSettings,
    ) -> Result<Vec<crate::dto::BatchImagePlanRowDto>, CommandError> {
        let (active, index) = {
            let session = self.session.lock().await;
            let session = session.as_ref().ok_or_else(project_not_open)?;
            (session.active.clone(), Arc::clone(&session.index))
        };
        if active.access != ProjectAccess::ReadWrite {
            return Err(project_read_only_operation());
        }
        let nodes = indexed_nodes(index.as_ref(), &entity_ids)?;
        let service =
            BatchImageService::new(Arc::new(DesktopBatchImageIo::new(active.root.clone())));
        if !service.io().available_formats().contains(&settings.format) {
            return Err(batch_image_format_unavailable());
        }
        let rows = service.plan(&active.root, &nodes, &settings);
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn execute_batch_image(
        &self,
        entity_ids: Vec<EntityId>,
        settings: BatchImageSettings,
    ) -> Result<crate::dto::BatchImageExecutionDto, CommandError> {
        let (active, index, events) = {
            let session = self.session.lock().await;
            let session = session.as_ref().ok_or_else(project_not_open)?;
            (
                session.active.clone(),
                Arc::clone(&session.index),
                Arc::clone(&self.events),
            )
        };
        if active.access != ProjectAccess::ReadWrite {
            return Err(project_read_only_operation());
        }
        let nodes = indexed_nodes(index.as_ref(), &entity_ids)?;

        // One batch at a time: the lane mirrors the file-command write lane
        // so batch outputs and file operations never interleave.
        let _lane = self.batch_image_lane.lock().await;
        let cancellation = tokio_util::sync::CancellationToken::new();
        {
            let mut active_cancel = self
                .active_batch_image_cancel
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *active_cancel = Some(cancellation.clone());
        }
        let progress_events = Arc::clone(&events);
        let progress_session = active.session_id;
        let progress_generation = active.generation;
        let progress = move |progress: viewer_application::file_commands::BatchProgress| {
            progress_events.emit_operation(progress_session, progress_generation, progress);
        };

        let service =
            BatchImageService::new(Arc::new(DesktopBatchImageIo::new(active.root.clone())));
        let execution = service
            .execute(&active.root, &nodes, &settings, &cancellation, &progress)
            .await;

        {
            let mut active_cancel = self
                .active_batch_image_cancel
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *active_cancel = None;
        }
        self.ensure_session_active(active.session_id).await?;
        Ok(execution.into())
    }

    pub fn cancel_batch_image(&self) {
        let active_cancel = self
            .active_batch_image_cancel
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(token) = active_cancel.as_ref() {
            token.cancel();
        }
    }
}

fn indexed_nodes(
    index: &dyn BrowseIndexPort,
    entity_ids: &[EntityId],
) -> Result<Vec<viewer_domain::file::FileNode>, CommandError> {
    let mut nodes = Vec::with_capacity(entity_ids.len());
    for entity_id in entity_ids {
        let node = BrowseIndexPort::node(index, *entity_id)
            .map_err(CommandError::from)?
            .ok_or_else(batch_image_target_unavailable)?;
        nodes.push(node);
    }
    Ok(nodes)
}

fn batch_image_target_unavailable() -> CommandError {
    CommandError::new(
        "batch_image_target_unavailable",
        crate::error::ErrorCategory::Content,
        "部分所选文件已不可用，请刷新后重试。",
        true,
    )
}

fn batch_image_format_unavailable() -> CommandError {
    CommandError::new(
        "batch_image_format_unavailable",
        crate::error::ErrorCategory::Validation,
        "当前系统不能输出该格式，请选择其他目标格式。",
        false,
    )
}
