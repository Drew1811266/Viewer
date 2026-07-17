use crate::{
    dto::{
        FolderTreeItemDto, FolderWorkspaceDto, ImageRepresentationDto, ProjectSnapshot,
        TextPreviewDto, TextPreviewFormatDto,
    },
    error::{CommandError, ErrorCategory},
    image_protocol::ActiveImageSession,
    markdown::{ExternalUrl, image_destinations, render_safe_markdown},
};
use std::{collections::HashMap, path::Path, path::PathBuf, sync::Arc, time::Duration};
use tokio::{sync::Mutex, task::JoinHandle, time::Instant};
use viewer_application::{
    ActiveProject, BrowseIndexPort, BrowseService, ImageError, ImagePort, ImageRequest,
    ProjectAccess, ProjectOpenError, ProjectProbeError, ProjectProbePort, ProjectSessionService,
    ScanPort, TextEncoding, TextPreviewPort,
    scan::{CoordinatedScan, ScanEvent, ScanRequest},
    scheduler::{TaskClass, TaskCoordinator},
};
use viewer_domain::{
    EntityId, RelativePath, SessionId, TaskId, file::FileKind, image::ImageRepresentationKind,
};
use viewer_infrastructure::{
    image_cache::{ImageArtifactRegistry, ImageCacheKey, ImageCacheKeyInput},
    scan::walker::{ProjectWalker, is_macos_alias},
    search::index::SessionIndex,
    session_cache::{CachedImage, SessionCache},
    text::preview::TextPreviewReader,
};

pub use crate::dto::ScanEventDto;

pub trait DesktopEventSink: Send + Sync {
    fn emit_scan(&self, event: ScanEventDto);
}

#[derive(Default)]
struct NoopEventSink;

impl DesktopEventSink for NoopEventSink {
    fn emit_scan(&self, _event: ScanEventDto) {}
}

pub trait DesktopImageFactory: Send + Sync {
    fn create(&self, cache_root: &Path) -> Result<Arc<dyn ImagePort>, ImageError>;
}

#[derive(Default)]
pub struct MacDesktopImageFactory;

impl DesktopImageFactory for MacDesktopImageFactory {
    fn create(&self, cache_root: &Path) -> Result<Arc<dyn ImagePort>, ImageError> {
        Ok(Arc::new(viewer_platform_macos::image::MacImagePort::new(
            cache_root,
        )?))
    }
}

#[derive(Clone)]
struct SharedProjectProbe(Arc<dyn ProjectProbePort>);

impl ProjectProbePort for SharedProjectProbe {
    fn probe(&self, root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
        self.0.probe(root)
    }
}

struct DesktopSession {
    active: ActiveProject,
    snapshot: ProjectSnapshot,
    cache: Arc<SessionCache>,
    index: Arc<SessionIndex>,
    image: Arc<dyn ImagePort>,
    scan_task_id: TaskId,
    scan_task: Option<JoinHandle<Result<(), CommandError>>>,
}

pub struct DesktopRuntime {
    cache_base: PathBuf,
    coordinator: Arc<TaskCoordinator>,
    project_service: ProjectSessionService<SharedProjectProbe>,
    scanner: Arc<dyn ScanPort>,
    events: Arc<dyn DesktopEventSink>,
    image_factory: Arc<dyn DesktopImageFactory>,
    image_registry: Arc<ImageArtifactRegistry>,
    active_image_session: ActiveImageSession,
    text_reader: Arc<dyn TextPreviewPort>,
    session: Mutex<Option<DesktopSession>>,
}

impl DesktopRuntime {
    pub fn new(cache_base: PathBuf, probe: Arc<dyn ProjectProbePort>) -> Self {
        Self::new_with_dependencies(
            cache_base,
            probe,
            Arc::new(ProjectWalker),
            Arc::new(NoopEventSink),
        )
    }

    pub fn new_with_dependencies(
        cache_base: PathBuf,
        probe: Arc<dyn ProjectProbePort>,
        scanner: Arc<dyn ScanPort>,
        events: Arc<dyn DesktopEventSink>,
    ) -> Self {
        Self::new_with_image_factory(
            cache_base,
            probe,
            scanner,
            events,
            Arc::new(MacDesktopImageFactory),
            Arc::new(ImageArtifactRegistry::default()),
        )
    }

    pub fn new_with_image_factory(
        cache_base: PathBuf,
        probe: Arc<dyn ProjectProbePort>,
        scanner: Arc<dyn ScanPort>,
        events: Arc<dyn DesktopEventSink>,
        image_factory: Arc<dyn DesktopImageFactory>,
        image_registry: Arc<ImageArtifactRegistry>,
    ) -> Self {
        Self::new_with_image_services(
            cache_base,
            probe,
            scanner,
            events,
            image_factory,
            image_registry,
            ActiveImageSession::default(),
        )
    }

    pub fn new_with_image_services(
        cache_base: PathBuf,
        probe: Arc<dyn ProjectProbePort>,
        scanner: Arc<dyn ScanPort>,
        events: Arc<dyn DesktopEventSink>,
        image_factory: Arc<dyn DesktopImageFactory>,
        image_registry: Arc<ImageArtifactRegistry>,
        active_image_session: ActiveImageSession,
    ) -> Self {
        let coordinator = Arc::new(TaskCoordinator::default());
        Self {
            cache_base,
            project_service: ProjectSessionService::new(
                SharedProjectProbe(probe),
                Arc::clone(&coordinator),
            ),
            coordinator,
            scanner,
            events,
            image_factory,
            image_registry,
            active_image_session,
            text_reader: Arc::new(TextPreviewReader),
            session: Mutex::new(None),
        }
    }

    pub async fn open_project(&self, root: &Path) -> Result<ProjectSnapshot, CommandError> {
        let mut session = self.session.lock().await;
        if session.is_some() {
            return Err(CommandError::from(ProjectOpenError::AlreadyOpen));
        }
        let active = self
            .project_service
            .open(root)
            .map_err(CommandError::from)?;
        let cache = match SessionCache::create_in(&self.cache_base, active.session_id) {
            Ok(cache) => Arc::new(cache),
            Err(error) => {
                let _ = self.project_service.close();
                return Err(error.into());
            }
        };
        let index = match SessionIndex::open(cache.index_path()) {
            Ok(index) => Arc::new(index),
            Err(error) => {
                let _ = cache.cleanup();
                let _ = self.project_service.close();
                return Err(error.into());
            }
        };
        let image = match self.image_factory.create(&cache.image_root()) {
            Ok(image) => image,
            Err(error) => {
                drop(index);
                let _ = cache.cleanup();
                let _ = self.project_service.close();
                return Err(error.into());
            }
        };
        let snapshot = ProjectSnapshot::from(&active);
        let active_session_id = active.session_id;
        let scan_task_id = TaskId::new();
        let scan_task = tokio::spawn(run_scan(
            active.clone(),
            scan_task_id,
            Arc::clone(&self.scanner),
            Arc::clone(&self.coordinator),
            Arc::clone(&index),
            Arc::clone(&self.events),
        ));
        *session = Some(DesktopSession {
            active,
            snapshot: snapshot.clone(),
            cache,
            index,
            image,
            scan_task_id,
            scan_task: Some(scan_task),
        });
        self.active_image_session.set(Some(active_session_id));
        Ok(snapshot)
    }

    pub async fn close_project(&self) -> Result<(), CommandError> {
        let Some(mut session) = self.session.lock().await.take() else {
            return Ok(());
        };
        self.active_image_session.set(None);
        self.image_registry
            .remove_session(session.active.session_id);
        self.project_service.close().map_err(CommandError::from)?;
        session
            .image
            .cancel_session(session.active.session_id)
            .await;
        if let Some(scan_task) = session.scan_task.take() {
            scan_task.abort();
            let _ = scan_task.await;
        }
        drop(session.index);
        session.cache.cleanup().map_err(CommandError::from)
    }

    pub async fn wait_for_scan(&self) -> Result<(), CommandError> {
        let scan_task = {
            let mut session = self.session.lock().await;
            let Some(session) = session.as_mut() else {
                return Err(CommandError::new(
                    "project_not_open",
                    crate::error::ErrorCategory::Conflict,
                    "请先打开一个项目。",
                    false,
                ));
            };
            session.scan_task.take()
        };
        let Some(scan_task) = scan_task else {
            return Ok(());
        };
        match scan_task.await {
            Ok(result) => result,
            Err(error) if error.is_cancelled() => Ok(()),
            Err(_) => Err(CommandError::new(
                "scan_worker_failed",
                crate::error::ErrorCategory::Internal,
                "项目扫描意外终止，请重新打开项目。",
                true,
            )),
        }
    }

    pub async fn cancel_task(&self, task_id: &str) -> bool {
        let scan_task = {
            let mut session = self.session.lock().await;
            let Some(session) = session.as_mut() else {
                return false;
            };
            if session.scan_task_id.to_string() != task_id {
                return false;
            }
            session.scan_task.take()
        };
        let Some(scan_task) = scan_task else {
            return false;
        };
        scan_task.abort();
        let _ = scan_task.await;
        true
    }

    pub async fn snapshot(&self) -> Option<ProjectSnapshot> {
        self.session
            .lock()
            .await
            .as_ref()
            .map(|session| session.snapshot.clone())
    }

    pub async fn folder_tree(&self) -> Result<Vec<FolderTreeItemDto>, CommandError> {
        let index = self.active_index().await?;
        BrowseService::new(index.as_ref())
            .folder_tree()
            .map(|folders| folders.into_iter().map(FolderTreeItemDto::from).collect())
            .map_err(CommandError::from)
    }

    pub async fn query_folder(
        &self,
        folder: Option<EntityId>,
    ) -> Result<FolderWorkspaceDto, CommandError> {
        self.query_folder_projection(folder, false).await
    }

    pub async fn query_folder_projection(
        &self,
        folder: Option<EntityId>,
        aggregate: bool,
    ) -> Result<FolderWorkspaceDto, CommandError> {
        let index = self.active_index().await?;
        let service = BrowseService::new(index.as_ref());
        let result = if aggregate {
            service.aggregate_workspace(folder)
        } else {
            service.folder_workspace(folder)
        };
        result
            .map(FolderWorkspaceDto::from)
            .map_err(CommandError::from)
    }

    pub async fn request_image(
        &self,
        entity_id: EntityId,
        kind: ImageRepresentationKind,
    ) -> Result<ImageRepresentationDto, CommandError> {
        self.request_image_for_session(None, entity_id, kind).await
    }

    async fn request_image_for_session(
        &self,
        expected_session: Option<SessionId>,
        entity_id: EntityId,
        kind: ImageRepresentationKind,
    ) -> Result<ImageRepresentationDto, CommandError> {
        let (active, index, cache, image) = {
            let session = self.session.lock().await;
            let session = session.as_ref().ok_or_else(project_not_open)?;
            if expected_session.is_some_and(|expected| expected != session.active.session_id) {
                return Err(CommandError::from(ImageError::Cancelled));
            }
            (
                session.active.clone(),
                Arc::clone(&session.index),
                Arc::clone(&session.cache),
                Arc::clone(&session.image),
            )
        };
        let node = BrowseIndexPort::node(index.as_ref(), entity_id)
            .map_err(CommandError::from)?
            .ok_or_else(image_not_found)?;
        if !matches!(node.kind, FileKind::Jpeg | FileKind::Png) {
            return Err(image_not_found());
        }
        let (source, source_size, source_mtime_ns) =
            validated_indexed_source(&active, &node).map_err(|()| image_not_found())?;
        let cache_key = ImageCacheKey::from_request(&ImageCacheKeyInput {
            project_id: active.project_id,
            relative_path: &node.relative_path,
            source_size,
            source_mtime_ns,
            kind,
            renderer_version: 1,
        });
        let cached = match cache.lookup_image(cache_key) {
            Some(cached) => cached,
            None => {
                let artifact = image
                    .render(ImageRequest {
                        session_id: active.session_id,
                        entity_id,
                        source,
                        kind,
                    })
                    .await
                    .map_err(CommandError::from)?;
                let cached = CachedImage {
                    path: artifact.cache_path,
                    mime: artifact.mime.to_owned(),
                    width: artifact.width,
                    height: artifact.height,
                    backend: artifact.backend,
                };
                cache
                    .insert_image(cache_key, cached.clone())
                    .map_err(CommandError::from)?;
                cached
            }
        };
        let token = self
            .register_if_session_active(&active, entity_id, &cached)
            .await?;
        Ok(ImageRepresentationDto {
            cache_key: cache_key.to_hex(),
            url: format!(
                "viewer-image://localhost/{}/{}",
                active.session_id,
                token.as_str()
            ),
            width: cached.width,
            height: cached.height,
            backend: cached.backend.into(),
        })
    }

    pub async fn preview_text(
        &self,
        entity_id: EntityId,
        encoding: Option<TextEncoding>,
    ) -> Result<TextPreviewDto, CommandError> {
        let (active, index, text_reader) = {
            let session = self.session.lock().await;
            let session = session.as_ref().ok_or_else(project_not_open)?;
            (
                session.active.clone(),
                Arc::clone(&session.index),
                Arc::clone(&self.text_reader),
            )
        };
        let node = BrowseIndexPort::node(index.as_ref(), entity_id)
            .map_err(CommandError::from)?
            .ok_or_else(text_not_found)?;
        if !matches!(node.kind, FileKind::Markdown | FileKind::Text) {
            return Err(text_not_found());
        }
        let (source, _, _) =
            validated_indexed_source(&active, &node).map_err(|()| text_not_found())?;
        let preview = tokio::task::spawn_blocking(move || text_reader.read(&source, encoding))
            .await
            .map_err(|_| internal_command_error())?
            .map_err(CommandError::from)?;
        self.ensure_session_active(active.session_id).await?;

        if node.kind == FileKind::Text {
            return Ok(TextPreviewDto {
                entity_id: entity_id.to_string(),
                format: TextPreviewFormatDto::PlainText,
                plain_text: Some(preview.text),
                markdown_html: None,
                encoding: preview.encoding,
                truncated: preview.truncated,
            });
        }

        let mut images = HashMap::new();
        for destination in image_destinations(&preview.text) {
            let Some(relative_path) =
                resolve_markdown_image_path(&node.relative_path, &destination)
            else {
                continue;
            };
            let Some(image_node) =
                BrowseIndexPort::node_by_relative_path(index.as_ref(), &relative_path)
                    .map_err(CommandError::from)?
            else {
                continue;
            };
            if !matches!(image_node.kind, FileKind::Jpeg | FileKind::Png) {
                continue;
            }
            let representation = self
                .request_image_for_session(
                    Some(active.session_id),
                    image_node.entity_id,
                    ImageRepresentationKind::FitPreview {
                        max_width: 1_600,
                        max_height: 1_600,
                        scale_milli: 1_000,
                    },
                )
                .await;
            if let Ok(representation) = representation {
                images.insert(destination, representation.url);
            }
        }

        self.ensure_session_active(active.session_id).await?;
        Ok(TextPreviewDto {
            entity_id: entity_id.to_string(),
            format: TextPreviewFormatDto::Markdown,
            plain_text: None,
            markdown_html: Some(render_safe_markdown(&preview.text, &images)),
            encoding: preview.encoding,
            truncated: preview.truncated,
        })
    }

    pub async fn open_external_link(&self, value: &str) -> Result<(), CommandError> {
        let url = ExternalUrl::parse(value).map_err(|_| {
            CommandError::new(
                "invalid_external_url",
                ErrorCategory::Validation,
                "仅允许打开 HTTP 或 HTTPS 链接。",
                false,
            )
        })?;
        let value = url.as_str().to_owned();
        tokio::task::spawn_blocking(move || viewer_platform_macos::open_external_url(&value))
            .await
            .map_err(|_| internal_command_error())?
            .map_err(|_| {
                CommandError::new(
                    "external_url_unavailable",
                    ErrorCategory::Environment,
                    "无法使用系统浏览器打开该链接。",
                    true,
                )
            })
    }

    async fn active_index(&self) -> Result<Arc<SessionIndex>, CommandError> {
        self.session
            .lock()
            .await
            .as_ref()
            .map(|session| Arc::clone(&session.index))
            .ok_or_else(project_not_open)
    }

    async fn ensure_session_active(&self, expected: SessionId) -> Result<(), CommandError> {
        let session = self.session.lock().await;
        if session
            .as_ref()
            .is_some_and(|session| session.active.session_id == expected)
        {
            Ok(())
        } else {
            Err(CommandError::new(
                "text_preview_cancelled",
                ErrorCategory::Conflict,
                "文本预览已取消。",
                true,
            ))
        }
    }

    async fn register_if_session_active(
        &self,
        active: &ActiveProject,
        entity_id: EntityId,
        cached: &CachedImage,
    ) -> Result<viewer_infrastructure::image_cache::ImageArtifactToken, CommandError> {
        let session = self.session.lock().await;
        if session
            .as_ref()
            .is_none_or(|session| session.active.session_id != active.session_id)
            || self.active_image_session.get() != Some(active.session_id)
        {
            return Err(CommandError::from(ImageError::Cancelled));
        }
        self.image_registry
            .insert(
                active.session_id,
                entity_id,
                &cached.path,
                cached.mime.clone(),
            )
            .map_err(CommandError::from)
    }

    pub async fn resources_ready(&self) -> bool {
        let session = self.session.lock().await;
        let Some(session) = session.as_ref() else {
            return false;
        };
        session.active.root.is_dir()
            && session.cache.root().is_dir()
            && session.index.directory_children(None).is_ok()
    }
}

fn project_not_open() -> CommandError {
    CommandError::new(
        "project_not_open",
        crate::error::ErrorCategory::Conflict,
        "请先打开一个项目。",
        false,
    )
}

fn image_not_found() -> CommandError {
    CommandError::new(
        "image_not_found",
        crate::error::ErrorCategory::Content,
        "该图片已不可用，请刷新项目后重试。",
        true,
    )
}

fn text_not_found() -> CommandError {
    CommandError::new(
        "text_not_found",
        ErrorCategory::Content,
        "该文本文件已不可用，请刷新项目后重试。",
        true,
    )
}

fn internal_command_error() -> CommandError {
    CommandError::new(
        "internal_error",
        ErrorCategory::Internal,
        "Viewer 遇到内部错误，请重试。",
        true,
    )
}

fn validated_indexed_source(
    active: &ActiveProject,
    node: &viewer_domain::file::FileNode,
) -> Result<(PathBuf, u64, i128), ()> {
    let candidate = active.root.join(node.relative_path.as_str());
    let metadata = std::fs::symlink_metadata(&candidate).map_err(|_| ())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || is_macos_alias(&candidate) {
        return Err(());
    }
    if entity_id_for_metadata(&metadata, &node.relative_path) != node.entity_id {
        return Err(());
    }
    let parent = candidate.parent().ok_or(())?;
    let canonical_parent = std::fs::canonicalize(parent).map_err(|_| ())?;
    let canonical_source = std::fs::canonicalize(&candidate).map_err(|_| ())?;
    if !canonical_parent.starts_with(&active.root) || !canonical_source.starts_with(&active.root) {
        return Err(());
    }
    Ok((canonical_source, metadata.len(), modified_ns(&metadata)))
}

fn resolve_markdown_image_path(
    markdown_path: &RelativePath,
    destination: &str,
) -> Option<RelativePath> {
    if destination.is_empty()
        || destination.starts_with('/')
        || destination.contains("//")
        || destination
            .chars()
            .any(|character| matches!(character, '\0' | '\\' | '%' | '?' | '#' | ':'))
    {
        return None;
    }
    let extension = destination.rsplit_once('.')?.1;
    if !matches!(
        extension.to_ascii_lowercase().as_str(),
        "jpg" | "jpeg" | "png"
    ) {
        return None;
    }

    let mut segments = markdown_path.as_str().split('/').collect::<Vec<_>>();
    segments.pop()?;
    for segment in destination.split('/') {
        match segment {
            "" => return None,
            "." => {}
            ".." => {
                segments.pop()?;
            }
            _ if segment.eq_ignore_ascii_case(".viewer") => return None,
            _ => segments.push(segment),
        }
    }
    RelativePath::parse(&segments.join("/")).ok()
}

#[cfg(unix)]
fn entity_id_for_metadata(
    metadata: &std::fs::Metadata,
    _relative_path: &viewer_domain::RelativePath,
) -> EntityId {
    use std::os::unix::fs::MetadataExt;
    EntityId::from_u128((u128::from(metadata.dev()) << 64) | u128::from(metadata.ino()))
}

#[cfg(not(unix))]
fn entity_id_for_metadata(
    metadata: &std::fs::Metadata,
    relative_path: &viewer_domain::RelativePath,
) -> EntityId {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    relative_path.as_str().hash(&mut hasher);
    metadata.len().hash(&mut hasher);
    EntityId::from_u128(u128::from(hasher.finish()))
}

#[cfg(unix)]
fn modified_ns(metadata: &std::fs::Metadata) -> i128 {
    use std::os::unix::fs::MetadataExt;
    i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec())
}

#[cfg(not(unix))]
fn modified_ns(metadata: &std::fs::Metadata) -> i128 {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos() as i128)
}

async fn run_scan(
    active: ActiveProject,
    task_id: TaskId,
    scanner: Arc<dyn ScanPort>,
    coordinator: Arc<TaskCoordinator>,
    index: Arc<SessionIndex>,
    events: Arc<dyn DesktopEventSink>,
) -> Result<(), CommandError> {
    let request = ScanRequest {
        session_id: active.session_id,
        generation: active.generation,
        root: active.root.clone(),
    };
    let coordinated = CoordinatedScan::new(scanner, coordinator);
    let (sink, mut incoming) =
        tokio::sync::mpsc::channel(TaskClass::FolderPublication.queue_capacity());
    let worker = tokio::spawn(async move { coordinated.scan(request, sink).await });
    let mut deferred = None;
    let mut last_progress = None;

    while let Some(mut event) = match deferred.take() {
        Some(event) => Some(event),
        None => incoming.recv().await,
    } {
        commit_scan_event(&index, &event)?;

        if matches!(event, ScanEvent::Folders { .. } | ScanEvent::Files { .. }) {
            if let Some(last) = last_progress {
                let deadline = last + Duration::from_millis(50);
                let delay = tokio::time::sleep_until(deadline);
                tokio::pin!(delay);
                loop {
                    tokio::select! {
                        () = &mut delay => break,
                        next = incoming.recv() => {
                            let Some(next) = next else {
                                (&mut delay).await;
                                break;
                            };
                            if same_node_event_kind(&event, &next) {
                                commit_scan_event(&index, &next)?;
                                merge_node_event(&mut event, next);
                            } else {
                                deferred = Some(next);
                                (&mut delay).await;
                                break;
                            }
                        }
                    }
                }
            }
            last_progress = Some(Instant::now());
        }
        events.emit_scan(ScanEventDto::from_event(&active, task_id, &event));
    }

    match worker.await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(viewer_application::scan::ScanError::Cancelled)) => Ok(()),
        Ok(Err(_)) => Err(CommandError::new(
            "project_scan_failed",
            crate::error::ErrorCategory::Environment,
            "无法完成项目扫描，请检查文件夹后重试。",
            true,
        )),
        Err(_) => Err(CommandError::new(
            "scan_worker_failed",
            crate::error::ErrorCategory::Internal,
            "项目扫描意外终止，请重新打开项目。",
            true,
        )),
    }
}

fn commit_scan_event(index: &SessionIndex, event: &ScanEvent) -> Result<(), CommandError> {
    if let ScanEvent::Folders { nodes, .. } | ScanEvent::Files { nodes, .. } = event {
        index.upsert_batch(nodes).map_err(CommandError::from)?;
    }
    Ok(())
}

fn same_node_event_kind(left: &ScanEvent, right: &ScanEvent) -> bool {
    matches!(
        (left, right),
        (ScanEvent::Folders { .. }, ScanEvent::Folders { .. })
            | (ScanEvent::Files { .. }, ScanEvent::Files { .. })
    )
}

fn merge_node_event(target: &mut ScanEvent, source: ScanEvent) {
    match (target, source) {
        (ScanEvent::Folders { nodes: target, .. }, ScanEvent::Folders { nodes: source, .. })
        | (ScanEvent::Files { nodes: target, .. }, ScanEvent::Files { nodes: source, .. }) => {
            target.extend(source)
        }
        _ => unreachable!("scan event kinds were checked before merging"),
    }
}
