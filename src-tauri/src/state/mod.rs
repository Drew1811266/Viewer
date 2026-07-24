mod common;
mod markers;
mod organization;
mod preview;
mod scan_index;
mod session;

use common::{
    claim_search_revision, entity_id_for_metadata, invalid_rename_preview, modified_ns,
    operation_backend_unavailable, operation_runtime_error, project_read_only_operation,
    selection_not_found, validate_project_request,
};
pub(crate) use common::{
    internal_command_error, is_stale_derived_write_error, project_not_open, stale_project_session,
};
pub(crate) use scan_index::rebuild_derived_nodes;
use scan_index::run_scan;

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
