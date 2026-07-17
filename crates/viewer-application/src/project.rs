use crate::{
    ProjectAccess, ProjectProbeError, ProjectProbePort,
    scheduler::TaskCoordinator,
    session::{ProjectSession, SessionTransitionError},
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use viewer_domain::{ProjectId, SessionId, search::Generation};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveProject {
    pub project_id: ProjectId,
    pub session_id: SessionId,
    pub generation: Generation,
    pub root: PathBuf,
    pub display_name: String,
    pub access: ProjectAccess,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectOpenError {
    #[error("a project is already open")]
    AlreadyOpen,
    #[error("the selected project root is unsafe or unavailable")]
    UnsafeRoot,
    #[error(transparent)]
    Probe(#[from] ProjectProbeError),
    #[error(transparent)]
    Transition(#[from] SessionTransitionError),
}

#[derive(Default)]
struct ServiceState {
    session: ProjectSession,
    active: Option<ActiveProject>,
}

pub struct ProjectSessionService<P> {
    probe: P,
    coordinator: Arc<TaskCoordinator>,
    state: Mutex<ServiceState>,
}

impl<P> ProjectSessionService<P>
where
    P: ProjectProbePort,
{
    pub fn new(probe: P, coordinator: Arc<TaskCoordinator>) -> Self {
        Self {
            probe,
            coordinator,
            state: Mutex::new(ServiceState::default()),
        }
    }

    pub fn open(&self, requested: &Path) -> Result<ActiveProject, ProjectOpenError> {
        let mut state = self.lock_state();
        if state.active.is_some() {
            return Err(ProjectOpenError::AlreadyOpen);
        }
        state.session.begin_open()?;

        let result = self.prepare_active_project(requested);
        match result {
            Ok(prepared) => {
                state.session.activate(prepared.access)?;
                state.active = Some(prepared.clone());
                Ok(prepared)
            }
            Err(error) => {
                state.session.fail_open()?;
                state.session.finish_close()?;
                Err(error)
            }
        }
    }

    pub fn active(&self) -> Option<ActiveProject> {
        self.lock_state().active.clone()
    }

    pub fn close(&self) -> Result<Option<ActiveProject>, ProjectOpenError> {
        let mut state = self.lock_state();
        let Some(active) = state.active.take() else {
            return Ok(None);
        };
        state.session.begin_close()?;
        self.coordinator.cancel_session(active.session_id);
        state.session.finish_close()?;
        Ok(Some(active))
    }

    fn prepare_active_project(&self, requested: &Path) -> Result<ActiveProject, ProjectOpenError> {
        let metadata =
            std::fs::symlink_metadata(requested).map_err(|_| ProjectOpenError::UnsafeRoot)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(ProjectOpenError::UnsafeRoot);
        }
        let root = std::fs::canonicalize(requested).map_err(|_| ProjectOpenError::UnsafeRoot)?;
        let canonical_metadata =
            std::fs::symlink_metadata(&root).map_err(|_| ProjectOpenError::UnsafeRoot)?;
        if canonical_metadata.file_type().is_symlink() || !canonical_metadata.is_dir() {
            return Err(ProjectOpenError::UnsafeRoot);
        }
        let access = self.probe.probe(&root)?;
        let session_id = SessionId::new();
        let generation = self.coordinator.begin_session(session_id);
        let display_name = root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "项目".to_owned());
        Ok(ActiveProject {
            project_id: ProjectId::new(),
            session_id,
            generation,
            root,
            display_name,
            access,
        })
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, ServiceState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::{ProjectOpenError, ProjectSessionService};
    use crate::{ProjectAccess, ProjectProbeError, ProjectProbePort, scheduler::TaskCoordinator};
    use std::{
        path::Path,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    #[derive(Clone)]
    struct Probe {
        access: ProjectAccess,
        calls: Arc<AtomicUsize>,
    }

    impl Probe {
        fn new(access: ProjectAccess) -> Self {
            Self {
                access,
                calls: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    impl ProjectProbePort for Probe {
        fn probe(&self, _root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(self.access)
        }
    }

    #[test]
    fn open_canonicalizes_one_real_directory_and_rejects_a_second_open() {
        let root = tempfile::tempdir().unwrap();
        let service = ProjectSessionService::new(
            Probe::new(ProjectAccess::ReadWrite),
            Arc::new(TaskCoordinator::default()),
        );

        let opened = service.open(root.path()).unwrap();

        assert_eq!(opened.root, root.path().canonicalize().unwrap());
        assert_eq!(opened.access, ProjectAccess::ReadWrite);
        assert!(matches!(
            service.open(root.path()),
            Err(ProjectOpenError::AlreadyOpen)
        ));
    }

    #[test]
    fn open_preserves_read_only_access_and_close_cancels_the_generation() {
        let root = tempfile::tempdir().unwrap();
        let coordinator = Arc::new(TaskCoordinator::default());
        let service = ProjectSessionService::new(
            Probe::new(ProjectAccess::ReadOnly),
            Arc::clone(&coordinator),
        );

        let opened = service.open(root.path()).unwrap();
        assert_eq!(opened.access, ProjectAccess::ReadOnly);
        assert!(coordinator.is_publishable(opened.session_id, opened.generation));

        let closed = service.close().unwrap().unwrap();
        assert_eq!(closed.session_id, opened.session_id);
        assert!(!coordinator.is_publishable(opened.session_id, opened.generation));
        assert!(service.active().is_none());
    }

    #[test]
    fn open_rejects_a_file_without_calling_the_probe() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("front.jpg");
        std::fs::write(&file, b"fixture").unwrap();
        let probe = Probe::new(ProjectAccess::ReadWrite);
        let calls = Arc::clone(&probe.calls);
        let service = ProjectSessionService::new(probe, Arc::new(TaskCoordinator::default()));

        assert!(matches!(
            service.open(&file),
            Err(ProjectOpenError::UnsafeRoot)
        ));
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }

    #[cfg(unix)]
    #[test]
    fn open_rejects_a_symlinked_root_before_probe() {
        let actual = tempfile::tempdir().unwrap();
        let parent = tempfile::tempdir().unwrap();
        let linked = parent.path().join("linked");
        std::os::unix::fs::symlink(actual.path(), &linked).unwrap();
        let probe = Probe::new(ProjectAccess::ReadWrite);
        let calls = Arc::clone(&probe.calls);
        let service = ProjectSessionService::new(probe, Arc::new(TaskCoordinator::default()));

        assert!(matches!(
            service.open(&linked),
            Err(ProjectOpenError::UnsafeRoot)
        ));
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }
}
