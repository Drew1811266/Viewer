use super::{SqliteContinuousReviewAuthoringStore, codec};
use rusqlite::{TransactionBehavior, params};
use std::collections::HashSet;
use viewer_application::review_workspace::{
    GeneratedReviewIds, ReviewAuthoringHead, ReviewBarrierKind, ReviewCommitError, ReviewHeads,
    StoredAuthoringState, StoredContinuousSnapshot,
};
use viewer_domain::{
    FeedbackId, ReviewArchiveId, ReviewStreamId, ReviewTargetId, ReviewTargetRevisionId,
    ReviewTextRevisionId, review::continuous::ReviewChangeKind,
};

impl SqliteContinuousReviewAuthoringStore {
    pub(in crate::review) fn bootstrap_published(
        &self,
        stream: ReviewStreamId,
        current: Option<StoredAuthoringState>,
    ) -> Result<ReviewHeads, ReviewCommitError> {
        if !self.writable {
            return Err(ReviewCommitError::ReadOnly);
        }
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(super::store::map_database_error)?;
        let existing = super::store::load_heads_from(&transaction, stream)?;
        if existing.authoring.is_some() || existing.published.is_some() {
            transaction
                .commit()
                .map_err(super::store::map_database_error)?;
            return Ok(existing);
        }
        let production_scope = match current.as_ref() {
            Some(value) => codec::encode(self.project_id, stream, value)?.production_scope,
            None => codec::encode_production_scope(None)?,
        };
        let updated_at_ms = current
            .as_ref()
            .map_or(0, |value| value.generated.created_at_ms);
        super::store::ensure_stream(
            &transaction,
            self.project_id,
            stream,
            &production_scope,
            updated_at_ms,
        )?;
        if let Some(current) = current {
            if current.head.sequence != 1 {
                return Err(ReviewCommitError::Integrity);
            }
            let encoded = codec::encode(self.project_id, stream, &current)?;
            transaction
                .execute(
                    "INSERT INTO review_authoring_snapshots(
                        stream_id, seq, snapshot_id, command_id, parent_seq, payload_digest,
                        logical_state, transition, generated_ids, barrier_kind, created_at_ms
                     ) VALUES (?1, 1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        stream.to_string(),
                        current.head.snapshot_id.to_string(),
                        current.command_id.to_string(),
                        current.payload_digest.as_slice(),
                        encoded.logical_state,
                        encoded.transition,
                        encoded.generated_ids,
                        super::store::barrier_code(current.barrier),
                        current.generated.created_at_ms,
                    ],
                )
                .map_err(super::store::map_database_error)?;
            transaction
                .execute(
                    "UPDATE review_authoring_streams
                     SET authoring_seq = 1, authoring_snapshot_id = ?2,
                         published_seq = 1, published_snapshot_id = ?2, updated_at_ms = ?3
                     WHERE stream_id = ?1",
                    params![
                        stream.to_string(),
                        current.head.snapshot_id.to_string(),
                        current.generated.created_at_ms,
                    ],
                )
                .map_err(super::store::map_database_error)?;
        }
        transaction
            .commit()
            .map_err(super::store::map_database_error)?;
        drop(connection);
        let connection = self.connection()?;
        super::store::load_heads_from(&connection, stream)
    }
}

pub(in crate::review) fn from_published(value: StoredContinuousSnapshot) -> StoredAuthoringState {
    let StoredContinuousSnapshot {
        production,
        state,
        command_id,
        payload_digest,
        changes,
        ..
    } = value;
    let snapshot_id = state.snapshot_id;
    let feedback_id = changes
        .iter()
        .filter_map(|change| change.after.map(|key| key.feedback_id))
        .next()
        .or_else(|| state.feedback.first().map(|feedback| feedback.id))
        .unwrap_or_else(|| FeedbackId::from_u128(0));
    let text_revision_id = changes
        .iter()
        .filter_map(|change| change.after.map(|key| key.text_revision_id))
        .next()
        .or_else(|| {
            state
                .feedback
                .first()
                .map(|feedback| feedback.text_revision_id)
        })
        .unwrap_or_else(|| ReviewTextRevisionId::from_u128(0));
    let archive_id = changes
        .iter()
        .find_map(|change| change.archive_id)
        .unwrap_or_else(|| ReviewArchiveId::from_u128(0));
    let targets = generated_targets(&changes);
    let created_at_ms = state
        .feedback
        .iter()
        .map(|feedback| feedback.created_at_ms)
        .max()
        .unwrap_or(0);
    let barrier = if changes
        .iter()
        .any(|change| change.kind == ReviewChangeKind::Archived)
    {
        ReviewBarrierKind::Archive
    } else if changes
        .iter()
        .any(|change| change.kind == ReviewChangeKind::Restored)
    {
        ReviewBarrierKind::Restore
    } else {
        ReviewBarrierKind::Migration
    };
    StoredAuthoringState {
        head: ReviewAuthoringHead {
            sequence: 1,
            snapshot_id,
        },
        production,
        state,
        command_id,
        payload_digest,
        generated: GeneratedReviewIds {
            snapshot_id,
            feedback_id,
            text_revision_id,
            archive_id,
            targets,
            migration: vec![],
            created_at_ms,
        },
        changes,
        archives: vec![],
        adopted_usage: vec![],
        barrier,
    }
}

fn generated_targets(
    changes: &[viewer_domain::review::continuous::ReviewChange],
) -> Vec<(ReviewTargetId, ReviewTargetRevisionId)> {
    let mut target_ids = HashSet::new();
    let mut targets = changes
        .iter()
        .rev()
        .filter_map(|change| {
            change
                .after
                .map(|key| (key.target_id, key.target_revision_id))
        })
        .filter(|identity| target_ids.insert(identity.0))
        .collect::<Vec<_>>();
    targets.reverse();
    targets
}
