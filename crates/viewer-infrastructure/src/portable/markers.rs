use super::schema::{PortableSchemaError, open_database};
use rusqlite::{
    Connection, OptionalExtension, TransactionBehavior, params, params_from_iter, types::Value,
};
use std::{collections::HashSet, path::Path, str::FromStr, sync::Mutex};
use viewer_application::metadata::{
    FavoritePatch, FilePathMove, Marker, MarkerChange, MarkerPatch, MarkerRestore,
    MarkerStoreError, MarkerTarget, PortableMarker, PortableMetadataPort, ReviewPatch,
};
use viewer_domain::{
    EntityId, OperationId, RelativePath,
    file::{FileKind, ReviewState},
};

#[derive(Debug, thiserror::Error)]
pub enum PortableMarkerStoreError {
    #[error(transparent)]
    Schema(#[from] PortableSchemaError),
}

pub struct PortableMarkerStore {
    connection: Mutex<Connection>,
    writable: bool,
}

impl PortableMarkerStore {
    pub fn open(path: &Path, writable: bool) -> Result<Self, PortableMarkerStoreError> {
        Ok(Self {
            connection: Mutex::new(open_database(path, writable)?),
            writable,
        })
    }

    fn lock_connection(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl PortableMetadataPort for PortableMarkerStore {
    fn markers_for_paths(
        &self,
        paths: &[RelativePath],
    ) -> Result<Vec<PortableMarker>, MarkerStoreError> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }
        let connection = self.lock_connection();
        let mut sql = String::from(
            "SELECT relative_path, kind, review_state, favorite,
                    evidence_size, evidence_modified_ns, content_hash
             FROM markers WHERE relative_path IN (",
        );
        push_placeholders(&mut sql, paths.len());
        sql.push_str(") ORDER BY relative_path");
        let values = paths
            .iter()
            .map(|path| Value::Text(path.as_str().to_owned()))
            .collect::<Vec<_>>();
        let mut statement = connection
            .prepare(&sql)
            .map_err(|_| MarkerStoreError::Unavailable)?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), read_portable_marker)
            .map_err(|_| MarkerStoreError::Unavailable)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| MarkerStoreError::Unavailable)
    }

    fn apply_batch(
        &self,
        targets: &[MarkerTarget],
        patch: MarkerPatch,
        updated_at_ms: i64,
    ) -> Result<Vec<MarkerChange>, MarkerStoreError> {
        if !self.writable {
            return Err(MarkerStoreError::ReadOnly);
        }
        if targets.is_empty() || updated_at_ms < 0 {
            return Err(MarkerStoreError::InvalidTarget);
        }
        let mut entities = HashSet::with_capacity(targets.len());
        let mut paths = HashSet::with_capacity(targets.len());
        if targets.iter().any(|target| {
            !entities.insert(target.entity_id) || !paths.insert(target.relative_path.clone())
        }) {
            return Err(MarkerStoreError::InvalidTarget);
        }

        let mut connection = self.lock_connection();
        let transaction = connection
            .transaction()
            .map_err(|_| MarkerStoreError::Unavailable)?;
        let mut changes = Vec::with_capacity(targets.len());
        for target in targets {
            let current = transaction
                .query_row(
                    "SELECT review_state, favorite FROM markers WHERE relative_path = ?1",
                    [target.relative_path.as_str()],
                    |row| {
                        Ok(Marker {
                            review_state: row
                                .get::<_, Option<i64>>(0)?
                                .map(decode_review_state)
                                .transpose()?,
                            favorite: row.get(1)?,
                        })
                    },
                )
                .optional()
                .map_err(|_| MarkerStoreError::Unavailable)?
                .unwrap_or_default();
            let marker = apply_patch(current, patch);
            if marker == Marker::default() {
                transaction
                    .execute(
                        "DELETE FROM markers WHERE relative_path = ?1",
                        [target.relative_path.as_str()],
                    )
                    .map_err(|_| MarkerStoreError::Unavailable)?;
            } else {
                let evidence_size = if target.kind == FileKind::Directory {
                    None
                } else {
                    Some(i64::try_from(target.size).map_err(|_| MarkerStoreError::InvalidTarget)?)
                };
                let evidence_modified_ns =
                    (target.kind != FileKind::Directory).then(|| target.modified_ns.to_string());
                transaction
                    .execute(
                        "INSERT INTO markers(
                            marker_id, relative_path, kind, review_state, favorite,
                            evidence_size, evidence_modified_ns, updated_at_ms
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                         ON CONFLICT(relative_path) DO UPDATE SET
                            kind = excluded.kind,
                            review_state = excluded.review_state,
                            favorite = excluded.favorite,
                            evidence_size = excluded.evidence_size,
                            evidence_modified_ns = excluded.evidence_modified_ns,
                            updated_at_ms = excluded.updated_at_ms",
                        params![
                            EntityId::new().to_string(),
                            target.relative_path.as_str(),
                            encode_kind(target.kind),
                            marker.review_state.map(encode_review_state),
                            marker.favorite,
                            evidence_size,
                            evidence_modified_ns,
                            updated_at_ms,
                        ],
                    )
                    .map_err(|_| MarkerStoreError::Unavailable)?;
            }
            changes.push(MarkerChange {
                target: target.clone(),
                marker,
            });
        }
        transaction
            .commit()
            .map_err(|_| MarkerStoreError::Unavailable)?;
        Ok(changes)
    }

    fn restore_batch(
        &self,
        restores: &[MarkerRestore],
        updated_at_ms: i64,
    ) -> Result<Vec<MarkerChange>, MarkerStoreError> {
        if !self.writable {
            return Err(MarkerStoreError::ReadOnly);
        }
        if restores.is_empty() || updated_at_ms < 0 {
            return Err(MarkerStoreError::InvalidTarget);
        }
        let mut entities = HashSet::with_capacity(restores.len());
        let mut paths = HashSet::with_capacity(restores.len());
        if restores.iter().any(|restore| {
            !entities.insert(restore.target.entity_id)
                || !paths.insert(restore.target.relative_path.clone())
        }) {
            return Err(MarkerStoreError::InvalidTarget);
        }
        let mut connection = self.lock_connection();
        let transaction = connection
            .transaction()
            .map_err(|_| MarkerStoreError::Unavailable)?;
        for restore in restores {
            let current = read_current_marker(&transaction, &restore.target.relative_path)?;
            if current != restore.expected {
                return Err(MarkerStoreError::InvalidTarget);
            }
        }
        let mut changes = Vec::with_capacity(restores.len());
        for restore in restores {
            write_marker(
                &transaction,
                &restore.target,
                restore.previous,
                updated_at_ms,
            )?;
            changes.push(MarkerChange {
                target: restore.target.clone(),
                marker: restore.previous,
            });
        }
        transaction
            .commit()
            .map_err(|_| MarkerStoreError::Unavailable)?;
        Ok(changes)
    }

    fn clear_paths(
        &self,
        paths: &[RelativePath],
        updated_at_ms: i64,
    ) -> Result<usize, MarkerStoreError> {
        if !self.writable {
            return Err(MarkerStoreError::ReadOnly);
        }
        if updated_at_ms < 0 {
            return Err(MarkerStoreError::InvalidTarget);
        }
        if paths.is_empty() {
            return Ok(0);
        }
        let unique = paths.iter().cloned().collect::<HashSet<_>>();
        if unique.len() != paths.len() {
            return Err(MarkerStoreError::InvalidTarget);
        }
        let mut connection = self.lock_connection();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| MarkerStoreError::Unavailable)?;
        let mut removed = 0_usize;
        {
            let mut delete = transaction
                .prepare_cached("DELETE FROM markers WHERE relative_path = ?1")
                .map_err(|_| MarkerStoreError::Unavailable)?;
            for path in paths {
                removed = removed
                    .checked_add(
                        delete
                            .execute([path.as_str()])
                            .map_err(|_| MarkerStoreError::Unavailable)?,
                    )
                    .ok_or(MarkerStoreError::Unavailable)?;
            }
        }
        transaction
            .commit()
            .map_err(|_| MarkerStoreError::Unavailable)?;
        Ok(removed)
    }

    fn move_paths(
        &self,
        moves: &[FilePathMove],
        case_sensitive: bool,
        updated_at_ms: i64,
    ) -> Result<usize, MarkerStoreError> {
        if !self.writable {
            return Err(MarkerStoreError::ReadOnly);
        }
        validate_path_moves(moves, case_sensitive, updated_at_ms)?;

        let mut connection = self.lock_connection();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| MarkerStoreError::Unavailable)?;
        let moved = apply_path_moves(&transaction, moves, case_sensitive, updated_at_ms)?;
        transaction
            .commit()
            .map_err(|_| MarkerStoreError::Unavailable)?;
        Ok(moved)
    }

    fn commit_replace(
        &self,
        operation_id: OperationId,
        replaced_destination: Option<&RelativePath>,
        moves: &[FilePathMove],
        case_sensitive: bool,
        updated_at_ms: i64,
    ) -> Result<usize, MarkerStoreError> {
        if !self.writable || updated_at_ms < 0 {
            return Err(if self.writable {
                MarkerStoreError::InvalidTarget
            } else {
                MarkerStoreError::ReadOnly
            });
        }
        if !moves.is_empty() {
            validate_path_moves(moves, case_sensitive, updated_at_ms)?;
        }
        if replaced_destination
            .is_some_and(|destination| moves.iter().any(|mapping| mapping.source == *destination))
        {
            return Err(MarkerStoreError::InvalidTarget);
        }

        let mut connection = self.lock_connection();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| MarkerStoreError::Unavailable)?;
        let state = transaction
            .query_row(
                "SELECT state FROM operation_items WHERE operation_id = ?1",
                [operation_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|_| MarkerStoreError::Unavailable)?;
        match state.as_deref() {
            Some("meta_committed") => {
                transaction
                    .commit()
                    .map_err(|_| MarkerStoreError::Unavailable)?;
                return Ok(0);
            }
            Some("verified") => {}
            _ => return Err(MarkerStoreError::InvalidTarget),
        }

        let mut changed = 0_usize;
        if let Some(destination) = replaced_destination {
            changed = transaction
                .execute(
                    "DELETE FROM markers WHERE relative_path = ?1",
                    [destination.as_str()],
                )
                .map_err(|_| MarkerStoreError::Unavailable)?;
        }
        if !moves.is_empty() {
            changed = changed
                .checked_add(apply_path_moves(
                    &transaction,
                    moves,
                    case_sensitive,
                    updated_at_ms,
                )?)
                .ok_or(MarkerStoreError::Unavailable)?;
        }
        let advanced = transaction
            .execute(
                "UPDATE operation_items
                 SET state = 'meta_committed', updated_at_ms = ?2
                 WHERE operation_id = ?1 AND state = 'verified'",
                params![operation_id.to_string(), updated_at_ms],
            )
            .map_err(|_| MarkerStoreError::Unavailable)?;
        if advanced != 1 {
            return Err(MarkerStoreError::InvalidTarget);
        }
        transaction
            .commit()
            .map_err(|_| MarkerStoreError::Unavailable)?;
        Ok(changed)
    }
}

fn apply_path_moves(
    transaction: &rusqlite::Transaction<'_>,
    moves: &[FilePathMove],
    case_sensitive: bool,
    updated_at_ms: i64,
) -> Result<usize, MarkerStoreError> {
    let rows = {
        let mut statement = transaction
            .prepare(
                "SELECT marker_id, relative_path, kind, review_state, favorite,
                        evidence_size, evidence_modified_ns, content_hash
                 FROM markers ORDER BY relative_path, marker_id",
            )
            .map_err(|_| MarkerStoreError::Unavailable)?;
        statement
            .query_map([], read_marker_row)
            .map_err(|_| MarkerStoreError::Unavailable)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| MarkerStoreError::Unavailable)?
    };

    let mut affected = Vec::new();
    let mut unaffected_keys = HashSet::new();
    for row in rows {
        if RelativePath::parse(&row.relative_path).is_err() {
            return Err(MarkerStoreError::Unavailable);
        }
        if let Some(destination) = projected_marker_path(&row.relative_path, moves) {
            if RelativePath::parse(&destination).is_err() {
                return Err(MarkerStoreError::InvalidTarget);
            }
            affected.push((row, destination));
        } else if !unaffected_keys.insert(path_key(&row.relative_path, case_sensitive)) {
            return Err(MarkerStoreError::InvalidTarget);
        }
    }

    let mut destination_keys = HashSet::with_capacity(affected.len());
    for (_, destination) in &affected {
        let key = path_key(destination, case_sensitive);
        if unaffected_keys.contains(&key) || !destination_keys.insert(key) {
            return Err(MarkerStoreError::InvalidTarget);
        }
    }

    {
        let mut delete = transaction
            .prepare_cached("DELETE FROM markers WHERE marker_id = ?1")
            .map_err(|_| MarkerStoreError::Unavailable)?;
        for (row, _) in &affected {
            delete
                .execute([&row.marker_id])
                .map_err(|_| MarkerStoreError::Unavailable)?;
        }
    }
    {
        let mut insert = transaction
            .prepare_cached(
                "INSERT INTO markers(
                    marker_id, relative_path, kind, review_state, favorite,
                    evidence_size, evidence_modified_ns, content_hash, updated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )
            .map_err(|_| MarkerStoreError::Unavailable)?;
        for (row, destination) in &affected {
            insert
                .execute(params![
                    &row.marker_id,
                    destination,
                    row.kind,
                    row.review_state,
                    row.favorite,
                    row.evidence_size,
                    &row.evidence_modified_ns,
                    &row.content_hash,
                    updated_at_ms,
                ])
                .map_err(|_| MarkerStoreError::Unavailable)?;
        }
    }
    Ok(affected.len())
}

fn read_current_marker(
    connection: &Connection,
    path: &RelativePath,
) -> Result<Marker, MarkerStoreError> {
    connection
        .query_row(
            "SELECT review_state, favorite FROM markers WHERE relative_path = ?1",
            [path.as_str()],
            |row| {
                Ok(Marker {
                    review_state: row
                        .get::<_, Option<i64>>(0)?
                        .map(decode_review_state)
                        .transpose()?,
                    favorite: row.get(1)?,
                })
            },
        )
        .optional()
        .map(|marker| marker.unwrap_or_default())
        .map_err(|_| MarkerStoreError::Unavailable)
}

fn write_marker(
    connection: &Connection,
    target: &MarkerTarget,
    marker: Marker,
    updated_at_ms: i64,
) -> Result<(), MarkerStoreError> {
    if marker == Marker::default() {
        connection
            .execute(
                "DELETE FROM markers WHERE relative_path = ?1",
                [target.relative_path.as_str()],
            )
            .map_err(|_| MarkerStoreError::Unavailable)?;
        return Ok(());
    }
    let evidence_size = if target.kind == FileKind::Directory {
        None
    } else {
        Some(i64::try_from(target.size).map_err(|_| MarkerStoreError::InvalidTarget)?)
    };
    let evidence_modified_ns =
        (target.kind != FileKind::Directory).then(|| target.modified_ns.to_string());
    connection
        .execute(
            "INSERT INTO markers(
                marker_id, relative_path, kind, review_state, favorite,
                evidence_size, evidence_modified_ns, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(relative_path) DO UPDATE SET
                kind = excluded.kind,
                review_state = excluded.review_state,
                favorite = excluded.favorite,
                evidence_size = excluded.evidence_size,
                evidence_modified_ns = excluded.evidence_modified_ns,
                updated_at_ms = excluded.updated_at_ms",
            params![
                EntityId::new().to_string(),
                target.relative_path.as_str(),
                encode_kind(target.kind),
                marker.review_state.map(encode_review_state),
                marker.favorite,
                evidence_size,
                evidence_modified_ns,
                updated_at_ms,
            ],
        )
        .map_err(|_| MarkerStoreError::Unavailable)?;
    Ok(())
}

#[derive(Debug)]
struct MarkerRow {
    marker_id: String,
    relative_path: String,
    kind: i64,
    review_state: Option<i64>,
    favorite: bool,
    evidence_size: Option<i64>,
    evidence_modified_ns: Option<String>,
    content_hash: Option<Vec<u8>>,
}

fn read_marker_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MarkerRow> {
    Ok(MarkerRow {
        marker_id: row.get(0)?,
        relative_path: row.get(1)?,
        kind: row.get(2)?,
        review_state: row.get(3)?,
        favorite: row.get(4)?,
        evidence_size: row.get(5)?,
        evidence_modified_ns: row.get(6)?,
        content_hash: row.get(7)?,
    })
}

fn validate_path_moves(
    moves: &[FilePathMove],
    case_sensitive: bool,
    updated_at_ms: i64,
) -> Result<(), MarkerStoreError> {
    if moves.is_empty() || updated_at_ms < 0 {
        return Err(MarkerStoreError::InvalidTarget);
    }
    let mut sources = HashSet::with_capacity(moves.len());
    let mut destinations = HashSet::with_capacity(moves.len());
    for mapping in moves {
        if mapping.source == mapping.destination
            || !sources.insert(path_key(mapping.source.as_str(), case_sensitive))
            || !destinations.insert(path_key(mapping.destination.as_str(), case_sensitive))
            || is_descendant(
                mapping.destination.as_str(),
                mapping.source.as_str(),
                case_sensitive,
            )
        {
            return Err(MarkerStoreError::InvalidTarget);
        }
    }
    for (index, mapping) in moves.iter().enumerate() {
        if moves[index + 1..].iter().any(|other| {
            is_descendant(
                mapping.source.as_str(),
                other.source.as_str(),
                case_sensitive,
            ) || is_descendant(
                other.source.as_str(),
                mapping.source.as_str(),
                case_sensitive,
            )
        }) {
            return Err(MarkerStoreError::InvalidTarget);
        }
    }
    Ok(())
}

fn projected_marker_path(path: &str, moves: &[FilePathMove]) -> Option<String> {
    moves.iter().find_map(|mapping| {
        if path == mapping.source.as_str() {
            return Some(mapping.destination.as_str().to_owned());
        }
        path.strip_prefix(mapping.source.as_str())
            .and_then(|suffix| suffix.strip_prefix('/'))
            .map(|suffix| format!("{}/{suffix}", mapping.destination.as_str()))
    })
}

fn is_descendant(candidate: &str, ancestor: &str, case_sensitive: bool) -> bool {
    let candidate = path_key(candidate, case_sensitive);
    let ancestor = path_key(ancestor, case_sensitive);
    candidate
        .strip_prefix(&ancestor)
        .is_some_and(|suffix| suffix.starts_with('/'))
}

fn path_key(path: &str, case_sensitive: bool) -> String {
    if case_sensitive {
        path.to_owned()
    } else {
        path.to_lowercase()
    }
}

fn apply_patch(current: Marker, patch: MarkerPatch) -> Marker {
    let review_state = match patch.review {
        ReviewPatch::Unchanged => current.review_state,
        ReviewPatch::Set(state) => Some(state),
        ReviewPatch::Clear => None,
    };
    let favorite = match patch.favorite {
        FavoritePatch::Unchanged => current.favorite,
        FavoritePatch::Set(value) => value,
        FavoritePatch::Toggle => !current.favorite,
    };
    Marker {
        review_state,
        favorite,
    }
}

fn read_portable_marker(row: &rusqlite::Row<'_>) -> rusqlite::Result<PortableMarker> {
    let relative_path = RelativePath::parse(&row.get::<_, String>(0)?)
        .map_err(|_| persisted_error("relative_path"))?;
    let kind = decode_kind(row.get(1)?)?;
    let review_state = row
        .get::<_, Option<i64>>(2)?
        .map(decode_review_state)
        .transpose()?;
    let evidence_size = row
        .get::<_, Option<i64>>(4)?
        .map(|value| u64::try_from(value).map_err(|_| persisted_error("evidence_size")))
        .transpose()?;
    let evidence_modified_ns = row
        .get::<_, Option<String>>(5)?
        .map(|value| i128::from_str(&value).map_err(|_| persisted_error("evidence_modified_ns")))
        .transpose()?;
    let content_hash = row
        .get::<_, Option<Vec<u8>>>(6)?
        .map(|bytes| {
            <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| persisted_error("content_hash"))
        })
        .transpose()?;
    Ok(PortableMarker {
        relative_path,
        kind,
        marker: Marker {
            review_state,
            favorite: row.get(3)?,
        },
        evidence_size,
        evidence_modified_ns,
        content_hash,
    })
}

fn push_placeholders(sql: &mut String, count: usize) {
    for index in 0..count {
        if index > 0 {
            sql.push(',');
        }
        sql.push('?');
    }
}

fn encode_kind(kind: FileKind) -> i64 {
    kind.encode()
}

fn decode_kind(value: i64) -> rusqlite::Result<FileKind> {
    match value {
        0 => Ok(FileKind::Directory),
        1 => Ok(FileKind::Jpeg),
        2 => Ok(FileKind::Png),
        3 => Ok(FileKind::Markdown),
        4 => Ok(FileKind::Text),
        5 => Ok(FileKind::UnsupportedImage),
        6 => Ok(FileKind::Other),
        7 => Ok(FileKind::Video),
        _ => Err(persisted_error("kind")),
    }
}

fn encode_review_state(state: ReviewState) -> i64 {
    match state {
        ReviewState::Keep => 0,
        ReviewState::Pending => 1,
        ReviewState::Reject => 2,
    }
}

fn decode_review_state(value: i64) -> rusqlite::Result<ReviewState> {
    match value {
        0 => Ok(ReviewState::Keep),
        1 => Ok(ReviewState::Pending),
        2 => Ok(ReviewState::Reject),
        _ => Err(persisted_error("review_state")),
    }
}

fn persisted_error(field: &'static str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid persisted {field}"),
        )),
    )
}
