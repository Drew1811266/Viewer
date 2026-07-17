use crate::portable::schema::{PortableSchemaError, open_database};
use rusqlite::{Connection, OptionalExtension, params};
use std::{path::Path, str::FromStr, sync::Mutex};
use viewer_domain::{
    EntityId, OperationId, RelativePath,
    operation::{
        ConflictPolicy, OperationItemPlan, OperationKind, OperationState, OperationTransitionError,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JournalItem {
    pub operation_id: OperationId,
    pub batch_id: OperationId,
    pub entity_id: EntityId,
    pub kind: OperationKind,
    pub state: OperationState,
    pub source: RelativePath,
    pub destination: Option<RelativePath>,
    pub temporary: Option<RelativePath>,
    pub expected_size: Option<u64>,
    pub expected_hash: Option<[u8; 32]>,
    pub conflict_policy: ConflictPolicy,
    pub error_code: Option<String>,
    pub updated_at_ms: i64,
    pub result_code: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BatchState {
    Running,
    Completed,
}

impl BatchState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JournalBatch {
    pub batch_id: OperationId,
    pub kind: OperationKind,
    pub state: BatchState,
    pub requested_count: u32,
    pub completed_count: u32,
    pub failed_count: u32,
    pub skipped_count: u32,
    pub created_at_ms: i64,
    pub started_at_ms: i64,
    pub completed_at_ms: Option<i64>,
}

#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    #[error("operation journal database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error(transparent)]
    InvalidTransition(#[from] OperationTransitionError),
    #[error("operation {operation_id} did not have expected state {expected:?}")]
    ConcurrentStateChange {
        operation_id: OperationId,
        expected: OperationState,
    },
    #[error("unsupported portable metadata schema version {0}")]
    UnsupportedSchema(i64),
    #[error(transparent)]
    Schema(#[from] PortableSchemaError),
    #[error("invalid persisted {field}: {value}")]
    InvalidPersistedValue { field: &'static str, value: String },
    #[error(
        "operation result code must contain 1-64 lowercase ASCII letters, digits or underscores"
    )]
    InvalidResultCode,
    #[error("operation batch item count must be between 1 and 10000")]
    InvalidBatchCount,
    #[error("operation result page size must be between 1 and 200")]
    InvalidPageSize,
    #[error("operation batch {0} is missing, complete, or already contains every requested item")]
    BatchNotWritable(OperationId),
    #[error(
        "operation batch {batch_id} cannot finish: requested {requested}, accounted {accounted}"
    )]
    IncompleteBatch {
        batch_id: OperationId,
        requested: u32,
        accounted: u32,
    },
    #[error("terminal operation transitions require a result-aware journal API")]
    TerminalTransitionRequiresResultApi,
}

pub struct OperationJournal {
    connection: Mutex<Connection>,
}

impl OperationJournal {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, JournalError> {
        let connection = open_database(path.as_ref(), true)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn begin_batch(
        &self,
        batch_id: OperationId,
        kind: OperationKind,
        requested_count: u32,
        created_at_ms: i64,
    ) -> Result<(), JournalError> {
        if !(1..=10_000).contains(&requested_count) {
            return Err(JournalError::InvalidBatchCount);
        }
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO operation_batches(
                    batch_id, kind, created_at_ms, state, requested_count,
                    completed_count, failed_count, skipped_count, started_at_ms
                 ) VALUES (?1, ?2, ?3, 'running', ?4, 0, 0, 0, ?3)",
                params![
                    batch_id.to_string(),
                    kind.as_str(),
                    created_at_ms,
                    requested_count,
                ],
            )?;
            Ok(())
        })
    }

    pub fn record_item(
        &self,
        item: &OperationItemPlan,
        updated_at_ms: i64,
    ) -> Result<(), JournalError> {
        self.with_connection(|connection| {
            let changed = connection.execute(
                "INSERT INTO operation_items(
                    operation_id, batch_id, entity_id, kind, state, source_path,
                    destination_path, conflict_policy, updated_at_ms
                 )
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9
                 WHERE EXISTS (
                    SELECT 1 FROM operation_batches AS batch
                    WHERE batch.batch_id = ?2
                      AND batch.state = 'running'
                      AND batch.kind = ?4
                      AND (
                        SELECT COUNT(*) FROM operation_items AS existing
                        WHERE existing.batch_id = batch.batch_id
                      ) < batch.requested_count
                 )",
                params![
                    item.operation_id.to_string(),
                    item.batch_id.to_string(),
                    item.entity_id.to_string(),
                    item.kind.as_str(),
                    OperationState::Prepared.as_str(),
                    item.source.as_str(),
                    item.destination.as_ref().map(RelativePath::as_str),
                    item.conflict_policy.as_str(),
                    updated_at_ms,
                ],
            )?;
            if changed != 1 {
                return Err(JournalError::BatchNotWritable(item.batch_id));
            }
            Ok(())
        })
    }

    pub fn advance(
        &self,
        operation_id: OperationId,
        expected_current: OperationState,
        next: OperationState,
        updated_at_ms: i64,
    ) -> Result<(), JournalError> {
        if next == OperationState::Completed {
            return Err(JournalError::TerminalTransitionRequiresResultApi);
        }
        let mut checked = expected_current;
        checked.transition_to(next)?;
        self.with_connection(|connection| {
            let changed = connection.execute(
                "UPDATE operation_items
                 SET state = ?3, updated_at_ms = ?4
                 WHERE operation_id = ?1 AND state = ?2",
                params![
                    operation_id.to_string(),
                    expected_current.as_str(),
                    next.as_str(),
                    updated_at_ms,
                ],
            )?;
            if changed != 1 {
                return Err(JournalError::ConcurrentStateChange {
                    operation_id,
                    expected: expected_current,
                });
            }
            Ok(())
        })
    }

    pub fn complete_item(
        &self,
        operation_id: OperationId,
        expected_current: OperationState,
        result_code: &str,
        updated_at_ms: i64,
    ) -> Result<(), JournalError> {
        let mut checked = expected_current;
        checked.transition_to(OperationState::Completed)?;
        self.finish_item(
            operation_id,
            expected_current,
            OperationState::Completed,
            result_code,
            None,
            "completed_count",
            updated_at_ms,
        )
    }

    pub fn fail_item(
        &self,
        operation_id: OperationId,
        expected_current: OperationState,
        error_code: &str,
        updated_at_ms: i64,
    ) -> Result<(), JournalError> {
        let mut checked = expected_current;
        checked.transition_to(OperationState::Failed)?;
        self.finish_item(
            operation_id,
            expected_current,
            OperationState::Failed,
            error_code,
            Some(error_code),
            "failed_count",
            updated_at_ms,
        )
    }

    pub fn fail(
        &self,
        operation_id: OperationId,
        expected_current: OperationState,
        error_code: &str,
        updated_at_ms: i64,
    ) -> Result<(), JournalError> {
        self.fail_item(operation_id, expected_current, error_code, updated_at_ms)
    }

    pub fn skip_item(
        &self,
        operation_id: OperationId,
        expected_current: OperationState,
        result_code: &str,
        updated_at_ms: i64,
    ) -> Result<(), JournalError> {
        if matches!(
            expected_current,
            OperationState::Completed | OperationState::Failed
        ) {
            return Err(OperationTransitionError::Invalid {
                from: expected_current,
                to: OperationState::Completed,
            }
            .into());
        }
        self.finish_item(
            operation_id,
            expected_current,
            OperationState::Completed,
            result_code,
            None,
            "skipped_count",
            updated_at_ms,
        )
    }

    pub fn register_temporary(
        &self,
        operation_id: OperationId,
        expected_current: OperationState,
        temporary: &RelativePath,
        updated_at_ms: i64,
    ) -> Result<(), JournalError> {
        self.update_expected_state(
            operation_id,
            expected_current,
            "UPDATE operation_items
             SET temporary_path = ?3, updated_at_ms = ?4
             WHERE operation_id = ?1 AND state = ?2",
            params![
                operation_id.to_string(),
                expected_current.as_str(),
                temporary.as_str(),
                updated_at_ms,
            ],
        )
    }

    pub fn record_prepared_evidence(
        &self,
        operation_id: OperationId,
        temporary: Option<&RelativePath>,
        expected_size: u64,
        expected_hash: [u8; 32],
        updated_at_ms: i64,
    ) -> Result<(), JournalError> {
        let expected_size = checked_size(expected_size)?;
        self.update_expected_state(
            operation_id,
            OperationState::Prepared,
            "UPDATE operation_items
             SET temporary_path = ?3, expected_size = ?4, expected_hash = ?5,
                 updated_at_ms = ?6
             WHERE operation_id = ?1 AND state = ?2",
            params![
                operation_id.to_string(),
                OperationState::Prepared.as_str(),
                temporary.map(RelativePath::as_str),
                expected_size,
                expected_hash.as_slice(),
                updated_at_ms,
            ],
        )
    }

    pub fn record_fs_applied(
        &self,
        operation_id: OperationId,
        expected_current: OperationState,
        expected_size: u64,
        expected_hash: [u8; 32],
        updated_at_ms: i64,
    ) -> Result<(), JournalError> {
        let mut checked = expected_current;
        checked.transition_to(OperationState::FsApplied)?;
        let expected_size = checked_size(expected_size)?;
        self.update_expected_state(
            operation_id,
            expected_current,
            "UPDATE operation_items
             SET state = 'fs_applied', expected_size = ?3, expected_hash = ?4,
                 updated_at_ms = ?5
             WHERE operation_id = ?1 AND state = ?2",
            params![
                operation_id.to_string(),
                expected_current.as_str(),
                expected_size,
                expected_hash.as_slice(),
                updated_at_ms,
            ],
        )
    }

    pub fn item(&self, operation_id: OperationId) -> Result<Option<JournalItem>, JournalError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT operation_id, batch_id, entity_id, kind, state, source_path,
                            destination_path, temporary_path, expected_size, expected_hash,
                            conflict_policy, error_code, updated_at_ms, result_code
                     FROM operation_items WHERE operation_id = ?1",
                    [operation_id.to_string()],
                    read_journal_row,
                )
                .optional()
                .map_err(Into::into)
        })
    }

    pub fn incomplete_items(&self) -> Result<Vec<JournalItem>, JournalError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT operation_id, batch_id, entity_id, kind, state, source_path,
                        destination_path, temporary_path, expected_size, expected_hash,
                        conflict_policy, error_code, updated_at_ms, result_code
                 FROM operation_items
                 WHERE state NOT IN ('completed', 'failed')
                 ORDER BY updated_at_ms, operation_id",
            )?;
            let rows = statement.query_map([], read_journal_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
    }

    pub fn batch(&self, batch_id: OperationId) -> Result<Option<JournalBatch>, JournalError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT batch_id, kind, state, requested_count, completed_count,
                            failed_count, skipped_count, created_at_ms, started_at_ms,
                            completed_at_ms
                     FROM operation_batches WHERE batch_id = ?1",
                    [batch_id.to_string()],
                    read_batch_row,
                )
                .optional()
                .map_err(Into::into)
        })
    }

    pub fn results_page(
        &self,
        batch_id: OperationId,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<JournalItem>, JournalError> {
        if !(1..=200).contains(&limit) {
            return Err(JournalError::InvalidPageSize);
        }
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT operation_id, batch_id, entity_id, kind, state, source_path,
                        destination_path, temporary_path, expected_size, expected_hash,
                        conflict_policy, error_code, updated_at_ms, result_code
                 FROM operation_items
                 WHERE batch_id = ?1 AND result_code IS NOT NULL
                 ORDER BY updated_at_ms, operation_id
                 LIMIT ?2 OFFSET ?3",
            )?;
            let rows = statement.query_map(
                params![batch_id.to_string(), i64::from(limit), i64::from(offset)],
                read_journal_row,
            )?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
    }

    pub fn finish_batch(
        &self,
        batch_id: OperationId,
        completed_at_ms: i64,
    ) -> Result<(), JournalError> {
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let transaction = connection.transaction()?;
        let counts = transaction
            .query_row(
                "SELECT requested_count, completed_count, failed_count, skipped_count
                 FROM operation_batches
                 WHERE batch_id = ?1 AND state = 'running'",
                [batch_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, u32>(0)?,
                        row.get::<_, u32>(1)?,
                        row.get::<_, u32>(2)?,
                        row.get::<_, u32>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((requested, completed, failed, skipped)) = counts else {
            return Err(JournalError::BatchNotWritable(batch_id));
        };
        let accounted = completed
            .checked_add(failed)
            .and_then(|value| value.checked_add(skipped))
            .ok_or(JournalError::InvalidBatchCount)?;
        if requested != accounted {
            return Err(JournalError::IncompleteBatch {
                batch_id,
                requested,
                accounted,
            });
        }
        let changed = transaction.execute(
            "UPDATE operation_batches
             SET state = ?2, completed_at_ms = ?3
             WHERE batch_id = ?1 AND state = 'running'",
            params![
                batch_id.to_string(),
                BatchState::Completed.as_str(),
                completed_at_ms,
            ],
        )?;
        if changed != 1 {
            return Err(JournalError::BatchNotWritable(batch_id));
        }
        transaction.commit()?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_item(
        &self,
        operation_id: OperationId,
        expected_current: OperationState,
        terminal_state: OperationState,
        result_code: &str,
        error_code: Option<&str>,
        counter_column: &'static str,
        updated_at_ms: i64,
    ) -> Result<(), JournalError> {
        if !is_valid_result_code(result_code) {
            return Err(JournalError::InvalidResultCode);
        }
        debug_assert!(matches!(
            counter_column,
            "completed_count" | "failed_count" | "skipped_count"
        ));
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let transaction = connection.transaction()?;
        let batch_id = transaction
            .query_row(
                "SELECT batch_id FROM operation_items
                 WHERE operation_id = ?1 AND state = ?2",
                params![operation_id.to_string(), expected_current.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(batch_id) = batch_id else {
            return Err(JournalError::ConcurrentStateChange {
                operation_id,
                expected: expected_current,
            });
        };
        let changed = transaction.execute(
            "UPDATE operation_items
             SET state = ?3, result_code = ?4, error_code = ?5, updated_at_ms = ?6
             WHERE operation_id = ?1 AND state = ?2",
            params![
                operation_id.to_string(),
                expected_current.as_str(),
                terminal_state.as_str(),
                result_code,
                error_code,
                updated_at_ms,
            ],
        )?;
        if changed != 1 {
            return Err(JournalError::ConcurrentStateChange {
                operation_id,
                expected: expected_current,
            });
        }
        let sql = format!(
            "UPDATE operation_batches
             SET {counter_column} = {counter_column} + 1
             WHERE batch_id = ?1 AND state = 'running'
               AND completed_count + failed_count + skipped_count < requested_count"
        );
        if transaction.execute(&sql, [&batch_id])? != 1 {
            return Err(JournalError::BatchNotWritable(
                parse_id(batch_id, "batch_id").map_err(JournalError::Database)?,
            ));
        }
        transaction.commit()?;
        Ok(())
    }

    fn with_connection<T>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T, JournalError>,
    ) -> Result<T, JournalError> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        operation(&connection)
    }

    fn update_expected_state(
        &self,
        operation_id: OperationId,
        expected_current: OperationState,
        sql: &str,
        parameters: impl rusqlite::Params,
    ) -> Result<(), JournalError> {
        self.with_connection(|connection| {
            let changed = connection.execute(sql, parameters)?;
            if changed != 1 {
                return Err(JournalError::ConcurrentStateChange {
                    operation_id,
                    expected: expected_current,
                });
            }
            Ok(())
        })
    }
}

fn checked_size(expected_size: u64) -> Result<i64, JournalError> {
    i64::try_from(expected_size).map_err(|_| JournalError::InvalidPersistedValue {
        field: "expected_size",
        value: expected_size.to_string(),
    })
}

fn read_journal_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<JournalItem> {
    let expected_size = row
        .get::<_, Option<i64>>(8)?
        .map(|value| u64::try_from(value).map_err(|_| persisted_error("expected_size", value)))
        .transpose()?;
    let expected_hash = row
        .get::<_, Option<Vec<u8>>>(9)?
        .map(|value| {
            <[u8; 32]>::try_from(value.as_slice())
                .map_err(|_| persisted_error("expected_hash", "invalid byte length"))
        })
        .transpose()?;

    Ok(JournalItem {
        operation_id: parse_id(row.get(0)?, "operation_id")?,
        batch_id: parse_id(row.get(1)?, "batch_id")?,
        entity_id: parse_id(row.get(2)?, "entity_id")?,
        kind: parse_kind(row.get(3)?)?,
        state: parse_state(row.get(4)?)?,
        source: parse_path(row.get(5)?, "source_path")?,
        destination: row
            .get::<_, Option<String>>(6)?
            .map(|value| parse_path(value, "destination_path"))
            .transpose()?,
        temporary: row
            .get::<_, Option<String>>(7)?
            .map(|value| parse_path(value, "temporary_path"))
            .transpose()?,
        expected_size,
        expected_hash,
        conflict_policy: parse_conflict_policy(row.get(10)?)?,
        error_code: row.get(11)?,
        updated_at_ms: row.get(12)?,
        result_code: row.get(13)?,
    })
}

fn read_batch_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<JournalBatch> {
    Ok(JournalBatch {
        batch_id: parse_id(row.get(0)?, "batch_id")?,
        kind: parse_kind(row.get(1)?)?,
        state: parse_batch_state(row.get(2)?)?,
        requested_count: row.get(3)?,
        completed_count: row.get(4)?,
        failed_count: row.get(5)?,
        skipped_count: row.get(6)?,
        created_at_ms: row.get(7)?,
        started_at_ms: row.get(8)?,
        completed_at_ms: row.get(9)?,
    })
}

fn parse_batch_state(value: String) -> rusqlite::Result<BatchState> {
    match value.as_str() {
        "running" => Ok(BatchState::Running),
        "completed" => Ok(BatchState::Completed),
        _ => Err(persisted_error("batch_state", value)),
    }
}

fn is_valid_result_code(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn parse_id<T>(value: String, field: &'static str) -> rusqlite::Result<T>
where
    T: FromStr,
{
    value
        .parse()
        .map_err(|_| persisted_error(field, value.as_str()))
}

fn parse_path(value: String, field: &'static str) -> rusqlite::Result<RelativePath> {
    RelativePath::parse(&value).map_err(|_| persisted_error(field, &value))
}

fn parse_kind(value: String) -> rusqlite::Result<OperationKind> {
    match value.as_str() {
        "rename" => Ok(OperationKind::Rename),
        "copy" => Ok(OperationKind::Copy),
        "move" => Ok(OperationKind::Move),
        "trash" => Ok(OperationKind::Trash),
        "set_review_state" => Ok(OperationKind::SetReviewState),
        "set_favorite" => Ok(OperationKind::SetFavorite),
        _ => Err(persisted_error("kind", &value)),
    }
}

fn parse_state(value: String) -> rusqlite::Result<OperationState> {
    match value.as_str() {
        "prepared" => Ok(OperationState::Prepared),
        "staged" => Ok(OperationState::Staged),
        "fs_applied" => Ok(OperationState::FsApplied),
        "verified" => Ok(OperationState::Verified),
        "meta_committed" => Ok(OperationState::MetaCommitted),
        "index_synced" => Ok(OperationState::IndexSynced),
        "completed" => Ok(OperationState::Completed),
        "failed" => Ok(OperationState::Failed),
        _ => Err(persisted_error("state", &value)),
    }
}

fn parse_conflict_policy(value: String) -> rusqlite::Result<ConflictPolicy> {
    match value.as_str() {
        "skip" => Ok(ConflictPolicy::Skip),
        "keep_both" => Ok(ConflictPolicy::KeepBoth),
        "replace" => Ok(ConflictPolicy::Replace),
        _ => Err(persisted_error("conflict_policy", &value)),
    }
}

fn persisted_error(field: &'static str, value: impl ToString) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(JournalError::InvalidPersistedValue {
            field,
            value: value.to_string(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::{JournalError, OperationJournal};
    use viewer_domain::{
        EntityId, OperationId, RelativePath,
        operation::{ConflictPolicy, OperationItemPlan, OperationKind, OperationState},
    };

    fn copy_item(batch_id: OperationId) -> OperationItemPlan {
        OperationItemPlan {
            batch_id,
            operation_id: OperationId::new(),
            entity_id: EntityId::new(),
            kind: OperationKind::Copy,
            source: RelativePath::parse("products/a.jpg").unwrap(),
            destination: Some(RelativePath::parse("exports/a.jpg").unwrap()),
            conflict_policy: ConflictPolicy::Skip,
        }
    }

    #[test]
    fn operation_journal_persists_incomplete_item_across_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("metadata.sqlite");
        let batch_id = OperationId::new();
        let item = copy_item(batch_id);

        {
            let journal = OperationJournal::open(&database).unwrap();
            journal
                .begin_batch(batch_id, OperationKind::Copy, 1, 100)
                .unwrap();
            journal.record_item(&item, 101).unwrap();
            journal
                .advance(
                    item.operation_id,
                    OperationState::Prepared,
                    OperationState::Staged,
                    102,
                )
                .unwrap();
            journal
                .advance(
                    item.operation_id,
                    OperationState::Staged,
                    OperationState::FsApplied,
                    103,
                )
                .unwrap();
        }

        let reopened = OperationJournal::open(&database).unwrap();
        let incomplete = reopened.incomplete_items().unwrap();
        assert_eq!(incomplete.len(), 1);
        assert_eq!(incomplete[0].operation_id, item.operation_id);
        assert_eq!(incomplete[0].state, OperationState::FsApplied);
        assert_eq!(incomplete[0].source, item.source);
        assert_eq!(incomplete[0].destination, item.destination);
    }

    #[test]
    fn operation_journal_rejects_stale_or_illegal_advance() {
        let directory = tempfile::tempdir().unwrap();
        let journal = OperationJournal::open(directory.path().join("metadata.sqlite")).unwrap();
        let batch_id = OperationId::new();
        let item = copy_item(batch_id);
        journal
            .begin_batch(batch_id, OperationKind::Copy, 1, 100)
            .unwrap();
        journal.record_item(&item, 101).unwrap();

        assert!(matches!(
            journal.advance(
                item.operation_id,
                OperationState::Staged,
                OperationState::FsApplied,
                102,
            ),
            Err(JournalError::ConcurrentStateChange { .. })
        ));
        assert!(matches!(
            journal.advance(
                item.operation_id,
                OperationState::Prepared,
                OperationState::Verified,
                103,
            ),
            Err(JournalError::InvalidTransition(_))
        ));
    }

    #[test]
    fn operation_journal_uses_delete_full_and_foreign_keys() {
        let directory = tempfile::tempdir().unwrap();
        let journal = OperationJournal::open(directory.path().join("metadata.sqlite")).unwrap();
        let connection = journal
            .connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let journal_mode: String = connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        let synchronous: i64 = connection
            .query_row("PRAGMA synchronous", [], |row| row.get(0))
            .unwrap();
        let foreign_keys: i64 = connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        let busy_timeout: i64 = connection
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .unwrap();

        assert_eq!(journal_mode, "delete");
        assert_eq!(synchronous, 2);
        assert_eq!(foreign_keys, 1);
        assert_eq!(busy_timeout, 5_000);
    }
}
