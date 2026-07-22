use async_trait::async_trait;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use viewer_application::{
    ClockPort, FileContentEvidence, FileMutationPort, FileOperationError, FileSnapshot,
    LocalFileCommandPort, OperationCommit, OperationCommitPort, TrashPort, VolumePort,
    browse::BrowseIndexPort,
    file_commands::{
        BatchId, BatchResultCode, FileCommand, FileCommandAction, FileCommandCancellation,
        FileCommandItemExecution, FileCommandKind, FileCommandPreflightState,
        LocalFileCommandError, LocalFileCommandOutcome, LocalFileCommandPreflightItem,
    },
    rename::{SingleRenameRequest, preview_single_rename},
    undo::{UndoAction, UndoBatch, UndoError, UndoFilePort},
    watcher::FileIdentity,
};
use viewer_domain::{
    EntityId, OperationId, RelativePath,
    file::{FileKind, FileNode},
    operation::{
        ConflictPolicy, OperationItemPlan, OperationKind, OperationPlan, OperationState,
        RenameErrorCode, RenamePreflight,
    },
};

use super::{
    conflict::{ReplaceExecutor, keep_both_destination},
    copy::hash_file_sync,
    journal::OperationJournal,
    rename::{
        RenameExecutor, RenameItemStatus, RenameMapping, RenameParentIdentities, RenamePlan,
        RenamePlanner,
    },
};
use crate::scan::reconcile::{ExpectedChange, ExpectedChangeLedger};

const EXPECTED_CHANGE_TTL_MS: u64 = 5_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreparedRoute {
    RenameGroup,
    RenameConflict,
    Copy,
    AtomicMove,
    CrossVolumeMove,
    Trash,
}

#[derive(Clone)]
struct PreparedItem {
    plan: OperationItemPlan,
    state: FileCommandPreflightState,
    route: PreparedRoute,
    source_evidence: Option<FileSnapshot>,
    destination_evidence: Option<FileContentEvidence>,
    source_parent_identity: Option<FileIdentity>,
    destination_parent_identity: Option<FileIdentity>,
}

struct PreparedBatch {
    plan: OperationPlan,
    items: HashMap<EntityId, PreparedItem>,
    rename_plan: Option<RenamePlan>,
    outcomes: HashMap<EntityId, LocalFileCommandOutcome>,
    delivered: HashSet<EntityId>,
    journal_started: bool,
    journal_finished: bool,
    rename_executed: bool,
}

/// Project-scoped implementation of the application file-command port.
///
/// The application owns serialization and lifecycle. This adapter owns path
/// resolution, durable intent, verified mutation and truthful commit barriers.
pub struct LocalFileCommandAdapter {
    project_root: PathBuf,
    index: Arc<dyn BrowseIndexPort>,
    journal: Arc<OperationJournal>,
    mutation: Arc<dyn FileMutationPort>,
    trash: Arc<dyn TrashPort>,
    volume: Arc<dyn VolumePort>,
    clock: Arc<dyn ClockPort>,
    commits: Arc<dyn OperationCommitPort>,
    expected_changes: ExpectedChangeLedger,
    batches: Mutex<HashMap<BatchId, PreparedBatch>>,
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

    fn prepare(&self, batch_id: BatchId, command: &FileCommand) -> PreparedBatch {
        if command.kind == FileCommandKind::Rename {
            return self.prepare_rename(batch_id, command);
        }
        let mut items = HashMap::with_capacity(command.items.len());
        for command_item in &command.items {
            let prepared = self.prepare_simple(batch_id, command.kind, command_item);
            items.insert(command_item.entity_id, prepared);
        }
        let mut requested_destinations = HashSet::new();
        let case_sensitive = self.case_sensitive_root();
        for command_item in &command.items {
            let Some(item) = items.get_mut(&command_item.entity_id) else {
                continue;
            };
            if item.state != FileCommandPreflightState::Ready {
                continue;
            }
            let Some(destination) = item.plan.destination.as_ref() else {
                continue;
            };
            let key = if case_sensitive {
                destination.as_str().to_owned()
            } else {
                destination.as_str().to_lowercase()
            };
            if !requested_destinations.insert(key) {
                item.state = FileCommandPreflightState::Conflict;
            }
        }
        let plan = OperationPlan {
            batch_id,
            kind: command.kind.into(),
            items: command
                .items
                .iter()
                .filter_map(|item| items.get(&item.entity_id).map(|item| item.plan.clone()))
                .collect(),
        };
        PreparedBatch {
            plan,
            items,
            rename_plan: None,
            outcomes: HashMap::new(),
            delivered: HashSet::new(),
            journal_started: false,
            journal_finished: false,
            rename_executed: false,
        }
    }

    fn prepare_simple(
        &self,
        batch_id: BatchId,
        kind: FileCommandKind,
        command_item: &viewer_application::file_commands::FileCommandItem,
    ) -> PreparedItem {
        let fallback = RelativePath::parse("invalid-target").expect("static relative path");
        let node = self.index.node(command_item.entity_id).ok().flatten();
        let source = node
            .as_ref()
            .map(|node| node.relative_path.clone())
            .unwrap_or(fallback);
        let operation_id = OperationId::new();
        let mut plan = OperationItemPlan {
            batch_id,
            operation_id,
            entity_id: command_item.entity_id,
            kind: kind.into(),
            source,
            destination: None,
            conflict_policy: ConflictPolicy::Skip,
        };
        let Some(node) = node else {
            return blocked(plan, BatchResultCode::SourceMissing, PreparedRoute::Trash);
        };
        let source_evidence = self
            .resolve_source(&node)
            .and_then(|source| snapshot_sync(&source));
        if !supported_regular_kind(node.kind) || source_evidence.is_err() {
            return blocked(plan, BatchResultCode::InvalidTarget, PreparedRoute::Trash);
        }
        let source_evidence = source_evidence.ok();
        let source_parent = self
            .project_root
            .join(node.relative_path.as_str())
            .parent()
            .map(Path::to_path_buf);
        let source_parent_identity = source_parent
            .as_deref()
            .and_then(|parent| directory_identity_sync(parent).ok());
        if matches!(kind, FileCommandKind::Move | FileCommandKind::Trash)
            && source_parent
                .as_deref()
                .is_none_or(|parent| !directory_is_writable(parent))
        {
            return blocked(
                plan,
                BatchResultCode::PermissionDenied,
                PreparedRoute::Trash,
            );
        }

        match &command_item.action {
            FileCommandAction::Trash if kind == FileCommandKind::Trash => PreparedItem {
                plan,
                state: FileCommandPreflightState::Ready,
                route: PreparedRoute::Trash,
                source_evidence,
                destination_evidence: None,
                source_parent_identity,
                destination_parent_identity: None,
            },
            FileCommandAction::Copy { destination_folder }
            | FileCommandAction::Move { destination_folder }
                if matches!(kind, FileCommandKind::Copy | FileCommandKind::Move) =>
            {
                let Some((destination, parent)) = self.destination_for(&node, *destination_folder)
                else {
                    return blocked(plan, BatchResultCode::InvalidTarget, PreparedRoute::Copy);
                };
                plan.destination = Some(destination.clone());
                if !directory_is_writable(&parent) {
                    return blocked(plan, BatchResultCode::PermissionDenied, PreparedRoute::Copy);
                }
                if kind == FileCommandKind::Move && destination == node.relative_path {
                    return blocked(plan, BatchResultCode::InvalidTarget, PreparedRoute::Copy);
                }
                let candidate = self.project_root.join(destination.as_str());
                let (state, destination_evidence) = match std::fs::symlink_metadata(&candidate) {
                    Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => (
                        FileCommandPreflightState::Conflict,
                        stable_content_evidence_sync(&candidate).ok(),
                    ),
                    Ok(_) => (
                        FileCommandPreflightState::Blocked(BatchResultCode::InvalidTarget),
                        None,
                    ),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        (FileCommandPreflightState::Ready, None)
                    }
                    Err(_) => (
                        FileCommandPreflightState::Blocked(BatchResultCode::PermissionDenied),
                        None,
                    ),
                };
                let route = if kind == FileCommandKind::Copy {
                    PreparedRoute::Copy
                } else {
                    let source = self.project_root.join(node.relative_path.as_str());
                    match (
                        self.volume.volume_id(&source),
                        self.volume.volume_id(&parent),
                    ) {
                        (Ok(source), Ok(destination)) if source == destination => {
                            PreparedRoute::AtomicMove
                        }
                        (Ok(_), Ok(_)) => PreparedRoute::CrossVolumeMove,
                        _ => {
                            return blocked(
                                plan,
                                BatchResultCode::BackendUnavailable,
                                PreparedRoute::AtomicMove,
                            );
                        }
                    }
                };
                PreparedItem {
                    plan,
                    state,
                    route,
                    source_evidence,
                    destination_evidence,
                    source_parent_identity,
                    destination_parent_identity: directory_identity_sync(&parent).ok(),
                }
            }
            _ => blocked(plan, BatchResultCode::InvalidTarget, PreparedRoute::Trash),
        }
    }

    fn prepare_rename(&self, batch_id: BatchId, command: &FileCommand) -> PreparedBatch {
        let mut previews = Vec::with_capacity(command.items.len());
        let mut operation_ids = HashMap::with_capacity(command.items.len());
        for item in &command.items {
            operation_ids.insert(item.entity_id, OperationId::new());
            let node = self.index.node(item.entity_id).ok().flatten();
            let Some(node) = node.filter(|node| supported_regular_kind(node.kind)) else {
                previews.push(viewer_domain::operation::RenamePreviewRow {
                    entity_id: item.entity_id,
                    source: RelativePath::parse("invalid-target").expect("static path"),
                    destination: None,
                    proposed_name: String::new(),
                    errors: vec![RenameErrorCode::SourceMissing],
                });
                continue;
            };
            let FileCommandAction::Rename {
                proposed_name,
                edit_extension,
            } = &item.action
            else {
                unreachable!("application validates command action kinds")
            };
            previews.push(
                preview_single_rename(&SingleRenameRequest {
                    target: viewer_domain::operation::RenameTarget {
                        entity_id: item.entity_id,
                        relative_path: node.relative_path,
                    },
                    requested_name: proposed_name.clone(),
                    edit_extension: *edit_extension,
                })
                .rows
                .into_iter()
                .next()
                .expect("single rename always returns one row"),
            );
        }
        mark_duplicate_rename_destinations(&mut previews, self.case_sensitive_root());
        let validated = RenamePlanner::preflight_preview(
            &self.project_root,
            self.volume.as_ref(),
            RenamePreflight {
                rows: previews,
                executable: false,
            },
        );
        let rows = match validated {
            Ok(prepared) => prepared.preview.rows,
            Err(_) => command
                .items
                .iter()
                .map(|item| viewer_domain::operation::RenamePreviewRow {
                    entity_id: item.entity_id,
                    source: RelativePath::parse("invalid-target").expect("static path"),
                    destination: None,
                    proposed_name: String::new(),
                    errors: vec![RenameErrorCode::UnsafeParent],
                })
                .collect(),
        };
        let mut rows = rows;
        for row in &mut rows {
            let source_parent = self
                .project_root
                .join(row.source.as_str())
                .parent()
                .map(Path::to_path_buf);
            let destination_parent = row.destination.as_ref().and_then(|destination| {
                self.project_root
                    .join(destination.as_str())
                    .parent()
                    .map(Path::to_path_buf)
            });
            if source_parent
                .as_deref()
                .is_none_or(|parent| !directory_is_writable(parent))
                || destination_parent
                    .as_deref()
                    .is_none_or(|parent| !directory_is_writable(parent))
            {
                row.push_error(RenameErrorCode::DestinationReadOnly);
            }
        }

        let mut items = HashMap::with_capacity(rows.len());
        let mut ready_mappings = Vec::new();
        for row in &rows {
            let conflict = row.errors == [RenameErrorCode::DestinationOccupied];
            let state = if row.errors.is_empty() {
                FileCommandPreflightState::Ready
            } else if conflict {
                FileCommandPreflightState::Conflict
            } else {
                FileCommandPreflightState::Blocked(rename_error_code(&row.errors))
            };
            let operation_id = operation_ids[&row.entity_id];
            let plan = OperationItemPlan {
                batch_id,
                operation_id,
                entity_id: row.entity_id,
                kind: OperationKind::Rename,
                source: row.source.clone(),
                destination: row.destination.clone(),
                conflict_policy: ConflictPolicy::Skip,
            };
            let source_evidence = snapshot_sync(&self.project_root.join(row.source.as_str())).ok();
            let destination_evidence = row.destination.as_ref().and_then(|destination| {
                stable_content_evidence_sync(&self.project_root.join(destination.as_str())).ok()
            });
            if state == FileCommandPreflightState::Ready
                && let Some(destination) = row.destination.as_ref()
            {
                ready_mappings.push(RenameMapping {
                    operation_id,
                    entity_id: row.entity_id,
                    source: PathBuf::from(row.source.as_str()),
                    destination: PathBuf::from(destination.as_str()),
                });
            }
            items.insert(
                row.entity_id,
                PreparedItem {
                    plan,
                    state,
                    route: if conflict {
                        PreparedRoute::RenameConflict
                    } else {
                        PreparedRoute::RenameGroup
                    },
                    source_evidence,
                    destination_evidence,
                    source_parent_identity: self
                        .project_root
                        .join(row.source.as_str())
                        .parent()
                        .and_then(|parent| directory_identity_sync(parent).ok()),
                    destination_parent_identity: row.destination.as_ref().and_then(|destination| {
                        self.project_root
                            .join(destination.as_str())
                            .parent()
                            .and_then(|parent| directory_identity_sync(parent).ok())
                    }),
                },
            );
        }
        let rename_plan = if ready_mappings.is_empty() {
            None
        } else {
            RenamePlanner::plan(
                &self.project_root,
                self.case_sensitive_root(),
                &ready_mappings,
            )
            .ok()
        };
        let plan = OperationPlan {
            batch_id,
            kind: OperationKind::Rename,
            items: command
                .items
                .iter()
                .filter_map(|item| items.get(&item.entity_id).map(|item| item.plan.clone()))
                .collect(),
        };
        PreparedBatch {
            plan,
            items,
            rename_plan,
            outcomes: HashMap::new(),
            delivered: HashSet::new(),
            journal_started: false,
            journal_finished: false,
            rename_executed: false,
        }
    }

    fn destination_for(
        &self,
        source: &FileNode,
        folder: EntityId,
    ) -> Option<(RelativePath, PathBuf)> {
        let folder = self.index.node(folder).ok().flatten()?;
        if folder.kind != FileKind::Directory {
            return None;
        }
        let candidate = self.project_root.join(folder.relative_path.as_str());
        let parent = std::fs::canonicalize(&candidate).ok()?;
        if parent != candidate || !parent.starts_with(&self.project_root) || !parent.is_dir() {
            return None;
        }
        let name = Path::new(source.relative_path.as_str())
            .file_name()?
            .to_str()?;
        let destination = format!("{}/{}", folder.relative_path.as_str(), name);
        RelativePath::parse(&destination)
            .ok()
            .map(|path| (path, parent))
    }

    fn resolve_source(&self, node: &FileNode) -> Result<PathBuf, FileOperationError> {
        let candidate = self.project_root.join(node.relative_path.as_str());
        let metadata = std::fs::symlink_metadata(&candidate).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                FileOperationError::SourceMissing
            } else {
                FileOperationError::io("inspect file command source", &candidate, &error)
            }
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(FileOperationError::OutsideProject);
        }
        let canonical = std::fs::canonicalize(&candidate).map_err(|error| {
            FileOperationError::io("resolve file command source", &candidate, &error)
        })?;
        if canonical != candidate || !canonical.starts_with(&self.project_root) {
            return Err(FileOperationError::OutsideProject);
        }
        Ok(canonical)
    }

    fn revalidate_source(&self, item: &PreparedItem) -> Result<(), FileOperationError> {
        let node = self
            .index
            .node(item.plan.entity_id)
            .map_err(|_| FileOperationError::SourceMissing)?
            .filter(|node| {
                node.relative_path == item.plan.source && supported_regular_kind(node.kind)
            })
            .ok_or(FileOperationError::SourceMissing)?;
        let source = self.resolve_source(&node)?;
        let expected = item
            .source_evidence
            .as_ref()
            .ok_or(FileOperationError::IdentityChanged)?;
        if snapshot_sync(&source)? != *expected {
            return Err(FileOperationError::IdentityChanged);
        }
        if matches!(
            item.plan.kind,
            OperationKind::Rename | OperationKind::Move | OperationKind::Trash
        ) && source
            .parent()
            .is_none_or(|parent| !directory_is_writable(parent))
        {
            return Err(permission_error(&source));
        }
        if let Some(destination) = item.plan.destination.as_ref() {
            let destination = self.project_root.join(destination.as_str());
            let parent = destination
                .parent()
                .ok_or(FileOperationError::OutsideProject)?;
            let metadata = std::fs::symlink_metadata(parent).map_err(|error| {
                FileOperationError::io("inspect destination parent", parent, &error)
            })?;
            let canonical = std::fs::canonicalize(parent).map_err(|error| {
                FileOperationError::io("canonicalize destination parent", parent, &error)
            })?;
            if metadata.file_type().is_symlink()
                || !metadata.is_dir()
                || canonical != parent
                || !canonical.starts_with(&self.project_root)
            {
                return Err(FileOperationError::OutsideProject);
            }
            if !directory_is_writable(&canonical) {
                return Err(permission_error(&destination));
            }
            if item.destination_parent_identity != directory_identity_sync(&canonical).ok() {
                return Err(FileOperationError::IdentityChanged);
            }
        }
        let source_parent = source.parent().ok_or(FileOperationError::OutsideProject)?;
        if item.source_parent_identity != directory_identity_sync(source_parent).ok() {
            return Err(FileOperationError::IdentityChanged);
        }
        Ok(())
    }

    fn revalidate_replace_destination(
        &self,
        item: &PreparedItem,
        policy: Option<ConflictPolicy>,
    ) -> Result<(), FileOperationError> {
        if policy != Some(ConflictPolicy::Replace) {
            return Ok(());
        }
        let destination = item
            .plan
            .destination
            .as_ref()
            .ok_or(FileOperationError::DestinationRequired)?;
        let expected = item
            .destination_evidence
            .as_ref()
            .ok_or(FileOperationError::IdentityChanged)?;
        if stable_content_evidence_sync(&self.project_root.join(destination.as_str()))? != *expected
        {
            return Err(FileOperationError::IdentityChanged);
        }
        Ok(())
    }

    fn revalidate_move_route(&self, item: &PreparedItem) -> Result<(), FileOperationError> {
        if item.plan.kind != OperationKind::Move {
            return Ok(());
        }
        let source = self.project_root.join(item.plan.source.as_str());
        let destination = item
            .plan
            .destination
            .as_ref()
            .ok_or(FileOperationError::DestinationRequired)?;
        let destination_path = self.project_root.join(destination.as_str());
        let destination_parent = destination_path
            .parent()
            .and_then(|parent| std::fs::canonicalize(parent).ok())
            .ok_or(FileOperationError::OutsideProject)?;
        let same_volume =
            self.volume.volume_id(&source)? == self.volume.volume_id(&destination_parent)?;
        if (same_volume && item.route != PreparedRoute::AtomicMove)
            || (!same_volume && item.route != PreparedRoute::CrossVolumeMove)
        {
            return Err(FileOperationError::IdentityChanged);
        }
        Ok(())
    }

    fn revalidate_destination_entity(
        &self,
        request: &FileCommandItemExecution,
        item: &PreparedItem,
    ) -> Result<(), FileOperationError> {
        let folder_id = match &request.item.action {
            FileCommandAction::Copy { destination_folder }
            | FileCommandAction::Move { destination_folder } => *destination_folder,
            FileCommandAction::Rename { .. } | FileCommandAction::Trash => return Ok(()),
        };
        let folder = self
            .index
            .node(folder_id)
            .map_err(|_| FileOperationError::OutsideProject)?
            .filter(|folder| folder.kind == FileKind::Directory)
            .ok_or(FileOperationError::OutsideProject)?;
        let destination = item
            .plan
            .destination
            .as_ref()
            .ok_or(FileOperationError::DestinationRequired)?;
        let parent = Path::new(destination.as_str())
            .parent()
            .and_then(Path::to_str)
            .ok_or(FileOperationError::OutsideProject)?;
        if folder.relative_path.as_str() != parent {
            return Err(FileOperationError::OutsideProject);
        }
        Ok(())
    }

    fn case_sensitive_root(&self) -> bool {
        self.volume
            .is_case_sensitive(&self.project_root)
            .unwrap_or(true)
    }

    fn ensure_journal_started(&self, batch_id: BatchId) -> Result<(), LocalFileCommandError> {
        let mut batches = self.lock_batches();
        let batch = batches
            .get_mut(&batch_id)
            .ok_or(LocalFileCommandError::Unavailable)?;
        if !batch.journal_started {
            self.journal
                .begin_plan(&batch.plan, self.now())
                .map_err(|_| LocalFileCommandError::Unavailable)?;
            batch.journal_started = true;
        }
        Ok(())
    }

    fn prepared_item(&self, request: &FileCommandItemExecution) -> Option<PreparedItem> {
        let batches = self.lock_batches();
        let batch = batches.get(&request.batch_id)?;
        let item = batch.items.get(&request.item.entity_id)?;
        (batch.plan.kind == request.kind.into()
            && item.plan.source == request.relative_path
            && !matches!(item.state, FileCommandPreflightState::Blocked(_)))
        .then(|| item.clone())
    }

    async fn execute_rename_group(
        &self,
        batch_id: BatchId,
        entity_id: EntityId,
        cancellation: &FileCommandCancellation,
    ) -> LocalFileCommandOutcome {
        let (already, plan) = {
            let batches = self.lock_batches();
            let Some(batch) = batches.get(&batch_id) else {
                return LocalFileCommandOutcome::failed(BatchResultCode::BackendUnavailable);
            };
            (
                batch.outcomes.get(&entity_id).copied(),
                (!batch.rename_executed)
                    .then(|| batch.rename_plan.clone())
                    .flatten(),
            )
        };
        if let Some(outcome) = already {
            return outcome;
        }
        let Some(plan) = plan else {
            return LocalFileCommandOutcome::failed(BatchResultCode::BackendUnavailable);
        };
        let (items_by_operation, entity_by_operation) = {
            let batches = self.lock_batches();
            let items = batches[&batch_id]
                .items
                .iter()
                .filter(|(_, item)| item.route == PreparedRoute::RenameGroup);
            (
                items
                    .clone()
                    .map(|(_, item)| (item.plan.operation_id, item.clone()))
                    .collect::<HashMap<_, _>>(),
                items
                    .map(|(entity, item)| (item.plan.operation_id, *entity))
                    .collect::<HashMap<_, _>>(),
            )
        };
        let executor = match RenameExecutor::new(
            &self.project_root,
            Arc::clone(&self.journal),
            Arc::clone(&self.mutation),
            Arc::clone(&self.clock),
            Arc::clone(&self.commits),
        ) {
            Ok(executor) => executor,
            Err(error) => return self.fail_outcome_for_entity(batch_id, entity_id, &error),
        };
        let mut outcomes = HashMap::new();
        for component in plan.independent_components() {
            if cancellation.is_cancelled() {
                break;
            }
            let component_operations = component
                .stages
                .iter()
                .map(|stage| stage.operation_id())
                .collect::<HashSet<_>>();
            let component_items = component_operations
                .iter()
                .filter_map(|operation_id| items_by_operation.get(operation_id))
                .collect::<Vec<_>>();
            let mut validation_error = None;
            for item in &component_items {
                if let Err(error) = self.revalidate_source(item) {
                    validation_error = Some(error);
                    break;
                }
                let source = self.project_root.join(item.plan.source.as_str());
                let Some(destination) = item.plan.destination.as_ref() else {
                    continue;
                };
                let destination = self.project_root.join(destination.as_str());
                if let Err(error) = self
                    .register_expected_change(
                        item.plan.operation_id,
                        &source,
                        &destination,
                        &source,
                        item.source_evidence.as_ref(),
                    )
                    .await
                {
                    validation_error = Some(error);
                    break;
                }
            }
            if let Some(error) = validation_error {
                let code = classify_message(&error.to_string());
                for item in component_items {
                    self.mark_failed(item.plan.operation_id, code);
                    if let Some(entity) = entity_by_operation.get(&item.plan.operation_id) {
                        outcomes.insert(*entity, LocalFileCommandOutcome::failed(code));
                    }
                }
                continue;
            }
            let expected = component_items
                .iter()
                .filter_map(|item| {
                    item.source_evidence
                        .clone()
                        .map(|evidence| (item.plan.operation_id, evidence))
                })
                .collect::<HashMap<_, _>>();
            let parents = component_items
                .iter()
                .filter_map(|item| {
                    Some((
                        item.plan.operation_id,
                        RenameParentIdentities {
                            source: item.source_parent_identity?,
                            destination: item.destination_parent_identity?,
                        },
                    ))
                })
                .collect::<HashMap<_, _>>();
            let result = executor
                .execute_with_bound_expectations(&component, &expected, &parents)
                .await;
            for result in result.items {
                let Some(entity) = entity_by_operation.get(&result.operation_id).copied() else {
                    continue;
                };
                let outcome = match result.status {
                    RenameItemStatus::Completed => {
                        LocalFileCommandOutcome::completed(BatchResultCode::Renamed)
                    }
                    RenameItemStatus::Failed(message) => {
                        let code = classify_message(&message);
                        self.mark_failed(result.operation_id, code);
                        LocalFileCommandOutcome::failed(code)
                    }
                };
                outcomes.insert(entity, outcome);
            }
        }
        let mut batches = self.lock_batches();
        let batch = batches
            .get_mut(&batch_id)
            .expect("batch remains registered");
        batch.rename_executed = true;
        batch.outcomes.extend(outcomes);
        batch
            .outcomes
            .get(&entity_id)
            .copied()
            .unwrap_or(LocalFileCommandOutcome::failed(
                BatchResultCode::BackendUnavailable,
            ))
    }

    async fn execute_single_rename(
        &self,
        mut item: PreparedItem,
        policy: ConflictPolicy,
    ) -> Result<LocalFileCommandOutcome, FileOperationError> {
        item.plan.conflict_policy = policy;
        let requested = item
            .plan
            .destination
            .clone()
            .ok_or(FileOperationError::DestinationRequired)?;
        if policy == ConflictPolicy::KeepBoth {
            let path = keep_both_destination(&self.project_root.join(requested.as_str()))?;
            item.plan.destination = Some(self.relative_path(&path)?);
            self.journal
                .set_destination(
                    item.plan.operation_id,
                    item.plan
                        .destination
                        .as_ref()
                        .expect("keep-both destination"),
                    self.now(),
                )
                .map_err(journal_file_error)?;
        }
        let source = self.project_root.join(item.plan.source.as_str());
        let destination = self.project_root.join(
            item.plan
                .destination
                .as_ref()
                .ok_or(FileOperationError::DestinationRequired)?
                .as_str(),
        );
        self.register_expected_change(
            item.plan.operation_id,
            &source,
            &destination,
            &source,
            item.source_evidence.as_ref(),
        )
        .await?;
        if policy == ConflictPolicy::Replace && destination.exists() {
            self.register_expected_change(
                item.plan.operation_id,
                &destination,
                &destination,
                &destination,
                item.destination_evidence
                    .as_ref()
                    .map(|evidence| &evidence.snapshot),
            )
            .await?;
        }
        if policy == ConflictPolicy::Replace {
            self.revalidate_replace_destination(&item, Some(policy))?;
            let expected_destination = item
                .destination_evidence
                .as_ref()
                .ok_or(FileOperationError::IdentityChanged)?;
            let executor = ReplaceExecutor::new(
                &self.project_root,
                Arc::clone(&self.journal),
                Arc::clone(&self.mutation),
                Arc::clone(&self.trash),
                Arc::clone(&self.clock),
                Arc::clone(&self.commits),
            )?;
            executor
                .execute_with_bound_evidence(
                    &item.plan,
                    expected_destination,
                    item.source_evidence
                        .as_ref()
                        .ok_or(FileOperationError::IdentityChanged)?,
                    item.source_parent_identity
                        .ok_or(FileOperationError::IdentityChanged)?,
                    item.destination_parent_identity
                        .ok_or(FileOperationError::IdentityChanged)?,
                )
                .await
                .map_err(replace_file_error)?;
        } else {
            let mapping = RenameMapping {
                operation_id: item.plan.operation_id,
                entity_id: item.plan.entity_id,
                source: PathBuf::from(item.plan.source.as_str()),
                destination: PathBuf::from(
                    item.plan
                        .destination
                        .as_ref()
                        .expect("rename destination")
                        .as_str(),
                ),
            };
            let plan =
                RenamePlanner::plan(&self.project_root, self.case_sensitive_root(), &[mapping])
                    .map_err(|_| FileOperationError::DestinationExists)?;
            let executor = RenameExecutor::new(
                &self.project_root,
                Arc::clone(&self.journal),
                Arc::clone(&self.mutation),
                Arc::clone(&self.clock),
                Arc::clone(&self.commits),
            )?;
            let expected = item
                .source_evidence
                .clone()
                .map(|evidence| HashMap::from([(item.plan.operation_id, evidence)]))
                .unwrap_or_default();
            let parents = item
                .source_parent_identity
                .zip(item.destination_parent_identity)
                .map(|(source, destination)| {
                    HashMap::from([(
                        item.plan.operation_id,
                        RenameParentIdentities {
                            source,
                            destination,
                        },
                    )])
                })
                .unwrap_or_default();
            let result = executor
                .execute_with_bound_expectations(&plan, &expected, &parents)
                .await;
            if !matches!(
                result.items.first().map(|item| &item.status),
                Some(RenameItemStatus::Completed)
            ) {
                return Err(FileOperationError::Io {
                    action: "execute rename",
                    path: self.project_root.join(item.plan.source.as_str()),
                    message: result
                        .items
                        .first()
                        .and_then(|item| match &item.status {
                            RenameItemStatus::Failed(message) => Some(message.clone()),
                            RenameItemStatus::Completed => None,
                        })
                        .unwrap_or_else(|| "missing rename result".into()),
                });
            }
        }
        Ok(LocalFileCommandOutcome::completed(match item.plan.kind {
            OperationKind::Rename => BatchResultCode::Renamed,
            OperationKind::Move => BatchResultCode::Moved,
            _ => BatchResultCode::BackendUnavailable,
        }))
    }

    async fn execute_copy_route(
        &self,
        mut item: PreparedItem,
        policy: Option<ConflictPolicy>,
        cancellation: &FileCommandCancellation,
    ) -> Result<LocalFileCommandOutcome, FileOperationError> {
        let requested = item
            .plan
            .destination
            .clone()
            .ok_or(FileOperationError::DestinationRequired)?;
        if policy == Some(ConflictPolicy::KeepBoth) {
            let path = keep_both_destination(&self.project_root.join(requested.as_str()))?;
            let destination = self.relative_path(&path)?;
            self.journal
                .set_destination(item.plan.operation_id, &destination, self.now())
                .map_err(journal_file_error)?;
            item.plan.destination = Some(destination);
        }
        let destination = item.plan.destination.as_ref().expect("copy destination");
        let source_path = self.project_root.join(item.plan.source.as_str());
        self.validate_regular_source(&source_path)?;
        let destination_path = self.project_root.join(destination.as_str());
        if source_path == destination_path && policy == Some(ConflictPolicy::Replace) {
            return Err(FileOperationError::OutsideProject);
        }
        if destination_path.exists() && policy != Some(ConflictPolicy::Replace) {
            return Err(FileOperationError::DestinationExists);
        }
        let temporary = temporary_for(destination, item.plan.operation_id)?;
        let temporary_path = self.project_root.join(temporary.as_str());
        let source_parent_identity = item
            .source_parent_identity
            .ok_or(FileOperationError::IdentityChanged)?;
        let destination_parent_identity = item
            .destination_parent_identity
            .ok_or(FileOperationError::IdentityChanged)?;
        self.journal
            .register_temporary(
                item.plan.operation_id,
                OperationState::Prepared,
                &temporary,
                self.now(),
            )
            .map_err(journal_file_error)?;
        let before = item
            .source_evidence
            .clone()
            .ok_or(FileOperationError::IdentityChanged)?;
        let staged = match Arc::clone(&self.mutation)
            .create_staged_copy_cancellable_verified(
                &source_path,
                &temporary_path,
                cancellation,
                &before,
                source_parent_identity,
                destination_parent_identity,
            )
            .await
        {
            Ok(staged) => staged,
            Err(error) => return Err(error),
        };
        let copied = staged.evidence().clone();
        self.journal
            .advance(
                item.plan.operation_id,
                OperationState::Prepared,
                OperationState::Staged,
                self.now(),
            )
            .map_err(journal_file_error)?;
        let after = FileContentEvidence {
            snapshot: before.clone(),
            hash: copied.hash,
        };
        self.journal
            .record_fs_applied(
                item.plan.operation_id,
                OperationState::Staged,
                copied.snapshot.len,
                copied.hash,
                self.now(),
            )
            .map_err(journal_file_error)?;
        self.journal
            .advance(
                item.plan.operation_id,
                OperationState::FsApplied,
                OperationState::Verified,
                self.now(),
            )
            .map_err(journal_file_error)?;

        self.register_expected_change_for_snapshot(
            item.plan.operation_id,
            &destination_path,
            &destination_path,
            &copied.snapshot,
        )?;
        if item.route == PreparedRoute::CrossVolumeMove {
            self.register_expected_change(
                item.plan.operation_id,
                &source_path,
                &source_path,
                &source_path,
                Some(&after.snapshot),
            )
            .await?;
        }

        if policy == Some(ConflictPolicy::Replace) && destination_path.exists() {
            self.revalidate_replace_destination(&item, policy)?;
            self.validate_regular_source(&destination_path)?;
            self.register_expected_change(
                item.plan.operation_id,
                &destination_path,
                &destination_path,
                &destination_path,
                item.destination_evidence
                    .as_ref()
                    .map(|evidence| &evidence.snapshot),
            )
            .await?;
            self.trash
                .trash_verified(
                    &destination_path,
                    &item
                        .destination_evidence
                        .as_ref()
                        .ok_or(FileOperationError::IdentityChanged)?
                        .snapshot,
                    destination_parent_identity,
                )
                .await?;
        }
        if destination_path.exists() {
            return Err(FileOperationError::DestinationExists);
        }
        staged
            .place(&destination_path, destination_parent_identity)
            .await?;

        if item.route == PreparedRoute::CrossVolumeMove {
            let before_trash = stable_content_evidence_async(&source_path).await?;
            if before_trash != after {
                let _ = self
                    .trash
                    .trash_verified(
                        &destination_path,
                        &copied.snapshot,
                        destination_parent_identity,
                    )
                    .await;
                return Err(FileOperationError::IdentityChanged);
            }
            let source_temporary = trash_temporary_for(&item.plan.source, item.plan.operation_id)?;
            let source_temporary_path = self.project_root.join(source_temporary.as_str());
            self.journal
                .register_temporary(
                    item.plan.operation_id,
                    OperationState::Verified,
                    &source_temporary,
                    self.now(),
                )
                .map_err(journal_file_error)?;
            if let Err(error) = self
                .mutation
                .rename_verified(
                    &source_path,
                    &source_temporary_path,
                    &after.snapshot,
                    source_parent_identity,
                    source_parent_identity,
                )
                .await
            {
                let _ = self
                    .trash
                    .trash_verified(
                        &destination_path,
                        &copied.snapshot,
                        destination_parent_identity,
                    )
                    .await;
                return Err(error);
            }
            if !same_staged_content_evidence(
                &after,
                &stable_content_evidence_async(&source_temporary_path).await?,
            ) {
                if !source_path.exists() {
                    let actual = self.mutation.snapshot(&source_temporary_path).await?;
                    let _ = self
                        .mutation
                        .rename_verified(
                            &source_temporary_path,
                            &source_path,
                            &actual,
                            source_parent_identity,
                            source_parent_identity,
                        )
                        .await;
                }
                let _ = self
                    .trash
                    .trash_verified(
                        &destination_path,
                        &copied.snapshot,
                        destination_parent_identity,
                    )
                    .await;
                return Err(FileOperationError::IdentityChanged);
            }
            if let Err(error) = self
                .trash
                .trash_verified(
                    &source_temporary_path,
                    &after.snapshot,
                    source_parent_identity,
                )
                .await
            {
                let _ = self
                    .trash
                    .trash_verified(
                        &destination_path,
                        &copied.snapshot,
                        destination_parent_identity,
                    )
                    .await;
                return Err(error);
            }
            if source_path.exists() || source_temporary_path.exists() {
                return Err(FileOperationError::VerificationFailed);
            }
        }
        self.finish_commit(&item.plan).await?;
        Ok(LocalFileCommandOutcome::completed(
            if item.plan.kind == OperationKind::Copy {
                BatchResultCode::Copied
            } else {
                BatchResultCode::Moved
            },
        ))
    }

    async fn execute_trash(
        &self,
        item: &PreparedItem,
    ) -> Result<LocalFileCommandOutcome, FileOperationError> {
        let source = self.project_root.join(item.plan.source.as_str());
        self.validate_regular_source(&source)?;
        let evidence = stable_content_evidence_async(&source).await?;
        if item
            .source_evidence
            .as_ref()
            .is_none_or(|expected| *expected != evidence.snapshot)
        {
            return Err(FileOperationError::IdentityChanged);
        }
        let parent_identity = item
            .source_parent_identity
            .ok_or(FileOperationError::IdentityChanged)?;
        let temporary = trash_temporary_for(&item.plan.source, item.plan.operation_id)?;
        let temporary_path = self.project_root.join(temporary.as_str());
        let before = evidence.snapshot.clone();
        let hash = evidence.hash;
        self.journal
            .record_prepared_evidence(
                item.plan.operation_id,
                Some(&temporary),
                before.len,
                hash,
                self.now(),
            )
            .map_err(journal_file_error)?;
        self.register_expected_change(
            item.plan.operation_id,
            &source,
            &source,
            &source,
            Some(&evidence.snapshot),
        )
        .await?;
        self.mutation
            .rename_verified(
                &source,
                &temporary_path,
                &before,
                parent_identity,
                parent_identity,
            )
            .await?;
        if !same_staged_content_evidence(
            &evidence,
            &stable_content_evidence_async(&temporary_path).await?,
        ) {
            if !source.exists() {
                let actual = self.mutation.snapshot(&temporary_path).await?;
                let _ = self
                    .mutation
                    .rename_verified(
                        &temporary_path,
                        &source,
                        &actual,
                        parent_identity,
                        parent_identity,
                    )
                    .await;
            }
            return Err(FileOperationError::IdentityChanged);
        }
        self.journal
            .advance(
                item.plan.operation_id,
                OperationState::Prepared,
                OperationState::Staged,
                self.now(),
            )
            .map_err(journal_file_error)?;
        self.trash
            .trash_verified(&temporary_path, &before, parent_identity)
            .await?;
        if temporary_path.exists() || source.exists() {
            return Err(FileOperationError::VerificationFailed);
        }
        self.journal
            .record_fs_applied(
                item.plan.operation_id,
                OperationState::Staged,
                before.len,
                hash,
                self.now(),
            )
            .map_err(journal_file_error)?;
        self.journal
            .advance(
                item.plan.operation_id,
                OperationState::FsApplied,
                OperationState::Verified,
                self.now(),
            )
            .map_err(journal_file_error)?;
        self.finish_commit(&item.plan).await?;
        Ok(LocalFileCommandOutcome::completed(
            BatchResultCode::MovedToTrash,
        ))
    }

    async fn finish_commit(&self, item: &OperationItemPlan) -> Result<(), FileOperationError> {
        let persisted = self
            .journal
            .item(item.operation_id)
            .map_err(journal_file_error)?
            .ok_or(FileOperationError::SourceMissing)?;
        let commit = OperationCommit {
            operation_id: item.operation_id,
            entity_id: item.entity_id,
            kind: item.kind,
            source: item.source.clone(),
            destination: persisted.destination,
        };
        let outcome = self
            .commits
            .commit_metadata_barrier(&commit)
            .await
            .map_err(commit_file_error)?;
        if outcome == viewer_application::MetadataCommitOutcome::CallerAdvancesJournal {
            self.journal
                .advance(
                    item.operation_id,
                    OperationState::Verified,
                    OperationState::MetaCommitted,
                    self.now(),
                )
                .map_err(journal_file_error)?;
        }
        self.commits
            .sync_index(&commit)
            .await
            .map_err(commit_file_error)?;
        self.journal
            .advance(
                item.operation_id,
                OperationState::MetaCommitted,
                OperationState::IndexSynced,
                self.now(),
            )
            .map_err(journal_file_error)?;
        self.journal
            .complete_item(
                item.operation_id,
                OperationState::IndexSynced,
                "completed",
                self.now(),
            )
            .map_err(journal_file_error)
    }

    async fn register_expected_change(
        &self,
        operation_id: OperationId,
        old_path: &Path,
        new_path: &Path,
        identity_path: &Path,
        expected_identity: Option<&FileSnapshot>,
    ) -> Result<(), FileOperationError> {
        let snapshot = self.mutation.snapshot(identity_path).await?;
        if expected_identity.is_some_and(|expected| *expected != snapshot) {
            return Err(FileOperationError::IdentityChanged);
        }
        let file = snapshot
            .file_id
            .and_then(|file| u64::try_from(file).ok())
            .ok_or(FileOperationError::IdentityChanged)?;
        self.expected_changes.register(ExpectedChange {
            operation_id,
            old_canonical_path: old_path.to_path_buf(),
            new_canonical_path: new_path.to_path_buf(),
            expected_identity: FileIdentity {
                volume: snapshot.volume_id,
                file,
            },
            expires_at_ms: self.now_u64().saturating_add(EXPECTED_CHANGE_TTL_MS),
        });
        Ok(())
    }

    fn register_expected_change_for_snapshot(
        &self,
        operation_id: OperationId,
        old_path: &Path,
        new_path: &Path,
        snapshot: &FileSnapshot,
    ) -> Result<(), FileOperationError> {
        let file = snapshot
            .file_id
            .and_then(|file| u64::try_from(file).ok())
            .ok_or(FileOperationError::IdentityChanged)?;
        self.expected_changes.register(ExpectedChange {
            operation_id,
            old_canonical_path: old_path.to_path_buf(),
            new_canonical_path: new_path.to_path_buf(),
            expected_identity: FileIdentity {
                volume: snapshot.volume_id,
                file,
            },
            expires_at_ms: self.now_u64().saturating_add(EXPECTED_CHANGE_TTL_MS),
        });
        Ok(())
    }

    fn validate_regular_source(&self, path: &Path) -> Result<(), FileOperationError> {
        let metadata = std::fs::symlink_metadata(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                FileOperationError::SourceMissing
            } else {
                FileOperationError::io("revalidate file command source", path, &error)
            }
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(FileOperationError::OutsideProject);
        }
        let canonical = std::fs::canonicalize(path).map_err(|error| {
            FileOperationError::io("canonicalize file command source", path, &error)
        })?;
        if !canonical.starts_with(&self.project_root) {
            return Err(FileOperationError::OutsideProject);
        }
        Ok(())
    }

    fn relative_path(&self, path: &Path) -> Result<RelativePath, FileOperationError> {
        let relative = path
            .strip_prefix(&self.project_root)
            .map_err(|_| FileOperationError::OutsideProject)?
            .to_str()
            .ok_or(FileOperationError::OutsideProject)?;
        RelativePath::parse(relative).map_err(|_| FileOperationError::ReservedPath)
    }

    fn mark_failed(&self, operation_id: OperationId, code: BatchResultCode) {
        let Ok(Some(item)) = self.journal.item(operation_id) else {
            return;
        };
        let safe_to_terminalize = match item.state {
            OperationState::Prepared => {
                let source_exists = self.project_root.join(item.source.as_str()).is_file();
                let temporary_exists = item
                    .temporary
                    .as_ref()
                    .is_some_and(|path| self.project_root.join(path.as_str()).exists());
                source_exists && !temporary_exists
            }
            OperationState::Staged => {
                let source_exists = self.project_root.join(item.source.as_str()).is_file();
                let destination_exists = item
                    .destination
                    .as_ref()
                    .is_some_and(|path| self.project_root.join(path.as_str()).exists());
                let temporary_exists = item
                    .temporary
                    .as_ref()
                    .is_some_and(|path| self.project_root.join(path.as_str()).exists());
                source_exists && !destination_exists && !temporary_exists
            }
            OperationState::FsApplied
            | OperationState::Verified
            | OperationState::MetaCommitted
            | OperationState::IndexSynced
            | OperationState::Completed
            | OperationState::Failed => false,
        };
        if safe_to_terminalize {
            let _ = self
                .journal
                .fail_item(operation_id, item.state, code.as_str(), self.now());
        }
    }

    fn mark_cancelled(&self, operation_id: OperationId) {
        let Ok(Some(item)) = self.journal.item(operation_id) else {
            return;
        };
        if !matches!(
            item.state,
            OperationState::Completed | OperationState::Failed
        ) {
            let _ = self.journal.skip_item(
                operation_id,
                item.state,
                BatchResultCode::Cancelled.as_str(),
                self.now(),
            );
        }
    }

    fn settle_recovery_required(
        &self,
        operation_id: OperationId,
        primary: &FileOperationError,
    ) -> Result<LocalFileCommandOutcome, LocalFileCommandError> {
        let code = classify_file_error(primary);
        let item = self
            .journal
            .item(operation_id)
            .map_err(|_| LocalFileCommandError::Unavailable)?
            .ok_or(LocalFileCommandError::Unavailable)?;
        if matches!(primary.primary(), FileOperationError::Cancelled) {
            self.journal
                .skip_item_recovery_required(
                    operation_id,
                    item.state,
                    BatchResultCode::Cancelled.as_str(),
                    self.now(),
                )
                .map_err(|_| LocalFileCommandError::Unavailable)?;
            Ok(LocalFileCommandOutcome::cancelled(
                BatchResultCode::Cancelled,
            ))
        } else {
            self.journal
                .fail_item_recovery_required(operation_id, item.state, code.as_str(), self.now())
                .map_err(|_| LocalFileCommandError::Unavailable)?;
            Ok(LocalFileCommandOutcome::failed(code))
        }
    }

    fn fail_outcome_for_entity(
        &self,
        batch_id: BatchId,
        entity_id: EntityId,
        error: &FileOperationError,
    ) -> LocalFileCommandOutcome {
        let code = classify_file_error(error);
        if let Some(item) = self
            .lock_batches()
            .get(&batch_id)
            .and_then(|batch| batch.items.get(&entity_id))
        {
            self.mark_failed(item.plan.operation_id, code);
        }
        LocalFileCommandOutcome::failed(code)
    }

    fn maybe_finish_batch(&self, batch_id: BatchId) {
        let should_finish = self
            .journal
            .batch(batch_id)
            .ok()
            .flatten()
            .is_some_and(|batch| {
                batch.completed_count + batch.failed_count + batch.skipped_count
                    == batch.requested_count
            });
        if !should_finish {
            return;
        }
        let mut batches = self.lock_batches();
        let Some(batch) = batches.get_mut(&batch_id) else {
            return;
        };
        if !batch.journal_finished && self.journal.finish_batch(batch_id, self.now()).is_ok() {
            batch.journal_finished = true;
        }
    }

    fn mark_delivered(&self, batch_id: BatchId, entity_id: EntityId) {
        let mut batches = self.lock_batches();
        let should_remove = batches.get_mut(&batch_id).is_some_and(|batch| {
            batch.delivered.insert(entity_id);
            batch.journal_finished
                && batch.delivered.len() == batch.plan.items.len()
                && !matches!(batch.plan.kind, OperationKind::Rename | OperationKind::Move)
        });
        if should_remove {
            batches.remove(&batch_id);
        }
    }

    fn now(&self) -> i64 {
        self.clock.unix_millis()
    }

    fn now_u64(&self) -> u64 {
        u64::try_from(self.now()).unwrap_or_default()
    }

    fn lock_batches(&self) -> std::sync::MutexGuard<'_, HashMap<BatchId, PreparedBatch>> {
        self.batches
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[async_trait]
impl LocalFileCommandPort for LocalFileCommandAdapter {
    async fn preflight(
        &self,
        batch_id: BatchId,
        command: &FileCommand,
    ) -> Result<Vec<LocalFileCommandPreflightItem>, LocalFileCommandError> {
        let batch = self.prepare(batch_id, command);
        let rows = command
            .items
            .iter()
            .map(|item| {
                let prepared = batch
                    .items
                    .get(&item.entity_id)
                    .expect("every command item has a prepared row");
                LocalFileCommandPreflightItem {
                    entity_id: item.entity_id,
                    relative_path: prepared.plan.source.clone(),
                    state: prepared.state,
                }
            })
            .collect::<Vec<_>>();
        if rows
            .iter()
            .all(|row| !matches!(row.state, FileCommandPreflightState::Blocked(_)))
        {
            self.lock_batches().insert(batch_id, batch);
        }
        Ok(rows)
    }

    async fn discard_preflight(&self, batch_id: BatchId) -> Result<(), LocalFileCommandError> {
        let mut batches = self.lock_batches();
        match batches.get(&batch_id) {
            Some(batch) if batch.journal_started => Err(LocalFileCommandError::Unavailable),
            Some(_) => {
                batches.remove(&batch_id);
                Ok(())
            }
            None => Ok(()),
        }
    }

    async fn execute_item(
        &self,
        request: FileCommandItemExecution,
        cancellation: FileCommandCancellation,
    ) -> Result<LocalFileCommandOutcome, LocalFileCommandError> {
        let Some(item) = self.prepared_item(&request) else {
            return Err(LocalFileCommandError::Unavailable);
        };
        self.ensure_journal_started(request.batch_id)?;
        if let Some(policy) = request.conflict_policy {
            self.journal
                .set_conflict_policy(item.plan.operation_id, policy, self.now())
                .map_err(|_| LocalFileCommandError::Unavailable)?;
        }
        if cancellation.is_cancelled() {
            self.mark_cancelled(item.plan.operation_id);
            self.maybe_finish_batch(request.batch_id);
            self.mark_delivered(request.batch_id, request.item.entity_id);
            return Ok(LocalFileCommandOutcome::cancelled(
                BatchResultCode::Cancelled,
            ));
        }
        if item.route != PreparedRoute::RenameGroup
            && let Err(error) = self
                .revalidate_source(&item)
                .and_then(|()| self.revalidate_replace_destination(&item, request.conflict_policy))
                .and_then(|()| self.revalidate_move_route(&item))
                .and_then(|()| self.revalidate_destination_entity(&request, &item))
        {
            let outcome =
                self.fail_outcome_for_entity(request.batch_id, request.item.entity_id, &error);
            self.maybe_finish_batch(request.batch_id);
            self.mark_delivered(request.batch_id, request.item.entity_id);
            return Ok(outcome);
        }
        let outcome = match item.route {
            PreparedRoute::RenameGroup => {
                self.execute_rename_group(request.batch_id, request.item.entity_id, &cancellation)
                    .await
            }
            PreparedRoute::RenameConflict => match self
                .execute_single_rename(
                    item.clone(),
                    request.conflict_policy.unwrap_or(ConflictPolicy::Skip),
                )
                .await
            {
                Ok(outcome) => outcome,
                Err(error) => {
                    self.fail_outcome_for_entity(request.batch_id, request.item.entity_id, &error)
                }
            },
            PreparedRoute::AtomicMove => match self
                .execute_single_rename(
                    item.clone(),
                    request.conflict_policy.unwrap_or(ConflictPolicy::Skip),
                )
                .await
            {
                Ok(outcome) => outcome,
                Err(error) => {
                    self.fail_outcome_for_entity(request.batch_id, request.item.entity_id, &error)
                }
            },
            PreparedRoute::Copy | PreparedRoute::CrossVolumeMove => match self
                .execute_copy_route(item.clone(), request.conflict_policy, &cancellation)
                .await
            {
                Ok(outcome) => outcome,
                Err(FileOperationError::RegisteredTemporaryCleanupRequired { primary, .. }) => {
                    self.settle_recovery_required(item.plan.operation_id, &primary)?
                }
                Err(FileOperationError::Cancelled) => {
                    self.mark_cancelled(item.plan.operation_id);
                    LocalFileCommandOutcome::cancelled(BatchResultCode::Cancelled)
                }
                Err(error) => {
                    self.fail_outcome_for_entity(request.batch_id, request.item.entity_id, &error)
                }
            },
            PreparedRoute::Trash => match self.execute_trash(&item).await {
                Ok(outcome) => outcome,
                Err(error) => {
                    self.fail_outcome_for_entity(request.batch_id, request.item.entity_id, &error)
                }
            },
        };
        self.maybe_finish_batch(request.batch_id);
        self.mark_delivered(request.batch_id, request.item.entity_id);
        Ok(outcome)
    }

    async fn settle_unstarted(
        &self,
        request: FileCommandItemExecution,
        outcome: LocalFileCommandOutcome,
    ) -> Result<LocalFileCommandOutcome, LocalFileCommandError> {
        let item = self
            .prepared_item(&request)
            .ok_or(LocalFileCommandError::Unavailable)?;
        self.ensure_journal_started(request.batch_id)?;
        if let Some(policy) = request.conflict_policy {
            self.journal
                .set_conflict_policy(item.plan.operation_id, policy, self.now())
                .map_err(|_| LocalFileCommandError::Unavailable)?;
        }
        self.journal
            .skip_item(
                item.plan.operation_id,
                OperationState::Prepared,
                outcome.code().as_str(),
                self.now(),
            )
            .map_err(|_| LocalFileCommandError::Unavailable)?;
        self.maybe_finish_batch(request.batch_id);
        self.mark_delivered(request.batch_id, request.item.entity_id);
        Ok(outcome)
    }

    async fn take_undo_actions(
        &self,
        batch_id: BatchId,
    ) -> Result<Vec<UndoAction>, LocalFileCommandError> {
        let batch = {
            let mut batches = self.lock_batches();
            if !batches
                .get(&batch_id)
                .is_some_and(|batch| batch.journal_finished)
            {
                return Ok(Vec::new());
            }
            batches
                .remove(&batch_id)
                .ok_or(LocalFileCommandError::Unavailable)?
        };
        if !matches!(batch.plan.kind, OperationKind::Rename | OperationKind::Move) {
            return Ok(Vec::new());
        }
        let mut actions = Vec::new();
        for planned in &batch.plan.items {
            let persisted = self
                .journal
                .item(planned.operation_id)
                .map_err(|_| LocalFileCommandError::Unavailable)?;
            let Some(persisted) = persisted.filter(|item| item.state == OperationState::Completed)
            else {
                continue;
            };
            let Some(current) = persisted.destination else {
                continue;
            };
            let expected = self
                .mutation
                .snapshot(&self.project_root.join(current.as_str()))
                .await
                .map_err(|_| LocalFileCommandError::Unavailable)?;
            let current_entity_id = self
                .index
                .node_by_relative_path(&current)
                .map_err(|_| LocalFileCommandError::Unavailable)?
                .map(|node| node.entity_id)
                .ok_or(LocalFileCommandError::Unavailable)?;
            actions.push(UndoAction::File {
                entity_id: current_entity_id,
                current,
                restore: persisted.source,
                expected,
            });
        }
        Ok(actions)
    }
}

#[async_trait]
impl UndoFilePort for LocalFileCommandAdapter {
    async fn reverse_batch(&self, project_root: &Path, batch: &UndoBatch) -> Result<(), UndoError> {
        let root = std::fs::canonicalize(project_root).map_err(|error| {
            FileOperationError::io("canonicalize inverse operation root", project_root, &error)
        })?;
        if root != self.project_root
            || !matches!(batch.kind, OperationKind::Rename | OperationKind::Move)
        {
            return Err(UndoError::OutsideProject);
        }
        let inverse_batch_id = OperationId::new();
        let mut prepared = Vec::with_capacity(batch.actions.len());
        for action in &batch.actions {
            let UndoAction::File {
                entity_id,
                current,
                restore,
                expected,
            } = action
            else {
                return Err(UndoError::OutsideProject);
            };
            let source = root.join(current.as_str());
            if self.mutation.snapshot(&source).await? != *expected {
                return Err(UndoError::IdentityChanged);
            }
            let destination = root.join(restore.as_str());
            let destination_parent = destination
                .parent()
                .and_then(|parent| std::fs::canonicalize(parent).ok())
                .ok_or(UndoError::OutsideProject)?;
            if !destination_parent.starts_with(&root) {
                return Err(UndoError::OutsideProject);
            }
            let route = if batch.kind == OperationKind::Rename {
                PreparedRoute::RenameGroup
            } else if self.volume.volume_id(&source)?
                == self.volume.volume_id(&destination_parent)?
            {
                PreparedRoute::AtomicMove
            } else {
                PreparedRoute::CrossVolumeMove
            };
            prepared.push(PreparedItem {
                plan: OperationItemPlan {
                    batch_id: inverse_batch_id,
                    operation_id: OperationId::new(),
                    entity_id: *entity_id,
                    kind: batch.kind,
                    source: current.clone(),
                    destination: Some(restore.clone()),
                    conflict_policy: ConflictPolicy::Skip,
                },
                state: FileCommandPreflightState::Ready,
                route,
                source_evidence: Some(expected.clone()),
                destination_evidence: None,
                source_parent_identity: source
                    .parent()
                    .and_then(|parent| directory_identity_sync(parent).ok()),
                destination_parent_identity: Some(directory_identity_sync(&destination_parent)?),
            });
        }
        let plan = OperationPlan {
            batch_id: inverse_batch_id,
            kind: batch.kind,
            items: prepared.iter().map(|item| item.plan.clone()).collect(),
        };
        self.journal
            .begin_plan(&plan, self.now())
            .map_err(journal_file_error)?;

        let atomic = prepared
            .iter()
            .filter(|item| item.route != PreparedRoute::CrossVolumeMove)
            .collect::<Vec<_>>();
        if !atomic.is_empty() {
            let mappings = atomic
                .iter()
                .map(|item| RenameMapping {
                    operation_id: item.plan.operation_id,
                    entity_id: item.plan.entity_id,
                    source: PathBuf::from(item.plan.source.as_str()),
                    destination: PathBuf::from(
                        item.plan
                            .destination
                            .as_ref()
                            .expect("inverse file action has a destination")
                            .as_str(),
                    ),
                })
                .collect::<Vec<_>>();
            let rename_plan = RenamePlanner::plan(&root, self.case_sensitive_root(), &mappings)
                .map_err(|_| UndoError::DestinationOccupied)?;
            for item in &atomic {
                let source = root.join(item.plan.source.as_str());
                let destination = root.join(
                    item.plan
                        .destination
                        .as_ref()
                        .expect("inverse destination")
                        .as_str(),
                );
                self.register_expected_change(
                    item.plan.operation_id,
                    &source,
                    &destination,
                    &source,
                    item.source_evidence.as_ref(),
                )
                .await?;
            }
            let executor = RenameExecutor::new(
                &root,
                Arc::clone(&self.journal),
                Arc::clone(&self.mutation),
                Arc::clone(&self.clock),
                Arc::clone(&self.commits),
            )?;
            let expected = atomic
                .iter()
                .filter_map(|item| {
                    item.source_evidence
                        .clone()
                        .map(|evidence| (item.plan.operation_id, evidence))
                })
                .collect::<HashMap<_, _>>();
            let parents = atomic
                .iter()
                .filter_map(|item| {
                    Some((
                        item.plan.operation_id,
                        RenameParentIdentities {
                            source: item.source_parent_identity?,
                            destination: item.destination_parent_identity?,
                        },
                    ))
                })
                .collect::<HashMap<_, _>>();
            let result = executor
                .execute_with_bound_expectations(&rename_plan, &expected, &parents)
                .await;
            if let Some(message) = result.items.iter().find_map(|item| match &item.status {
                RenameItemStatus::Completed => None,
                RenameItemStatus::Failed(message) => Some(message.clone()),
            }) {
                return Err(FileOperationError::Io {
                    action: "execute inverse rename",
                    path: root,
                    message,
                }
                .into());
            }
        }
        for item in prepared
            .iter()
            .filter(|item| item.route == PreparedRoute::CrossVolumeMove)
        {
            self.execute_copy_route(item.clone(), None, &FileCommandCancellation::default())
                .await?;
        }
        self.journal
            .finish_batch(inverse_batch_id, self.now())
            .map_err(journal_file_error)?;
        Ok(())
    }
}

fn supported_regular_kind(kind: FileKind) -> bool {
    matches!(
        kind,
        FileKind::Jpeg | FileKind::Png | FileKind::Markdown | FileKind::Text
    )
}

fn snapshot_sync(path: &Path) -> Result<FileSnapshot, FileOperationError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            FileOperationError::SourceMissing
        } else {
            FileOperationError::io("read preflight file identity", path, &error)
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(FileOperationError::OutsideProject);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(FileSnapshot {
            len: metadata.len(),
            volume_id: metadata.dev(),
            file_id: Some(u128::from(metadata.ino())),
            modified_ns: Some(unix_timestamp_ns(metadata.mtime(), metadata.mtime_nsec())),
            changed_ns: Some(unix_timestamp_ns(metadata.ctime(), metadata.ctime_nsec())),
        })
    }
    #[cfg(not(unix))]
    {
        Ok(FileSnapshot {
            len: metadata.len(),
            volume_id: 0,
            file_id: None,
            modified_ns: system_modified_ns(&metadata),
            changed_ns: None,
        })
    }
}

fn directory_identity_sync(path: &Path) -> Result<FileIdentity, FileOperationError> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| FileOperationError::io("read directory identity", path, &error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(FileOperationError::OutsideProject);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(FileIdentity {
            volume: metadata.dev(),
            file: metadata.ino(),
        })
    }
    #[cfg(not(unix))]
    {
        Ok(FileIdentity { volume: 0, file: 0 })
    }
}

fn stable_content_evidence_sync(path: &Path) -> Result<FileContentEvidence, FileOperationError> {
    let before = snapshot_sync(path)?;
    let (len, hash) = hash_file_sync(path)?;
    let after = snapshot_sync(path)?;
    if before != after || after.len != len {
        return Err(FileOperationError::IdentityChanged);
    }
    Ok(FileContentEvidence {
        snapshot: after,
        hash,
    })
}

fn same_staged_content_evidence(
    expected: &FileContentEvidence,
    actual: &FileContentEvidence,
) -> bool {
    expected.hash == actual.hash
        && expected.snapshot.volume_id == actual.snapshot.volume_id
        && expected.snapshot.len == actual.snapshot.len
        && expected.snapshot.file_id == actual.snapshot.file_id
        && expected.snapshot.modified_ns == actual.snapshot.modified_ns
}

async fn stable_content_evidence_async(
    path: &Path,
) -> Result<FileContentEvidence, FileOperationError> {
    let path = path.to_path_buf();
    let error_path = path.clone();
    tokio::task::spawn_blocking(move || stable_content_evidence_sync(&path))
        .await
        .map_err(|error| worker_file_error("fingerprint stable file", &error_path, error))?
}

#[cfg(unix)]
fn unix_timestamp_ns(seconds: i64, nanoseconds: i64) -> i128 {
    i128::from(seconds)
        .saturating_mul(1_000_000_000)
        .saturating_add(i128::from(nanoseconds))
}

#[cfg(not(unix))]
fn system_modified_ns(metadata: &std::fs::Metadata) -> Option<i128> {
    metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i128::try_from(duration.as_nanos()).ok())
}

#[cfg(unix)]
fn directory_is_writable(path: &Path) -> bool {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: `path` is a valid NUL-terminated filesystem path and `access`
    // performs a read-only capability check for the current effective user.
    unsafe { libc::access(path.as_ptr(), libc::W_OK) == 0 }
}

#[cfg(not(unix))]
fn directory_is_writable(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.is_dir() && !metadata.permissions().readonly())
        .unwrap_or(false)
}

fn permission_error(path: &Path) -> FileOperationError {
    FileOperationError::Io {
        action: "revalidate destination permission",
        path: path.to_path_buf(),
        message: "Permission denied".into(),
    }
}

fn blocked(plan: OperationItemPlan, code: BatchResultCode, route: PreparedRoute) -> PreparedItem {
    PreparedItem {
        plan,
        state: FileCommandPreflightState::Blocked(code),
        route,
        source_evidence: None,
        destination_evidence: None,
        source_parent_identity: None,
        destination_parent_identity: None,
    }
}

fn mark_duplicate_rename_destinations(
    rows: &mut [viewer_domain::operation::RenamePreviewRow],
    case_sensitive: bool,
) {
    let mut positions: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        if let Some(destination) = row.destination.as_ref() {
            let key = if case_sensitive {
                destination.as_str().to_owned()
            } else {
                destination.as_str().to_lowercase()
            };
            positions.entry(key).or_default().push(index);
        }
    }
    for duplicates in positions.values().filter(|positions| positions.len() > 1) {
        for index in duplicates {
            rows[*index].push_error(if case_sensitive {
                RenameErrorCode::DuplicateDestination
            } else {
                RenameErrorCode::CaseCollision
            });
        }
    }
}

fn rename_error_code(errors: &[RenameErrorCode]) -> BatchResultCode {
    if errors.contains(&RenameErrorCode::SourceMissing) {
        BatchResultCode::SourceMissing
    } else if errors.contains(&RenameErrorCode::DestinationReadOnly) {
        BatchResultCode::PermissionDenied
    } else if errors.contains(&RenameErrorCode::DestinationOccupied) {
        BatchResultCode::DestinationOccupied
    } else {
        BatchResultCode::InvalidTarget
    }
}

fn classify_file_error(error: &FileOperationError) -> BatchResultCode {
    match error {
        FileOperationError::RegisteredTemporaryCleanupRequired { primary, .. } => {
            classify_file_error(primary)
        }
        FileOperationError::SourceMissing => BatchResultCode::SourceMissing,
        FileOperationError::DestinationExists => BatchResultCode::DestinationOccupied,
        FileOperationError::VerificationFailed | FileOperationError::IdentityChanged => {
            BatchResultCode::VerificationFailed
        }
        FileOperationError::Cancelled => BatchResultCode::Cancelled,
        FileOperationError::OutsideProject
        | FileOperationError::ReservedPath
        | FileOperationError::DestinationRequired => BatchResultCode::InvalidTarget,
        FileOperationError::Io { message, .. }
            if message.to_ascii_lowercase().contains("permission denied")
                || message
                    .to_ascii_lowercase()
                    .contains("operation not permitted") =>
        {
            BatchResultCode::PermissionDenied
        }
        FileOperationError::Io { .. } => BatchResultCode::BackendUnavailable,
    }
}

fn classify_message(message: &str) -> BatchResultCode {
    let lowercase = message.to_ascii_lowercase();
    if lowercase.contains("source does not exist") || lowercase.contains("source is missing") {
        BatchResultCode::SourceMissing
    } else if lowercase.contains("destination already exists")
        || lowercase.contains("destination is occupied")
    {
        BatchResultCode::DestinationOccupied
    } else if lowercase.contains("permission denied")
        || lowercase.contains("operation not permitted")
    {
        BatchResultCode::PermissionDenied
    } else if lowercase.contains("identity changed") || lowercase.contains("did not match") {
        BatchResultCode::VerificationFailed
    } else {
        BatchResultCode::BackendUnavailable
    }
}

fn temporary_for(
    destination: &RelativePath,
    operation_id: OperationId,
) -> Result<RelativePath, FileOperationError> {
    let path = Path::new(destination.as_str());
    let name = format!(".viewer-copy-{operation_id}.part");
    let temporary = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from(&name), |parent| parent.join(&name));
    RelativePath::parse(
        temporary
            .to_str()
            .ok_or(FileOperationError::OutsideProject)?,
    )
    .map_err(|_| FileOperationError::ReservedPath)
}

pub(crate) fn trash_temporary_for(
    source: &RelativePath,
    operation_id: OperationId,
) -> Result<RelativePath, FileOperationError> {
    let source = Path::new(source.as_str());
    let parent = source.parent().ok_or(FileOperationError::OutsideProject)?;
    let temporary = parent.join(format!(".viewer-trash-{operation_id}.part"));
    RelativePath::parse(
        temporary
            .to_str()
            .ok_or(FileOperationError::OutsideProject)?,
    )
    .map_err(|_| FileOperationError::OutsideProject)
}

fn worker_file_error(
    action: &'static str,
    path: &Path,
    error: tokio::task::JoinError,
) -> FileOperationError {
    FileOperationError::Io {
        action,
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

fn journal_file_error(error: super::journal::JournalError) -> FileOperationError {
    FileOperationError::Io {
        action: "persist operation journal",
        path: PathBuf::new(),
        message: error.to_string(),
    }
}

fn commit_file_error(error: viewer_application::OperationCommitError) -> FileOperationError {
    FileOperationError::Io {
        action: "commit operation projection",
        path: PathBuf::new(),
        message: error.to_string(),
    }
}

fn replace_file_error(error: super::conflict::ReplaceError) -> FileOperationError {
    match error {
        super::conflict::ReplaceError::File(error) => error,
        error => FileOperationError::Io {
            action: "execute replace",
            path: PathBuf::new(),
            message: error.to_string(),
        },
    }
}
