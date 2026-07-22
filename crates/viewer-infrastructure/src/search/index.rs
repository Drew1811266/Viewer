use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    str::FromStr,
    sync::Mutex,
    time::Duration,
};
use viewer_application::{
    browse::{BrowseIndexError, BrowseIndexPort},
    metadata::{
        FileCopyProjection, FileMoveProjection, FilePathMove, IndexProgress, IndexedNode, Marker,
        MarkerChange, MarkerProjectionError, MarkerProjectionPort, OperationProjectionError,
        OperationProjectionPort, PortableMarker,
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
    #[error("reconcile snapshot is inconsistent with the requested project subtrees")]
    InvalidReconcile,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SessionReconcileSummary {
    pub added: u64,
    pub removed: u64,
    pub modified: u64,
    pub moved: u64,
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
            return Err(SessionIndexError::MissingNode(entity_id));
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
            return Err(SessionIndexError::MissingNode(entity_id));
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

    pub fn reconcile_identity_moves(
        &self,
        scopes: &[Option<RelativePath>],
        protected: &[Option<RelativePath>],
        observed: &[FileNode],
    ) -> Result<Vec<FilePathMove>, SessionIndexError> {
        let connection = self.lock_connection();
        let existing = reconcile_nodes(&connection)?;
        validate_reconcile_inputs(scopes, observed)?;
        let existing_by_entity = existing
            .iter()
            .cloned()
            .map(|node| (node.entity_id, node))
            .collect::<HashMap<_, _>>();
        let observed_entities = observed
            .iter()
            .map(|node| node.entity_id)
            .collect::<HashSet<_>>();
        let removed_entities = existing
            .iter()
            .filter(|node| {
                path_in_scopes(&node.relative_path, scopes)
                    && !path_in_scopes(&node.relative_path, protected)
                    && !observed_entities.contains(&node.entity_id)
            })
            .map(|node| node.entity_id)
            .collect::<HashSet<_>>();
        let mut future = existing_by_entity
            .iter()
            .filter(|(entity, _)| !removed_entities.contains(entity))
            .map(|(entity, node)| (*entity, node.clone()))
            .collect::<HashMap<_, _>>();
        for node in observed {
            future.insert(node.entity_id, node.clone());
        }
        validate_future_nodes(&future)?;
        let mut moves = observed
            .iter()
            .filter_map(|node| {
                let prior = existing_by_entity.get(&node.entity_id)?;
                (prior.relative_path != node.relative_path).then(|| FilePathMove {
                    source: prior.relative_path.clone(),
                    destination: node.relative_path.clone(),
                })
            })
            .collect::<Vec<_>>();
        moves.sort_by(|left, right| {
            path_depth(left.source.as_str())
                .cmp(&path_depth(right.source.as_str()))
                .then_with(|| left.source.as_str().cmp(right.source.as_str()))
        });
        let mut minimized = Vec::<FilePathMove>::new();
        for mapping in moves {
            if minimized.iter().any(|parent| {
                projected_reconcile_path(mapping.source.as_str(), parent)
                    .is_some_and(|projected| projected == mapping.destination.as_str())
            }) {
                continue;
            }
            minimized.push(mapping);
        }
        Ok(minimized)
    }

    pub fn reconcile_subtrees(
        &self,
        scopes: &[Option<RelativePath>],
        protected: &[Option<RelativePath>],
        observed: &[FileNode],
    ) -> Result<SessionReconcileSummary, SessionIndexError> {
        validate_reconcile_inputs(scopes, observed)?;
        let mut connection = self.lock_connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch("PRAGMA defer_foreign_keys = ON;")?;
        let existing = reconcile_nodes(&transaction)?;
        let existing_by_entity = existing
            .iter()
            .cloned()
            .map(|node| (node.entity_id, node))
            .collect::<HashMap<_, _>>();
        let observed_by_entity = observed
            .iter()
            .cloned()
            .map(|node| (node.entity_id, node))
            .collect::<HashMap<_, _>>();
        let removed = existing
            .iter()
            .filter(|node| {
                path_in_scopes(&node.relative_path, scopes)
                    && !path_in_scopes(&node.relative_path, protected)
                    && !observed_by_entity.contains_key(&node.entity_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        let removed_entities = removed
            .iter()
            .map(|node| node.entity_id)
            .collect::<HashSet<_>>();
        let added = observed
            .iter()
            .filter(|node| !existing_by_entity.contains_key(&node.entity_id))
            .cloned()
            .collect::<Vec<_>>();
        let retained = observed
            .iter()
            .filter_map(|node| {
                existing_by_entity
                    .get(&node.entity_id)
                    .map(|prior| (prior.clone(), node.clone()))
            })
            .collect::<Vec<_>>();

        let mut future = existing_by_entity
            .into_iter()
            .filter(|(entity, _)| !removed_entities.contains(entity))
            .collect::<HashMap<_, _>>();
        for node in observed {
            future.insert(node.entity_id, node.clone());
        }
        validate_future_nodes(&future)?;

        for (prior, current) in retained
            .iter()
            .filter(|(prior, current)| prior.relative_path != current.relative_path)
        {
            let parent = reconcile_parent_id(current, &future)?;
            transaction.execute(
                "UPDATE nodes
                 SET parent_entity_id = ?2, relative_path = ?3, name = ?4
                 WHERE entity_id = ?1",
                params![
                    prior.entity_id.to_string(),
                    parent,
                    format!(".viewer-reconcile/{}", prior.entity_id),
                    format!("reconcile-{}", prior.entity_id),
                ],
            )?;
        }

        for node in &removed {
            transaction.execute(
                "DELETE FROM text_fts WHERE entity_id = ?1",
                [node.entity_id.to_string()],
            )?;
        }
        for node in &removed {
            transaction.execute(
                "DELETE FROM nodes WHERE entity_id = ?1",
                [node.entity_id.to_string()],
            )?;
        }

        let mut ordered_added = added.iter().collect::<Vec<_>>();
        ordered_added.sort_by_key(|node| path_depth(node.relative_path.as_str()));
        for node in ordered_added {
            let parent = reconcile_parent_id(node, &future)?;
            transaction.execute(
                "INSERT INTO nodes(
                    entity_id, parent_entity_id, relative_path, name, kind, size, modified_ns
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    node.entity_id.to_string(),
                    parent,
                    node.relative_path.as_str(),
                    projection_name(node.relative_path.as_str()),
                    encode_kind(node.kind),
                    i64::try_from(node.size)
                        .map_err(|_| SessionIndexError::SizeOutOfRange(node.size))?,
                    node.modified_ns.to_string(),
                ],
            )?;
        }

        let mut moved = 0_u64;
        let mut modified = 0_u64;
        for (prior, current) in &retained {
            let path_changed = prior.relative_path != current.relative_path;
            let content_changed = prior.kind != current.kind
                || (current.kind != FileKind::Directory
                    && (prior.size != current.size || prior.modified_ns != current.modified_ns));
            moved = moved.saturating_add(u64::from(path_changed));
            modified = modified.saturating_add(u64::from(content_changed));
            let parent = reconcile_parent_id(current, &future)?;
            if content_changed {
                transaction.execute(
                    "DELETE FROM text_fts WHERE entity_id = ?1",
                    [current.entity_id.to_string()],
                )?;
                transaction.execute(
                    "UPDATE nodes
                     SET parent_entity_id = ?2, relative_path = ?3, name = ?4,
                         kind = ?5, size = ?6, modified_ns = ?7,
                         image_width = NULL, image_height = NULL,
                         image_status = 0, text_status = 0
                     WHERE entity_id = ?1",
                    params![
                        current.entity_id.to_string(),
                        parent,
                        current.relative_path.as_str(),
                        projection_name(current.relative_path.as_str()),
                        encode_kind(current.kind),
                        i64::try_from(current.size)
                            .map_err(|_| SessionIndexError::SizeOutOfRange(current.size))?,
                        current.modified_ns.to_string(),
                    ],
                )?;
            } else {
                transaction.execute(
                    "UPDATE nodes
                     SET parent_entity_id = ?2, relative_path = ?3, name = ?4,
                         kind = ?5, size = ?6, modified_ns = ?7
                     WHERE entity_id = ?1",
                    params![
                        current.entity_id.to_string(),
                        parent,
                        current.relative_path.as_str(),
                        projection_name(current.relative_path.as_str()),
                        encode_kind(current.kind),
                        i64::try_from(current.size)
                            .map_err(|_| SessionIndexError::SizeOutOfRange(current.size))?,
                        current.modified_ns.to_string(),
                    ],
                )?;
                if path_changed {
                    transaction.execute(
                        "UPDATE text_fts SET relative_path = ?2 WHERE entity_id = ?1",
                        params![
                            current.entity_id.to_string(),
                            current.relative_path.as_str()
                        ],
                    )?;
                }
            }
        }
        transaction.commit()?;
        Ok(SessionReconcileSummary {
            added: u64::try_from(added.len()).unwrap_or(u64::MAX),
            removed: u64::try_from(removed.len()).unwrap_or(u64::MAX),
            modified,
            moved,
        })
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

fn reconcile_nodes(connection: &Connection) -> Result<Vec<FileNode>, SessionIndexError> {
    let mut statement = connection.prepare(
        "SELECT entity_id, relative_path, kind, size, modified_ns
         FROM nodes ORDER BY relative_path, entity_id",
    )?;
    statement
        .query_map([], read_node)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn validate_reconcile_inputs(
    scopes: &[Option<RelativePath>],
    observed: &[FileNode],
) -> Result<(), SessionIndexError> {
    if scopes.is_empty() {
        return Err(SessionIndexError::InvalidReconcile);
    }
    let mut entities = HashSet::with_capacity(observed.len());
    let mut paths = HashSet::with_capacity(observed.len());
    for node in observed {
        if !path_in_scopes(&node.relative_path, scopes)
            || !entities.insert(node.entity_id)
            || !paths.insert(node.relative_path.clone())
            || i64::try_from(node.size).is_err()
        {
            return Err(SessionIndexError::InvalidReconcile);
        }
    }
    Ok(())
}

fn validate_future_nodes(future: &HashMap<EntityId, FileNode>) -> Result<(), SessionIndexError> {
    let mut paths = HashMap::<&str, &FileNode>::with_capacity(future.len());
    for node in future.values() {
        if paths.insert(node.relative_path.as_str(), node).is_some() {
            return Err(SessionIndexError::InvalidReconcile);
        }
    }
    for node in future.values() {
        if let Some((parent, _)) = node.relative_path.as_str().rsplit_once('/') {
            let Some(parent) = paths.get(parent) else {
                return Err(SessionIndexError::InvalidReconcile);
            };
            if parent.kind != FileKind::Directory {
                return Err(SessionIndexError::InvalidReconcile);
            }
        }
    }
    Ok(())
}

fn reconcile_parent_id(
    node: &FileNode,
    future: &HashMap<EntityId, FileNode>,
) -> Result<Option<String>, SessionIndexError> {
    let Some((parent_path, _)) = node.relative_path.as_str().rsplit_once('/') else {
        return Ok(None);
    };
    future
        .values()
        .find(|candidate| candidate.relative_path.as_str() == parent_path)
        .filter(|parent| parent.kind == FileKind::Directory)
        .map(|parent| Some(parent.entity_id.to_string()))
        .ok_or(SessionIndexError::InvalidReconcile)
}

fn path_in_scopes(path: &RelativePath, scopes: &[Option<RelativePath>]) -> bool {
    scopes.iter().any(|scope| match scope {
        None => true,
        Some(scope) => {
            path == scope
                || path
                    .as_str()
                    .strip_prefix(scope.as_str())
                    .is_some_and(|suffix| suffix.starts_with('/'))
        }
    })
}

fn path_depth(path: &str) -> usize {
    path.matches('/').count()
}

fn projected_reconcile_path(path: &str, mapping: &FilePathMove) -> Option<String> {
    if path == mapping.source.as_str() {
        return Some(mapping.destination.as_str().to_owned());
    }
    path.strip_prefix(mapping.source.as_str())
        .and_then(|suffix| suffix.strip_prefix('/'))
        .map(|suffix| format!("{}/{suffix}", mapping.destination.as_str()))
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

impl OperationProjectionPort for SessionIndex {
    fn apply_copy(
        &self,
        copies: &[FileCopyProjection],
        case_sensitive: bool,
    ) -> Result<(), OperationProjectionError> {
        validate_copy_projections(copies, case_sensitive)?;
        let mut connection = self.lock_connection();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| OperationProjectionError::Unavailable)?;

        for copy in copies {
            if !node_matches(&transaction, &copy.source)? {
                return Err(OperationProjectionError::Stale);
            }
            let entity_exists = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM nodes WHERE entity_id = ?1)",
                    [copy.destination.entity_id.to_string()],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(|_| OperationProjectionError::Unavailable)?;
            if entity_exists {
                return Err(OperationProjectionError::Conflict);
            }
        }

        let existing = projection_nodes(&transaction)?;
        let mut occupied = HashSet::with_capacity(existing.len() + copies.len());
        for row in &existing {
            if !occupied.insert(projection_path_key(&row.path, case_sensitive)) {
                return Err(OperationProjectionError::Stale);
            }
        }
        for copy in copies {
            if !occupied.insert(projection_path_key(
                copy.destination.relative_path.as_str(),
                case_sensitive,
            )) {
                return Err(OperationProjectionError::Conflict);
            }
        }

        let mut future_nodes = existing
            .into_iter()
            .map(|row| (row.path, (row.entity_id, row.kind)))
            .collect::<HashMap<_, _>>();
        for copy in copies {
            future_nodes.insert(
                copy.destination.relative_path.as_str().to_owned(),
                (
                    copy.destination.entity_id.to_string(),
                    encode_kind(copy.destination.kind),
                ),
            );
        }

        let mut ordered = copies.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|copy| copy.destination.relative_path.as_str().matches('/').count());
        for copy in ordered {
            let parent_id =
                projection_parent(copy.destination.relative_path.as_str(), &future_nodes)?;
            let size = i64::try_from(copy.destination.size)
                .map_err(|_| OperationProjectionError::InvalidInput)?;
            transaction
                .execute(
                    "INSERT INTO nodes(
                        entity_id, parent_entity_id, relative_path, name, kind, size, modified_ns
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        copy.destination.entity_id.to_string(),
                        parent_id,
                        copy.destination.relative_path.as_str(),
                        projection_name(copy.destination.relative_path.as_str()),
                        encode_kind(copy.destination.kind),
                        size,
                        copy.destination.modified_ns.to_string(),
                    ],
                )
                .map_err(|_| OperationProjectionError::Unavailable)?;
        }
        transaction
            .commit()
            .map_err(|_| OperationProjectionError::Unavailable)
    }

    fn apply_move(
        &self,
        moves: &[FileMoveProjection],
        case_sensitive: bool,
    ) -> Result<(), OperationProjectionError> {
        validate_move_projections(moves, case_sensitive)?;
        let mut connection = self.lock_connection();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| OperationProjectionError::Unavailable)?;
        for mapping in moves {
            if !node_matches(&transaction, &mapping.source)? {
                return Err(OperationProjectionError::Stale);
            }
            if mapping.destination.entity_id != mapping.source.entity_id {
                let destination_exists = transaction
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM nodes WHERE entity_id = ?1)",
                        [mapping.destination.entity_id.to_string()],
                        |row| row.get::<_, bool>(0),
                    )
                    .map_err(|_| OperationProjectionError::Unavailable)?;
                if destination_exists {
                    return Err(OperationProjectionError::Conflict);
                }
            }
        }

        let existing = projection_nodes(&transaction)?;
        let mut affected = Vec::new();
        let mut unaffected = Vec::new();
        for row in existing {
            if let Some((mapping_index, destination)) = projected_session_path(&row.path, moves) {
                if RelativePath::parse(&destination).is_err() {
                    return Err(OperationProjectionError::InvalidInput);
                }
                affected.push(ProjectedSessionNode {
                    entity_id: row.entity_id,
                    old_path: row.path,
                    new_path: destination,
                    kind: row.kind,
                    mapping_index,
                });
            } else {
                unaffected.push(row);
            }
        }

        let mut occupied = HashSet::with_capacity(unaffected.len() + affected.len());
        for row in &unaffected {
            if !occupied.insert(projection_path_key(&row.path, case_sensitive)) {
                return Err(OperationProjectionError::Conflict);
            }
        }
        for row in &affected {
            if !occupied.insert(projection_path_key(&row.new_path, case_sensitive)) {
                return Err(OperationProjectionError::Conflict);
            }
        }

        let mut future_nodes = unaffected
            .into_iter()
            .map(|row| (row.path, (row.entity_id, row.kind)))
            .collect::<HashMap<_, _>>();
        for row in &affected {
            let mapping = &moves[row.mapping_index];
            let entity_id = if row.old_path == mapping.source.relative_path.as_str() {
                mapping.destination.entity_id.to_string()
            } else {
                row.entity_id.clone()
            };
            future_nodes.insert(row.new_path.clone(), (entity_id, row.kind));
        }
        let mut root_parents = HashMap::with_capacity(moves.len());
        for (index, mapping) in moves.iter().enumerate() {
            root_parents.insert(
                index,
                projection_parent(mapping.destination.relative_path.as_str(), &future_nodes)?,
            );
        }

        for row in &affected {
            let temporary = format!(".viewer-projection/{}", row.entity_id);
            transaction
                .execute(
                    "UPDATE nodes SET relative_path = ?2, name = ?3 WHERE entity_id = ?1",
                    params![
                        &row.entity_id,
                        temporary,
                        format!("projection-{}", row.entity_id),
                    ],
                )
                .map_err(|_| OperationProjectionError::Unavailable)?;
        }
        for row in &affected {
            let parent_id = (row.old_path
                == moves[row.mapping_index].source.relative_path.as_str())
            .then(|| root_parents[&row.mapping_index].clone())
            .flatten();
            if row.old_path == moves[row.mapping_index].source.relative_path.as_str() {
                let destination = &moves[row.mapping_index].destination;
                let size = i64::try_from(destination.size)
                    .map_err(|_| OperationProjectionError::InvalidInput)?;
                transaction
                    .execute(
                        "UPDATE nodes
                         SET entity_id = ?2, parent_entity_id = ?3, relative_path = ?4,
                             name = ?5, size = ?6, modified_ns = ?7
                         WHERE entity_id = ?1",
                        params![
                            &row.entity_id,
                            destination.entity_id.to_string(),
                            parent_id,
                            &row.new_path,
                            projection_name(&row.new_path),
                            size,
                            destination.modified_ns.to_string(),
                        ],
                    )
                    .map_err(|_| OperationProjectionError::Unavailable)?;
            } else {
                transaction
                    .execute(
                        "UPDATE nodes SET relative_path = ?2, name = ?3 WHERE entity_id = ?1",
                        params![
                            &row.entity_id,
                            &row.new_path,
                            projection_name(&row.new_path),
                        ],
                    )
                    .map_err(|_| OperationProjectionError::Unavailable)?;
            }
            transaction
                .execute(
                    "UPDATE text_fts SET entity_id = ?2, relative_path = ?3 WHERE entity_id = ?1",
                    params![
                        &row.entity_id,
                        if row.old_path == moves[row.mapping_index].source.relative_path.as_str() {
                            moves[row.mapping_index].destination.entity_id.to_string()
                        } else {
                            row.entity_id.clone()
                        },
                        &row.new_path,
                    ],
                )
                .map_err(|_| OperationProjectionError::Unavailable)?;
        }
        transaction
            .commit()
            .map_err(|_| OperationProjectionError::Unavailable)
    }

    fn apply_trash(&self, sources: &[FileNode]) -> Result<(), OperationProjectionError> {
        validate_trash_projections(sources)?;
        let mut connection = self.lock_connection();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| OperationProjectionError::Unavailable)?;
        for source in sources {
            if !node_matches(&transaction, source)? {
                return Err(OperationProjectionError::Stale);
            }
        }
        for source in sources {
            transaction
                .execute(
                    "DELETE FROM text_fts WHERE entity_id IN (
                        SELECT entity_id FROM nodes
                        WHERE relative_path = ?1
                           OR substr(relative_path, 1, length(?1) + 1) = ?1 || '/'
                     )",
                    [source.relative_path.as_str()],
                )
                .map_err(|_| OperationProjectionError::Unavailable)?;
            transaction
                .execute(
                    "DELETE FROM nodes
                     WHERE relative_path = ?1
                        OR substr(relative_path, 1, length(?1) + 1) = ?1 || '/'",
                    [source.relative_path.as_str()],
                )
                .map_err(|_| OperationProjectionError::Unavailable)?;
        }
        transaction
            .commit()
            .map_err(|_| OperationProjectionError::Unavailable)
    }
}

#[derive(Debug)]
struct ProjectionNodeRow {
    entity_id: String,
    path: String,
    kind: i64,
}

#[derive(Debug)]
struct ProjectedSessionNode {
    entity_id: String,
    old_path: String,
    new_path: String,
    kind: i64,
    mapping_index: usize,
}

fn projection_nodes(
    transaction: &rusqlite::Transaction<'_>,
) -> Result<Vec<ProjectionNodeRow>, OperationProjectionError> {
    let mut statement = transaction
        .prepare("SELECT entity_id, relative_path, kind FROM nodes ORDER BY relative_path")
        .map_err(|_| OperationProjectionError::Unavailable)?;
    let rows = statement
        .query_map([], |row| {
            Ok(ProjectionNodeRow {
                entity_id: row.get(0)?,
                path: row.get(1)?,
                kind: row.get(2)?,
            })
        })
        .map_err(|_| OperationProjectionError::Unavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| OperationProjectionError::Unavailable)?;
    if rows.iter().any(|row| {
        EntityId::from_str(&row.entity_id).is_err()
            || RelativePath::parse(&row.path).is_err()
            || !(0..=4).contains(&row.kind)
    }) {
        return Err(OperationProjectionError::Stale);
    }
    Ok(rows)
}

fn node_matches(
    transaction: &rusqlite::Transaction<'_>,
    node: &FileNode,
) -> Result<bool, OperationProjectionError> {
    let size = i64::try_from(node.size).map_err(|_| OperationProjectionError::InvalidInput)?;
    transaction
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM nodes
                WHERE entity_id = ?1 AND relative_path = ?2 AND kind = ?3
                  AND size = ?4 AND modified_ns = ?5
             )",
            params![
                node.entity_id.to_string(),
                node.relative_path.as_str(),
                encode_kind(node.kind),
                size,
                node.modified_ns.to_string(),
            ],
            |row| row.get(0),
        )
        .map_err(|_| OperationProjectionError::Unavailable)
}

fn validate_copy_projections(
    copies: &[FileCopyProjection],
    case_sensitive: bool,
) -> Result<(), OperationProjectionError> {
    if copies.is_empty() {
        return Err(OperationProjectionError::InvalidInput);
    }
    let mut entities = HashSet::with_capacity(copies.len());
    let mut destinations = HashSet::with_capacity(copies.len());
    for copy in copies {
        if copy.source.kind != copy.destination.kind
            || copy.source.size != copy.destination.size
            || copy.source.entity_id == copy.destination.entity_id
            || !entities.insert(copy.destination.entity_id)
        {
            return Err(OperationProjectionError::InvalidInput);
        }
        if !destinations.insert(projection_path_key(
            copy.destination.relative_path.as_str(),
            case_sensitive,
        )) {
            return Err(OperationProjectionError::Conflict);
        }
    }
    Ok(())
}

fn validate_move_projections(
    moves: &[FileMoveProjection],
    case_sensitive: bool,
) -> Result<(), OperationProjectionError> {
    if moves.is_empty() {
        return Err(OperationProjectionError::InvalidInput);
    }
    let mut entities = HashSet::with_capacity(moves.len());
    let mut destination_entities = HashSet::with_capacity(moves.len());
    let mut sources = HashSet::with_capacity(moves.len());
    let mut destinations = HashSet::with_capacity(moves.len());
    for mapping in moves {
        if mapping.source.relative_path == mapping.destination.relative_path
            || mapping.source.kind != mapping.destination.kind
            || mapping.source.size != mapping.destination.size
            || (mapping.source.kind == FileKind::Directory
                && mapping.source.entity_id != mapping.destination.entity_id)
            || !entities.insert(mapping.source.entity_id)
            || !destination_entities.insert(mapping.destination.entity_id)
            || !sources.insert(projection_path_key(
                mapping.source.relative_path.as_str(),
                case_sensitive,
            ))
            || projection_is_descendant(
                mapping.destination.relative_path.as_str(),
                mapping.source.relative_path.as_str(),
                case_sensitive,
            )
        {
            return Err(OperationProjectionError::InvalidInput);
        }
        if !destinations.insert(projection_path_key(
            mapping.destination.relative_path.as_str(),
            case_sensitive,
        )) {
            return Err(OperationProjectionError::Conflict);
        }
    }
    for (index, mapping) in moves.iter().enumerate() {
        if moves[index + 1..].iter().any(|other| {
            projection_is_descendant(
                mapping.source.relative_path.as_str(),
                other.source.relative_path.as_str(),
                case_sensitive,
            ) || projection_is_descendant(
                other.source.relative_path.as_str(),
                mapping.source.relative_path.as_str(),
                case_sensitive,
            )
        }) {
            return Err(OperationProjectionError::InvalidInput);
        }
    }
    Ok(())
}

fn validate_trash_projections(sources: &[FileNode]) -> Result<(), OperationProjectionError> {
    if sources.is_empty() {
        return Err(OperationProjectionError::InvalidInput);
    }
    let mut entities = HashSet::with_capacity(sources.len());
    let mut paths = HashSet::with_capacity(sources.len());
    for source in sources {
        if !entities.insert(source.entity_id) || !paths.insert(source.relative_path.clone()) {
            return Err(OperationProjectionError::InvalidInput);
        }
    }
    for (index, source) in sources.iter().enumerate() {
        if sources[index + 1..].iter().any(|other| {
            projection_is_descendant(
                source.relative_path.as_str(),
                other.relative_path.as_str(),
                true,
            ) || projection_is_descendant(
                other.relative_path.as_str(),
                source.relative_path.as_str(),
                true,
            )
        }) {
            return Err(OperationProjectionError::InvalidInput);
        }
    }
    Ok(())
}

fn projected_session_path(path: &str, moves: &[FileMoveProjection]) -> Option<(usize, String)> {
    moves.iter().enumerate().find_map(|(index, mapping)| {
        if path == mapping.source.relative_path.as_str() {
            return Some((index, mapping.destination.relative_path.as_str().to_owned()));
        }
        path.strip_prefix(mapping.source.relative_path.as_str())
            .and_then(|suffix| suffix.strip_prefix('/'))
            .map(|suffix| {
                (
                    index,
                    format!("{}/{suffix}", mapping.destination.relative_path.as_str()),
                )
            })
    })
}

fn projection_parent(
    path: &str,
    future_nodes: &HashMap<String, (String, i64)>,
) -> Result<Option<String>, OperationProjectionError> {
    let Some((parent, _)) = path.rsplit_once('/') else {
        return Ok(None);
    };
    let Some((entity_id, kind)) = future_nodes.get(parent) else {
        return Err(OperationProjectionError::Stale);
    };
    if *kind != encode_kind(FileKind::Directory) {
        return Err(OperationProjectionError::Stale);
    }
    Ok(Some(entity_id.clone()))
}

fn projection_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn projection_path_key(path: &str, case_sensitive: bool) -> String {
    if case_sensitive {
        path.to_owned()
    } else {
        path.to_lowercase()
    }
}

fn projection_is_descendant(candidate: &str, ancestor: &str, case_sensitive: bool) -> bool {
    let candidate = projection_path_key(candidate, case_sensitive);
    let ancestor = projection_path_key(ancestor, case_sensitive);
    candidate
        .strip_prefix(&ancestor)
        .is_some_and(|suffix| suffix.starts_with('/'))
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

    fn all_indexed_nodes(&self) -> Result<Vec<IndexedNode>, BrowseIndexError> {
        let connection = self.lock_connection();
        let mut statement = connection
            .prepare_cached(
                "SELECT entity_id, relative_path, kind, size, modified_ns,
                        review_state, favorite, image_width, image_height,
                        image_status, text_status
                 FROM nodes
                 ORDER BY relative_path COLLATE NOCASE, relative_path, entity_id",
            )
            .map_err(SessionIndexError::from)?;
        statement
            .query_map([], read_indexed_node)
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
