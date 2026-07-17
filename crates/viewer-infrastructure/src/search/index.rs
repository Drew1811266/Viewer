use rusqlite::{Connection, OptionalExtension, params};
use std::{path::Path, str::FromStr, sync::Mutex, time::Duration};
use viewer_domain::{
    EntityId, RelativePath,
    file::{FileKind, FileNode},
};

const SESSION_MIGRATION_V1: &str = include_str!("../../migrations/session/0001_initial.sql");

#[derive(Debug, thiserror::Error)]
pub enum SessionIndexError {
    #[error("session index database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("parent {parent_path} must be indexed before {path}")]
    MissingParent { path: String, parent_path: String },
    #[error("file size cannot be represented in SQLite: {0}")]
    SizeOutOfRange(u64),
    #[error("invalid persisted {field}: {value}")]
    InvalidPersistedValue { field: &'static str, value: String },
}

pub struct SessionIndex {
    connection: Mutex<Connection>,
}

impl SessionIndex {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SessionIndexError> {
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch(SESSION_MIGRATION_V1)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn upsert_batch(&self, nodes: &[FileNode]) -> Result<(), SessionIndexError> {
        let mut connection = self.lock_connection();
        let transaction = connection.transaction()?;
        {
            let mut upsert = transaction.prepare_cached(
                "INSERT INTO nodes(
                    entity_id, parent_entity_id, relative_path, name, kind, size, modified_ns
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(entity_id) DO UPDATE SET
                    parent_entity_id = excluded.parent_entity_id,
                    relative_path = excluded.relative_path,
                    name = excluded.name,
                    kind = excluded.kind,
                    size = excluded.size,
                    modified_ns = excluded.modified_ns",
            )?;
            let mut find_parent = transaction
                .prepare_cached("SELECT entity_id FROM nodes WHERE relative_path = ?1")?;
            for node in nodes {
                let path = node.relative_path.as_str();
                let parent_path = path.rsplit_once('/').map(|(parent, _)| parent);
                let parent_id = parent_path
                    .map(|parent| {
                        find_parent
                            .query_row([parent], |row| row.get::<_, String>(0))
                            .optional()?
                            .ok_or_else(|| SessionIndexError::MissingParent {
                                path: path.to_owned(),
                                parent_path: parent.to_owned(),
                            })
                    })
                    .transpose()?;
                let name = path.rsplit('/').next().unwrap_or(path);
                let size = i64::try_from(node.size)
                    .map_err(|_| SessionIndexError::SizeOutOfRange(node.size))?;
                upsert.execute(params![
                    node.entity_id.to_string(),
                    parent_id,
                    path,
                    name,
                    encode_kind(node.kind),
                    size,
                    node.modified_ns.to_string(),
                ])?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn remove_subtree(&self, root: EntityId) -> Result<usize, SessionIndexError> {
        let mut connection = self.lock_connection();
        let transaction = connection.transaction()?;
        let Some(path) = transaction
            .query_row(
                "SELECT relative_path FROM nodes WHERE entity_id = ?1",
                [root.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        else {
            return Ok(0);
        };
        let removed = transaction.query_row(
            "SELECT COUNT(*) FROM nodes
             WHERE relative_path = ?1
                OR substr(relative_path, 1, length(?1) + 1) = ?1 || '/'",
            [&path],
            |row| row.get::<_, i64>(0),
        )?;
        let removed =
            usize::try_from(removed).map_err(|_| SessionIndexError::InvalidPersistedValue {
                field: "subtree_count",
                value: removed.to_string(),
            })?;
        transaction.execute(
            "DELETE FROM text_fts WHERE entity_id IN (
                SELECT entity_id FROM nodes
                WHERE relative_path = ?1
                   OR substr(relative_path, 1, length(?1) + 1) = ?1 || '/'
             )",
            [&path],
        )?;
        transaction.execute(
            "DELETE FROM nodes
             WHERE relative_path = ?1
                OR substr(relative_path, 1, length(?1) + 1) = ?1 || '/'",
            [&path],
        )?;
        transaction.commit()?;
        Ok(removed)
    }

    pub fn directory_children(
        &self,
        parent: Option<EntityId>,
    ) -> Result<Vec<FileNode>, SessionIndexError> {
        let connection = self.lock_connection();
        let mut statement = connection.prepare_cached(
            "SELECT entity_id, relative_path, kind, size, modified_ns
             FROM nodes
             WHERE parent_entity_id IS ?1
             ORDER BY name COLLATE NOCASE, relative_path, entity_id",
        )?;
        let parent = parent.map(|entity_id| entity_id.to_string());
        statement
            .query_map([parent], read_node)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn close(self) -> Result<(), SessionIndexError> {
        let connection = self
            .connection
            .into_inner()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        connection
            .close()
            .map_err(|(_, error)| SessionIndexError::Database(error))
    }

    fn lock_connection(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
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
        _ => Err(persisted_error("kind", value)),
    }
}

fn read_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<FileNode> {
    let entity_id = parse_id(row.get(0)?, "entity_id")?;
    let relative_path = parse_path(row.get(1)?, "relative_path")?;
    let kind = decode_kind(row.get(2)?)?;
    let persisted_size = row.get::<_, i64>(3)?;
    let size =
        u64::try_from(persisted_size).map_err(|_| persisted_error("size", persisted_size))?;
    let persisted_modified = row.get::<_, String>(4)?;
    let modified_ns = persisted_modified
        .parse::<i128>()
        .map_err(|_| persisted_error("modified_ns", &persisted_modified))?;
    Ok(FileNode {
        entity_id,
        relative_path,
        kind,
        size,
        modified_ns,
    })
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

fn persisted_error(field: &'static str, value: impl ToString) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(SessionIndexError::InvalidPersistedValue {
            field,
            value: value.to_string(),
        }),
    )
}
