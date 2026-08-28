use super::*;
use crate::{
    ClockPort, PreparedReviewAsset, ReviewTaskCancellation,
    review_assets::ContinuousReviewAssetPort, review_evidence::ReviewEvidencePort,
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use viewer_domain::{
    review::{AssetVersion, MAX_ASSETS_PER_ROUND, MAX_TARGETS_PER_FEEDBACK, continuous::*},
    *,
};

pub struct ContinuousReviewService {
    pub(super) context: ReviewWorkspaceContext,
    pub(super) provider: Arc<dyn ContinuousReviewRepositoryProviderPort>,
    pub(super) assets: Arc<dyn ContinuousReviewAssetPort>,
    pub(super) evidence: Arc<dyn ReviewEvidencePort>,
    pub(super) codec: Arc<dyn ReviewCommandCodecPort>,
    clock: Arc<dyn ClockPort>,
    pub(super) prepared: Mutex<HashMap<AssetVersionId, PreparedReviewAsset>>,
    envelopes: Mutex<HashMap<ReviewCommandId, ReviewCommandEnvelope>>,
    gate: Mutex<()>,
    pub(super) usage_importer: Option<Arc<dyn UsageImportPort>>,
    pub(super) usage_previews: Mutex<HashMap<ReviewUsageId, UsageImportPreview>>,
    pub(super) migration_inspection: Mutex<Option<MigrationInspection>>,
}
impl ContinuousReviewService {
    pub fn new(
        context: ReviewWorkspaceContext,
        provider: Arc<dyn ContinuousReviewRepositoryProviderPort>,
        assets: Arc<dyn ContinuousReviewAssetPort>,
        evidence: Arc<dyn ReviewEvidencePort>,
        codec: Arc<dyn ReviewCommandCodecPort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            context,
            provider,
            assets,
            evidence,
            codec,
            clock,
            prepared: Mutex::new(HashMap::new()),
            envelopes: Mutex::new(HashMap::new()),
            gate: Mutex::new(()),
            usage_importer: None,
            usage_previews: Mutex::new(HashMap::new()),
            migration_inspection: Mutex::new(None),
        }
    }

    /// Called before displaying the review preview. Saving only references these captured IDs;
    /// it never prepares the current path again to substitute a different version for that preview.
    pub async fn prepare_assets(
        &self,
        entities: &[EntityId],
        cancellation: ReviewTaskCancellation,
    ) -> Result<Vec<AssetVersion>, ReviewWorkspaceError> {
        if entities.len() > MAX_ASSETS_PER_ROUND {
            return Err(ContinuousReviewError::LimitExceeded.into());
        }
        let values = self
            .assets
            .prepare_additions(entities, cancellation)
            .await?;
        let mut prepared = self.prepared.lock().await;
        let new_count = values
            .iter()
            .filter(|v| !prepared.contains_key(&v.asset.id))
            .count();
        if prepared.len() + new_count > MAX_ASSETS_PER_ROUND {
            return Err(ContinuousReviewError::LimitExceeded.into());
        }
        for value in &values {
            if value.failure.is_some() {
                return Err(crate::ReviewAssetError::Unavailable.into());
            }
            if prepared
                .get(&value.asset.id)
                .is_some_and(|old| old.asset != value.asset)
            {
                return Err(ContinuousReviewError::DuplicateIdentity.into());
            }
        }
        let assets = values.iter().map(|v| v.asset.clone()).collect();
        for value in values {
            prepared.insert(value.asset.id, value);
        }
        Ok(assets)
    }

    pub async fn prepare(
        &self,
        command_id: ReviewCommandId,
        expected_snapshot_id: Option<ReviewSnapshotId>,
        command: ReviewWorkspaceCommand,
    ) -> Result<ReviewCommandEnvelope, ReviewWorkspaceError> {
        let count = match &command {
            ReviewWorkspaceCommand::SaveFeedback { targets, .. } => targets.len(),
            ReviewWorkspaceCommand::ContinueHistorical { bindings, .. } => bindings.len(),
            ReviewWorkspaceCommand::ContinueLegacy { bindings, .. } => bindings.len(),
            _ => 1,
        };
        if count > MAX_TARGETS_PER_FEEDBACK {
            return Err(ContinuousReviewError::LimitExceeded.into());
        }
        let mut envelopes = self.envelopes.lock().await;
        if let Some(old) = envelopes.get(&command_id) {
            if old.command != command || old.expected_snapshot_id != expected_snapshot_id {
                return Err(ReviewCommitError::CommandConflict.into());
            }
            return Ok(old.clone());
        }
        let retained_bytes = envelopes.values().fold(0_usize, |sum, e| {
            sum.saturating_add(super::budget::command_bytes(&e.command))
                .saturating_add(super::usage::selection_bytes(&e.usage_selections))
                .saturating_add(super::migration::generated_bytes(&e.generated.migration))
        });
        let migration = if let ReviewWorkspaceCommand::Migrate(plan) = &command {
            let inspection = self.migration_inspection.lock().await;
            super::migration_state::generate(
                inspection
                    .as_ref()
                    .ok_or(ReviewWorkspaceError::PreviewRequired)?,
                &self.context,
                plan,
            )?
        } else {
            vec![]
        };
        let usage_selections = self.prepare_usages(&command).await;
        if envelopes.len() >= 128
            || retained_bytes
                .saturating_add(super::budget::command_bytes(&command))
                .saturating_add(super::migration::generated_bytes(&migration))
                .saturating_add(super::usage::selection_bytes(&usage_selections))
                > super::budget::MAX_PREPARED_BYTES
        {
            return Err(ContinuousReviewError::LimitExceeded.into());
        }
        let mut envelope = ReviewCommandEnvelope {
            usage_selections,
            context: self.context.clone(),
            command_id,
            expected_snapshot_id,
            payload_digest: [0; 32],
            command,
            generated: GeneratedReviewIds {
                snapshot_id: ReviewSnapshotId::new(),
                feedback_id: FeedbackId::new(),
                text_revision_id: ReviewTextRevisionId::new(),
                archive_id: ReviewArchiveId::new(),
                targets: (0..count)
                    .map(|_| (ReviewTargetId::new(), ReviewTargetRevisionId::new()))
                    .collect(),
                migration,
                created_at_ms: self.clock.unix_millis(),
            },
        };
        envelope.payload_digest = self.codec.digest(&envelope)?;
        envelopes.insert(command_id, envelope.clone());
        Ok(envelope)
    }

    pub async fn apply(
        &self,
        envelope: ReviewCommandEnvelope,
    ) -> Result<ReviewApplyResult, ReviewWorkspaceError> {
        self.apply_with_cancellation(envelope, ReviewTaskCancellation::default())
            .await
    }

    pub async fn apply_with_cancellation(
        &self,
        envelope: ReviewCommandEnvelope,
        cancellation: ReviewTaskCancellation,
    ) -> Result<ReviewApplyResult, ReviewWorkspaceError> {
        let _guard = self.gate.lock().await;
        if envelope.context != self.context {
            return Err(ReviewWorkspaceError::WrongContext);
        }
        if self.codec.digest(&envelope)? != envelope.payload_digest {
            return Err(ReviewCommitError::CommandConflict.into());
        }
        if matches!(envelope.command, ReviewWorkspaceCommand::Migrate(_)) {
            return self.apply_migration(envelope, cancellation).await;
        }
        let provider = self.provider.clone();
        let writer = io(move || provider.open_writer()).await?;
        let repository = writer.clone();
        let stream = self.context.stream_id;
        let command = envelope.command_id;
        match io(move || repository.find_command(stream, command)).await? {
            CommandLookup::Found(receipt) => {
                if receipt.payload_digest != envelope.payload_digest {
                    return Err(ReviewCommitError::CommandConflict.into());
                }
                return self.refresh_after_commit(receipt).await;
            }
            CommandLookup::Unavailable => return Err(ReviewCommitError::LookupUnavailable.into()),
            CommandLookup::Absent => {}
        }
        let repository = writer.clone();
        let current = io(move || repository.load_current(stream)).await?;
        if current.as_ref().map(|s| s.state.snapshot_id) != envelope.expected_snapshot_id {
            let draft = super::recovery::draft(&envelope, ReviewRecoveryFailure::StaleSnapshot)?;
            let repository = writer.clone();
            io(move || repository.save_recovery(&draft)).await?;
            return Err(ReviewCommitError::StaleSnapshot.into());
        }
        let empty =
            ContinuousReviewState::empty(self.context.project_id, stream, ReviewSnapshotId::new());
        let draft = super::recovery::draft(&envelope, ReviewRecoveryFailure::WriteFailed)?;
        let mut recovery_saved = false;
        let result = async {
            let adopted_usage = self
                .usages_for(
                    &envelope.command,
                    writer.clone(),
                    Some(&envelope.usage_selections),
                )
                .await?;
            let assets = self.prepared.lock().await;
            let repository = writer.clone();
            let saved = current.clone();
            let empty_state = empty.clone();
            let request = envelope.clone();
            let prepared = assets.clone();
            let usages = adopted_usage.clone();
            let transition = work(move || {
                super::transition::prepare(
                    repository.as_ref(),
                    saved.as_ref(),
                    &empty_state,
                    &request,
                    &prepared,
                    &usages,
                )
            })
            .await?;
            let next = transition.next;
            let repository = writer.clone();
            let recovery = draft.clone();
            io(move || repository.save_recovery(&recovery)).await?;
            recovery_saved = true;
            if cancellation.is_cancelled() {
                return Err(ReviewWorkspaceError::Cancelled);
            }
            let evidence = super::evidence::prepare(
                self.evidence.as_ref(),
                writer.clone(),
                current.as_ref(),
                &next,
                &assets,
                cancellation.clone(),
            )
            .await?;
            self.assets
                .check_sources(&next.assets, cancellation.clone())
                .await?;
            if cancellation.is_cancelled() {
                return Err(ReviewWorkspaceError::Cancelled);
            }
            let request = ReviewCommitRequest {
                expected: current.as_ref().map(|s| s.reference),
                production: self.context.production.clone(),
                next: PreparedContinuousSnapshot {
                    state: next,
                    command_id: command,
                    payload_digest: envelope.payload_digest,
                    changes: transition.changes,
                    evidence: evidence.bindings,
                },
                archives: transition.archives,
                adopted_usage: adopted_usage.into_iter().map(|u| u.declaration).collect(),
                staged_evidence: evidence.files,
            };
            let repository = writer.clone();
            let receipt = io(move || repository.commit(request)).await?;
            drop(evidence.leases);
            Ok(receipt)
        }
        .await;
        match result {
            Ok(receipt) => self.refresh_after_commit(receipt).await,
            Err(error) => {
                let mut draft = draft;
                draft.failure = super::recovery::failure(&error);
                // Recovery already exists. A failed diagnostic update must not hide OutcomeUnknown.
                if recovery_saved
                    || !draft.editor_input.text.is_empty()
                    || !draft.editor_input.targets.is_empty()
                    || !draft.editor_input.selections.is_empty()
                {
                    let repository = writer.clone();
                    let _ = io(move || repository.save_recovery(&draft)).await;
                }
                Err(error)
            }
        }
    }

    pub(super) async fn refresh_after_commit(
        &self,
        receipt: ReviewCommitReceipt,
    ) -> Result<ReviewApplyResult, ReviewWorkspaceError> {
        self.envelopes.lock().await.remove(&receipt.command_id);
        let view = self
            .view(self.context.stream_id)
            .await
            .map_err(|_| ReviewWorkspaceError::CommittedViewUnavailable(receipt))?;
        Ok(ReviewApplyResult { receipt, view })
    }
}

pub(super) async fn io<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, ReviewCommitError> + Send + 'static,
) -> Result<T, ReviewWorkspaceError> {
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|_| ReviewCommitError::Io)?
        .map_err(Into::into)
}
pub(super) async fn work<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, ReviewWorkspaceError> + Send + 'static,
) -> Result<T, ReviewWorkspaceError> {
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|_| ReviewCommitError::Io)?
}
