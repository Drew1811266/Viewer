use super::schema::{PortableSchemaError, open_database};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter, types::Value};
use std::{collections::HashSet, path::Path, str::FromStr, sync::Mutex};
use viewer_application::metadata::{
    FavoritePatch, Marker, MarkerChange, MarkerPatch, MarkerStoreError, MarkerTarget,
    PortableMarker, PortableMetadataPort, ReviewPatch,
};
use viewer_domain::{
    EntityId, RelativePath,
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
    match kind {
        FileKind::Directory => 0,
        FileKind::Jpeg => 1,
        FileKind::Png => 2,
        FileKind::Markdown => 3,
        FileKind::Text => 4,
    }
}

fn decode_kind(value: i64) -> rusqlite::Result<FileKind> {
    match value {
        0 => Ok(FileKind::Directory),
        1 => Ok(FileKind::Jpeg),
        2 => Ok(FileKind::Png),
        3 => Ok(FileKind::Markdown),
        4 => Ok(FileKind::Text),
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
