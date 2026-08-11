use crate::{
    scan::walker::{SubtreeSnapshotError, snapshot_subtrees},
    search::index::SessionIndex,
};
use std::{path::Path, path::PathBuf, sync::Arc};
use viewer_application::{
    ClockPort,
    metadata::PortableMetadataPort,
    scheduler::TaskCoordinator,
    watcher::{ReconcileRequest, ReconcileSummary},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProjectReconcileError {
    #[error("the reconcile request belongs to a stale project session")]
    StaleSession,
    #[error("the reconcile root is outside the active project")]
    InvalidRoot,
    #[error("the session index could not commit the reconcile delta")]
    IndexUnavailable,
    #[error("portable markers could not be relocated")]
    MarkerUnavailable,
}

pub struct ProjectReconciler {
    project_root: PathBuf,
    coordinator: Arc<TaskCoordinator>,
    index: Arc<SessionIndex>,
    markers: Option<Arc<dyn PortableMetadataPort>>,
    clock: Arc<dyn ClockPort>,
    write_lane: Arc<tokio::sync::Mutex<()>>,
    case_sensitive: bool,
}

impl ProjectReconciler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        project_root: impl AsRef<Path>,
        coordinator: Arc<TaskCoordinator>,
        index: Arc<SessionIndex>,
        markers: Option<Arc<dyn PortableMetadataPort>>,
        clock: Arc<dyn ClockPort>,
        write_lane: Arc<tokio::sync::Mutex<()>>,
        case_sensitive: bool,
    ) -> Result<Self, ProjectReconcileError> {
        let project_root = project_root.as_ref();
        let metadata = std::fs::symlink_metadata(project_root)
            .map_err(|_| ProjectReconcileError::InvalidRoot)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(ProjectReconcileError::InvalidRoot);
        }
        let project_root =
            std::fs::canonicalize(project_root).map_err(|_| ProjectReconcileError::InvalidRoot)?;
        Ok(Self {
            project_root,
            coordinator,
            index,
            markers,
            clock,
            write_lane,
            case_sensitive,
        })
    }

    pub async fn reconcile(
        &self,
        request: ReconcileRequest,
    ) -> Result<ReconcileSummary, ProjectReconcileError> {
        self.ensure_current(&request)?;
        let _lane = self.write_lane.lock().await;
        self.ensure_current(&request)?;
        let project_root = self.project_root.clone();
        let roots = request.roots.clone();
        let snapshot =
            tokio::task::spawn_blocking(move || snapshot_subtrees(&project_root, &roots))
                .await
                .map_err(|_| ProjectReconcileError::IndexUnavailable)?
                .map_err(map_snapshot_error)?;
        self.ensure_current(&request)?;

        let index = Arc::clone(&self.index);
        let scopes = snapshot.scopes.clone();
        let protected = snapshot.protected.clone();
        let nodes = snapshot.nodes.clone();
        let moves = tokio::task::spawn_blocking(move || {
            index.reconcile_identity_moves(&scopes, &protected, &nodes)
        })
        .await
        .map_err(|_| ProjectReconcileError::IndexUnavailable)?
        .map_err(|_| ProjectReconcileError::IndexUnavailable)?;
        self.ensure_current(&request)?;

        let marker_paths_moved = if moves.is_empty() {
            0
        } else if let Some(markers) = self.markers.as_ref() {
            let markers = Arc::clone(markers);
            let case_sensitive = self.case_sensitive;
            let updated_at_ms = self.clock.unix_millis();
            u64::try_from(
                tokio::task::spawn_blocking(move || {
                    markers.move_paths(&moves, case_sensitive, updated_at_ms)
                })
                .await
                .map_err(|_| ProjectReconcileError::MarkerUnavailable)?
                .map_err(|_| ProjectReconcileError::MarkerUnavailable)?,
            )
            .unwrap_or(u64::MAX)
        } else {
            0
        };
        self.ensure_current(&request)?;

        let index = Arc::clone(&self.index);
        let generation = request.generation;
        let summary = tokio::task::spawn_blocking(move || {
            index.reconcile_subtrees(
                &snapshot.scopes,
                &snapshot.protected,
                &snapshot.nodes,
                generation,
            )
        })
        .await
        .map_err(|_| ProjectReconcileError::IndexUnavailable)?
        .map_err(|_| ProjectReconcileError::IndexUnavailable)?;
        self.ensure_current(&request)?;
        Ok(ReconcileSummary {
            reason: request.reason,
            added: summary.added,
            removed: summary.removed,
            modified: summary.modified,
            moved: summary.moved,
            marker_paths_moved,
            failed: snapshot.failed,
        })
    }

    fn ensure_current(&self, request: &ReconcileRequest) -> Result<(), ProjectReconcileError> {
        self.coordinator
            .is_publishable(request.session_id, request.generation)
            .then_some(())
            .ok_or(ProjectReconcileError::StaleSession)
    }
}

fn map_snapshot_error(_error: SubtreeSnapshotError) -> ProjectReconcileError {
    ProjectReconcileError::InvalidRoot
}
