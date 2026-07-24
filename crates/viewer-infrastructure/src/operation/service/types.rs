use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use viewer_application::{
    ClockPort, FileContentEvidence, FileMutationPort, FileOperationError, FileSnapshot,
    OperationCommitPort, TrashPort, VolumePort,
    browse::BrowseIndexPort,
    file_commands::{BatchId, FileCommandPreflightState, LocalFileCommandOutcome},
    watcher::FileIdentity,
};
use viewer_domain::{
    EntityId,
    operation::{OperationItemPlan, OperationPlan},
};

use super::super::{journal::OperationJournal, rename::RenamePlan};
use crate::scan::reconcile::ExpectedChangeLedger;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PreparedRoute {
    RenameGroup,
    RenameConflict,
    Copy,
    AtomicMove,
    CrossVolumeMove,
    Trash,
}

#[derive(Clone)]
pub(super) struct PreparedItem {
    pub(super) plan: OperationItemPlan,
    pub(super) state: FileCommandPreflightState,
    pub(super) route: PreparedRoute,
    pub(super) source_evidence: Option<FileSnapshot>,
    pub(super) destination_evidence: Option<FileContentEvidence>,
    pub(super) source_parent_identity: Option<FileIdentity>,
    pub(super) destination_parent_identity: Option<FileIdentity>,
}

pub(super) struct PreparedBatch {
    pub(super) plan: OperationPlan,
    pub(super) items: HashMap<EntityId, PreparedItem>,
    pub(super) rename_plan: Option<RenamePlan>,
    pub(super) outcomes: HashMap<EntityId, LocalFileCommandOutcome>,
    pub(super) delivered: HashSet<EntityId>,
    pub(super) journal_started: bool,
    pub(super) journal_finished: bool,
    pub(super) rename_executed: bool,
}

/// Project-scoped implementation of the application file-command port.
///
/// The application owns serialization and lifecycle. This adapter owns path
/// resolution, durable intent, verified mutation and truthful commit barriers.
pub struct LocalFileCommandAdapter {
    pub(super) project_root: PathBuf,
    pub(super) index: Arc<dyn BrowseIndexPort>,
    pub(super) journal: Arc<OperationJournal>,
    pub(super) mutation: Arc<dyn FileMutationPort>,
    pub(super) trash: Arc<dyn TrashPort>,
    pub(super) volume: Arc<dyn VolumePort>,
    pub(super) clock: Arc<dyn ClockPort>,
    pub(super) commits: Arc<dyn OperationCommitPort>,
    pub(super) expected_changes: ExpectedChangeLedger,
    pub(super) batches: Mutex<HashMap<BatchId, PreparedBatch>>,
}

impl LocalFileCommandAdapter {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        project_root: impl AsRef<Path>,
        index: Arc<dyn BrowseIndexPort>,
        journal: Arc<OperationJournal>,
        mutation: Arc<dyn FileMutationPort>,
        trash: Arc<dyn TrashPort>,
        volume: Arc<dyn VolumePort>,
        clock: Arc<dyn ClockPort>,
        commits: Arc<dyn OperationCommitPort>,
    ) -> Result<Self, FileOperationError> {
        let project_root = std::fs::canonicalize(project_root.as_ref()).map_err(|error| {
            FileOperationError::io(
                "canonicalize file command root",
                project_root.as_ref(),
                &error,
            )
        })?;
        Ok(Self {
            project_root,
            index,
            journal,
            mutation,
            trash,
            volume,
            clock,
            commits,
            expected_changes: ExpectedChangeLedger::default(),
            batches: Mutex::new(HashMap::new()),
        })
    }

    pub fn expected_change_ledger(&self) -> ExpectedChangeLedger {
        self.expected_changes.clone()
    }

    pub fn prepared_batch_count(&self) -> usize {
        self.lock_batches().len()
    }
}
