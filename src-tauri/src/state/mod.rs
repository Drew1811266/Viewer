mod common;
mod markers;
mod organization;
mod preview;
mod scan_index;
mod session;

use common::{
    claim_search_revision, image_not_found, invalid_rename_preview, operation_backend_unavailable,
    operation_runtime_error, project_read_only_operation, resolve_markdown_image_path,
    selection_not_found, text_not_found, validate_project_request, validated_indexed_source,
    validated_marker_target,
};
pub(crate) use common::{
    internal_command_error, is_stale_derived_write_error, project_not_open, stale_project_session,
};

use crate::{
    dto::{
        FolderTreeItemDto, FolderWorkspaceDto, ImageRepresentationDto, IndexProgressDto,
        MarkerBatchResultDto, ProjectSnapshot, RecoveryReportDto, SearchPageDto, SelectionInfoDto,
        TextPreviewDto, TextPreviewFormatDto, TextSnippetDto,
    },
    error::{CommandError, ErrorCategory, stale_search_revision},
    image_protocol::ActiveImageSession,
    markdown::{ExternalUrl, image_destinations, render_safe_markdown},
    operation_runtime::{
        DesktopOperationCommitPort, OperationRuntime, OperationRuntimeError, OperationStarted,
        adapter_as_undo_port,
    },
    watcher_runtime::{WatcherDerivedServices, WatcherRuntime},
};
use std::{
    collections::HashMap,
    path::Path,
    path::PathBuf,
    sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{sync::Mutex, task::JoinHandle, time::Instant};
use viewer_application::{
    ActiveProject, BrowseIndexPort, BrowseService, ClockPort, ImageError, ImagePort, ImageRequest,
    PreparedFinderDrag, ProjectAccess, ProjectOpenError, ProjectProbeError, ProjectProbePort,
    ProjectSessionService, ScanPort, SearchPort, SearchSnippetPort, TextEncoding, TextPreviewPort,
    VolumePort,
    file_commands::{
        BatchId, BatchProgress, BatchResultPage, ConflictResolution, FileCommand, FileCommandItem,
        FileCommandKind, FileCommandPreflight, FileCommandService,
    },
    metadata::{
        FavoritePatch, MarkerPatch, MarkerProjectionPort, MarkerService, MarkerTarget,
        PortableMetadataPort, ReviewPatch,
    },
    scan::{CoordinatedScan, ScanEvent, ScanRequest},
    scheduler::{TaskClass, TaskCoordinator},
    undo::{UndoFilePort, UndoReceipt, UndoService, UndoStack},
};
use viewer_domain::{
    EntityId, RelativePath, SessionId, TaskId,
    file::{FileKind, FileNode, ImageIndexStatus, ImageMetadata, ReviewState, TextIndexStatus},
    image::ImageRepresentationKind,
    operation::{RenamePreflight, RenameRuleSet, RenameTarget},
    search::{Generation, SearchQuery, SearchScope},
};
use viewer_infrastructure::operation::{
    copy::LocalFileMutation, journal::OperationJournal, recovery::RecoveryService,
    rename::RenamePlanner, service::LocalFileCommandAdapter,
};
use viewer_infrastructure::{
    SystemClock,
    image_cache::{ImageArtifactRegistry, ImageCacheKey, ImageCacheKeyInput},
    portable::{PortableMarkerStore, PortableProjectMetadata},
    scan::walker::{ProjectWalker, is_macos_alias},
    scan::{reconcile::ExpectedChangeLedger, reconcile_service::ProjectReconciler},
    search::{
        index::{SessionIndex, SessionIndexError},
        query::SessionSearch,
        text::TextExtractor,
    },
    session_cache::{CachedImage, SessionCache},
    text::preview::TextPreviewReader,
};
use viewer_platform_macos::{
    files::{MacTrashPort, MacVolumePort},
    watcher::MacWatcherPort,
};

pub use crate::dto::ScanEventDto;

pub trait DesktopEventSink: Send + Sync {
    fn emit_scan(&self, event: ScanEventDto);
    fn emit_index(&self, _event: IndexProgressDto) {}
    fn emit_operation(
        &self,
        _session_id: SessionId,
        _generation: Generation,
        _event: viewer_application::file_commands::BatchProgress,
    ) {
    }
    fn emit_project_changed(
        &self,
        _session_id: SessionId,
        _generation: Generation,
        _summary: viewer_application::watcher::ReconcileSummary,
    ) {
    }
    fn emit_close_blocked(
        &self,
        _session_id: SessionId,
        _generation: Generation,
        _batch_id: BatchId,
        _target: CloseTarget,
    ) {
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseChoice {
    Wait,
    CancelPending,
    Stay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseRequestOutcome {
    Closed,
    Stayed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseTarget {
    Project,
    Window,
    Application,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseCompletionAction {
    KeepOpen,
    ShowEmptyProject,
    HideWindow,
    ExitApplication,
}

impl CloseRequestOutcome {
    pub const fn completion_action(self, target: CloseTarget) -> CloseCompletionAction {
        match (self, target) {
            (Self::Stayed, _) => CloseCompletionAction::KeepOpen,
            (Self::Closed, CloseTarget::Project) => CloseCompletionAction::ShowEmptyProject,
            (Self::Closed, CloseTarget::Window) => CloseCompletionAction::HideWindow,
            (Self::Closed, CloseTarget::Application) => CloseCompletionAction::ExitApplication,
        }
    }
}

#[derive(Default)]
struct NoopEventSink;

impl DesktopEventSink for NoopEventSink {
    fn emit_scan(&self, _event: ScanEventDto) {}
}

pub trait DesktopImageFactory: Send + Sync {
    fn create(&self, cache_root: &Path) -> Result<Arc<dyn ImagePort>, ImageError>;
}

pub trait DesktopMarkerProjectionFactory: Send + Sync {
    fn create(&self, index: Arc<SessionIndex>) -> Arc<dyn MarkerProjectionPort>;
}

#[derive(Default)]
struct SessionMarkerProjectionFactory;

impl DesktopMarkerProjectionFactory for SessionMarkerProjectionFactory {
    fn create(&self, index: Arc<SessionIndex>) -> Arc<dyn MarkerProjectionPort> {
        index
    }
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
    portable_store: Option<Arc<PortableMarkerStore>>,
    marker_projection: Arc<dyn MarkerProjectionPort>,
    marker_lock: Arc<Mutex<()>>,
    undo_stack: Arc<StdMutex<UndoStack>>,
    file_undo_port: Arc<dyn UndoFilePort>,
    operations: Option<Arc<OperationRuntime>>,
    watcher: Option<WatcherRuntime>,
    search_revision: Arc<AtomicU64>,
    image: Arc<dyn ImagePort>,
    scan_task_id: TaskId,
    scan_task: Option<JoinHandle<Result<(), CommandError>>>,
}

struct UnavailableFileUndoPort;

#[async_trait::async_trait]
impl UndoFilePort for UnavailableFileUndoPort {
    async fn reverse_batch(
        &self,
        _project_root: &Path,
        _batch: &viewer_application::undo::UndoBatch,
    ) -> Result<(), viewer_application::undo::UndoError> {
        Err(viewer_application::undo::UndoError::OutsideProject)
    }
}

struct ScanServices {
    scanner: Arc<dyn ScanPort>,
    coordinator: Arc<TaskCoordinator>,
    index: Arc<SessionIndex>,
    image: Arc<dyn ImagePort>,
    events: Arc<dyn DesktopEventSink>,
    portable_store: Option<Arc<PortableMarkerStore>>,
    marker_lock: Arc<Mutex<()>>,
    scan_ready: tokio::sync::watch::Sender<bool>,
}

struct OrganizationServices {
    file_undo_port: Arc<dyn UndoFilePort>,
    operations: Option<Arc<OperationRuntime>>,
    expected_changes: ExpectedChangeLedger,
    recovery_report: Option<RecoveryReportDto>,
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
    clock: Arc<dyn ClockPort>,
    marker_projection_factory: Arc<dyn DesktopMarkerProjectionFactory>,
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
        Self::new_with_runtime_services(
            cache_base,
            probe,
            scanner,
            events,
            image_factory,
            image_registry,
            active_image_session,
            Arc::new(SystemClock),
            Arc::new(SessionMarkerProjectionFactory),
        )
    }

    pub fn new_with_marker_projection_factory(
        cache_base: PathBuf,
        probe: Arc<dyn ProjectProbePort>,
        scanner: Arc<dyn ScanPort>,
        events: Arc<dyn DesktopEventSink>,
        image_factory: Arc<dyn DesktopImageFactory>,
        image_registry: Arc<ImageArtifactRegistry>,
        marker_projection_factory: Arc<dyn DesktopMarkerProjectionFactory>,
    ) -> Self {
        Self::new_with_runtime_services(
            cache_base,
            probe,
            scanner,
            events,
            image_factory,
            image_registry,
            ActiveImageSession::default(),
            Arc::new(SystemClock),
            marker_projection_factory,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_runtime_services(
        cache_base: PathBuf,
        probe: Arc<dyn ProjectProbePort>,
        scanner: Arc<dyn ScanPort>,
        events: Arc<dyn DesktopEventSink>,
        image_factory: Arc<dyn DesktopImageFactory>,
        image_registry: Arc<ImageArtifactRegistry>,
        active_image_session: ActiveImageSession,
        clock: Arc<dyn ClockPort>,
        marker_projection_factory: Arc<dyn DesktopMarkerProjectionFactory>,
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
            clock,
            marker_projection_factory,
            session: Mutex::new(None),
        }
    }

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

    pub async fn set_review_state(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        entity_ids: &[EntityId],
        review_state: Option<ReviewState>,
    ) -> Result<MarkerBatchResultDto, CommandError> {
        let review = review_state.map_or(ReviewPatch::Clear, ReviewPatch::Set);
        self.apply_marker_patch(
            expected_session,
            expected_generation,
            entity_ids,
            MarkerPatch {
                review,
                favorite: FavoritePatch::Unchanged,
            },
        )
        .await
    }

    pub async fn toggle_favorite(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        entity_ids: &[EntityId],
    ) -> Result<MarkerBatchResultDto, CommandError> {
        self.apply_marker_patch(
            expected_session,
            expected_generation,
            entity_ids,
            MarkerPatch {
                review: ReviewPatch::Unchanged,
                favorite: FavoritePatch::Toggle,
            },
        )
        .await
    }

    pub async fn selection_info(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        entity_ids: &[EntityId],
    ) -> Result<SelectionInfoDto, CommandError> {
        let index = {
            let session = self.session.lock().await;
            let session = session.as_ref().ok_or_else(project_not_open)?;
            validate_project_request(&session.active, expected_session, expected_generation)?;
            Arc::clone(&session.index)
        };
        BrowseService::new(index.as_ref())
            .selection_info(entity_ids)
            .map(SelectionInfoDto::from)
            .map_err(CommandError::from)
    }

    async fn apply_marker_patch(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        entity_ids: &[EntityId],
        patch: MarkerPatch,
    ) -> Result<MarkerBatchResultDto, CommandError> {
        let (active, index, store, projection, marker_lock, undo_stack) = {
            let session = self.session.lock().await;
            let session = session.as_ref().ok_or_else(project_not_open)?;
            validate_project_request(&session.active, expected_session, expected_generation)?;
            if session.active.access != ProjectAccess::ReadWrite {
                return Err(CommandError::new(
                    "project_read_only",
                    ErrorCategory::Conflict,
                    "当前项目为只读，无法保存标记。",
                    false,
                ));
            }
            let store = session.portable_store.clone().ok_or_else(|| {
                CommandError::new(
                    "portable_metadata_unavailable",
                    ErrorCategory::Consistency,
                    "项目审阅数据不可用，请重新打开项目。",
                    true,
                )
            })?;
            (
                session.active.clone(),
                Arc::clone(&session.index),
                store,
                Arc::clone(&session.marker_projection),
                Arc::clone(&session.marker_lock),
                Arc::clone(&session.undo_stack),
            )
        };
        let _marker_guard = marker_lock.lock().await;
        self.ensure_project_current(active.session_id, active.generation)
            .await?;
        let ids = entity_ids.to_vec();
        let active_for_work = active.clone();
        let index_for_work = Arc::clone(&index);
        let updated_at_ms = self.clock.unix_millis();
        let changes = tokio::task::spawn_blocking(move || {
            let targets = ids
                .into_iter()
                .map(|entity_id| {
                    let indexed = index_for_work
                        .indexed_node(entity_id)
                        .map_err(CommandError::from)?
                        .ok_or_else(selection_not_found)?;
                    validated_marker_target(&active_for_work, &indexed.node)
                        .ok_or_else(selection_not_found)
                })
                .collect::<Result<Vec<_>, CommandError>>()?;
            MarkerService::new(store.as_ref(), projection.as_ref(), true)
                .apply_with_undo(
                    &targets,
                    patch,
                    updated_at_ms,
                    &mut undo_stack
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner()),
                )
                .map_err(CommandError::from)
        })
        .await
        .map_err(|_| internal_command_error())??;
        self.ensure_project_current(active.session_id, active.generation)
            .await?;
        Ok(changes.into())
    }

    pub async fn preview_rename(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        entity_ids: &[EntityId],
        rules: RenameRuleSet,
    ) -> Result<RenamePreflight, CommandError> {
        let (active, index) = {
            let session = self.session.lock().await;
            let session = session.as_ref().ok_or_else(project_not_open)?;
            validate_project_request(&session.active, expected_session, expected_generation)?;
            if session.active.access != ProjectAccess::ReadWrite {
                return Err(project_read_only_operation());
            }
            (session.active.clone(), Arc::clone(&session.index))
        };
        let ids = entity_ids.to_vec();
        let prepared = tokio::task::spawn_blocking(move || {
            let targets = ids
                .into_iter()
                .map(|entity_id| {
                    index
                        .node(entity_id)
                        .map_err(CommandError::from)?
                        .filter(|node| node.kind != FileKind::Directory)
                        .map(|node| RenameTarget {
                            entity_id: node.entity_id,
                            relative_path: node.relative_path,
                        })
                        .ok_or_else(selection_not_found)
                })
                .collect::<Result<Vec<_>, CommandError>>()?;
            RenamePlanner::preflight(&active.root, &MacVolumePort, &targets, &rules)
                .map(|prepared| prepared.preview)
                .map_err(|_| invalid_rename_preview())
        })
        .await
        .map_err(|_| internal_command_error())??;
        self.ensure_project_current(expected_session, expected_generation)
            .await?;
        Ok(prepared)
    }

    pub async fn prepare_finder_drag(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        entity_ids: &[EntityId],
    ) -> Result<PreparedFinderDrag, CommandError> {
        let (root, index) = {
            let session = self.session.lock().await;
            let session = session.as_ref().ok_or_else(project_not_open)?;
            validate_project_request(&session.active, expected_session, expected_generation)?;
            (session.active.root.clone(), Arc::clone(&session.index))
        };
        let ids = entity_ids.to_vec();
        let prepared = tokio::task::spawn_blocking(move || {
            viewer_application::prepare_finder_drag(&root, index.as_ref(), &ids)
        })
        .await
        .map_err(|_| internal_command_error())?
        .map_err(CommandError::from)?;
        self.ensure_project_current(expected_session, expected_generation)
            .await?;
        Ok(prepared)
    }

    pub fn run_if_project_current<T>(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        action: impl FnOnce() -> T,
    ) -> Result<T, CommandError> {
        self.coordinator
            .run_if_current(expected_session, expected_generation, action)
            .ok_or_else(stale_project_session)
    }

    pub async fn execute_file_command(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        kind: FileCommandKind,
        items: Vec<FileCommandItem>,
        conflicts: Vec<ConflictResolution>,
    ) -> Result<OperationStarted, CommandError> {
        let operations = self
            .active_operations(expected_session, expected_generation)
            .await?;
        operations
            .start(
                expected_session,
                expected_generation,
                kind,
                items,
                conflicts,
            )
            .await
            .map_err(operation_runtime_error)
    }

    pub async fn preflight_file_command(
        &self,
        command: FileCommand,
    ) -> Result<FileCommandPreflight, CommandError> {
        let operations = self
            .active_operations(command.session_id, command.generation)
            .await?;
        operations
            .preflight(command)
            .await
            .map_err(operation_runtime_error)
    }

    pub async fn operation_status(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        batch_id: BatchId,
    ) -> Result<BatchProgress, CommandError> {
        self.active_operations(expected_session, expected_generation)
            .await?
            .status(batch_id)
            .map_err(operation_runtime_error)
    }

    pub async fn operation_results(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        batch_id: BatchId,
        offset: usize,
        limit: usize,
    ) -> Result<BatchResultPage, CommandError> {
        let operations = self
            .active_operations(expected_session, expected_generation)
            .await?;
        operations
            .wait(batch_id)
            .await
            .map_err(operation_runtime_error)?;
        operations
            .results(batch_id, offset, limit)
            .map_err(operation_runtime_error)
    }

    pub async fn cancel_operation(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
        batch_id: BatchId,
    ) -> Result<bool, CommandError> {
        self.active_operations(expected_session, expected_generation)
            .await?
            .cancel(batch_id)
            .map_err(operation_runtime_error)
    }

    pub async fn wait_for_operation(&self, batch_id: BatchId) -> Result<(), CommandError> {
        let operations = {
            let session = self.session.lock().await;
            let session = session.as_ref().ok_or_else(project_not_open)?;
            session
                .operations
                .clone()
                .ok_or_else(project_read_only_operation)?
        };
        operations
            .wait(batch_id)
            .await
            .map_err(operation_runtime_error)
    }

    pub async fn undo_last_operation(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
    ) -> Result<Option<UndoReceipt>, CommandError> {
        self.undo_last(expected_session, expected_generation).await
    }

    async fn active_operations(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
    ) -> Result<Arc<OperationRuntime>, CommandError> {
        let session = self.session.lock().await;
        let session = session.as_ref().ok_or_else(project_not_open)?;
        validate_project_request(&session.active, expected_session, expected_generation)?;
        session
            .operations
            .clone()
            .ok_or_else(project_read_only_operation)
    }

    pub async fn undo_last(
        &self,
        expected_session: SessionId,
        expected_generation: Generation,
    ) -> Result<Option<UndoReceipt>, CommandError> {
        let (active, store, projection, undo_stack, write_lane, file_undo_port) = {
            let session = self.session.lock().await;
            let session = session.as_ref().ok_or_else(project_not_open)?;
            validate_project_request(&session.active, expected_session, expected_generation)?;
            if session.active.access != ProjectAccess::ReadWrite {
                return Err(project_read_only_operation());
            }
            let store = session.portable_store.clone().ok_or_else(|| {
                CommandError::new(
                    "portable_metadata_unavailable",
                    ErrorCategory::Consistency,
                    "项目审阅数据不可用，请重新打开项目。",
                    true,
                )
            })?;
            (
                session.active.clone(),
                store,
                Arc::clone(&session.marker_projection),
                Arc::clone(&session.undo_stack),
                Arc::clone(&session.marker_lock),
                Arc::clone(&session.file_undo_port),
            )
        };
        let service = UndoService::new(
            active.session_id,
            active.access,
            active.root.clone(),
            Arc::new(LocalFileMutation),
            file_undo_port,
            store,
            projection,
            Arc::clone(&self.clock),
            undo_stack,
            write_lane,
        );
        let outcome = service
            .undo_last(expected_session)
            .await
            .map_err(CommandError::from)?;
        self.ensure_project_current(expected_session, expected_generation)
            .await?;
        Ok(outcome)
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
}

async fn run_scan(
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

    for node in nodes.into_iter().filter(|node| {
        matches!(
            node.kind,
            FileKind::Jpeg | FileKind::Png | FileKind::Markdown | FileKind::Text
        )
    }) {
        if !coordinator.is_publishable(active.session_id, active.generation) {
            return Ok(());
        }
        let current = match index.indexed_node(node.entity_id) {
            Ok(Some(current)) if current.node == node => current,
            Ok(_) => continue,
            Err(error) => return Err(CommandError::from(error)),
        };
        let pending = match node.kind {
            FileKind::Jpeg | FileKind::Png => current.image_status == ImageIndexStatus::Pending,
            FileKind::Markdown | FileKind::Text => current.text_status == TextIndexStatus::Pending,
            FileKind::Directory => false,
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
            FileKind::Directory => unreachable!("directories were filtered out"),
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
mod derived_error_tests {
    use super::is_stale_derived_write_error;
    use viewer_domain::EntityId;
    use viewer_infrastructure::search::index::SessionIndexError;

    #[test]
    fn derived_rebuild_ignores_only_explicit_stale_node_errors() {
        let entity_id = EntityId::new();
        assert!(is_stale_derived_write_error(
            &SessionIndexError::MissingTextNode {
                entity_id,
                path: "gone.txt".into(),
            }
        ));
        assert!(is_stale_derived_write_error(
            &SessionIndexError::MissingNode(entity_id)
        ));
        assert!(!is_stale_derived_write_error(
            &SessionIndexError::InvalidDerivedMetadata(entity_id)
        ));
        assert!(!is_stale_derived_write_error(
            &SessionIndexError::InvalidPersistedValue {
                field: "text_status",
                value: "corrupt".into(),
            }
        ));
    }
}
