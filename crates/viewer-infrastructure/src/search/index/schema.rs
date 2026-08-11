use rusqlite::Connection;
use std::{path::Path, time::Duration};

use super::{SessionIndex, SessionIndexError};

const SESSION_MIGRATION_V1: &str = include_str!("../../../migrations/session/0001_initial.sql");
const SESSION_MIGRATION_V2: &str =
    include_str!("../../../migrations/session/0002_video_metadata.sql");

pub(super) fn open_connection(path: &Path) -> Result<Connection, SessionIndexError> {
    let connection = Connection::open(path)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch(SESSION_MIGRATION_V1)?;
    connection.execute_batch(SESSION_MIGRATION_V2)?;
    Ok(connection)
}

impl SessionIndex {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SessionIndexError> {
        let connection = open_connection(path.as_ref())?;
        Ok(Self {
            connection: std::sync::Mutex::new(connection),
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

    pub(in crate::search) fn lock_connection(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::{SESSION_MIGRATION_V1, SessionIndex};
    use rusqlite::Connection;

    #[test]
    fn opens_v1_index_then_adds_video_metadata_without_reencoding_nodes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session-v1.sqlite");
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch(SESSION_MIGRATION_V1).unwrap();
        for (offset, (name, kind)) in [
            ("folder", 0_i64),
            ("sample.jpg", 1),
            ("sample.png", 2),
            ("sample.md", 3),
            ("sample.txt", 4),
            ("sample.webp", 5),
            ("sample.bin", 6),
        ]
        .into_iter()
        .enumerate()
        {
            connection
                .execute(
                    "INSERT INTO nodes(
                        entity_id, relative_path, name, kind, size, modified_ns
                     ) VALUES (?1, ?2, ?2, ?3, 0, '0')",
                    rusqlite::params![
                        viewer_domain::EntityId::from_u128(offset as u128 + 1).to_string(),
                        name,
                        kind,
                    ],
                )
                .unwrap();
        }
        drop(connection);

        let index = SessionIndex::open(&path).unwrap();
        let connection = index.lock_connection();
        let persisted_kinds = connection
            .prepare("SELECT kind FROM nodes ORDER BY kind")
            .unwrap()
            .query_map([], |row| row.get::<_, i64>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let has_video_metadata = connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sqlite_master
                    WHERE type = 'table' AND name = 'video_metadata'
                 )",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap();

        assert_eq!(persisted_kinds, [0, 1, 2, 3, 4, 5, 6]);
        assert!(has_video_metadata);
    }
}
