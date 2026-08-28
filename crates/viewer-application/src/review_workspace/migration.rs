use super::*;
use crate::ReviewTaskCancellation;
use viewer_domain::review::{ReviewMedia, continuous::*};

impl ContinuousReviewService {
    pub async fn inspect_migration(
        &self,
    ) -> Result<Option<MigrationInspection>, ReviewWorkspaceError> {
        let provider = self.provider.clone();
        let inspection = super::service::io(move || provider.inspect_migration()).await?;
        *self.migration_inspection.lock().await = inspection.clone();
        Ok(inspection)
    }

    pub(super) async fn apply_migration(
        &self,
        envelope: ReviewCommandEnvelope,
        cancellation: ReviewTaskCancellation,
    ) -> Result<ReviewApplyResult, ReviewWorkspaceError> {
        let Some(inspection) = self.inspect_migration().await? else {
            let provider = self.provider.clone();
            let stream = self.context.stream_id;
            let command = envelope.command_id;
            let lookup =
                super::service::io(move || provider.open_reader()?.find_command(stream, command))
                    .await?;
            return match lookup {
                CommandLookup::Found(receipt)
                    if receipt.payload_digest == envelope.payload_digest =>
                {
                    self.refresh_after_commit(receipt, cancellation).await
                }
                CommandLookup::Found(_) => Err(ReviewCommitError::CommandConflict.into()),
                CommandLookup::Unavailable => Err(ReviewCommitError::LookupUnavailable.into()),
                CommandLookup::Absent => Err(ReviewWorkspaceError::NoChanges),
            };
        };
        let mut draft = super::recovery::draft(&envelope, ReviewRecoveryFailure::WriteFailed)?;
        let recovery = draft.clone();
        let provider = self.provider.clone();
        super::service::io(move || provider.save_migration_recovery(&recovery)).await?;
        let result = self
            .commit_migration(envelope, inspection, cancellation.clone())
            .await;
        match result {
            Ok(receipt) => self.refresh_after_commit(receipt, cancellation).await,
            Err(error) => {
                draft.failure = super::recovery::failure(&error);
                let provider = self.provider.clone();
                // The first durable input remains even if updating its diagnostic fails.
                let _ = super::service::io(move || provider.save_migration_recovery(&draft)).await;
                Err(error)
            }
        }
    }

    async fn commit_migration(
        &self,
        envelope: ReviewCommandEnvelope,
        inspection: MigrationInspection,
        cancellation: ReviewTaskCancellation,
    ) -> Result<ReviewCommitReceipt, ReviewWorkspaceError> {
        if cancellation.is_cancelled() {
            return Err(ReviewWorkspaceError::Cancelled);
        }
        let assets = self.prepared.lock().await;
        let versions: Vec<_> = assets.values().map(|a| a.asset.clone()).collect();
        let state = prepare_migration_state(&inspection, &envelope, &versions)?;
        // Only explicitly rebound, preview-prepared assets can acquire fresh evidence.
        // Legacy image assets retain LegacyAbsent; no current source is passed off as old pixels.
        let mut fresh = state.clone();
        let explicit: std::collections::HashSet<_> = match &envelope.command {
            ReviewWorkspaceCommand::Migrate(MigrationPlan {
                choice: MigrationChoice::ContinueSelected { bindings, .. },
                ..
            }) => bindings.iter().map(|b| b.new_asset_version_id).collect(),
            _ => Default::default(),
        };
        fresh.assets.retain(|a| explicit.contains(&a.id));
        let fresh_ids: std::collections::HashSet<_> = fresh.assets.iter().map(|a| a.id).collect();
        for feedback in &mut fresh.feedback {
            feedback
                .targets
                .retain(|t| fresh_ids.contains(&t.asset_version_id));
        }
        fresh.feedback.retain(|f| !f.targets.is_empty());
        let provider = self.provider.clone();
        // Before migration a v3 reader is deliberately unavailable; all fresh bases are captured.
        let evidence = super::evidence::prepare_new(
            self.evidence.as_ref(),
            &fresh,
            &assets,
            cancellation.clone(),
        )
        .await?;
        let mut bindings = evidence.bindings;
        for asset in &state.assets {
            if !fresh_ids.contains(&asset.id) {
                bindings.push(ReviewEvidenceBinding {
                    asset_version_id: asset.id,
                    capability: if matches!(asset.media, ReviewMedia::Image { .. }) {
                        EvidenceCapability::LegacyAbsent
                    } else {
                        EvidenceCapability::NotImage
                    },
                });
            }
        }
        self.assets
            .check_sources(&state.assets, cancellation.clone())
            .await?;
        if cancellation.is_cancelled() {
            return Err(ReviewWorkspaceError::Cancelled);
        }
        let changes = state
            .feedback
            .iter()
            .flat_map(|f| {
                f.targets.iter().map(move |t| ReviewChange {
                    target_id: t.id,
                    before: None,
                    after: Some(TargetVersionKey {
                        feedback_id: f.id,
                        text_revision_id: f.text_revision_id,
                        target_id: t.id,
                        target_revision_id: t.revision_id,
                    }),
                    kind: ReviewChangeKind::Added,
                    archive_id: None,
                    historical_key: None,
                })
            })
            .collect();
        let next = PreparedContinuousSnapshot {
            state,
            command_id: envelope.command_id,
            payload_digest: envelope.payload_digest,
            changes,
            evidence: bindings,
        };
        let receipt = super::service::io(move || {
            provider.migrate(MigrationCommitRequest {
                envelope,
                next,
                staged_evidence: evidence.files,
            })
        })
        .await?;
        drop(evidence.leases);
        drop(assets);
        Ok(receipt)
    }
}

pub(super) fn generated_bytes(ids: &[MigrationFeedbackIds]) -> usize {
    ids.iter().fold(0_usize, |sum, f| {
        sum.saturating_add(256)
            .saturating_add(f.targets.len().saturating_mul(64))
    })
}
