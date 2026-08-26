use rusqlite::OptionalExtension;
use viewer_application::{
    browse::{BrowseIndexError, BrowseIndexPort},
    metadata::{IndexProgress, IndexedNode},
};
use viewer_domain::{EntityId, RelativePath, file::FileNode};

use super::{SessionIndex, SessionIndexError, read_count, read_indexed_node, read_node};

impl SessionIndex {
    pub fn pending_video_nodes_after(
        &self,
        after: Option<&FileNode>,
        limit: usize,
    ) -> Result<Vec<FileNode>, SessionIndexError> {
        const MAX_PAGE: usize = 32;
        if limit == 0 {
            return Ok(Vec::new());
        }
        let connection = self.lock_connection();
        let mut statement = connection.prepare_cached(
            "SELECT nodes.entity_id, nodes.relative_path, nodes.kind, nodes.size,
                    nodes.modified_ns
             FROM nodes
             JOIN video_metadata ON video_metadata.node_id = nodes.entity_id
             WHERE nodes.kind = 7 AND video_metadata.probe_status = 0
               AND (
                    ?1 IS NULL
                    OR nodes.relative_path > ?1 COLLATE BINARY
                    OR (nodes.relative_path = ?1 AND nodes.entity_id > ?2 COLLATE BINARY)
               )
             ORDER BY nodes.relative_path COLLATE BINARY, nodes.entity_id COLLATE BINARY
             LIMIT ?3",
        )?;
        let after_path = after.map(|node| node.relative_path.as_str());
        let after_entity = after.map(|node| node.entity_id.to_string());
        let limit = i64::try_from(limit.min(MAX_PAGE)).expect("bounded video page fits in i64");
        statement
            .query_map(
                rusqlite::params![after_path, after_entity, limit],
                read_node,
            )?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn indexed_node(
        &self,
        entity_id: EntityId,
    ) -> Result<Option<IndexedNode>, SessionIndexError> {
        let connection = self.lock_connection();
        connection
            .query_row(
                "SELECT nodes.entity_id, nodes.relative_path, nodes.kind, nodes.size,
                        nodes.modified_ns, nodes.review_state, nodes.favorite,
                        nodes.image_width, nodes.image_height, nodes.image_status,
                        nodes.text_status, video_metadata.duration_us,
                        video_metadata.display_width, video_metadata.display_height,
                        video_metadata.rotation_degrees,
                        video_metadata.frame_rate_millihertz, video_metadata.video_codec,
                        video_metadata.audio_codec, video_metadata.probe_status,
                        video_metadata.failure_kind, video_metadata.updated_generation
                 FROM nodes
                 LEFT JOIN video_metadata ON video_metadata.node_id = nodes.entity_id
                 WHERE nodes.entity_id = ?1",
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
                "SELECT nodes.entity_id, nodes.relative_path, nodes.kind, nodes.size,
                        nodes.modified_ns, nodes.review_state, nodes.favorite,
                        nodes.image_width, nodes.image_height, nodes.image_status,
                        nodes.text_status, video_metadata.duration_us,
                        video_metadata.display_width, video_metadata.display_height,
                        video_metadata.rotation_degrees,
                        video_metadata.frame_rate_millihertz, video_metadata.video_codec,
                        video_metadata.audio_codec, video_metadata.probe_status,
                        video_metadata.failure_kind, video_metadata.updated_generation
                 FROM nodes
                 LEFT JOIN video_metadata ON video_metadata.node_id = nodes.entity_id
                 ORDER BY nodes.relative_path COLLATE NOCASE, nodes.relative_path,
                          nodes.entity_id",
            )
            .map_err(SessionIndexError::from)?;
        statement
            .query_map([], read_indexed_node)
            .map_err(SessionIndexError::from)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(SessionIndexError::from)
            .map_err(Into::into)
    }

    fn indexed_node(&self, entity_id: EntityId) -> Result<Option<IndexedNode>, BrowseIndexError> {
        SessionIndex::indexed_node(self, entity_id).map_err(Into::into)
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

#[cfg(test)]
mod tests {
    use super::*;
    use viewer_domain::{
        RelativePath,
        file::{FileKind, FileNode},
        search::Generation,
        video::{VideoMetadata, VideoProbeStatus},
    };

    #[test]
    fn pending_video_pages_are_bounded_and_keyset_ordered() {
        let directory = tempfile::tempdir().unwrap();
        let index = SessionIndex::open(directory.path().join("session.sqlite")).unwrap();
        let generation = Generation::new(7);
        let nodes = (0..7)
            .map(|position| FileNode {
                entity_id: EntityId::from_u128(position + 1),
                relative_path: RelativePath::parse(&format!("clip-{position}.mp4")).unwrap(),
                kind: FileKind::Video,
                size: position as u64,
                modified_ns: position as i128,
            })
            .collect::<Vec<_>>();
        index.upsert_batch(&nodes, generation).unwrap();
        index
            .replace_video_metadata(
                nodes[2].entity_id,
                &VideoMetadata {
                    duration_us: None,
                    display_width: None,
                    display_height: None,
                    rotation_degrees: 0,
                    frame_rate_millihertz: None,
                    video_codec: None,
                    audio_codec: None,
                    probe_status: VideoProbeStatus::Failed(
                        viewer_domain::video::VideoFailureKind::Damaged,
                    ),
                },
                generation,
            )
            .unwrap();

        let first = index.pending_video_nodes_after(None, 3).unwrap();
        assert_eq!(
            first
                .iter()
                .map(|node| node.relative_path.as_str())
                .collect::<Vec<_>>(),
            ["clip-0.mp4", "clip-1.mp4", "clip-3.mp4"]
        );
        let second = index.pending_video_nodes_after(first.last(), 3).unwrap();
        assert_eq!(
            second
                .iter()
                .map(|node| node.relative_path.as_str())
                .collect::<Vec<_>>(),
            ["clip-4.mp4", "clip-5.mp4", "clip-6.mp4"]
        );
        assert!(
            index
                .pending_video_nodes_after(second.last(), 3)
                .unwrap()
                .is_empty()
        );
    }
}
