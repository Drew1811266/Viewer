use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};
use viewer_application::{
    FileContentEvidence, FileOperationError, FileSnapshot,
    file_commands::{
        BatchId, BatchResultCode, FileCommand, FileCommandAction, FileCommandItemExecution,
        FileCommandKind, FileCommandPreflightState, LocalFileCommandError,
        LocalFileCommandPreflightItem,
    },
    rename::{SingleRenameRequest, preview_single_rename},
    watcher::FileIdentity,
};
use viewer_domain::{
    EntityId, OperationId, RelativePath,
    file::{FileKind, FileNode},
    operation::{
        ConflictPolicy, OperationItemPlan, OperationKind, OperationPlan, RenameErrorCode,
        RenamePreflight,
    },
};

use super::super::{
    copy::hash_file_sync,
    rename::{RenameMapping, RenamePlanner},
};
use super::types::{LocalFileCommandAdapter, PreparedBatch, PreparedItem, PreparedRoute};

impl LocalFileCommandAdapter {
    pub(super) fn prepare(&self, batch_id: BatchId, command: &FileCommand) -> PreparedBatch {
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

    pub(super) fn prepare_simple(
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

    pub(super) fn prepare_rename(&self, batch_id: BatchId, command: &FileCommand) -> PreparedBatch {
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
        self.mark_duplicate_rename_destinations(&mut previews, self.case_sensitive_root());
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
                FileCommandPreflightState::Blocked(self.rename_error_code(&row.errors))
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

    pub(super) fn destination_for(
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

    pub(super) fn resolve_source(&self, node: &FileNode) -> Result<PathBuf, FileOperationError> {
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

    pub(super) fn revalidate_source(&self, item: &PreparedItem) -> Result<(), FileOperationError> {
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

    pub(super) fn revalidate_replace_destination(
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

    pub(super) fn revalidate_move_route(
        &self,
        item: &PreparedItem,
    ) -> Result<(), FileOperationError> {
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

    pub(super) fn revalidate_destination_entity(
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

    pub(super) fn case_sensitive_root(&self) -> bool {
        self.volume
            .is_case_sensitive(&self.project_root)
            .unwrap_or(true)
    }

    pub(super) async fn preflight_command(
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

    pub(super) async fn discard_preflight_command(
        &self,
        batch_id: BatchId,
    ) -> Result<(), LocalFileCommandError> {
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

    pub(super) fn mark_duplicate_rename_destinations(
        &self,
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

    pub(super) fn rename_error_code(&self, errors: &[RenameErrorCode]) -> BatchResultCode {
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
}

fn supported_regular_kind(kind: FileKind) -> bool {
    kind != FileKind::Directory
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

pub(super) fn directory_identity_sync(path: &Path) -> Result<FileIdentity, FileOperationError> {
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

pub(super) fn stable_content_evidence_sync(
    path: &Path,
) -> Result<FileContentEvidence, FileOperationError> {
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
