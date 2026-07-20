use rusqlite::{Connection, OpenFlags};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};

const PORTABLE_MIGRATION_V1: &str = include_str!("../../migrations/portable/0001_initial.sql");
const PORTABLE_MIGRATION_V2: &str = include_str!("../../migrations/portable/0002_markers.sql");
const PORTABLE_MIGRATION_V3: &str =
    include_str!("../../migrations/portable/0003_operation_results.sql");
pub const LATEST_PORTABLE_SCHEMA_VERSION: i64 = 3;

#[derive(Debug, thiserror::Error)]
pub enum PortableSchemaError {
    #[error("portable metadata database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("portable metadata I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("portable metadata database is missing")]
    Missing,
    #[error("portable metadata database requires migration")]
    RequiresMigration,
    #[error("unsupported portable metadata schema version {0}")]
    UnsupportedSchema(i64),
    #[error("portable metadata schema history is not contiguous from version 1")]
    InvalidSchemaHistory,
    #[error("portable metadata database path is unsafe")]
    UnsafePath,
}

pub fn open_database(path: &Path, writable: bool) -> Result<Connection, PortableSchemaError> {
    validate_database_path(path)?;
    if !path.exists() {
        if !writable {
            return Err(PortableSchemaError::Missing);
        }
        let connection = open_connection(path, true, true)?;
        configure(&connection, true)?;
        connection.execute_batch(PORTABLE_MIGRATION_V1)?;
        connection.execute_batch(PORTABLE_MIGRATION_V2)?;
        connection.execute_batch(PORTABLE_MIGRATION_V3)?;
        return Ok(connection);
    }

    let version = inspect_version(path)?;
    if version < LATEST_PORTABLE_SCHEMA_VERSION && !writable {
        return Err(PortableSchemaError::RequiresMigration);
    }
    if version == 1 {
        sanitize_legacy_error_codes(path)?;
        backup_schema(path, 1)?;
        apply_migration(path, PORTABLE_MIGRATION_V2)?;
    }
    if version <= 2 {
        sanitize_legacy_error_codes(path)?;
        backup_schema(path, 2)?;
        apply_migration(path, PORTABLE_MIGRATION_V3)?;
    }
    if writable {
        sanitize_retained_backups(path)?;
    }

    let connection = open_connection(path, writable, false)?;
    configure(&connection, writable)?;
    Ok(connection)
}

fn open_connection(
    path: &Path,
    writable: bool,
    create: bool,
) -> Result<Connection, rusqlite::Error> {
    let mut flags = if writable {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    } else {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    if create {
        flags |= OpenFlags::SQLITE_OPEN_CREATE;
    }
    Connection::open_with_flags(path, flags)
}

fn configure(connection: &Connection, writable: bool) -> Result<(), rusqlite::Error> {
    connection.busy_timeout(Duration::from_secs(5))?;
    if writable {
        connection.execute_batch(
            "PRAGMA foreign_keys = ON; PRAGMA journal_mode = DELETE; PRAGMA synchronous = FULL;",
        )?;
    } else {
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
    }
    Ok(())
}

fn inspect_version(path: &Path) -> Result<i64, PortableSchemaError> {
    let connection = open_connection(path, false, false)?;
    let has_table = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master
            WHERE type = 'table' AND name = 'schema_migrations'
         )",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    if !has_table {
        return Err(PortableSchemaError::UnsupportedSchema(0));
    }
    let mut statement =
        connection.prepare("SELECT version FROM schema_migrations ORDER BY version")?;
    let versions = statement
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    match versions.as_slice() {
        [1] => Ok(1),
        [1, 2] => Ok(2),
        [1, 2, 3] => Ok(3),
        [] => Err(PortableSchemaError::UnsupportedSchema(0)),
        _ if versions
            .last()
            .is_some_and(|version| *version > LATEST_PORTABLE_SCHEMA_VERSION) =>
        {
            Err(PortableSchemaError::UnsupportedSchema(
                *versions.last().expect("non-empty version history"),
            ))
        }
        _ => Err(PortableSchemaError::InvalidSchemaHistory),
    }
}

fn apply_migration(path: &Path, migration: &str) -> Result<(), PortableSchemaError> {
    let mut connection = open_connection(path, true, false)?;
    configure(&connection, true)?;
    let transaction = connection.transaction()?;
    transaction.execute_batch(migration)?;
    transaction.commit()?;
    drop(connection);
    sync_parent(path)?;
    Ok(())
}

fn backup_schema(path: &Path, version: i64) -> Result<(), PortableSchemaError> {
    let backup = backup_path(path, version)?;
    if let Ok(metadata) = fs::symlink_metadata(&backup) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(PortableSchemaError::UnsafePath);
        }
        if backup_is_valid(&backup, version)? {
            sanitize_legacy_error_codes(&backup)?;
            if files_are_identical(path, &backup)? {
                return Ok(());
            }
        }
    }

    let temporary = backup_temporary_path(&backup)?;
    if let Ok(metadata) = fs::symlink_metadata(&temporary) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(PortableSchemaError::UnsafePath);
        }
        fs::remove_file(&temporary)?;
    }
    let bytes = fs::read(path)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    output.write_all(&bytes)?;
    output.sync_all()?;
    drop(output);
    sanitize_legacy_error_codes(&temporary)?;
    if !backup_is_valid(&temporary, version)? {
        return Err(PortableSchemaError::InvalidSchemaHistory);
    }
    fs::rename(&temporary, &backup)?;
    sync_parent(&backup)?;
    Ok(())
}

fn backup_is_valid(path: &Path, expected_version: i64) -> Result<bool, PortableSchemaError> {
    validate_database_path(path)?;
    let version = match inspect_version(path) {
        Ok(version) => version,
        Err(_) => return Ok(false),
    };
    if version != expected_version {
        return Ok(false);
    }
    let connection = match open_connection(path, false, false) {
        Ok(connection) => connection,
        Err(_) => return Ok(false),
    };
    let integrity =
        connection.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0));
    Ok(matches!(integrity, Ok(result) if result == "ok"))
}

fn backup_temporary_path(backup: &Path) -> Result<PathBuf, PortableSchemaError> {
    let name = backup
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(PortableSchemaError::UnsafePath)?;
    Ok(backup.with_file_name(format!(".{name}.tmp")))
}

fn files_are_identical(left: &Path, right: &Path) -> Result<bool, io::Error> {
    if fs::metadata(left)?.len() != fs::metadata(right)?.len() {
        return Ok(false);
    }
    Ok(fs::read(left)? == fs::read(right)?)
}

fn sanitize_retained_backups(path: &Path) -> Result<(), PortableSchemaError> {
    for version in 1..LATEST_PORTABLE_SCHEMA_VERSION {
        let backup = backup_path(path, version)?;
        if backup.exists() {
            sanitize_legacy_error_codes(&backup)?;
        }
    }
    Ok(())
}

fn sanitize_legacy_error_codes(path: &Path) -> Result<(), PortableSchemaError> {
    validate_database_path(path)?;
    let connection = open_connection(path, true, false)?;
    configure(&connection, true)?;
    connection.execute_batch("PRAGMA secure_delete = ON;")?;
    let unsafe_count = connection.query_row(
        "SELECT COUNT(*)
         FROM operation_items
         WHERE error_code IS NOT NULL
           AND (
             length(error_code) NOT BETWEEN 1 AND 64
             OR error_code GLOB '*[^a-z0-9_]*'
           )",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    if unsafe_count > 0 {
        connection.execute(
            "UPDATE operation_items
             SET error_code = 'legacy_failure'
             WHERE error_code IS NOT NULL
               AND (
                 length(error_code) NOT BETWEEN 1 AND 64
                 OR error_code GLOB '*[^a-z0-9_]*'
               )",
            [],
        )?;
    }
    drop(connection);
    sync_parent(path)?;
    Ok(())
}

fn backup_path(path: &Path, version: i64) -> Result<PathBuf, PortableSchemaError> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(PortableSchemaError::UnsafePath)?;
    Ok(path.with_file_name(format!("{name}.v{version}.bak")))
}

fn validate_database_path(path: &Path) -> Result<(), PortableSchemaError> {
    if let Ok(metadata) = fs::symlink_metadata(path)
        && (metadata.file_type().is_symlink() || !metadata.is_file())
    {
        return Err(PortableSchemaError::UnsafePath);
    }
    let parent = path.parent().ok_or(PortableSchemaError::UnsafePath)?;
    let metadata = fs::symlink_metadata(parent)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(PortableSchemaError::UnsafePath);
    }
    Ok(())
}

fn sync_parent(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing parent"))?;
    fs::File::open(parent)?.sync_all()
}
