use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};
use viewer_application::{
    FileContentEvidence, FileOperationError, FileSnapshot, OperationCommit,
    file_commands::{
        BatchId, BatchResultCode, FileCommandCancellation, FileCommandItemExecution,
        FileCommandPreflightState, LocalFileCommandError, LocalFileCommandOutcome,
    },
    watcher::FileIdentity,
};
use viewer_domain::{
    EntityId, OperationId, RelativePath,
    operation::{ConflictPolicy, OperationItemPlan, OperationKind, OperationState},
};

use super::super::{
    conflict::{ReplaceExecutor, keep_both_destination},
    rename::{
        RenameExecutor, RenameItemStatus, RenameMapping, RenameParentIdentities, RenamePlanner,
    },
};
use super::{
    preflight::stable_content_evidence_sync,
    results::classify_message,
    types::{LocalFileCommandAdapter, PreparedItem, PreparedRoute},
};
use crate::scan::reconcile::ExpectedChange;

const EXPECTED_CHANGE_TTL_MS: u64 = 5_000;

impl LocalFileCommandAdapter {
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

    pub(super) async fn execute_copy_route(
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

    pub(super) async fn register_expected_change(
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

    pub(super) async fn execute_prepared_item(
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

    pub(super) async fn settle_unstarted_item(
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

pub(super) fn journal_file_error(error: super::journal::JournalError) -> FileOperationError {
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
