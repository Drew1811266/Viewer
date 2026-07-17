use crate::{dto::ProjectSnapshot, error::CommandError};
use std::{path::Path, path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
use viewer_application::{
    ActiveProject, ProjectAccess, ProjectOpenError, ProjectProbeError, ProjectProbePort,
    ProjectSessionService, scheduler::TaskCoordinator,
};
use viewer_infrastructure::{search::index::SessionIndex, session_cache::SessionCache};

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
    cache: SessionCache,
    index: SessionIndex,
}

pub struct DesktopRuntime {
    cache_base: PathBuf,
    project_service: ProjectSessionService<SharedProjectProbe>,
    session: Mutex<Option<DesktopSession>>,
}

impl DesktopRuntime {
    pub fn new(cache_base: PathBuf, probe: Arc<dyn ProjectProbePort>) -> Self {
        Self {
            cache_base,
            project_service: ProjectSessionService::new(
                SharedProjectProbe(probe),
                Arc::new(TaskCoordinator::default()),
            ),
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
            Ok(cache) => cache,
            Err(error) => {
                let _ = self.project_service.close();
                return Err(error.into());
            }
        };
        let index = match SessionIndex::open(cache.index_path()) {
            Ok(index) => index,
            Err(error) => {
                let _ = cache.cleanup();
                let _ = self.project_service.close();
                return Err(error.into());
            }
        };
        let snapshot = ProjectSnapshot::from(&active);
        *session = Some(DesktopSession {
            active,
            snapshot: snapshot.clone(),
            cache,
            index,
        });
        Ok(snapshot)
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
