use super::*;
use std::collections::{VecDeque, hash_map::Entry};
use tokio_util::sync::CancellationToken;
use viewer_domain::video::VideoProbeStatus;
use viewer_infrastructure::video_probe::{MediaFileIdentity, VideoProbeError};

pub(super) const IMAGE_REQUEST_TERMINAL_LIMIT: usize = 1_024;

#[derive(Debug)]
enum ImageRequestLifecycleState {
    PreCancelled,
    Active(ImageRequestCancellation),
    Completed,
}

#[derive(Debug, Default)]
pub(super) struct ImageRequestLifecycles {
    entries: HashMap<ImageRequestId, ImageRequestLifecycleState>,
    terminal_order: VecDeque<ImageRequestId>,
}

impl ImageRequestLifecycles {
    fn cancel(&mut self, request_id: ImageRequestId) -> bool {
        match self.entries.entry(request_id) {
            Entry::Vacant(entry) => {
                entry.insert(ImageRequestLifecycleState::PreCancelled);
                self.record_terminal(request_id);
                true
            }
            Entry::Occupied(entry) => match entry.get() {
                ImageRequestLifecycleState::PreCancelled => true,
                ImageRequestLifecycleState::Active(cancellation) => {
                    cancellation.cancel();
                    true
                }
                ImageRequestLifecycleState::Completed => false,
            },
        }
    }

    fn complete_active(&mut self, request_id: ImageRequestId) {
        let Some(state) = self.entries.get_mut(&request_id) else {
            return;
        };
        if matches!(state, ImageRequestLifecycleState::Active(_)) {
            *state = ImageRequestLifecycleState::Completed;
            self.record_terminal(request_id);
        }
    }

    fn record_terminal(&mut self, request_id: ImageRequestId) {
        self.terminal_order.push_back(request_id);
        while self.terminal_order.len() > IMAGE_REQUEST_TERMINAL_LIMIT {
            let Some(expired) = self.terminal_order.pop_front() else {
                break;
            };
            if self.entries.get(&expired).is_some_and(|state| {
                matches!(
                    state,
                    ImageRequestLifecycleState::PreCancelled
                        | ImageRequestLifecycleState::Completed
                )
            }) {
                self.entries.remove(&expired);
            }
        }
    }
}

#[derive(Debug)]
struct ImageRequestLease {
    requests: Arc<StdMutex<ImageRequestLifecycles>>,
    request_id: ImageRequestId,
    cancellation: ImageRequestCancellation,
    completed: bool,
}

impl ImageRequestLease {
    fn start(
        requests: Arc<StdMutex<ImageRequestLifecycles>>,
        request_id: ImageRequestId,
    ) -> Result<Self, CommandError> {
        let cancellation = ImageRequestCancellation::new();
        {
            let mut requests_guard = requests
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            match requests_guard.entries.entry(request_id) {
                Entry::Vacant(entry) => {
                    entry.insert(ImageRequestLifecycleState::Active(cancellation.clone()));
                }
                Entry::Occupied(mut entry) => match entry.get() {
                    ImageRequestLifecycleState::PreCancelled => {
                        entry.insert(ImageRequestLifecycleState::Completed);
                        return Err(CommandError::from(ImageError::Cancelled));
                    }
                    ImageRequestLifecycleState::Active(_)
                    | ImageRequestLifecycleState::Completed => {
                        return Err(duplicate_image_request());
                    }
                },
            }
        }
        Ok(Self {
            requests,
            request_id,
            cancellation,
            completed: false,
        })
    }

    fn cancellation(&self) -> &ImageRequestCancellation {
        &self.cancellation
    }

    fn publish<T>(
        &mut self,
        publication: impl FnOnce() -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        let mut requests = self
            .requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let active = matches!(
            requests.entries.get(&self.request_id),
            Some(ImageRequestLifecycleState::Active(cancellation))
                if !cancellation.is_cancelled()
        );
        if !active {
            return Err(CommandError::from(ImageError::Cancelled));
        }
        let published = publication()?;
        requests.complete_active(self.request_id);
        self.completed = true;
        Ok(published)
    }
}

impl Drop for ImageRequestLease {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        self.cancellation.cancel();
        self.requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .complete_active(self.request_id);
    }
}

impl DesktopRuntime {
    pub async fn resolve_video_entity(
        &self,
        entity_id: EntityId,
    ) -> Result<AuthorizedVideoSource, RuntimeError> {
        self.resolve_video_entity_for_open(entity_id, CancellationToken::new())
            .await
    }

    pub async fn resolve_video_entity_for_open(
        &self,
        entity_id: EntityId,
        cancellation: CancellationToken,
    ) -> Result<AuthorizedVideoSource, RuntimeError> {
        let (active, index, video_probe, scheduler) = {
            let session = self.session.lock().await;
            ensure_video_open_active(&cancellation)?;
            let session = session.as_ref().ok_or(RuntimeError::StaleSession)?;
            (
                session.active.clone(),
                Arc::clone(&session.index),
                Arc::clone(&self.video_probe),
                Arc::clone(&self.derived_scheduler),
            )
        };
        let indexed = index
            .indexed_node(entity_id)
            .map_err(|_| RuntimeError::StaleSession)?
            .ok_or(RuntimeError::StaleSession)?;
        if indexed.node.kind != FileKind::Video {
            return Err(RuntimeError::NotVideo);
        }
        let mut metadata = indexed
            .video_metadata
            .ok_or(RuntimeError::MetadataUnavailable)?;
        let (canonical_path, _, _) = validated_indexed_source(&active, &indexed.node)
            .map_err(|()| RuntimeError::PathNotAuthorized)?;
        let mut metadata_refreshed_for_retry = false;
        if matches!(metadata.probe_status, VideoProbeStatus::Failed(_)) {
            let source_metadata = std::fs::symlink_metadata(&canonical_path)
                .map_err(|_| RuntimeError::PathNotAuthorized)?;
            let expected_identity = MediaFileIdentity::from_metadata(&source_metadata);
            let _permit = scheduler.acquire(DerivedWorkClass::VisibleDerived).await;
            ensure_video_open_active(&cancellation)?;
            metadata = video_probe
                .probe_identity_bound(&canonical_path, &expected_identity, cancellation.clone())
                .await
                .map_err(map_video_retry_error)?;
            ensure_video_open_active(&cancellation)?;
            if metadata.display_width.is_none_or(|width| width == 0)
                || metadata.display_height.is_none_or(|height| height == 0)
            {
                return Err(RuntimeError::VideoRetryFailed(VideoFailureKind::Damaged));
            }
            metadata_refreshed_for_retry = true;
        }
        ensure_video_open_active(&cancellation)?;
        Ok(AuthorizedVideoSource {
            entity_id,
            session_id: active.session_id,
            canonical_path,
            metadata,
            metadata_refreshed_for_retry,
        })
    }

    pub async fn folder_tree(&self) -> Result<Vec<FolderTreeItemDto>, CommandError> {
        let _permit = self
            .derived_scheduler
            .acquire(DerivedWorkClass::CurrentQuery)
            .await;
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
        let _permit = self
            .derived_scheduler
            .acquire(DerivedWorkClass::CurrentQuery)
            .await;
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
        self.request_image_with_id(ImageRequestId::new(), entity_id, kind)
            .await
    }

    pub async fn request_image_with_id(
        &self,
        request_id: ImageRequestId,
        entity_id: EntityId,
        kind: ImageRepresentationKind,
    ) -> Result<ImageRepresentationDto, CommandError> {
        self.request_image_for_session(None, request_id, entity_id, kind)
            .await
    }

    pub async fn cancel_image_request(&self, request_id: ImageRequestId) -> bool {
        self.image_requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .cancel(request_id)
    }

    async fn request_image_for_session(
        &self,
        expected_session: Option<SessionId>,
        request_id: ImageRequestId,
        entity_id: EntityId,
        kind: ImageRepresentationKind,
    ) -> Result<ImageRepresentationDto, CommandError> {
        let mut lease = ImageRequestLease::start(Arc::clone(&self.image_requests), request_id)?;
        self.render_image_for_session(expected_session, request_id, &mut lease, entity_id, kind)
            .await
    }

    async fn render_image_for_session(
        &self,
        expected_session: Option<SessionId>,
        request_id: ImageRequestId,
        lease: &mut ImageRequestLease,
        entity_id: EntityId,
        kind: ImageRepresentationKind,
    ) -> Result<ImageRepresentationDto, CommandError> {
        ensure_image_request_active(lease.cancellation())?;
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
        let (cached, rendered) = match cache.lookup_image(cache_key) {
            Some(cached) => (cached, false),
            None => {
                ensure_image_request_active(lease.cancellation())?;
                let artifact = render_visible_image(
                    &self.derived_scheduler,
                    image.as_ref(),
                    ImageRequest {
                        request_id,
                        cancellation: lease.cancellation().clone(),
                        session_id: active.session_id,
                        entity_id,
                        source,
                        kind,
                    },
                )
                .await
                .map_err(CommandError::from)?;
                if lease.cancellation().is_cancelled() {
                    let _ = cache.discard_owned_image_artifact(&artifact.cache_path);
                    return Err(CommandError::from(ImageError::Cancelled));
                }
                let cached = CachedImage {
                    path: artifact.cache_path,
                    mime: artifact.mime.to_owned(),
                    width: artifact.width,
                    height: artifact.height,
                    backend: artifact.backend,
                };
                (cached, true)
            }
        };
        let publication = self
            .publish_image_request(
                &active, entity_id, cache_key, &cache, &cached, rendered, lease,
            )
            .await;
        if publication.is_err() && rendered {
            let _ = cache.discard_owned_image_artifact(&cached.path);
        }
        publication
    }

    #[allow(clippy::too_many_arguments)]
    async fn publish_image_request(
        &self,
        active: &ActiveProject,
        entity_id: EntityId,
        cache_key: ImageCacheKey,
        cache: &SessionCache,
        cached: &CachedImage,
        rendered: bool,
        lease: &mut ImageRequestLease,
    ) -> Result<ImageRepresentationDto, CommandError> {
        let session = self.session.lock().await;
        if session
            .as_ref()
            .is_none_or(|session| session.active.session_id != active.session_id)
            || self.active_image_session.get() != Some(active.session_id)
        {
            return Err(CommandError::from(ImageError::Cancelled));
        }
        lease.publish(|| {
            if rendered {
                cache
                    .insert_image(cache_key, cached.clone())
                    .map_err(CommandError::from)?;
            }
            let token = self
                .image_registry
                .insert(
                    active.session_id,
                    entity_id,
                    &cached.path,
                    cached.mime.clone(),
                )
                .map_err(CommandError::from)?;
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
                    ImageRequestId::new(),
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
}

fn map_video_retry_error(error: VideoProbeError) -> RuntimeError {
    match error {
        VideoProbeError::Failed(kind) => RuntimeError::VideoRetryFailed(kind),
        VideoProbeError::Cancelled | VideoProbeError::SourceChanged => RuntimeError::StaleSession,
    }
}

fn ensure_video_open_active(cancellation: &CancellationToken) -> Result<(), RuntimeError> {
    if cancellation.is_cancelled() {
        Err(RuntimeError::StaleSession)
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedVideoSource {
    pub entity_id: EntityId,
    pub session_id: SessionId,
    pub canonical_path: PathBuf,
    pub metadata: viewer_domain::video::VideoMetadata,
    pub metadata_refreshed_for_retry: bool,
}

fn ensure_image_request_active(
    cancellation: &ImageRequestCancellation,
) -> Result<(), CommandError> {
    if cancellation.is_cancelled() {
        Err(CommandError::from(ImageError::Cancelled))
    } else {
        Ok(())
    }
}

fn duplicate_image_request() -> CommandError {
    CommandError::new(
        "duplicate_image_request",
        ErrorCategory::Conflict,
        "图片预览请求重复，请重试。",
        true,
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

pub(super) fn validated_indexed_source(
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

async fn render_visible_image(
    scheduler: &Arc<DerivedWorkScheduler>,
    image: &dyn ImagePort,
    request: ImageRequest,
) -> Result<viewer_application::ImageArtifact, ImageError> {
    let _permit = scheduler.acquire(DerivedWorkClass::VisibleDerived).await;
    image.render(request).await
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

#[cfg(test)]
mod scheduler_tests {
    use super::*;
    use viewer_application::{ImageArtifact, ImageBackend};

    struct BlockingRenderer {
        started: tokio::sync::Notify,
        release: tokio::sync::Notify,
    }

    #[async_trait::async_trait]
    impl ImagePort for BlockingRenderer {
        async fn probe(
            &self,
            _source: &Path,
        ) -> Result<viewer_domain::image::ImageProbe, ImageError> {
            Err(ImageError::Unsupported)
        }

        async fn render(&self, request: ImageRequest) -> Result<ImageArtifact, ImageError> {
            self.started.notify_one();
            self.release.notified().await;
            Ok(ImageArtifact {
                cache_path: request.source,
                mime: "image/jpeg",
                width: 1,
                height: 1,
                backend: ImageBackend::ImageIo,
            })
        }

        async fn cancel_session(&self, _session_id: SessionId) {}
    }

    #[tokio::test]
    async fn real_visible_render_blocks_video_probe_until_render_finishes() {
        let scheduler = Arc::new(DerivedWorkScheduler::default());
        let renderer = Arc::new(BlockingRenderer {
            started: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        });
        let source = tempfile::NamedTempFile::new().unwrap();
        let render = tokio::spawn({
            let scheduler = Arc::clone(&scheduler);
            let renderer = Arc::clone(&renderer);
            let source = source.path().to_path_buf();
            async move {
                render_visible_image(
                    &scheduler,
                    renderer.as_ref(),
                    ImageRequest {
                        request_id: ImageRequestId::new(),
                        cancellation: ImageRequestCancellation::new(),
                        session_id: SessionId::new(),
                        entity_id: EntityId::new(),
                        source,
                        kind: ImageRepresentationKind::Original100Percent,
                    },
                )
                .await
            }
        });
        renderer.started.notified().await;

        let video = scheduler.acquire(DerivedWorkClass::VideoProbe);
        assert!(
            tokio::time::timeout(Duration::from_millis(50), video)
                .await
                .is_err()
        );
        renderer.release.notify_one();
        render.await.unwrap().unwrap();
        let permit = tokio::time::timeout(
            Duration::from_secs(1),
            scheduler.acquire(DerivedWorkClass::VideoProbe),
        )
        .await
        .unwrap();
        drop(permit);
    }
}

#[cfg(test)]
mod image_request_lifecycle_tests {
    use super::{IMAGE_REQUEST_TERMINAL_LIMIT, ImageRequestLease, ImageRequestLifecycles};
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    };
    use viewer_domain::ImageRequestId;

    #[test]
    fn publication_and_cancellation_have_one_linearized_winner() {
        let requests = Arc::new(Mutex::new(ImageRequestLifecycles::default()));
        let request_id = ImageRequestId::new();
        let mut lease = ImageRequestLease::start(Arc::clone(&requests), request_id).unwrap();
        let published = Arc::new(AtomicBool::new(false));
        let (publication_started, observe_publication) = mpsc::channel();
        let (release_publication, continue_publication) = mpsc::channel();
        let published_in_thread = Arc::clone(&published);
        let publication = std::thread::spawn(move || {
            lease.publish(|| {
                publication_started.send(()).unwrap();
                continue_publication.recv().unwrap();
                published_in_thread.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        observe_publication.recv().unwrap();
        let requests_for_cancel = Arc::clone(&requests);
        let (cancel_started, observe_cancel) = mpsc::channel();
        let cancellation = std::thread::spawn(move || {
            cancel_started.send(()).unwrap();
            requests_for_cancel.lock().unwrap().cancel(request_id)
        });
        observe_cancel.recv().unwrap();

        release_publication.send(()).unwrap();

        publication.join().unwrap().unwrap();
        assert!(published.load(Ordering::SeqCst));
        assert!(!cancellation.join().unwrap());

        let request_id = ImageRequestId::new();
        let mut lease = ImageRequestLease::start(Arc::clone(&requests), request_id).unwrap();
        assert!(requests.lock().unwrap().cancel(request_id));
        let published = AtomicBool::new(false);
        let result = lease.publish(|| {
            published.store(true, Ordering::SeqCst);
            Ok(())
        });
        assert_eq!(result.unwrap_err().code, "image_request_cancelled");
        assert!(!published.load(Ordering::SeqCst));
    }

    #[test]
    fn terminal_request_history_is_bounded_and_evicts_the_oldest_tombstone() {
        let requests = Arc::new(Mutex::new(ImageRequestLifecycles::default()));
        let ids = (0..=IMAGE_REQUEST_TERMINAL_LIMIT)
            .map(|_| ImageRequestId::new())
            .collect::<Vec<_>>();
        for request_id in &ids {
            assert!(requests.lock().unwrap().cancel(*request_id));
        }

        let oldest = ImageRequestLease::start(Arc::clone(&requests), ids[0]);
        assert!(
            oldest.is_ok(),
            "oldest terminal tombstone should be evicted"
        );
        let newest = ImageRequestLease::start(
            Arc::clone(&requests),
            *ids.last().expect("at least one request id"),
        )
        .unwrap_err();
        assert_eq!(newest.code, "image_request_cancelled");
    }
}
