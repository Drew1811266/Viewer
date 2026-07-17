use rusqlite::{Connection, OptionalExtension, params};
use std::{path::Path, str::FromStr, sync::Mutex, time::Duration};
use viewer_application::{
    browse::{BrowseIndexError, BrowseIndexPort},
    metadata::{
        IndexProgress, IndexedNode, Marker, MarkerChange, MarkerProjectionError,
        MarkerProjectionPort, PortableMarker,
    },
};
use viewer_domain::{
    EntityId, RelativePath,
    file::{FileKind, FileNode, ImageIndexStatus, ImageMetadata, ReviewState, TextIndexStatus},
};

use super::text::TextStatus;

const SESSION_MIGRATION_V1: &str = include_str!("../../migrations/session/0001_initial.sql");

#[derive(Debug, thiserror::Error)]
pub enum SessionIndexError {
    #[error("session index database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("parent {parent_path} must be indexed before {path}")]
    MissingParent { path: String, parent_path: String },
    #[error("file size cannot be represented in SQLite: {0}")]
    SizeOutOfRange(u64),
    #[error("text index node does not exist or its path changed: {entity_id} at {path}")]
    MissingTextNode { entity_id: EntityId, path: String },
    #[error("session index node does not exist: {0}")]
    MissingNode(EntityId),
    #[error("derived metadata does not match a current supported node: {0}")]
    InvalidDerivedMetadata(EntityId),
    #[error("search query is invalid")]
    InvalidSearchQuery,
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

    pub fn replace_text(
        &self,
        entity_id: EntityId,
        relative_path: &RelativePath,
        status: &TextStatus,
    ) -> Result<(), SessionIndexError> {
        let mut connection = self.lock_connection();
        let transaction = connection.transaction()?;
        let exists = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM nodes WHERE entity_id = ?1 AND relative_path = ?2
             )",
            params![entity_id.to_string(), relative_path.as_str()],
            |row| row.get::<_, bool>(0),
        )?;
        if !exists {
            return Err(SessionIndexError::MissingTextNode {
                entity_id,
                path: relative_path.as_str().to_owned(),
            });
        }
        transaction.execute(
            "DELETE FROM text_fts WHERE entity_id = ?1",
            [entity_id.to_string()],
        )?;
        if let TextStatus::Indexed(body) = status {
            transaction.execute(
                "INSERT INTO text_fts(entity_id, relative_path, body) VALUES (?1, ?2, ?3)",
                params![entity_id.to_string(), relative_path.as_str(), body],
            )?;
        }
        transaction.execute(
            "UPDATE nodes SET text_status = ?2 WHERE entity_id = ?1",
            params![entity_id.to_string(), encode_text_status(status)],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn mark_text_failed(
        &self,
        entity_id: EntityId,
        relative_path: &RelativePath,
    ) -> Result<(), SessionIndexError> {
        let connection = self.lock_connection();
        let changed = connection.execute(
            "UPDATE nodes SET text_status = 4
             WHERE entity_id = ?1 AND relative_path = ?2 AND kind IN (3, 4)",
            params![entity_id.to_string(), relative_path.as_str()],
        )?;
        if changed == 0 {
            return Err(SessionIndexError::InvalidDerivedMetadata(entity_id));
        }
        Ok(())
    }

    pub fn replace_image_metadata(
        &self,
        entity_id: EntityId,
        relative_path: &RelativePath,
        result: Result<ImageMetadata, ImageIndexStatus>,
    ) -> Result<(), SessionIndexError> {
        let (width, height, status) = match result {
            Ok(metadata) if metadata.width > 0 && metadata.height > 0 => (
                Some(i64::from(metadata.width)),
                Some(i64::from(metadata.height)),
                ImageIndexStatus::Ready,
            ),
            Ok(_) => return Err(SessionIndexError::InvalidDerivedMetadata(entity_id)),
            Err(ImageIndexStatus::Failed) => (None, None, ImageIndexStatus::Failed),
            Err(_) => return Err(SessionIndexError::InvalidDerivedMetadata(entity_id)),
        };
        let connection = self.lock_connection();
        let changed = connection.execute(
            "UPDATE nodes
             SET image_width = ?3, image_height = ?4, image_status = ?5
             WHERE entity_id = ?1 AND relative_path = ?2 AND kind IN (1, 2)",
            params![
                entity_id.to_string(),
                relative_path.as_str(),
                width,
                height,
                encode_image_status(status),
            ],
        )?;
        if changed == 0 {
            return Err(SessionIndexError::InvalidDerivedMetadata(entity_id));
        }
        Ok(())
    }

    pub fn hydrate_markers(&self, markers: &[PortableMarker]) -> Result<usize, SessionIndexError> {
        let mut connection = self.lock_connection();
        let transaction = connection.transaction()?;
        let mut hydrated = 0_usize;
        {
            let mut update = transaction.prepare_cached(
                "UPDATE nodes SET review_state = ?3, favorite = ?4
                 WHERE relative_path = ?1 AND kind = ?2",
            )?;
            for stored in markers {
                let changed = update.execute(params![
                    stored.relative_path.as_str(),
                    encode_kind(stored.kind),
                    stored.marker.review_state.map(encode_review_state),
                    stored.marker.favorite,
                ])?;
                hydrated = hydrated.saturating_add(changed);
            }
        }
        transaction.commit()?;
        Ok(hydrated)
    }

    pub fn indexed_node(
        &self,
        entity_id: EntityId,
    ) -> Result<Option<IndexedNode>, SessionIndexError> {
        let connection = self.lock_connection();
        connection
            .query_row(
                "SELECT entity_id, relative_path, kind, size, modified_ns,
                        review_state, favorite, image_width, image_height,
                        image_status, text_status
                 FROM nodes WHERE entity_id = ?1",
                [entity_id.to_string()],
                read_indexed_node,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn index_progress(&self) -> Result<IndexProgress, SessionIndexError> {
        let connection = self.lock_connection();
        connection
            .query_row(
                "SELECT
                    SUM(CASE WHEN kind IN (1, 2) THEN 1 ELSE 0 END),
                    SUM(CASE WHEN kind IN (1, 2) AND image_status = 1 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN kind IN (1, 2) AND image_status = 2 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN kind IN (3, 4) THEN 1 ELSE 0 END),
                    SUM(CASE WHEN kind IN (3, 4) AND text_status = 1 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN kind IN (3, 4) AND text_status IN (2, 3) THEN 1 ELSE 0 END),
                    SUM(CASE WHEN kind IN (3, 4) AND text_status = 4 THEN 1 ELSE 0 END)
                 FROM nodes",
                [],
                |row| {
                    Ok(IndexProgress {
                        images_total: read_count(row, 0)?,
                        images_ready: read_count(row, 1)?,
                        images_failed: read_count(row, 2)?,
                        text_total: read_count(row, 3)?,
                        text_ready: read_count(row, 4)?,
                        text_skipped: read_count(row, 5)?,
                        text_failed: read_count(row, 6)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    pub fn set_review_metadata(
        &self,
        entity_id: EntityId,
        review_state: Option<ReviewState>,
        favorite: bool,
    ) -> Result<(), SessionIndexError> {
        let connection = self.lock_connection();
        let changed = connection.execute(
            "UPDATE nodes SET review_state = ?2, favorite = ?3 WHERE entity_id = ?1",
            params![
                entity_id.to_string(),
                review_state.map(encode_review_state),
                favorite,
            ],
        )?;
        if changed == 0 {
            return Err(SessionIndexError::MissingNode(entity_id));
        }
        Ok(())
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

    pub(super) fn lock_connection(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl MarkerProjectionPort for SessionIndex {
    fn sync_markers(&self, changes: &[MarkerChange]) -> Result<(), MarkerProjectionError> {
        let mut connection = self.lock_connection();
        let transaction = connection
            .transaction()
            .map_err(|_| MarkerProjectionError::Unavailable)?;
        for change in changes {
            let changed = transaction
                .execute(
                    "UPDATE nodes SET review_state = ?4, favorite = ?5
                     WHERE entity_id = ?1 AND relative_path = ?2 AND kind = ?3",
                    params![
                        change.target.entity_id.to_string(),
                        change.target.relative_path.as_str(),
                        encode_kind(change.target.kind),
                        change.marker.review_state.map(encode_review_state),
                        change.marker.favorite,
                    ],
                )
                .map_err(|_| MarkerProjectionError::Unavailable)?;
            if changed != 1 {
                return Err(MarkerProjectionError::Unavailable);
            }
        }
        transaction
            .commit()
            .map_err(|_| MarkerProjectionError::Unavailable)
    }
}

impl BrowseIndexPort for SessionIndex {
    fn all_folders(&self) -> Result<Vec<FileNode>, BrowseIndexError> {
        let connection = self.lock_connection();
        let mut statement = connection
            .prepare_cached(
                "SELECT entity_id, relative_path, kind, size, modified_ns
                 FROM nodes
                 WHERE kind = 0
                 ORDER BY relative_path COLLATE NOCASE, relative_path, entity_id",
            )
            .map_err(SessionIndexError::from)?;
        statement
            .query_map([], read_node)
            .map_err(SessionIndexError::from)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(SessionIndexError::from)
            .map_err(Into::into)
    }

    fn node(&self, entity_id: EntityId) -> Result<Option<FileNode>, BrowseIndexError> {
        let connection = self.lock_connection();
        connection
            .query_row(
                "SELECT entity_id, relative_path, kind, size, modified_ns
                 FROM nodes WHERE entity_id = ?1",
                [entity_id.to_string()],
                read_node,
            )
            .optional()
            .map_err(SessionIndexError::from)
            .map_err(Into::into)
    }

    fn node_by_relative_path(
        &self,
        path: &RelativePath,
    ) -> Result<Option<FileNode>, BrowseIndexError> {
        let connection = self.lock_connection();
        connection
            .query_row(
                "SELECT entity_id, relative_path, kind, size, modified_ns
                 FROM nodes WHERE relative_path = ?1",
                [path.as_str()],
                read_node,
            )
            .optional()
            .map_err(SessionIndexError::from)
            .map_err(Into::into)
    }

    fn direct_children(&self, folder: Option<EntityId>) -> Result<Vec<FileNode>, BrowseIndexError> {
        self.directory_children(folder).map_err(Into::into)
    }

    fn descendants(&self, folder: Option<EntityId>) -> Result<Vec<FileNode>, BrowseIndexError> {
        let connection = self.lock_connection();
        match folder {
            Some(folder) => {
                let Some(path) = connection
                    .query_row(
                        "SELECT relative_path FROM nodes WHERE entity_id = ?1 AND kind = 0",
                        [folder.to_string()],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()
                    .map_err(SessionIndexError::from)?
                else {
                    return Ok(Vec::new());
                };
                let mut statement = connection
                    .prepare_cached(
                        "SELECT entity_id, relative_path, kind, size, modified_ns
                         FROM nodes
                         WHERE substr(relative_path, 1, length(?1) + 1) = ?1 || '/'
                         ORDER BY relative_path COLLATE NOCASE, relative_path, entity_id",
                    )
                    .map_err(SessionIndexError::from)?;
                statement
                    .query_map([path], read_node)
                    .map_err(SessionIndexError::from)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(SessionIndexError::from)
                    .map_err(Into::into)
            }
            None => {
                let mut statement = connection
                    .prepare_cached(
                        "SELECT entity_id, relative_path, kind, size, modified_ns
                         FROM nodes
                         ORDER BY relative_path COLLATE NOCASE, relative_path, entity_id",
                    )
                    .map_err(SessionIndexError::from)?;
                statement
                    .query_map([], read_node)
                    .map_err(SessionIndexError::from)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(SessionIndexError::from)
                    .map_err(Into::into)
            }
        }
    }
}

impl From<SessionIndexError> for BrowseIndexError {
    fn from(error: SessionIndexError) -> Self {
        Self::Unavailable(error.to_string())
    }
}

pub(super) fn encode_kind(kind: FileKind) -> i64 {
    match kind {
        FileKind::Directory => 0,
        FileKind::Jpeg => 1,
        FileKind::Png => 2,
        FileKind::Markdown => 3,
        FileKind::Text => 4,
    }
}

pub(super) fn encode_review_state(state: ReviewState) -> i64 {
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
        _ => Err(persisted_error("review_state", value)),
    }
}

fn encode_image_status(status: ImageIndexStatus) -> i64 {
    match status {
        ImageIndexStatus::Pending => 0,
        ImageIndexStatus::Ready => 1,
        ImageIndexStatus::Failed => 2,
    }
}

fn decode_image_status(value: i64) -> rusqlite::Result<ImageIndexStatus> {
    match value {
        0 => Ok(ImageIndexStatus::Pending),
        1 => Ok(ImageIndexStatus::Ready),
        2 => Ok(ImageIndexStatus::Failed),
        _ => Err(persisted_error("image_status", value)),
    }
}

fn encode_text_status(status: &TextStatus) -> i64 {
    match status {
        TextStatus::Indexed(_) => 1,
        TextStatus::UnsupportedEncoding => 2,
        TextStatus::TooLarge => 3,
    }
}

fn decode_text_status(value: i64) -> rusqlite::Result<TextIndexStatus> {
    match value {
        0 => Ok(TextIndexStatus::Pending),
        1 => Ok(TextIndexStatus::Ready),
        2 => Ok(TextIndexStatus::UnsupportedEncoding),
        3 => Ok(TextIndexStatus::TooLarge),
        4 => Ok(TextIndexStatus::Failed),
        _ => Err(persisted_error("text_status", value)),
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

pub(super) fn read_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<FileNode> {
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

pub(super) fn read_indexed_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<IndexedNode> {
    let node = read_node(row)?;
    let review_state = row
        .get::<_, Option<i64>>(5)?
        .map(decode_review_state)
        .transpose()?;
    let width = row.get::<_, Option<i64>>(7)?;
    let height = row.get::<_, Option<i64>>(8)?;
    let image_metadata = match (width, height) {
        (Some(width), Some(height)) => Some(ImageMetadata {
            width: u32::try_from(width).map_err(|_| persisted_error("image_width", width))?,
            height: u32::try_from(height).map_err(|_| persisted_error("image_height", height))?,
        }),
        (None, None) => None,
        _ => return Err(persisted_error("image_dimensions", "partial")),
    };
    Ok(IndexedNode {
        node,
        marker: Marker {
            review_state,
            favorite: row.get(6)?,
        },
        image_metadata,
        image_status: decode_image_status(row.get(9)?)?,
        text_status: decode_text_status(row.get(10)?)?,
    })
}

fn read_count(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value = row.get::<_, Option<i64>>(index)?.unwrap_or(0);
    u64::try_from(value).map_err(|_| persisted_error("progress_count", value))
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
