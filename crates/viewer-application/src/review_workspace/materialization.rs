use super::*;
use crate::{
    ClockPort, PreparedReviewAsset, ReviewArtifactError, ReviewAssetError, ReviewTaskCancellation,
    review_assets::ContinuousReviewAssetPort,
    review_evidence::{ReviewEvidencePort, ReviewEvidenceStaging},
};
use async_trait::async_trait;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::Mutex as AsyncMutex;
use viewer_domain::AssetVersionId;
use viewer_domain::{ReviewStreamId, review::continuous::SnapshotRef};

#[derive(Clone, Debug, PartialEq)]
pub struct ClaimedReviewMaterialization {
    pub stream_id: ReviewStreamId,
    pub target: StoredAuthoringState,
    pub lease_epoch: u64,
    pub attempt_count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedReviewPublication {
    pub target: ReviewAuthoringHead,
    pub request: ReviewCommitRequest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewPublicationReceipt {
    pub target: ReviewAuthoringHead,
    pub snapshot: SnapshotRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewMaterializationOutcome {
    Idle,
    Published {
        receipt: ReviewPublicationReceipt,
    },
    Retrying {
        target: ReviewAuthoringHead,
        attempt_count: u32,
        next_attempt_at_ms: i64,
    },
    Blocked {
        target: ReviewAuthoringHead,
        code: ReviewMaterializationFailure,
    },
}

pub trait ReviewMaterializationQueuePort: Send + Sync {
    fn heads(&self, _stream: ReviewStreamId) -> Result<ReviewHeads, ReviewCommitError> {
        Err(ReviewCommitError::LookupUnavailable)
    }
    fn next(&self, now_ms: i64) -> Result<Option<ClaimedReviewMaterialization>, ReviewCommitError>;
    fn retry(
        &self,
        claim: &ClaimedReviewMaterialization,
        code: ReviewMaterializationFailure,
        next_attempt_at_ms: i64,
    ) -> Result<(), ReviewCommitError>;
    fn block(
        &self,
        claim: &ClaimedReviewMaterialization,
        code: ReviewMaterializationFailure,
    ) -> Result<(), ReviewCommitError>;
    fn mark_published(
        &self,
        claim: &ClaimedReviewMaterialization,
        receipt: ReviewPublicationReceipt,
    ) -> Result<(), ReviewCommitError>;
    fn status(&self, stream: ReviewStreamId) -> Result<ReviewPublicationStatus, ReviewCommitError>;
    fn requeue_expired(&self, now_ms: i64) -> Result<u32, ReviewCommitError>;
}

#[async_trait]
pub trait ReviewPublicationPort: Send + Sync {
    async fn materialize(
        &self,
        target: StoredAuthoringState,
        cancellation: ReviewTaskCancellation,
    ) -> Result<PreparedReviewPublication, ReviewWorkspaceError>;
    async fn publish(
        &self,
        prepared: PreparedReviewPublication,
    ) -> Result<ReviewPublicationReceipt, ReviewWorkspaceError>;
    async fn verify_publication(
        &self,
        target: &StoredAuthoringState,
    ) -> Result<Option<ReviewPublicationReceipt>, ReviewWorkspaceError>;
}

/// Adapts the existing verified v3 repository into the background publication port. It owns
/// temporary evidence leases across `materialize` and `publish`, but no lease is authoritative:
/// after a restart the exact target is safely materialized again or reconciled from v3.
pub struct ContinuousReviewPublication {
    stream_id: ReviewStreamId,
    provider: Arc<dyn ContinuousReviewRepositoryProviderPort>,
    assets: Arc<dyn ContinuousReviewAssetPort>,
    evidence: Arc<dyn ReviewEvidencePort>,
    evidence_cache: Arc<dyn ReviewEvidenceActionCachePort>,
    evidence_policy: ReviewEvidenceActionPolicy,
    staged: Mutex<Option<StagedPublication>>,
}

struct StagedPublication {
    target: ReviewAuthoringHead,
    leases: Vec<Arc<dyn ReviewEvidenceStaging>>,
    cache_actions: Vec<CachedReviewEvidenceAction>,
}

impl ContinuousReviewPublication {
    pub fn new(
        stream_id: ReviewStreamId,
        provider: Arc<dyn ContinuousReviewRepositoryProviderPort>,
        assets: Arc<dyn ContinuousReviewAssetPort>,
        evidence: Arc<dyn ReviewEvidencePort>,
    ) -> Self {
        Self::new_with_cache(
            stream_id,
            provider,
            assets,
            evidence,
            Arc::new(NoReviewEvidenceActionCache),
            CURRENT_REVIEW_EVIDENCE_ACTION_POLICY,
        )
    }

    pub fn new_with_cache(
        stream_id: ReviewStreamId,
        provider: Arc<dyn ContinuousReviewRepositoryProviderPort>,
        assets: Arc<dyn ContinuousReviewAssetPort>,
        evidence: Arc<dyn ReviewEvidencePort>,
        evidence_cache: Arc<dyn ReviewEvidenceActionCachePort>,
        evidence_policy: ReviewEvidenceActionPolicy,
    ) -> Self {
        Self {
            stream_id,
            provider,
            assets,
            evidence,
            evidence_cache,
            evidence_policy,
            staged: Mutex::new(None),
        }
    }

    fn clear_staging(&self, target: ReviewAuthoringHead) {
        if let Ok(mut staged) = self.staged.lock()
            && staged.as_ref().is_some_and(|value| value.target == target)
        {
            *staged = None;
        }
    }
}

#[async_trait]
impl ReviewPublicationPort for ContinuousReviewPublication {
    async fn materialize(
        &self,
        target: StoredAuthoringState,
        cancellation: ReviewTaskCancellation,
    ) -> Result<PreparedReviewPublication, ReviewWorkspaceError> {
        if target.state.stream_id != self.stream_id {
            return Err(ReviewCommitError::Integrity.into());
        }
        if target.barrier == ReviewBarrierKind::Migration {
            // Legacy migration requires its dedicated verified publication adapter; never treat a
            // legacy repository as an empty v3 stream and silently bypass migration validation.
            return Err(ReviewCommitError::MigrationRequired.into());
        }
        super::preview::check_cancelled(&cancellation)?;
        let provider = self.provider.clone();
        let stream = target.state.stream_id;
        let (reader, previous) = super::service::io(move || {
            let reader = provider.open_reader()?;
            let previous = reader.load_current_for_materialization(stream)?;
            Ok((reader, previous))
        })
        .await?;
        let expected = previous.as_ref().map(|value| value.reference);
        if expected.map(|value| value.snapshot_id)
            != target.state.parent.map(|value| value.snapshot_id)
        {
            return Err(ReviewCommitError::StaleSnapshot.into());
        }

        // The authoring record can identify an unpublished logical parent, but its eventual v3
        // object digest depends on evidence materialized here. Bind the exact verified public
        // reference only in the publication copy; never treat the authoring digest as a v3 hash.
        let mut public_state = target.state.clone();
        public_state.parent = expected;

        let previous_authoring = previous
            .clone()
            .map(ReviewWorkspaceCurrent::from_published)
            .map(|current| current.authoring);
        let dirty_ids: std::collections::HashSet<_> =
            dirty_evidence_assets(previous_authoring.as_ref(), &target, self.evidence_policy)?
                .into_iter()
                .collect();
        let dirty_assets: Vec<_> = target
            .state
            .assets
            .iter()
            .filter(|asset| dirty_ids.contains(&asset.id))
            .cloned()
            .collect();
        let reopened = self
            .assets
            .reopen_exact(&dirty_assets, cancellation.clone())
            .await?;
        let prepared = exact_assets(&dirty_assets, reopened)?;
        let evidence = super::evidence::prepare_materialized(
            self.evidence.as_ref(),
            super::evidence::EvidenceActionCacheContext {
                port: self.evidence_cache.clone(),
                policy: self.evidence_policy,
            },
            reader,
            previous.as_ref(),
            &StoredAuthoringState {
                state: public_state.clone(),
                ..target.clone()
            },
            &prepared,
            cancellation.clone(),
        )
        .await?;
        super::preview::check_cancelled(&cancellation)?;

        let request = ReviewCommitRequest {
            expected,
            production: target.production.clone(),
            next: PreparedContinuousSnapshot {
                state: public_state,
                command_id: target.command_id,
                payload_digest: target.payload_digest,
                changes: target.changes.clone(),
                evidence: evidence.bindings,
            },
            archives: target.archives.clone(),
            adopted_usage: target.adopted_usage.clone(),
            staged_evidence: evidence.files,
        };
        let mut staged = self.staged.lock().map_err(|_| ReviewCommitError::Io)?;
        *staged = Some(StagedPublication {
            target: target.head,
            leases: evidence.leases,
            cache_actions: evidence.cache_actions,
        });
        Ok(PreparedReviewPublication {
            target: target.head,
            request,
        })
    }

    async fn publish(
        &self,
        prepared: PreparedReviewPublication,
    ) -> Result<ReviewPublicationReceipt, ReviewWorkspaceError> {
        if prepared.request.next.state.snapshot_id != prepared.target.snapshot_id {
            return Err(ReviewCommitError::Integrity.into());
        }
        {
            let staged = self.staged.lock().map_err(|_| ReviewCommitError::Io)?;
            if staged
                .as_ref()
                .is_none_or(|value| value.target != prepared.target || value.leases.is_empty())
                && !prepared.request.staged_evidence.is_empty()
            {
                return Err(ReviewCommitError::Integrity.into());
            }
        }
        let provider = self.provider.clone();
        let target = prepared.target;
        let receipt = super::service::io(move || {
            let writer = provider.open_writer()?;
            writer.commit(prepared.request)
        })
        .await?;
        if receipt.snapshot.snapshot_id != target.snapshot_id {
            return Err(ReviewCommitError::Integrity.into());
        }
        let cache_actions = {
            let staged = self.staged.lock().map_err(|_| ReviewCommitError::Io)?;
            staged
                .as_ref()
                .filter(|value| value.target == target)
                .map(|value| value.cache_actions.clone())
                .unwrap_or_default()
        };
        if !cache_actions.is_empty() {
            let cache = self.evidence_cache.clone();
            // Cache installation is a non-authoritative optimization. Publication is already
            // durable, so a cache write failure must not turn a successful save into failure.
            let _ = super::service::io(move || {
                for action in &cache_actions {
                    cache.store_verified(action)?;
                }
                Ok::<_, ReviewCommitError>(())
            })
            .await;
        }
        self.clear_staging(target);
        Ok(ReviewPublicationReceipt {
            target,
            snapshot: receipt.snapshot,
        })
    }

    async fn verify_publication(
        &self,
        target: &StoredAuthoringState,
    ) -> Result<Option<ReviewPublicationReceipt>, ReviewWorkspaceError> {
        let provider = self.provider.clone();
        let stream = self.stream_id;
        let expected = target.clone();
        let current = super::service::io(move || {
            if expected.state.stream_id != stream {
                return Err(ReviewCommitError::Integrity);
            }
            let reader = provider.open_writer()?;
            reader.sync_publication()?;
            // Publication already verifies every newly-created or cache-reused dirty object while
            // installing it. Reconciliation only needs canonical state/index identity here;
            // native/Agent reads retain the complete evidence-object verification boundary.
            let current = reader.load_current_for_materialization(stream)?;
            if let Some(current) = &current
                && current.state.snapshot_id == expected.head.snapshot_id
            {
                for archive in &expected.archives {
                    if &reader.load_archive(stream, archive.archive_id)? != archive {
                        return Err(ReviewCommitError::Integrity);
                    }
                }
                for usage in &expected.adopted_usage {
                    if reader.load_usage(stream, usage.id)? != Some(usage.clone()) {
                        return Err(ReviewCommitError::Integrity);
                    }
                }
                let mut logical_state = current.state.clone();
                if logical_state.parent.map(|value| value.snapshot_id)
                    != expected.state.parent.map(|value| value.snapshot_id)
                {
                    return Err(ReviewCommitError::Integrity);
                }
                logical_state.parent = expected.state.parent;
                if logical_state != expected.state
                    || current.production != expected.production
                    || current.command_id != expected.command_id
                    || current.payload_digest != expected.payload_digest
                    || current.changes != expected.changes
                {
                    return Err(ReviewCommitError::Integrity);
                }
            }
            Ok(current)
        })
        .await?;
        let Some(current) =
            current.filter(|value| value.state.snapshot_id == target.head.snapshot_id)
        else {
            return Ok(None);
        };
        self.clear_staging(target.head);
        Ok(Some(ReviewPublicationReceipt {
            target: target.head,
            snapshot: current.reference,
        }))
    }
}

fn exact_assets(
    expected_assets: &[viewer_domain::review::AssetVersion],
    reopened: Vec<PreparedReviewAsset>,
) -> Result<HashMap<AssetVersionId, PreparedReviewAsset>, ReviewWorkspaceError> {
    if reopened.len() != expected_assets.len() {
        return Err(ReviewAssetError::Unavailable.into());
    }
    let mut prepared = HashMap::with_capacity(reopened.len());
    for value in reopened {
        let expected = expected_assets
            .iter()
            .find(|asset| asset.id == value.asset.id)
            .ok_or(ReviewAssetError::SourceChanged)?;
        if expected != &value.asset || prepared.insert(value.asset.id, value).is_some() {
            return Err(ReviewAssetError::SourceChanged.into());
        }
    }
    Ok(prepared)
}

pub struct ReviewMaterializationService {
    pub(super) queue: Arc<dyn ReviewMaterializationQueuePort>,
    publication: Arc<dyn ReviewPublicationPort>,
    clock: Arc<dyn ClockPort>,
    run_gate: AsyncMutex<()>,
}

impl ReviewMaterializationService {
    pub fn new(
        queue: Arc<dyn ReviewMaterializationQueuePort>,
        publication: Arc<dyn ReviewPublicationPort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            queue,
            publication,
            clock,
            run_gate: AsyncMutex::new(()),
        }
    }

    pub async fn run_one(
        &self,
        cancellation: ReviewTaskCancellation,
    ) -> Result<ReviewMaterializationOutcome, ReviewWorkspaceError> {
        // A service is project-scoped by construction. Serialize its queue consumer so two UI
        // wakeups cannot concurrently materialize the same project's images.
        let _run = self.run_gate.lock().await;
        let now_ms = self.clock.unix_millis();
        let queue = self.queue.clone();
        super::service::io(move || queue.requeue_expired(now_ms)).await?;
        let queue = self.queue.clone();
        let Some(claim) = super::service::io(move || queue.next(now_ms)).await? else {
            return Ok(ReviewMaterializationOutcome::Idle);
        };

        match self.publication.verify_publication(&claim.target).await {
            Ok(Some(receipt)) => return self.finish_published(claim, receipt).await,
            Ok(None) => {}
            Err(error) => return self.finish_failed(claim, error, now_ms).await,
        }

        if cancellation.is_cancelled() {
            return self
                .finish_failed(claim, ReviewWorkspaceError::Cancelled, now_ms)
                .await;
        }
        let prepared = match self
            .publication
            .materialize(claim.target.clone(), cancellation.clone())
            .await
        {
            Ok(value) => value,
            Err(error) => return self.finish_failed(claim, error, now_ms).await,
        };
        if prepared.target != claim.target.head {
            return self
                .finish_failed(claim, ReviewCommitError::Integrity.into(), now_ms)
                .await;
        }
        if cancellation.is_cancelled() {
            return self
                .finish_failed(claim, ReviewWorkspaceError::Cancelled, now_ms)
                .await;
        }
        let (published, publish_error) = match self.publication.publish(prepared).await {
            Ok(receipt) if receipt.target == claim.target.head => (Some(receipt), None),
            Ok(_) => {
                return self
                    .finish_failed(claim, ReviewCommitError::Integrity.into(), now_ms)
                    .await;
            }
            Err(error) => (None, Some(error)),
        };

        // A successful return is not the publication boundary. The exact immutable target must
        // be reread and verified before the SQLite published head advances. This also reconciles
        // a previous process that installed the v3 index but died before updating SQLite.
        match self.publication.verify_publication(&claim.target).await {
            Ok(Some(receipt)) if published.is_none_or(|value| value == receipt) => {
                self.finish_published(claim, receipt).await
            }
            Ok(Some(_)) => {
                self.finish_failed(claim, ReviewCommitError::Integrity.into(), now_ms)
                    .await
            }
            Ok(None) => {
                self.finish_failed(
                    claim,
                    publish_error.unwrap_or(ReviewCommitError::Io.into()),
                    now_ms,
                )
                .await
            }
            Err(verify_error) => {
                let error = publish_error
                    .filter(|value| classify_failure(*value).1)
                    .unwrap_or(verify_error);
                self.finish_failed(claim, error, now_ms).await
            }
        }
    }

    async fn finish_published(
        &self,
        claim: ClaimedReviewMaterialization,
        receipt: ReviewPublicationReceipt,
    ) -> Result<ReviewMaterializationOutcome, ReviewWorkspaceError> {
        if receipt.target != claim.target.head
            || receipt.snapshot.snapshot_id != claim.target.head.snapshot_id
        {
            return self
                .finish_failed(
                    claim,
                    ReviewCommitError::Integrity.into(),
                    self.clock.unix_millis(),
                )
                .await;
        }
        let queue = self.queue.clone();
        let claimed = claim.clone();
        super::service::io(move || queue.mark_published(&claimed, receipt)).await?;
        Ok(ReviewMaterializationOutcome::Published { receipt })
    }

    async fn finish_failed(
        &self,
        claim: ClaimedReviewMaterialization,
        error: ReviewWorkspaceError,
        now_ms: i64,
    ) -> Result<ReviewMaterializationOutcome, ReviewWorkspaceError> {
        let (code, permanent) = classify_failure(error);
        if permanent || claim.attempt_count > RETRY_DELAYS_MS.len() as u32 {
            let queue = self.queue.clone();
            let claimed = claim.clone();
            super::service::io(move || queue.block(&claimed, code)).await?;
            return Ok(ReviewMaterializationOutcome::Blocked {
                target: claim.target.head,
                code,
            });
        }
        let delay = RETRY_DELAYS_MS[claim.attempt_count.saturating_sub(1) as usize];
        let next_attempt_at_ms = now_ms.saturating_add(delay);
        let queue = self.queue.clone();
        let claimed = claim.clone();
        super::service::io(move || queue.retry(&claimed, code, next_attempt_at_ms)).await?;
        Ok(ReviewMaterializationOutcome::Retrying {
            target: claim.target.head,
            attempt_count: claim.attempt_count,
            next_attempt_at_ms,
        })
    }
}

const RETRY_DELAYS_MS: [i64; 5] = [100, 500, 2_000, 10_000, 30_000];

fn classify_failure(error: ReviewWorkspaceError) -> (ReviewMaterializationFailure, bool) {
    use ReviewMaterializationFailure as Failure;
    match error {
        ReviewWorkspaceError::Asset(ReviewAssetError::SourceChanged) => {
            (Failure::SourceChanged, true)
        }
        ReviewWorkspaceError::Asset(ReviewAssetError::NotFound) => (Failure::SourceMissing, true),
        ReviewWorkspaceError::Asset(
            ReviewAssetError::IndexUnavailable
            | ReviewAssetError::UnsafeSource
            | ReviewAssetError::Unavailable
            | ReviewAssetError::UnconfirmedLocation
            | ReviewAssetError::StaleLocator,
        ) => (Failure::SourceUnreadable, true),
        ReviewWorkspaceError::Asset(ReviewAssetError::LimitExceeded)
        | ReviewWorkspaceError::Evidence(ReviewArtifactError::LimitExceeded)
        | ReviewWorkspaceError::Repository(ReviewCommitError::LimitExceeded) => {
            (Failure::LimitExceeded, true)
        }
        ReviewWorkspaceError::Evidence(ReviewArtifactError::SourceChanged) => {
            (Failure::SourceChanged, true)
        }
        ReviewWorkspaceError::Evidence(
            ReviewArtifactError::InvalidRequest
            | ReviewArtifactError::UnsafeSource
            | ReviewArtifactError::DecodeFailed,
        )
        | ReviewWorkspaceError::Repository(ReviewCommitError::Integrity)
        | ReviewWorkspaceError::Domain(_)
        | ReviewWorkspaceError::WrongContext => (Failure::Integrity, true),
        ReviewWorkspaceError::Evidence(
            ReviewArtifactError::Unavailable | ReviewArtifactError::Cancelled,
        )
        | ReviewWorkspaceError::Asset(ReviewAssetError::Pending | ReviewAssetError::Cancelled)
        | ReviewWorkspaceError::Cancelled => (Failure::RenderFailed, false),
        ReviewWorkspaceError::Repository(ReviewCommitError::Io)
        | ReviewWorkspaceError::Repository(ReviewCommitError::LeaseBusy)
        | ReviewWorkspaceError::Repository(ReviewCommitError::OutcomeUnknown)
        | ReviewWorkspaceError::CommittedViewUnavailable(_)
        | ReviewWorkspaceError::CommittedAuthoringPatchUnavailable(_) => (Failure::Io, false),
        ReviewWorkspaceError::Asset(_) => (Failure::SourceUnreadable, true),
        ReviewWorkspaceError::Repository(_)
        | ReviewWorkspaceError::Usage(_)
        | ReviewWorkspaceError::NoChanges
        | ReviewWorkspaceError::CapabilityUnavailable
        | ReviewWorkspaceError::PreviewRequired => (Failure::Integrity, true),
    }
}
