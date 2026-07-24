mod execution;
mod preflight;
mod results;
mod types;
mod undo;

pub(crate) use execution::trash_temporary_for;
pub use types::LocalFileCommandAdapter;

use super::{conflict, journal};
use async_trait::async_trait;
use viewer_application::{
    LocalFileCommandPort,
    file_commands::{
        BatchId, FileCommand, FileCommandCancellation, FileCommandItemExecution,
        LocalFileCommandError, LocalFileCommandOutcome, LocalFileCommandPreflightItem,
    },
    undo::UndoAction,
};

#[async_trait]
impl LocalFileCommandPort for LocalFileCommandAdapter {
    async fn preflight(
        &self,
        batch_id: BatchId,
        command: &FileCommand,
    ) -> Result<Vec<LocalFileCommandPreflightItem>, LocalFileCommandError> {
        self.preflight_command(batch_id, command).await
    }

    async fn discard_preflight(&self, batch_id: BatchId) -> Result<(), LocalFileCommandError> {
        self.discard_preflight_command(batch_id).await
    }

    async fn execute_item(
        &self,
        request: FileCommandItemExecution,
        cancellation: FileCommandCancellation,
    ) -> Result<LocalFileCommandOutcome, LocalFileCommandError> {
        self.execute_prepared_item(request, cancellation).await
    }

    async fn settle_unstarted(
        &self,
        request: FileCommandItemExecution,
        outcome: LocalFileCommandOutcome,
    ) -> Result<LocalFileCommandOutcome, LocalFileCommandError> {
        self.settle_unstarted_item(request, outcome).await
    }

    async fn take_undo_actions(
        &self,
        batch_id: BatchId,
    ) -> Result<Vec<UndoAction>, LocalFileCommandError> {
        self.take_prepared_undo_actions(batch_id).await
    }
}
