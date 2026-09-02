use super::codec;
use super::queue::{MATERIALIZATION_LEASE_MS, failure_code, parse_failure};
use crate::portable::{PortablePersistenceMode, PortableProjectMetadata};
use rusqlite::{
    Connection, ErrorCode, OptionalExtension, Transaction, TransactionBehavior, params,
};
use std::{path::Path, str::FromStr, sync::Mutex};
use viewer_application::{
    ProjectAccess,
    review_workspace::{
        AuthoringCommandLookup, ClaimedReviewMaterialization, ContinuousReviewAuthoringStorePort,
        ReviewAuthoringCommitRequest, ReviewAuthoringHead, ReviewAuthoringReceipt,
        ReviewCommitError, ReviewHeads, ReviewMaterializationFailure,
        ReviewMaterializationQueuePort, ReviewPublicationReceipt, ReviewPublicationStatus,
        ReviewWorkspaceError, StoredAuthoringState,
    },
};
use viewer_domain::{ProjectId, ReviewCommandId, ReviewSnapshotId, ReviewStreamId};

pub struct SqliteContinuousReviewAuthoringStore {
    connection: Mutex<Connection>,
    pub(super) project_id: ProjectId,
    pub(super) writable: bool,
    persistence_mode: PortablePersistenceMode,
}

impl SqliteContinuousReviewAuthoringStore {
    pub fn open(
        project_root: &Path,
        project_id: ProjectId,
        access: ProjectAccess,
    ) -> Result<Self, ReviewCommitError> {
        let metadata = PortableProjectMetadata::open(project_root, access, 0)
            .map_err(|_| ReviewCommitError::Integrity)?;
        if metadata.project_id() != project_id {
            return Err(ReviewCommitError::Integrity);
        }
        let path = metadata
            .database_path()
            .ok_or(ReviewCommitError::Integrity)?
            .to_path_buf();
        drop(metadata);
        let writable = access == ProjectAccess::ReadWrite;
        let connection = crate::portable::schema::open_database(&path, writable)
            .map_err(|_| ReviewCommitError::Io)?;
        let stored_project: String = connection
            .query_row(
                "SELECT project_id FROM project_metadata WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .map_err(map_database_error)?;
        if stored_project != project_id.to_string() {
            return Err(ReviewCommitError::Integrity);
        }
        let persistence_mode =
            crate::portable::schema::persistence_mode(&connection).map_err(map_database_error)?;
        Ok(Self {
            connection: Mutex::new(connection),
            project_id,
            writable,
            persistence_mode,
        })
    }

    pub fn persistence_mode(&self) -> PortablePersistenceMode {
        self.persistence_mode
    }

    pub(super) fn connection(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Connection>, ReviewCommitError> {
        self.connection.lock().map_err(|_| ReviewCommitError::Io)
    }
}

impl ContinuousReviewAuthoringStorePort for SqliteContinuousReviewAuthoringStore {
    fn load_heads(&self, stream: ReviewStreamId) -> Result<ReviewHeads, ReviewCommitError> {
        let connection = self.connection()?;
        load_heads_from(&connection, stream)
    }

    fn load_current(
        &self,
        stream: ReviewStreamId,
    ) -> Result<Option<StoredAuthoringState>, ReviewCommitError> {
        let connection = self.connection()?;
        load_current_from(&connection, self.project_id, stream)
    }

    fn find_command(
        &self,
        stream: ReviewStreamId,
        command: ReviewCommandId,
    ) -> Result<AuthoringCommandLookup, ReviewCommitError> {
        let connection = self.connection()?;
        Ok(
            match find_command_in(&connection, self.project_id, stream, command)? {
                Some(receipt) => AuthoringCommandLookup::Found(receipt),
                None => AuthoringCommandLookup::Absent,
            },
        )
    }

    fn commit(
        &self,
        stream: ReviewStreamId,
        command: ReviewCommandId,
        payload_digest: [u8; 32],
        prepare: &mut dyn FnMut(
            Option<&StoredAuthoringState>,
        )
            -> Result<ReviewAuthoringCommitRequest, ReviewWorkspaceError>,
    ) -> Result<ReviewAuthoringReceipt, ReviewWorkspaceError> {
        if !self.writable {
            return Err(ReviewCommitError::ReadOnly.into());
        }
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_database_error)?;
        if let Some(receipt) = find_command_in(&transaction, self.project_id, stream, command)? {
            if receipt.payload_digest != payload_digest {
                return Err(ReviewCommitError::CommandConflict.into());
            }
            return Ok(receipt);
        }
        let current = load_current_from(&transaction, self.project_id, stream)?;
        let request = prepare(current.as_ref())?;
        let actual_snapshot_id = current.as_ref().map(|value| value.head.snapshot_id);
        if request.expected_snapshot_id != actual_snapshot_id {
            return Err(ReviewCommitError::StaleSnapshot.into());
        }
        validate_next(
            self.project_id,
            stream,
            command,
            payload_digest,
            current.as_ref(),
            &request.next,
        )?;
        let encoded = codec::encode(self.project_id, stream, &request.next)?;
        ensure_stream(
            &transaction,
            self.project_id,
            stream,
            &encoded.production_scope,
            request.next.generated.created_at_ms,
        )?;
        let parent_seq = current.as_ref().map(|value| value.head.sequence as i64);
        transaction
            .execute(
                "INSERT INTO review_authoring_snapshots(
                    stream_id, seq, snapshot_id, command_id, parent_seq, payload_digest,
                    logical_state, transition, generated_ids, barrier_kind, created_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    stream.to_string(),
                    request.next.head.sequence as i64,
                    request.next.head.snapshot_id.to_string(),
                    command.to_string(),
                    parent_seq,
                    payload_digest.as_slice(),
                    encoded.logical_state,
                    encoded.transition,
                    encoded.generated_ids,
                    barrier_code(request.next.barrier),
                    request.next.generated.created_at_ms,
                ],
            )
            .map_err(map_database_error)?;
        transaction
            .execute(
                "UPDATE review_authoring_streams
                 SET authoring_seq = ?2, authoring_snapshot_id = ?3, updated_at_ms = ?4
                 WHERE stream_id = ?1",
                params![
                    stream.to_string(),
                    request.next.head.sequence as i64,
                    request.next.head.snapshot_id.to_string(),
                    request.next.generated.created_at_ms,
                ],
            )
            .map_err(map_database_error)?;
        transaction
            .execute(
                "INSERT INTO review_materialization_jobs(
                    stream_id, target_seq, status, attempt_count, lease_epoch,
                    next_attempt_at_ms, error_code
                 ) VALUES (?1, ?2, 'queued', 0, 0, ?3, NULL)",
                params![
                    stream.to_string(),
                    request.next.head.sequence as i64,
                    request.next.generated.created_at_ms,
                ],
            )
            .map_err(map_database_error)?;
        transaction.commit().map_err(map_database_error)?;
        Ok(ReviewAuthoringReceipt {
            command_id: command,
            payload_digest,
            head: request.next.head,
        })
    }
}

impl ReviewMaterializationQueuePort for SqliteContinuousReviewAuthoringStore {
    fn next(&self, now_ms: i64) -> Result<Option<ClaimedReviewMaterialization>, ReviewCommitError> {
        if !self.writable {
            return Err(ReviewCommitError::ReadOnly);
        }
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_database_error)?;
        let row = transaction
            .query_row(
                "SELECT stream_id, target_seq, attempt_count, lease_epoch
                 FROM review_materialization_jobs
                 WHERE status IN ('queued', 'retryable') AND next_attempt_at_ms <= ?1
                 ORDER BY next_attempt_at_ms, stream_id, target_seq
                 LIMIT 1",
                [now_ms],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(map_database_error)?;
        let Some((stream_text, target_seq, attempt_count, lease_epoch)) = row else {
            transaction.commit().map_err(map_database_error)?;
            return Ok(None);
        };
        let stream = parse_id::<ReviewStreamId>(&stream_text)?;
        let target = load_snapshot_from(&transaction, self.project_id, stream, target_seq as u64)?;
        let next_attempt = attempt_count
            .checked_add(1)
            .ok_or(ReviewCommitError::LimitExceeded)?;
        let next_epoch = lease_epoch
            .checked_add(1)
            .ok_or(ReviewCommitError::LimitExceeded)?;
        transaction
            .execute(
                "UPDATE review_materialization_jobs
                 SET status = 'running', attempt_count = ?3, lease_epoch = ?4,
                     next_attempt_at_ms = ?5, error_code = NULL
                 WHERE stream_id = ?1 AND target_seq = ?2",
                params![
                    stream_text,
                    target_seq,
                    next_attempt,
                    next_epoch,
                    now_ms.saturating_add(MATERIALIZATION_LEASE_MS),
                ],
            )
            .map_err(map_database_error)?;
        transaction.commit().map_err(map_database_error)?;
        Ok(Some(ClaimedReviewMaterialization {
            stream_id: stream,
            target,
            lease_epoch: next_epoch as u64,
            attempt_count: next_attempt as u32,
        }))
    }

    fn retry(
        &self,
        claim: &ClaimedReviewMaterialization,
        code: ReviewMaterializationFailure,
        next_attempt_at_ms: i64,
    ) -> Result<(), ReviewCommitError> {
        self.finish_claim(claim, "retryable", Some(code), next_attempt_at_ms)
    }

    fn block(
        &self,
        claim: &ClaimedReviewMaterialization,
        code: ReviewMaterializationFailure,
    ) -> Result<(), ReviewCommitError> {
        self.finish_claim(claim, "blocked", Some(code), 0)
    }

    fn mark_published(
        &self,
        claim: &ClaimedReviewMaterialization,
        receipt: ReviewPublicationReceipt,
    ) -> Result<(), ReviewCommitError> {
        if !self.writable {
            return Err(ReviewCommitError::ReadOnly);
        }
        if receipt.target != claim.target.head
            || receipt.snapshot.snapshot_id != claim.target.head.snapshot_id
        {
            return Err(ReviewCommitError::Integrity);
        }
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_database_error)?;
        require_claim(&transaction, claim)?;
        let heads = load_heads_from(&transaction, claim.stream_id)?;
        if heads
            .authoring
            .is_none_or(|head| head.sequence < claim.target.head.sequence)
            || heads
                .published
                .is_some_and(|head| head.sequence > claim.target.head.sequence)
        {
            return Err(ReviewCommitError::Integrity);
        }
        transaction
            .execute(
                "UPDATE review_authoring_streams
                 SET published_seq = ?2, published_snapshot_id = ?3, updated_at_ms = ?4
                 WHERE stream_id = ?1",
                params![
                    claim.stream_id.to_string(),
                    claim.target.head.sequence as i64,
                    claim.target.head.snapshot_id.to_string(),
                    claim.target.generated.created_at_ms,
                ],
            )
            .map_err(map_database_error)?;
        transaction
            .execute(
                "DELETE FROM review_materialization_jobs
                 WHERE stream_id = ?1 AND target_seq <= ?2",
                params![
                    claim.stream_id.to_string(),
                    claim.target.head.sequence as i64
                ],
            )
            .map_err(map_database_error)?;
        transaction.commit().map_err(map_database_error)
    }

    fn status(&self, stream: ReviewStreamId) -> Result<ReviewPublicationStatus, ReviewCommitError> {
        let connection = self.connection()?;
        let heads = load_heads_from(&connection, stream)?;
        if heads.authoring == heads.published {
            return Ok(ReviewPublicationStatus::Ready);
        }
        let blocked = connection
            .query_row(
                "SELECT error_code FROM review_materialization_jobs
                 WHERE stream_id = ?1 AND status = 'blocked'
                 ORDER BY target_seq DESC LIMIT 1",
                [stream.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(map_database_error)?;
        if let Some(code) = blocked {
            return Ok(ReviewPublicationStatus::Blocked {
                code: parse_failure(&code)?,
            });
        }
        let authoring = heads.authoring.map_or(0, |head| head.sequence);
        let published = heads.published.map_or(0, |head| head.sequence);
        Ok(ReviewPublicationStatus::Pending {
            pending_revisions: authoring.saturating_sub(published).min(u32::MAX as u64) as u32,
        })
    }

    fn requeue_expired(&self, now_ms: i64) -> Result<u32, ReviewCommitError> {
        if !self.writable {
            return Err(ReviewCommitError::ReadOnly);
        }
        let connection = self.connection()?;
        let count = connection
            .execute(
                "UPDATE review_materialization_jobs
                 SET status = 'queued', error_code = NULL, next_attempt_at_ms = ?1
                 WHERE status = 'running' AND next_attempt_at_ms <= ?1",
                [now_ms],
            )
            .map_err(map_database_error)?;
        u32::try_from(count).map_err(|_| ReviewCommitError::LimitExceeded)
    }
}

impl SqliteContinuousReviewAuthoringStore {
    fn finish_claim(
        &self,
        claim: &ClaimedReviewMaterialization,
        status: &str,
        code: Option<ReviewMaterializationFailure>,
        next_attempt_at_ms: i64,
    ) -> Result<(), ReviewCommitError> {
        if !self.writable {
            return Err(ReviewCommitError::ReadOnly);
        }
        if next_attempt_at_ms < 0 {
            return Err(ReviewCommitError::Integrity);
        }
        let connection = self.connection()?;
        let updated = connection
            .execute(
                "UPDATE review_materialization_jobs
                 SET status = ?4, error_code = ?5, next_attempt_at_ms = ?6
                 WHERE stream_id = ?1 AND target_seq = ?2
                   AND status = 'running' AND lease_epoch = ?3",
                params![
                    claim.stream_id.to_string(),
                    claim.target.head.sequence as i64,
                    claim.lease_epoch as i64,
                    status,
                    code.map(failure_code),
                    next_attempt_at_ms,
                ],
            )
            .map_err(map_database_error)?;
        if updated == 1 {
            Ok(())
        } else {
            Err(ReviewCommitError::StaleSnapshot)
        }
    }
}

pub(super) fn ensure_stream(
    transaction: &Transaction<'_>,
    project_id: ProjectId,
    stream: ReviewStreamId,
    production_scope: &[u8],
    now_ms: i64,
) -> Result<(), ReviewCommitError> {
    transaction
        .execute(
            "INSERT OR IGNORE INTO review_authoring_streams(
                stream_id, project_id, production_scope, authoring_seq,
                authoring_snapshot_id, published_seq, published_snapshot_id, updated_at_ms
             ) VALUES (?1, ?2, ?3, NULL, NULL, NULL, NULL, ?4)",
            params![
                stream.to_string(),
                project_id.to_string(),
                production_scope,
                now_ms
            ],
        )
        .map_err(map_database_error)?;
    let stored: (String, Vec<u8>) = transaction
        .query_row(
            "SELECT project_id, production_scope FROM review_authoring_streams WHERE stream_id = ?1",
            [stream.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(map_database_error)?;
    if stored.0 != project_id.to_string() || stored.1 != production_scope {
        return Err(ReviewCommitError::Integrity);
    }
    Ok(())
}

fn validate_next(
    project_id: ProjectId,
    stream: ReviewStreamId,
    command: ReviewCommandId,
    payload_digest: [u8; 32],
    current: Option<&StoredAuthoringState>,
    next: &StoredAuthoringState,
) -> Result<(), ReviewCommitError> {
    let expected_sequence = current.map_or(1, |value| value.head.sequence.saturating_add(1));
    if next.head.sequence != expected_sequence
        || next.command_id != command
        || next.payload_digest != payload_digest
        || next.state.project_id != project_id
        || next.state.stream_id != stream
        || next.state.snapshot_id != next.head.snapshot_id
        || next.generated.snapshot_id != next.head.snapshot_id
        || current.is_some_and(|value| value.production != next.production)
    {
        return Err(ReviewCommitError::Integrity);
    }
    Ok(())
}

pub(super) fn load_heads_from(
    connection: &Connection,
    stream: ReviewStreamId,
) -> Result<ReviewHeads, ReviewCommitError> {
    let row = connection
        .query_row(
            "SELECT authoring_seq, authoring_snapshot_id, published_seq, published_snapshot_id
             FROM review_authoring_streams WHERE stream_id = ?1",
            [stream.to_string()],
            |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((authoring_seq, authoring_id, published_seq, published_id)) = row else {
        return Ok(ReviewHeads {
            authoring: None,
            published: None,
        });
    };
    Ok(ReviewHeads {
        authoring: parse_head(authoring_seq, authoring_id)?,
        published: parse_head(published_seq, published_id)?,
    })
}

fn parse_head(
    sequence: Option<i64>,
    snapshot_id: Option<String>,
) -> Result<Option<ReviewAuthoringHead>, ReviewCommitError> {
    match (sequence, snapshot_id) {
        (None, None) => Ok(None),
        (Some(sequence), Some(snapshot_id)) if sequence > 0 => Ok(Some(ReviewAuthoringHead {
            sequence: sequence as u64,
            snapshot_id: parse_id(&snapshot_id)?,
        })),
        _ => Err(ReviewCommitError::Integrity),
    }
}

fn load_current_from(
    connection: &Connection,
    project_id: ProjectId,
    stream: ReviewStreamId,
) -> Result<Option<StoredAuthoringState>, ReviewCommitError> {
    let sequence = connection
        .query_row(
            "SELECT authoring_seq FROM review_authoring_streams WHERE stream_id = ?1",
            [stream.to_string()],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()
        .map_err(map_database_error)?
        .flatten();
    sequence
        .map(|value| {
            if value <= 0 {
                Err(ReviewCommitError::Integrity)
            } else {
                load_snapshot_from(connection, project_id, stream, value as u64)
            }
        })
        .transpose()
}

fn load_snapshot_from(
    connection: &Connection,
    project_id: ProjectId,
    stream: ReviewStreamId,
    sequence: u64,
) -> Result<StoredAuthoringState, ReviewCommitError> {
    let row = connection
        .query_row(
            "SELECT snapshot_id, command_id, parent_seq, payload_digest, logical_state,
                    transition, generated_ids, barrier_kind, created_at_ms
             FROM review_authoring_snapshots
             WHERE stream_id = ?1 AND seq = ?2",
            params![stream.to_string(), sequence as i64],
            |row| {
                Ok(SnapshotRow {
                    snapshot_id: row.get(0)?,
                    command_id: row.get(1)?,
                    parent_seq: row.get(2)?,
                    payload_digest: row.get(3)?,
                    logical_state: row.get(4)?,
                    transition: row.get(5)?,
                    generated_ids: row.get(6)?,
                    barrier_kind: row.get(7)?,
                    created_at_ms: row.get(8)?,
                })
            },
        )
        .map_err(map_database_error)?;
    let value = codec::decode(project_id, stream, &row.logical_state)?;
    let encoded = codec::encode(project_id, stream, &value)?;
    let expected_parent = (sequence > 1).then_some((sequence - 1) as i64);
    let stored_production: Vec<u8> = connection
        .query_row(
            "SELECT production_scope FROM review_authoring_streams WHERE stream_id = ?1",
            [stream.to_string()],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if row.payload_digest.len() != 32 {
        return Err(ReviewCommitError::Integrity);
    }
    let mut stored_digest = [0_u8; 32];
    stored_digest.copy_from_slice(&row.payload_digest);
    if value.head.sequence != sequence
        || value.head.snapshot_id != parse_id::<ReviewSnapshotId>(&row.snapshot_id)?
        || value.command_id != parse_id::<ReviewCommandId>(&row.command_id)?
        || value.payload_digest != stored_digest
        || row.parent_seq != expected_parent
        || row.barrier_kind != barrier_code(value.barrier)
        || row.created_at_ms != value.generated.created_at_ms
        || row.logical_state != encoded.logical_state
        || row.transition != encoded.transition
        || row.generated_ids != encoded.generated_ids
        || stored_production != encoded.production_scope
    {
        return Err(ReviewCommitError::Integrity);
    }
    Ok(value)
}

struct SnapshotRow {
    snapshot_id: String,
    command_id: String,
    parent_seq: Option<i64>,
    payload_digest: Vec<u8>,
    logical_state: Vec<u8>,
    transition: Vec<u8>,
    generated_ids: Vec<u8>,
    barrier_kind: String,
    created_at_ms: i64,
}

fn find_command_in(
    connection: &Connection,
    project_id: ProjectId,
    stream: ReviewStreamId,
    command: ReviewCommandId,
) -> Result<Option<ReviewAuthoringReceipt>, ReviewCommitError> {
    let row = connection
        .query_row(
            "SELECT stream_id, seq, snapshot_id, payload_digest
             FROM review_authoring_snapshots WHERE command_id = ?1",
            [command.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((stored_stream, sequence, snapshot_id, digest)) = row else {
        return Ok(None);
    };
    if stored_stream != stream.to_string() || sequence <= 0 || digest.len() != 32 {
        return Err(ReviewCommitError::CommandConflict);
    }
    let mut payload_digest = [0_u8; 32];
    payload_digest.copy_from_slice(&digest);
    let receipt = ReviewAuthoringReceipt {
        command_id: command,
        payload_digest,
        head: ReviewAuthoringHead {
            sequence: sequence as u64,
            snapshot_id: parse_id(&snapshot_id)?,
        },
    };
    let stored = load_snapshot_from(connection, project_id, stream, receipt.head.sequence)?;
    if stored.command_id != receipt.command_id
        || stored.payload_digest != receipt.payload_digest
        || stored.head != receipt.head
    {
        return Err(ReviewCommitError::Integrity);
    }
    Ok(Some(receipt))
}

fn require_claim(
    connection: &Connection,
    claim: &ClaimedReviewMaterialization,
) -> Result<(), ReviewCommitError> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM review_materialization_jobs
                WHERE stream_id = ?1 AND target_seq = ?2
                  AND status = 'running' AND lease_epoch = ?3
             )",
            params![
                claim.stream_id.to_string(),
                claim.target.head.sequence as i64,
                claim.lease_epoch as i64,
            ],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if exists {
        Ok(())
    } else {
        Err(ReviewCommitError::StaleSnapshot)
    }
}

fn parse_id<T>(value: &str) -> Result<T, ReviewCommitError>
where
    T: FromStr + std::fmt::Display,
{
    if value.len() != 36 {
        return Err(ReviewCommitError::Integrity);
    }
    let parsed: T = value.parse().map_err(|_| ReviewCommitError::Integrity)?;
    if parsed.to_string() != value {
        return Err(ReviewCommitError::Integrity);
    }
    Ok(parsed)
}

pub(super) fn barrier_code(
    value: viewer_application::review_workspace::ReviewBarrierKind,
) -> &'static str {
    use viewer_application::review_workspace::ReviewBarrierKind::*;
    match value {
        None => "none",
        Archive => "archive",
        Restore => "restore",
        Migration => "migration",
    }
}

pub(super) fn map_database_error(error: rusqlite::Error) -> ReviewCommitError {
    match &error {
        rusqlite::Error::SqliteFailure(failure, _)
            if matches!(
                failure.code,
                ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked
            ) =>
        {
            ReviewCommitError::LeaseBusy
        }
        rusqlite::Error::SqliteFailure(failure, _)
            if matches!(
                failure.code,
                ErrorCode::ConstraintViolation | ErrorCode::ReadOnly
            ) =>
        {
            if failure.code == ErrorCode::ReadOnly {
                ReviewCommitError::ReadOnly
            } else {
                ReviewCommitError::Integrity
            }
        }
        _ => ReviewCommitError::Io,
    }
}
