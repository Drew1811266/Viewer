use rusqlite::Connection;
use std::{path::Path, time::Duration};

use super::{SessionIndex, SessionIndexError};

const SESSION_MIGRATION_V1: &str = include_str!("../../../migrations/session/0001_initial.sql");

pub(super) fn open_connection(path: &Path) -> Result<Connection, SessionIndexError> {
    let connection = Connection::open(path)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch(SESSION_MIGRATION_V1)?;
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
