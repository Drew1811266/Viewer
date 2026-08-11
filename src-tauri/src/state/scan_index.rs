use super::preview::validated_indexed_source;
use super::*;

#[async_trait::async_trait]
trait TextIndexWork: Send + Sync {
    async fn extract(
        &self,
        source: PathBuf,
    ) -> Result<viewer_infrastructure::search::text::TextStatus, ()>;
}

struct LocalTextIndexWork;

#[async_trait::async_trait]
impl TextIndexWork for LocalTextIndexWork {
    async fn extract(
        &self,
        source: PathBuf,
    ) -> Result<viewer_infrastructure::search::text::TextStatus, ()> {
        tokio::task::spawn_blocking(move || TextExtractor::extract(source))
            .await
            .map_err(|_| ())?
            .map_err(|_| ())
    }
}

impl DesktopRuntime {
    pub async fn index_progress(&self) -> Result<IndexProgressDto, CommandError> {
        let session = self.session.lock().await;
        let session = session.as_ref().ok_or_else(project_not_open)?;
        let progress = session.index.index_progress().map_err(CommandError::from)?;
        Ok(IndexProgressDto::from_progress(&session.active, progress))
    }

    pub async fn search_project(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        revision: u64,
        query: SearchQuery,
    ) -> Result<SearchPageDto, CommandError> {
        if !query.is_valid() {
            return Err(CommandError::from(
                viewer_application::search::SearchError::InvalidQuery,
            ));
        }
        let (active, index, latest_revision) = {
            let session = self.session.lock().await;
            let session = session.as_ref().ok_or_else(project_not_open)?;
            validate_project_request(&session.active, expected_session, expected_generation)?;
            (
                session.active.clone(),
                Arc::clone(&session.index),
                Arc::clone(&session.search_revision),
            )
        };
        if let SearchScope::Subtree(folder) = query.scope {
            let indexed = index
                .indexed_node(folder)
                .map_err(CommandError::from)?
                .ok_or_else(selection_not_found)?;
            if indexed.node.kind != FileKind::Directory {
                return Err(selection_not_found());
            }
        }
        if !claim_search_revision(&latest_revision, revision) {
            return Err(stale_search_revision());
        }

        let search = SessionSearch::new(active.session_id, Arc::clone(&index));
        let page = search
            .search(active.session_id, active.generation, query)
            .await
            .map_err(CommandError::from)?;
        self.ensure_search_current(
            active.session_id,
            active.generation,
            revision,
            &latest_revision,
        )
        .await?;
        Ok(SearchPageDto::from_page(revision, page))
    }

    pub async fn search_text_snippet(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        revision: u64,
        entity_id: EntityId,
        query: String,
    ) -> Result<TextSnippetDto, CommandError> {
        let (active, index, latest_revision) = {
            let session = self.session.lock().await;
            let session = session.as_ref().ok_or_else(project_not_open)?;
            validate_project_request(&session.active, expected_session, expected_generation)?;
            (
                session.active.clone(),
                Arc::clone(&session.index),
                Arc::clone(&session.search_revision),
            )
        };
        if latest_revision.load(Ordering::Acquire) != revision {
            return Err(stale_search_revision());
        }
        let indexed = index
            .indexed_node(entity_id)
            .map_err(CommandError::from)?
            .ok_or_else(selection_not_found)?;
        if !matches!(indexed.node.kind, FileKind::Markdown | FileKind::Text) {
            return Err(selection_not_found());
        }

        let search = SessionSearch::new(active.session_id, Arc::clone(&index));
        let snippet = search
            .text_snippet(active.session_id, active.generation, entity_id, query)
            .await
            .map_err(CommandError::from)?;
        self.ensure_search_current(
            active.session_id,
            active.generation,
            revision,
            &latest_revision,
        )
        .await?;
        Ok(TextSnippetDto {
            revision,
            entity_id: entity_id.to_string(),
            snippet,
        })
    }

    async fn ensure_search_current(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        revision: u64,
        latest_revision: &AtomicU64,
    ) -> Result<(), CommandError> {
        self.ensure_project_current(expected_session, expected_generation)
            .await?;
        if latest_revision.load(Ordering::Acquire) == revision {
            Ok(())
        } else {
            Err(stale_search_revision())
        }
    }

    pub(super) async fn active_index(&self) -> Result<Arc<SessionIndex>, CommandError> {
        self.session
            .lock()
            .await
            .as_ref()
            .map(|session| Arc::clone(&session.index))
            .ok_or_else(project_not_open)
    }
}

pub(super) async fn run_scan(
    active: ActiveProject,
    task_id: TaskId,
    services: ScanServices,
) -> Result<(), CommandError> {
    let ScanServices {
        scanner,
        coordinator,
        index,
        image,
        events,
        portable_store,
        marker_lock,
        scan_ready,
        derived_scheduler,
        video_index,
    } = services;
    let request = ScanRequest {
        session_id: active.session_id,
        generation: active.generation,
        root: active.root.clone(),
    };
    let coordinated = CoordinatedScan::new(scanner, Arc::clone(&coordinator));
    let (sink, mut incoming) =
        tokio::sync::mpsc::channel(TaskClass::FolderPublication.queue_capacity());
    let worker = tokio::spawn(async move { coordinated.scan(request, sink).await });
    let mut deferred = None;
    let mut last_progress = None;

    while let Some(mut event) = match deferred.take() {
        Some(event) => Some(event),
        None => incoming.recv().await,
    } {
        let publication_permit = derived_scheduler
            .acquire(DerivedWorkClass::FolderPublication)
            .await;
        if !coordinator.is_publishable(active.session_id, active.generation) {
            return Ok(());
        }
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
        drop(publication_permit);
    }

    match worker.await {
        Ok(Ok(())) => {}
        Ok(Err(viewer_application::scan::ScanError::Cancelled)) => return Ok(()),
        Ok(Err(_)) => {
            return Err(CommandError::new(
                "project_scan_failed",
                crate::error::ErrorCategory::Environment,
                "无法完成项目扫描，请检查文件夹后重试。",
                true,
            ));
        }
        Err(_) => {
            return Err(CommandError::new(
                "scan_worker_failed",
                crate::error::ErrorCategory::Internal,
                "项目扫描意外终止，请重新打开项目。",
                true,
            ));
        }
    }
    if let Some(store) = portable_store {
        let _marker_guard = marker_lock.lock().await;
        hydrate_portable_markers(&index, store.as_ref())?;
    }
    let result = run_derived_indexing(
        active,
        coordinator,
        index,
        image,
        events,
        derived_scheduler,
        video_index,
    )
    .await;
    scan_ready.send_replace(true);
    result
}

fn hydrate_portable_markers(
    index: &SessionIndex,
    store: &dyn PortableMetadataPort,
) -> Result<(), CommandError> {
    let paths = BrowseIndexPort::descendants(index, None)
        .map_err(CommandError::from)?
        .into_iter()
        .map(|node| node.relative_path)
        .collect::<Vec<_>>();
    let mut markers = Vec::new();
    for chunk in paths.chunks(400) {
        markers.extend(store.markers_for_paths(chunk).map_err(|_| {
            CommandError::new(
                "portable_metadata_unavailable",
                ErrorCategory::Consistency,
                "无法读取项目审阅数据，请重新打开项目。",
                true,
            )
        })?);
    }
    index
        .hydrate_markers(&markers)
        .map_err(CommandError::from)?;
    Ok(())
}

async fn run_derived_indexing(
    active: ActiveProject,
    coordinator: Arc<TaskCoordinator>,
    index: Arc<SessionIndex>,
    image: Arc<dyn ImagePort>,
    events: Arc<dyn DesktopEventSink>,
    scheduler: Arc<DerivedWorkScheduler>,
    video_index: Arc<VideoIndexRuntime>,
) -> Result<(), CommandError> {
    if !coordinator.is_publishable(active.session_id, active.generation) {
        return Ok(());
    }
    let nodes = BrowseIndexPort::descendants(index.as_ref(), None).map_err(CommandError::from)?;
    rebuild_derived_nodes(
        active.clone(),
        Arc::clone(&coordinator),
        Arc::clone(&index),
        image,
        events,
        scheduler,
        nodes.clone(),
    )
    .await?;
    video_index.notify_after_publication()?;
    Ok(())
}

pub(crate) async fn rebuild_derived_nodes(
    active: ActiveProject,
    coordinator: Arc<TaskCoordinator>,
    index: Arc<SessionIndex>,
    image: Arc<dyn ImagePort>,
    events: Arc<dyn DesktopEventSink>,
    scheduler: Arc<DerivedWorkScheduler>,
    nodes: Vec<FileNode>,
) -> Result<(), CommandError> {
    rebuild_derived_nodes_with_text_work(
        active,
        coordinator,
        index,
        image,
        events,
        scheduler,
        nodes,
        Arc::new(LocalTextIndexWork),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn rebuild_derived_nodes_with_text_work(
    active: ActiveProject,
    coordinator: Arc<TaskCoordinator>,
    index: Arc<SessionIndex>,
    image: Arc<dyn ImagePort>,
    events: Arc<dyn DesktopEventSink>,
    scheduler: Arc<DerivedWorkScheduler>,
    nodes: Vec<FileNode>,
    text_work: Arc<dyn TextIndexWork>,
) -> Result<(), CommandError> {
    emit_index_progress_if_current(&active, &coordinator, &index, events.as_ref())?;
    let mut last_progress = Instant::now();

    for node in nodes
        .into_iter()
        .filter(|node| node.kind.is_previewable_image() || node.kind.is_previewable_text())
    {
        if !coordinator.is_publishable(active.session_id, active.generation) {
            return Ok(());
        }
        let current = match index.indexed_node(node.entity_id) {
            Ok(Some(current)) if current.node == node => current,
            Ok(_) => continue,
            Err(error) => return Err(CommandError::from(error)),
        };
        let pending = if node.kind.is_previewable_image() {
            current.image_status == ImageIndexStatus::Pending
        } else {
            current.text_status == TextIndexStatus::Pending
        };
        if !pending {
            continue;
        }
        match node.kind {
            FileKind::Jpeg | FileKind::Png => {
                let permit = scheduler.acquire(DerivedWorkClass::VisibleDerived).await;
                if !coordinator.is_publishable(active.session_id, active.generation) {
                    return Ok(());
                }
                let source = validated_indexed_source(&active, &node)
                    .map(|(source, _, _)| source)
                    .ok();
                let result = match source {
                    Some(source) => match image.probe(&source).await {
                        Ok(probe) => {
                            let (width, height) = if matches!(probe.orientation, 5..=8) {
                                (probe.height, probe.width)
                            } else {
                                (probe.width, probe.height)
                            };
                            Ok(ImageMetadata { width, height })
                        }
                        Err(_) => Err(ImageIndexStatus::Failed),
                    },
                    None => Err(ImageIndexStatus::Failed),
                };
                if let Err(error) =
                    index.replace_image_metadata(node.entity_id, &node.relative_path, result)
                {
                    if is_stale_derived_write_error(&error) {
                        continue;
                    }
                    return Err(CommandError::from(error));
                }
                drop(permit);
            }
            FileKind::Markdown | FileKind::Text => {
                let current = run_text_index_work(&scheduler, async {
                    if !coordinator.is_publishable(active.session_id, active.generation) {
                        return Ok(false);
                    }
                    let source = validated_indexed_source(&active, &node)
                        .map(|(source, _, _)| source)
                        .ok();
                    match source {
                        Some(source) => match text_work.extract(source).await {
                            Ok(status) => {
                                if let Err(error) =
                                    index.replace_text(node.entity_id, &node.relative_path, &status)
                                {
                                    if is_stale_derived_write_error(&error) {
                                        return Ok(true);
                                    }
                                    return Err(CommandError::from(error));
                                }
                            }
                            Err(()) => {
                                if let Err(error) =
                                    index.mark_text_failed(node.entity_id, &node.relative_path)
                                {
                                    if is_stale_derived_write_error(&error) {
                                        return Ok(true);
                                    }
                                    return Err(CommandError::from(error));
                                }
                            }
                        },
                        None => {
                            if let Err(error) =
                                index.mark_text_failed(node.entity_id, &node.relative_path)
                            {
                                if is_stale_derived_write_error(&error) {
                                    return Ok(true);
                                }
                                return Err(CommandError::from(error));
                            }
                        }
                    }
                    Ok(true)
                })
                .await?;
                if !current {
                    return Ok(());
                }
            }
            FileKind::Directory
            | FileKind::UnsupportedImage
            | FileKind::Other
            | FileKind::Video => {
                unreachable!("non-derived kinds were filtered out")
            }
        }
        if last_progress.elapsed() >= Duration::from_millis(50) {
            emit_index_progress_if_current(&active, &coordinator, &index, events.as_ref())?;
            last_progress = Instant::now();
        }
    }
    emit_index_progress_if_current(&active, &coordinator, &index, events.as_ref())
}

async fn run_text_index_work<T>(
    scheduler: &Arc<DerivedWorkScheduler>,
    work: impl std::future::Future<Output = T>,
) -> T {
    let _permit = scheduler.acquire(DerivedWorkClass::TextIndex).await;
    work.await
}

fn emit_index_progress_if_current(
    active: &ActiveProject,
    coordinator: &TaskCoordinator,
    index: &SessionIndex,
    events: &dyn DesktopEventSink,
) -> Result<(), CommandError> {
    if coordinator.is_publishable(active.session_id, active.generation) {
        let progress = index.index_progress().map_err(CommandError::from)?;
        events.emit_index(IndexProgressDto::from_progress(active, progress));
    }
    Ok(())
}

fn commit_scan_event(index: &SessionIndex, event: &ScanEvent) -> Result<(), CommandError> {
    if let ScanEvent::Folders { nodes, generation } | ScanEvent::Files { nodes, generation } = event
    {
        index
            .upsert_batch(nodes, *generation)
            .map_err(CommandError::from)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    use viewer_application::{ImageArtifact, ProjectAccess};
    use viewer_domain::{ProjectId, image::ImageProbe};

    #[derive(Default)]
    struct CountingImage {
        probes: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl ImagePort for CountingImage {
        async fn probe(&self, _source: &Path) -> Result<ImageProbe, ImageError> {
            self.probes.fetch_add(1, Ordering::Relaxed);
            Err(ImageError::Unsupported)
        }

        async fn render(&self, _request: ImageRequest) -> Result<ImageArtifact, ImageError> {
            Err(ImageError::Unsupported)
        }

        async fn cancel_session(&self, _session_id: SessionId) {}
    }

    struct NoopEvents;

    impl DesktopEventSink for NoopEvents {
        fn emit_scan(&self, _event: ScanEventDto) {}
    }

    struct BlockingImage {
        started: tokio::sync::Notify,
        release: tokio::sync::Notify,
    }

    struct BlockingTextIndexWork {
        started: tokio::sync::Notify,
        release: tokio::sync::Notify,
    }

    #[async_trait::async_trait]
    impl TextIndexWork for BlockingTextIndexWork {
        async fn extract(
            &self,
            _source: std::path::PathBuf,
        ) -> Result<viewer_infrastructure::search::text::TextStatus, ()> {
            self.started.notify_one();
            self.release.notified().await;
            Ok(viewer_infrastructure::search::text::TextStatus::Indexed(
                "persisted text".into(),
            ))
        }
    }

    #[async_trait::async_trait]
    impl ImagePort for BlockingImage {
        async fn probe(&self, _source: &Path) -> Result<ImageProbe, ImageError> {
            self.started.notify_one();
            self.release.notified().await;
            Err(ImageError::Unsupported)
        }

        async fn render(&self, _request: ImageRequest) -> Result<ImageArtifact, ImageError> {
            Err(ImageError::Unsupported)
        }

        async fn cancel_session(&self, _session_id: SessionId) {}
    }

    #[tokio::test]
    async fn image_rebuild_blocks_lower_priority_video_work_until_probe_finishes() {
        let project = tempfile::tempdir().unwrap();
        let source = project.path().join("source.jpg");
        fs::write(&source, b"image source").unwrap();
        let source_metadata = fs::symlink_metadata(&source).unwrap();
        let relative_path = RelativePath::parse("source.jpg").unwrap();
        let node = FileNode {
            entity_id: entity_id_for_metadata(&source_metadata, &relative_path),
            relative_path,
            kind: FileKind::Jpeg,
            size: source_metadata.len(),
            modified_ns: modified_ns(&source_metadata),
        };
        let index = Arc::new(SessionIndex::open(project.path().join("session.sqlite")).unwrap());
        let coordinator = Arc::new(TaskCoordinator::default());
        let session_id = SessionId::new();
        let generation = coordinator.begin_session(session_id);
        index
            .upsert_batch(std::slice::from_ref(&node), generation)
            .unwrap();
        let active = ActiveProject {
            project_id: ProjectId::new(),
            session_id,
            generation,
            root: project.path().canonicalize().unwrap(),
            display_name: "fixture".into(),
            access: ProjectAccess::ReadWrite,
        };
        let scheduler = Arc::new(DerivedWorkScheduler::default());
        let image = Arc::new(BlockingImage {
            started: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        });
        let rebuild = {
            let scheduler = Arc::clone(&scheduler);
            let image = Arc::clone(&image);
            tokio::spawn(async move {
                rebuild_derived_nodes(
                    active,
                    coordinator,
                    index,
                    image,
                    Arc::new(NoopEvents),
                    scheduler,
                    vec![node],
                )
                .await
            })
        };
        image.started.notified().await;

        let video = tokio::spawn({
            let scheduler = Arc::clone(&scheduler);
            async move { scheduler.acquire(DerivedWorkClass::VideoProbe).await }
        });
        assert!(
            tokio::time::timeout(Duration::from_millis(50), video)
                .await
                .is_err(),
            "video work must remain queued while a real image probe is running"
        );

        image.release.notify_one();
        rebuild.await.unwrap().unwrap();
        let permit = tokio::time::timeout(
            Duration::from_secs(1),
            scheduler.acquire(DerivedWorkClass::VideoProbe),
        )
        .await
        .expect("video work should start after image rebuild releases its permit");
        drop(permit);
    }

    #[tokio::test]
    async fn real_text_index_work_blocks_lower_priority_video_until_persistence_finishes() {
        let project = tempfile::tempdir().unwrap();
        let source = project.path().join("source.txt");
        fs::write(&source, b"actual text source").unwrap();
        let source_metadata = fs::symlink_metadata(&source).unwrap();
        let relative_path = RelativePath::parse("source.txt").unwrap();
        let node = FileNode {
            entity_id: entity_id_for_metadata(&source_metadata, &relative_path),
            relative_path,
            kind: FileKind::Text,
            size: source_metadata.len(),
            modified_ns: modified_ns(&source_metadata),
        };
        let index = Arc::new(SessionIndex::open(project.path().join("session.sqlite")).unwrap());
        let coordinator = Arc::new(TaskCoordinator::default());
        let session_id = SessionId::new();
        let generation = coordinator.begin_session(session_id);
        index
            .upsert_batch(std::slice::from_ref(&node), generation)
            .unwrap();
        let active = ActiveProject {
            project_id: ProjectId::new(),
            session_id,
            generation,
            root: project.path().canonicalize().unwrap(),
            display_name: "fixture".into(),
            access: ProjectAccess::ReadWrite,
        };
        let scheduler = Arc::new(DerivedWorkScheduler::default());
        let text = Arc::new(BlockingTextIndexWork {
            started: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        });
        let rebuild = tokio::spawn({
            let scheduler = Arc::clone(&scheduler);
            let text = text.clone();
            let index = Arc::clone(&index);
            async move {
                rebuild_derived_nodes_with_text_work(
                    active,
                    coordinator,
                    index,
                    Arc::new(CountingImage::default()),
                    Arc::new(NoopEvents),
                    scheduler,
                    vec![node],
                    text,
                )
                .await
            }
        });
        text.started.notified().await;
        assert_eq!(
            index
                .all_indexed_nodes()
                .unwrap()
                .into_iter()
                .next()
                .unwrap()
                .text_status,
            TextIndexStatus::Pending
        );

        let video = tokio::spawn({
            let scheduler = Arc::clone(&scheduler);
            async move { scheduler.acquire(DerivedWorkClass::VideoProbe).await }
        });
        assert!(
            tokio::time::timeout(Duration::from_millis(50), video)
                .await
                .is_err(),
            "video must remain queued while real text indexing/persistence is active"
        );

        text.release.notify_one();
        rebuild.await.unwrap().unwrap();
        assert_eq!(
            index
                .all_indexed_nodes()
                .unwrap()
                .into_iter()
                .next()
                .unwrap()
                .text_status,
            TextIndexStatus::Ready
        );
        let permit = tokio::time::timeout(
            Duration::from_secs(1),
            scheduler.acquire(DerivedWorkClass::VideoProbe),
        )
        .await
        .expect("video should start after text indexing releases its permit");
        drop(permit);
    }

    #[tokio::test]
    async fn non_previewable_kinds_skip_image_and_text_extraction() {
        let project = tempfile::tempdir().unwrap();
        fs::write(project.path().join("source.psd"), b"image source").unwrap();
        fs::write(project.path().join("license.pdf"), b"text source").unwrap();
        let index = Arc::new(SessionIndex::open(project.path().join("session.sqlite")).unwrap());
        let unsupported = FileNode {
            entity_id: EntityId::new(),
            relative_path: RelativePath::parse("source.psd").unwrap(),
            kind: FileKind::UnsupportedImage,
            size: 12,
            modified_ns: 1,
        };
        let other = FileNode {
            entity_id: EntityId::new(),
            relative_path: RelativePath::parse("license.pdf").unwrap(),
            kind: FileKind::Other,
            size: 11,
            modified_ns: 1,
        };
        index
            .upsert_batch(
                &[unsupported.clone(), other.clone()],
                viewer_domain::search::Generation::new(1),
            )
            .unwrap();
        let coordinator = Arc::new(TaskCoordinator::default());
        let session_id = SessionId::new();
        let generation = coordinator.begin_session(session_id);
        let active = ActiveProject {
            project_id: ProjectId::new(),
            session_id,
            generation,
            root: project.path().canonicalize().unwrap(),
            display_name: "fixture".into(),
            access: ProjectAccess::ReadWrite,
        };
        let image = Arc::new(CountingImage::default());

        rebuild_derived_nodes(
            active,
            coordinator,
            Arc::clone(&index),
            image.clone(),
            Arc::new(NoopEvents),
            Arc::new(DerivedWorkScheduler::default()),
            vec![unsupported.clone(), other.clone()],
        )
        .await
        .unwrap();

        assert_eq!(image.probes.load(Ordering::Relaxed), 0);
        assert_eq!(
            index
                .indexed_node(unsupported.entity_id)
                .unwrap()
                .unwrap()
                .image_status,
            ImageIndexStatus::Pending
        );
        assert_eq!(
            index
                .indexed_node(other.entity_id)
                .unwrap()
                .unwrap()
                .text_status,
            TextIndexStatus::Pending
        );
    }
}
