use std::collections::HashMap;
use viewer_application::{
    FileOperationError,
    file_commands::{BatchId, BatchResultCode, LocalFileCommandError, LocalFileCommandOutcome},
};
use viewer_domain::{
    EntityId, OperationId,
    operation::{OperationKind, OperationState},
};

use super::types::{LocalFileCommandAdapter, PreparedBatch};

impl LocalFileCommandAdapter {
    pub(super) fn mark_failed(&self, operation_id: OperationId, code: BatchResultCode) {
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

    pub(super) fn mark_cancelled(&self, operation_id: OperationId) {
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

    pub(super) fn settle_recovery_required(
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

    pub(super) fn fail_outcome_for_entity(
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

    pub(super) fn maybe_finish_batch(&self, batch_id: BatchId) {
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

    pub(super) fn mark_delivered(&self, batch_id: BatchId, entity_id: EntityId) {
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

    pub(super) fn now(&self) -> i64 {
        self.clock.unix_millis()
    }

    pub(super) fn now_u64(&self) -> u64 {
        u64::try_from(self.now()).unwrap_or_default()
    }

    pub(super) fn lock_batches(
        &self,
    ) -> std::sync::MutexGuard<'_, HashMap<BatchId, PreparedBatch>> {
        self.batches
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

pub(super) fn classify_file_error(error: &FileOperationError) -> BatchResultCode {
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

pub(super) fn classify_message(message: &str) -> BatchResultCode {
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
