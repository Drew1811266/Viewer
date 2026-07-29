use super::preview::validated_indexed_source;
use super::*;

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
    let result = run_derived_indexing(active, coordinator, index, image, events).await;
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
) -> Result<(), CommandError> {
    if !coordinator.is_publishable(active.session_id, active.generation) {
        return Ok(());
    }
    let nodes = BrowseIndexPort::descendants(index.as_ref(), None).map_err(CommandError::from)?;
    rebuild_derived_nodes(active, coordinator, index, image, events, nodes).await
}

pub(crate) async fn rebuild_derived_nodes(
    active: ActiveProject,
    coordinator: Arc<TaskCoordinator>,
    index: Arc<SessionIndex>,
    image: Arc<dyn ImagePort>,
    events: Arc<dyn DesktopEventSink>,
    nodes: Vec<FileNode>,
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
        let source = validated_indexed_source(&active, &node)
            .map(|(source, _, _)| source)
            .ok();
        match node.kind {
            FileKind::Jpeg | FileKind::Png => {
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
            }
            FileKind::Markdown | FileKind::Text => match source {
                Some(source) => {
                    let extracted =
                        tokio::task::spawn_blocking(move || TextExtractor::extract(source)).await;
                    match extracted {
                        Ok(Ok(status)) => {
                            if let Err(error) =
                                index.replace_text(node.entity_id, &node.relative_path, &status)
                            {
                                if is_stale_derived_write_error(&error) {
                                    continue;
                                }
                                return Err(CommandError::from(error));
                            }
                        }
                        Ok(Err(_)) | Err(_) => {
                            if let Err(error) =
                                index.mark_text_failed(node.entity_id, &node.relative_path)
                            {
                                if is_stale_derived_write_error(&error) {
                                    continue;
                                }
                                return Err(CommandError::from(error));
                            }
                        }
                    }
                }
                None => {
                    if let Err(error) = index.mark_text_failed(node.entity_id, &node.relative_path)
                    {
                        if is_stale_derived_write_error(&error) {
                            continue;
                        }
                        return Err(CommandError::from(error));
                    }
                }
            },
            FileKind::Directory | FileKind::UnsupportedImage | FileKind::Other => {
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
            .upsert_batch(&[unsupported.clone(), other.clone()])
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
