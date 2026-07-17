use crate::{dto::ProjectSnapshot, error::CommandError};
use std::{path::Path, path::PathBuf, sync::Arc, time::Duration};
use tokio::{sync::Mutex, task::JoinHandle, time::Instant};
use viewer_application::{
    ActiveProject, ProjectAccess, ProjectOpenError, ProjectProbeError, ProjectProbePort,
    ProjectSessionService, ScanPort,
    scan::{CoordinatedScan, ScanEvent, ScanRequest},
    scheduler::{TaskClass, TaskCoordinator},
};
use viewer_domain::TaskId;
use viewer_infrastructure::{
    scan::walker::ProjectWalker, search::index::SessionIndex, session_cache::SessionCache,
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
    scan_task_id: TaskId,
    scan_task: Option<JoinHandle<Result<(), CommandError>>>,
}

pub struct DesktopRuntime {
    cache_base: PathBuf,
    coordinator: Arc<TaskCoordinator>,
    project_service: ProjectSessionService<SharedProjectProbe>,
    scanner: Arc<dyn ScanPort>,
    events: Arc<dyn DesktopEventSink>,
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
        let snapshot = ProjectSnapshot::from(&active);
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
            scan_task_id,
            scan_task: Some(scan_task),
        });
        Ok(snapshot)
    }

    pub async fn close_project(&self) -> Result<(), CommandError> {
        let Some(mut session) = self.session.lock().await.take() else {
            return Ok(());
        };
        self.project_service.close().map_err(CommandError::from)?;
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
