use super::*;
use crate::ReviewTaskCancellation;
use std::sync::Arc;
use viewer_domain::{ReviewSnapshotId, review::continuous::*};

impl ContinuousReviewService {
    /// Commits only the logical authoring revision and its durable outbox entry.
    /// Evidence rendering, source rehashing, v3 publication, and full view projection are
    /// deliberately excluded from this foreground acknowledgement path.
    pub async fn apply_authoring_with_cancellation(
        &self,
        envelope: ReviewCommandEnvelope,
        cancellation: ReviewTaskCancellation,
    ) -> Result<ReviewAuthoringApplyResult, ReviewWorkspaceError> {
        let _guard = self.gate.lock().await;
        if envelope.context != self.context {
            return Err(ReviewWorkspaceError::WrongContext);
        }
        if self.codec.digest(&envelope)? != envelope.payload_digest {
            return Err(ReviewCommitError::CommandConflict.into());
        }

        let provider = self.provider.clone();
        let store = super::service::io(move || provider.open_authoring_writer()).await?;
        let stream = self.context.stream_id;
        let command = envelope.command_id;
        let lookup_store = store.clone();
        match super::service::io(move || lookup_store.find_command(stream, command)).await? {
            AuthoringCommandLookup::Found(receipt) => {
                if receipt.payload_digest != envelope.payload_digest {
                    return Err(ReviewCommitError::CommandConflict.into());
                }
                return self
                    .authoring_result_from_store(store, receipt, cancellation)
                    .await;
            }
            AuthoringCommandLookup::Absent => {}
        }
        // Cancellation may stop a new mutation, but it must not hide a durable receipt for an
        // idempotent retry that was already committed before this invocation was accepted.
        super::preview::check_cancelled(&cancellation)?;

        let current_store = store.clone();
        let current = super::service::io(move || current_store.load_current(stream)).await?;
        if current.as_ref().map(|value| value.head.snapshot_id) != envelope.expected_snapshot_id {
            return Err(ReviewCommitError::StaleSnapshot.into());
        }

        let barrier = barrier_for(&envelope.command);
        if barrier != ReviewBarrierKind::None
            && let Some(basis) = current.as_ref()
        {
            let materializer = ReviewMaterializationService::new(
                store.clone(),
                Arc::new(ContinuousReviewPublication::new(
                    stream,
                    self.provider.clone(),
                    self.assets.clone(),
                    self.evidence.clone(),
                )),
                self.clock.clone(),
            );
            materializer
                .flush_through(stream, basis.head, cancellation.clone())
                .await?;
            let heads_store = store.clone();
            let heads = super::service::io(move || heads_store.load_heads(stream)).await?;
            if heads.authoring != Some(basis.head) || heads.published != Some(basis.head) {
                return Err(ReviewCommitError::Integrity.into());
            }
        }

        // Only commands whose semantics explicitly reference published history or usage open the
        // old repository, and they do so before the SQLite write transaction begins.
        let needs_published_reader = matches!(
            envelope.command,
            ReviewWorkspaceCommand::Archive(_)
                | ReviewWorkspaceCommand::Restore { .. }
                | ReviewWorkspaceCommand::Migrate(_)
                | ReviewWorkspaceCommand::ContinueHistorical { .. }
                | ReviewWorkspaceCommand::ContinueLegacy { .. }
        ) || super::usage::requires_repository(&envelope.command);
        let published = if needs_published_reader {
            let provider = self.provider.clone();
            Some(super::service::io(move || provider.open_reader()).await?)
        } else {
            None
        };

        let current_published = if barrier != ReviewBarrierKind::None {
            match current.as_ref() {
                Some(authoring) => {
                    let repository = published
                        .as_ref()
                        .cloned()
                        .ok_or(ReviewWorkspaceError::CapabilityUnavailable)?;
                    let published = super::service::io(move || repository.load_current(stream))
                        .await?
                        .ok_or(ReviewCommitError::Integrity)?;
                    let mut logical = published.state.clone();
                    logical.parent = authoring.state.parent;
                    if logical != authoring.state
                        || published.production != authoring.production
                        || published.command_id != authoring.command_id
                        || published.payload_digest != authoring.payload_digest
                    {
                        return Err(ReviewCommitError::Integrity.into());
                    }
                    Some(published)
                }
                None => None,
            }
        } else {
            current.as_ref().map(as_published_state)
        };

        let usages = if super::usage::requires_repository(&envelope.command) {
            self.usages_for(
                &envelope.command,
                published
                    .as_ref()
                    .cloned()
                    .ok_or(ReviewWorkspaceError::CapabilityUnavailable)?,
                Some(&envelope.usage_selections),
            )
            .await?
        } else {
            vec![]
        };
        super::preview::check_cancelled(&cancellation)?;

        let prepared_assets = self.prepared.lock().await.clone();
        let next = if matches!(envelope.command, ReviewWorkspaceCommand::Migrate(_)) {
            let inspection = self
                .migration_inspection
                .lock()
                .await
                .clone()
                .ok_or(ReviewWorkspaceError::PreviewRequired)?;
            let assets = prepared_assets
                .values()
                .map(|value| value.asset.clone())
                .collect::<Vec<_>>();
            let state = prepare_migration_state(&inspection, &envelope, &assets)?;
            let empty = ContinuousReviewState::empty(
                self.context.project_id,
                stream,
                ReviewSnapshotId::from_u128(0),
            );
            PreparedAuthoringTransition {
                changes: super::editing::changes(&empty, &state, ReviewChangeKind::Added),
                state,
                archives: vec![],
            }
        } else {
            let empty = ContinuousReviewState::empty(
                self.context.project_id,
                stream,
                ReviewSnapshotId::from_u128(0),
            );
            let transition = super::transition::prepare(
                published.as_deref(),
                current_published.as_ref(),
                &empty,
                &envelope,
                &prepared_assets,
                &usages,
            )?;
            PreparedAuthoringTransition {
                state: transition.next,
                changes: transition.changes,
                archives: transition.archives,
            }
        };
        super::preview::check_cancelled(&cancellation)?;

        let before = current.clone();
        let mut logical_state = next.state;
        // This is a control-plane parent fingerprint, not a public v3 object reference. The
        // materializer matches the snapshot identity and replaces the digest with the exact
        // verified v3 parent digest once evidence and the immutable state bytes exist.
        logical_state.parent = current.as_ref().map(|value| SnapshotRef {
            snapshot_id: value.head.snapshot_id,
            blake3: value.payload_digest,
        });
        let publication_protocol = current
            .as_ref()
            .map_or(ReviewPublicationProtocol::V3, |value| {
                value.publication_protocol
            })
            .promote_for(&logical_state);
        let next_state = StoredAuthoringState {
            head: ReviewAuthoringHead {
                sequence: current
                    .as_ref()
                    .map_or(1, |value| value.head.sequence.saturating_add(1)),
                snapshot_id: envelope.generated.snapshot_id,
            },
            publication_protocol,
            production: self.context.production.clone(),
            state: logical_state,
            command_id: envelope.command_id,
            payload_digest: envelope.payload_digest,
            generated: envelope.generated.clone(),
            changes: next.changes,
            archives: next.archives,
            adopted_usage: usages
                .iter()
                .map(|value| value.declaration.clone())
                .collect(),
            barrier,
        };
        let expected_snapshot_id = envelope.expected_snapshot_id;
        let commit_store = store.clone();
        let payload_digest = envelope.payload_digest;
        let transaction_next_state = next_state.clone();
        let mut prepare = move |transaction_current: Option<&StoredAuthoringState>| {
            if transaction_current.map(|value| value.head.snapshot_id) != expected_snapshot_id {
                return Err(ReviewCommitError::StaleSnapshot.into());
            }
            let mut next = transaction_next_state.clone();
            next.publication_protocol = transaction_current
                .map_or(ReviewPublicationProtocol::V3, |value| {
                    value.publication_protocol
                })
                .promote_for(&next.state);
            Ok(ReviewAuthoringCommitRequest {
                expected_snapshot_id,
                next,
            })
        };
        let receipt = super::service::work(move || {
            commit_store.commit(stream, command, payload_digest, &mut prepare)
        })
        .await?;

        self.envelopes.lock().await.remove(&receipt.command_id);
        let history_selectors = self.history_replacement_for(&next_state).await?;
        let after = logical_current(next_state);
        let patch = ReviewWorkspacePatch::between(
            before.map(logical_current).as_ref(),
            &after,
            history_selectors,
        );
        let status_store = store.clone();
        let publication = super::service::io(move || status_store.status(stream))
            .await
            .map_err(|_| ReviewWorkspaceError::CommittedAuthoringPatchUnavailable(receipt))?;
        Ok(ReviewAuthoringApplyResult {
            receipt,
            patch,
            publication,
        })
    }

    async fn authoring_result_from_store(
        &self,
        store: Arc<dyn ContinuousReviewAuthoringRepositoryPort>,
        receipt: ReviewAuthoringReceipt,
        cancellation: ReviewTaskCancellation,
    ) -> Result<ReviewAuthoringApplyResult, ReviewWorkspaceError> {
        if cancellation.is_cancelled() {
            return Err(ReviewWorkspaceError::CommittedAuthoringPatchUnavailable(
                receipt,
            ));
        }
        let stream = self.context.stream_id;
        let target_store = store.clone();
        let target = super::service::io(move || {
            let target = target_store.load_snapshot(stream, receipt.head.sequence)?;
            if target.head != receipt.head {
                return Err(ReviewCommitError::Integrity);
            }
            Ok(target)
        })
        .await
        .map_err(|_| ReviewWorkspaceError::CommittedAuthoringPatchUnavailable(receipt))?;
        let before = if receipt.head.sequence > 1 {
            let parent_store = store.clone();
            let parent = super::service::io(move || {
                parent_store.load_snapshot(stream, receipt.head.sequence - 1)
            })
            .await
            .map_err(|_| ReviewWorkspaceError::CommittedAuthoringPatchUnavailable(receipt))?;
            Some(logical_current(parent))
        } else {
            None
        };
        let history_selectors = self.history_replacement_for(&target).await?;
        let after = logical_current(target);
        let patch = ReviewWorkspacePatch::between(before.as_ref(), &after, history_selectors);
        let status_store = store.clone();
        let publication = super::service::io(move || status_store.status(stream))
            .await
            .map_err(|_| ReviewWorkspaceError::CommittedAuthoringPatchUnavailable(receipt))?;
        self.envelopes.lock().await.remove(&receipt.command_id);
        Ok(ReviewAuthoringApplyResult {
            receipt,
            patch,
            publication,
        })
    }

    async fn history_replacement_for(
        &self,
        target: &StoredAuthoringState,
    ) -> Result<Option<Vec<crate::review_evidence::HistorySelector>>, ReviewWorkspaceError> {
        use crate::review_evidence::HistorySelector;
        match target.barrier {
            ReviewBarrierKind::None => Ok(None),
            ReviewBarrierKind::Archive | ReviewBarrierKind::Restore => {
                let provider = self.provider.clone();
                let stream = self.context.stream_id;
                let archives = target
                    .archives
                    .iter()
                    .map(|value| value.archive_id)
                    .collect::<Vec<_>>();
                super::service::io(move || {
                    let reader = provider.open_reader()?;
                    let mut selectors = reader.load_history_selectors(stream)?;
                    for archive in archives {
                        let selector = HistorySelector::Archive(archive);
                        if !selectors.contains(&selector) {
                            selectors.push(selector);
                        }
                    }
                    Ok(selectors)
                })
                .await
                .map(Some)
            }
            ReviewBarrierKind::Migration => {
                let provider = self.provider.clone();
                super::service::io(move || {
                    let inspection = provider
                        .inspect_migration()?
                        .ok_or(ReviewCommitError::MigrationRequired)?;
                    Ok(inspection
                        .legacy_records
                        .into_iter()
                        .map(|value| HistorySelector::Legacy(value.round_id))
                        .collect())
                })
                .await
                .map(Some)
            }
        }
    }
}

struct PreparedAuthoringTransition {
    state: ContinuousReviewState,
    changes: Vec<ReviewChange>,
    archives: Vec<ArchiveCheckpoint>,
}

fn as_published_state(value: &StoredAuthoringState) -> StoredContinuousSnapshot {
    StoredContinuousSnapshot {
        reference: SnapshotRef {
            snapshot_id: value.head.snapshot_id,
            blake3: value.payload_digest,
        },
        publication_protocol: value.publication_protocol,
        production: value.production.clone(),
        state: value.state.clone(),
        command_id: value.command_id,
        payload_digest: value.payload_digest,
        changes: value.changes.clone(),
        evidence: vec![],
    }
}

fn logical_current(authoring: StoredAuthoringState) -> ReviewWorkspaceCurrent {
    ReviewWorkspaceCurrent {
        authoring,
        published_ref: None,
        evidence: vec![],
    }
}

fn barrier_for(command: &ReviewWorkspaceCommand) -> ReviewBarrierKind {
    match command {
        ReviewWorkspaceCommand::Archive(_) => ReviewBarrierKind::Archive,
        ReviewWorkspaceCommand::Restore { .. } => ReviewBarrierKind::Restore,
        ReviewWorkspaceCommand::Migrate(_) => ReviewBarrierKind::Migration,
        _ => ReviewBarrierKind::None,
    }
}
