use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::collections::{HashMap, HashSet};
use viewer_application::metadata::{FilePathMove, PortableMarker};
use viewer_domain::{
    EntityId, RelativePath,
    file::{FileKind, FileNode, ImageIndexStatus, ImageMetadata, ReviewState},
    video::{VideoFailureKind, VideoMetadata, VideoProbeStatus},
};

use super::{
    SessionIndex, SessionIndexError, SessionReconcileSummary, encode_kind, encode_review_state,
    projection::projection_name, read_node,
};
use crate::search::text::TextStatus;

impl SessionIndex {
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

    pub fn replace_video_metadata(
        &self,
        entity_id: EntityId,
        metadata: &VideoMetadata,
        generation: u64,
    ) -> Result<(), SessionIndexError> {
        let duration_us = metadata
            .duration_us
            .map(i64::try_from)
            .transpose()
            .map_err(|_| SessionIndexError::InvalidDerivedMetadata(entity_id))?;
        let generation = i64::try_from(generation)
            .map_err(|_| SessionIndexError::InvalidDerivedMetadata(entity_id))?;
        let (probe_status, failure_kind) = encode_video_probe_status(&metadata.probe_status);
        let mut connection = self.lock_connection();
        let transaction = connection.transaction()?;
        let is_video = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM nodes WHERE entity_id = ?1 AND kind = 7)",
            [entity_id.to_string()],
            |row| row.get::<_, bool>(0),
        )?;
        if !is_video {
            return Err(SessionIndexError::InvalidDerivedMetadata(entity_id));
        }
        transaction.execute(
            "INSERT INTO video_metadata(
                node_id, duration_us, display_width, display_height,
                rotation_degrees, frame_rate_millihertz, video_codec,
                audio_codec, probe_status, failure_kind, updated_generation
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(node_id) DO UPDATE SET
                duration_us = excluded.duration_us,
                display_width = excluded.display_width,
                display_height = excluded.display_height,
                rotation_degrees = excluded.rotation_degrees,
                frame_rate_millihertz = excluded.frame_rate_millihertz,
                video_codec = excluded.video_codec,
                audio_codec = excluded.audio_codec,
                probe_status = excluded.probe_status,
                failure_kind = excluded.failure_kind,
                updated_generation = excluded.updated_generation",
            params![
                entity_id.to_string(),
                duration_us,
                metadata.display_width.map(i64::from),
                metadata.display_height.map(i64::from),
                i64::from(metadata.rotation_degrees),
                metadata.frame_rate_millihertz.map(i64::from),
                metadata.video_codec,
                metadata.audio_codec,
                probe_status,
                failure_kind,
                generation,
            ],
        )?;
        transaction.commit()?;
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
                    "DELETE FROM video_metadata WHERE node_id = ?1",
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

fn encode_image_status(status: ImageIndexStatus) -> i64 {
    match status {
        ImageIndexStatus::Pending => 0,
        ImageIndexStatus::Ready => 1,
        ImageIndexStatus::Failed => 2,
    }
}

fn encode_text_status(status: &TextStatus) -> i64 {
    match status {
        TextStatus::Indexed(_) => 1,
        TextStatus::UnsupportedEncoding => 2,
        TextStatus::TooLarge => 3,
    }
}

fn encode_video_probe_status(status: &VideoProbeStatus) -> (i64, Option<i64>) {
    match status {
        VideoProbeStatus::Pending => (0, None),
        VideoProbeStatus::Ready => (1, None),
        VideoProbeStatus::Failed(failure) => (2, Some(encode_video_failure(*failure))),
    }
}

fn encode_video_failure(failure: VideoFailureKind) -> i64 {
    match failure {
        VideoFailureKind::Unsupported => 0,
        VideoFailureKind::Damaged => 1,
        VideoFailureKind::Unreadable => 2,
        VideoFailureKind::Missing => 3,
        VideoFailureKind::EngineInitialization => 4,
        VideoFailureKind::DecodeFallbackFailed => 5,
        VideoFailureKind::RenderSurface => 6,
        VideoFailureKind::ThumbnailUnavailable => 7,
    }
}

#[cfg(test)]
mod tests {
    use super::SessionIndex;
    use viewer_domain::{
        EntityId, RelativePath,
        file::{FileKind, FileNode},
        video::{VideoFailureKind, VideoMetadata, VideoProbeStatus},
    };

    fn video_metadata(probe_status: VideoProbeStatus) -> VideoMetadata {
        VideoMetadata {
            duration_us: Some(12_345_678),
            display_width: Some(1_920),
            display_height: Some(1_080),
            rotation_degrees: 90,
            frame_rate_millihertz: Some(29_970),
            video_codec: Some("h264".to_owned()),
            audio_codec: Some("aac".to_owned()),
            probe_status,
        }
    }

    fn index_with_video() -> (tempfile::TempDir, SessionIndex, EntityId) {
        let directory = tempfile::tempdir().unwrap();
        let index = SessionIndex::open(directory.path().join("session.sqlite")).unwrap();
        let entity_id = EntityId::from_u128(42);
        index
            .upsert_batch(&[FileNode {
                entity_id,
                relative_path: RelativePath::parse("clip.mp4").unwrap(),
                kind: FileKind::Video,
                size: 123,
                modified_ns: 456,
            }])
            .unwrap();
        (directory, index, entity_id)
    }

    #[test]
    fn replace_video_metadata_round_trips_values_and_generation() {
        let (_directory, index, entity_id) = index_with_video();
        let metadata = video_metadata(VideoProbeStatus::Ready);

        index
            .replace_video_metadata(entity_id, &metadata, 17)
            .unwrap();

        assert_eq!(
            index
                .indexed_node(entity_id)
                .unwrap()
                .unwrap()
                .video_metadata,
            Some(metadata)
        );
        let connection = index.lock_connection();
        assert_eq!(
            connection
                .query_row(
                    "SELECT probe_status, failure_kind, updated_generation
                     FROM video_metadata WHERE node_id = ?1",
                    [entity_id.to_string()],
                    |row| Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, i64>(2)?
                    )),
                )
                .unwrap(),
            (1, None, 17)
        );
    }

    #[test]
    fn replace_video_metadata_preserves_missing_media_properties() {
        let (_directory, index, entity_id) = index_with_video();
        let metadata = VideoMetadata {
            duration_us: None,
            display_width: None,
            display_height: None,
            rotation_degrees: 0,
            frame_rate_millihertz: None,
            video_codec: Some("vp9".to_owned()),
            audio_codec: None,
            probe_status: VideoProbeStatus::Ready,
        };

        index
            .replace_video_metadata(entity_id, &metadata, 5)
            .unwrap();

        assert_eq!(
            index
                .indexed_node(entity_id)
                .unwrap()
                .unwrap()
                .video_metadata,
            Some(metadata)
        );
    }

    #[test]
    fn probe_and_failure_persistence_encodings_are_explicit_and_stable() {
        let (_directory, index, entity_id) = index_with_video();
        let cases = [
            (VideoProbeStatus::Pending, 0_i64, None),
            (VideoProbeStatus::Ready, 1, None),
            (
                VideoProbeStatus::Failed(VideoFailureKind::Unsupported),
                2,
                Some(0),
            ),
            (
                VideoProbeStatus::Failed(VideoFailureKind::Damaged),
                2,
                Some(1),
            ),
            (
                VideoProbeStatus::Failed(VideoFailureKind::Unreadable),
                2,
                Some(2),
            ),
            (
                VideoProbeStatus::Failed(VideoFailureKind::Missing),
                2,
                Some(3),
            ),
            (
                VideoProbeStatus::Failed(VideoFailureKind::EngineInitialization),
                2,
                Some(4),
            ),
            (
                VideoProbeStatus::Failed(VideoFailureKind::DecodeFallbackFailed),
                2,
                Some(5),
            ),
            (
                VideoProbeStatus::Failed(VideoFailureKind::RenderSurface),
                2,
                Some(6),
            ),
            (
                VideoProbeStatus::Failed(VideoFailureKind::ThumbnailUnavailable),
                2,
                Some(7),
            ),
        ];

        for (generation, (status, expected_status, expected_failure)) in
            cases.into_iter().enumerate()
        {
            index
                .replace_video_metadata(entity_id, &video_metadata(status), generation as u64)
                .unwrap();
            let connection = index.lock_connection();
            let persisted = connection
                .query_row(
                    "SELECT probe_status, failure_kind FROM video_metadata WHERE node_id = ?1",
                    [entity_id.to_string()],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?)),
                )
                .unwrap();
            drop(connection);
            assert_eq!(persisted, (expected_status, expected_failure));
        }
    }

    #[test]
    fn deleting_a_video_node_cascades_to_its_metadata() {
        let (_directory, index, entity_id) = index_with_video();
        index
            .replace_video_metadata(entity_id, &video_metadata(VideoProbeStatus::Ready), 1)
            .unwrap();

        assert_eq!(index.remove_subtree(entity_id).unwrap(), 1);

        let connection = index.lock_connection();
        let metadata_rows = connection
            .query_row("SELECT COUNT(*) FROM video_metadata", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        assert_eq!(metadata_rows, 0);
    }

    #[test]
    fn reconciling_changed_video_content_clears_stale_metadata() {
        let (_directory, index, entity_id) = index_with_video();
        index
            .replace_video_metadata(entity_id, &video_metadata(VideoProbeStatus::Ready), 1)
            .unwrap();

        let changed = FileNode {
            entity_id,
            relative_path: RelativePath::parse("clip.mp4").unwrap(),
            kind: FileKind::Video,
            size: 999,
            modified_ns: 1_000,
        };
        index
            .reconcile_subtrees(&[None], &[], std::slice::from_ref(&changed))
            .unwrap();

        let indexed = index.indexed_node(entity_id).unwrap().unwrap();
        assert_eq!(indexed.node, changed);
        assert_eq!(indexed.video_metadata, None);
    }
}
