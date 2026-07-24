use async_trait::async_trait;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use viewer_application::{
    FileOperationError,
    file_commands::{
        BatchId, FileCommandCancellation, FileCommandPreflightState, LocalFileCommandError,
    },
    undo::{UndoAction, UndoBatch, UndoError, UndoFilePort},
};
use viewer_domain::{
    OperationId,
    operation::{ConflictPolicy, OperationItemPlan, OperationKind, OperationPlan, OperationState},
};

use super::super::rename::{
    RenameExecutor, RenameItemStatus, RenameMapping, RenameParentIdentities, RenamePlanner,
};
use super::{
    execution::journal_file_error,
    preflight::directory_identity_sync,
    types::{LocalFileCommandAdapter, PreparedItem, PreparedRoute},
};

impl LocalFileCommandAdapter {
    pub(super) async fn take_prepared_undo_actions(
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
