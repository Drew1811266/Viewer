use super::schema::{PortableSchemaError, open_database};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    str::FromStr,
};
use viewer_application::ProjectAccess;
use viewer_domain::ProjectId;

const MANIFEST_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, thiserror::Error)]
pub enum PortableMetadataError {
    #[error("portable project manifest is invalid")]
    InvalidManifest,
    #[error("unsupported portable project manifest version {0}")]
    UnsupportedManifestVersion(u16),
    #[error("portable project identity does not match its database")]
    ProjectIdentityMismatch,
    #[error("portable project metadata path is unsafe")]
    UnsafePath,
    #[error("portable project metadata is incomplete")]
    Incomplete,
    #[error("portable project metadata I/O error: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Schema(#[from] PortableSchemaError),
    #[error("portable project metadata database error: {0}")]
    Database(#[from] rusqlite::Error),
}

#[derive(Debug)]
pub struct PortableProjectMetadata {
    project_id: ProjectId,
    database_path: Option<PathBuf>,
    writable: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectManifest {
    schema_version: u16,
    project_id: String,
    created_at_ms: i64,
}

impl PortableProjectMetadata {
    pub fn open(
        root: &Path,
        access: ProjectAccess,
        now_ms: i64,
    ) -> Result<Self, PortableMetadataError> {
        validate_root(root)?;
        let viewer = locate_viewer_directory(root)?;
        let writable = access == ProjectAccess::ReadWrite;
        let Some(viewer) = viewer else {
            if !writable {
                return Ok(Self {
                    project_id: ProjectId::new(),
                    database_path: None,
                    writable: false,
                });
            }
            let viewer = root.join(".viewer");
            fs::create_dir(&viewer)?;
            sync_directory(root)?;
            return Self::open_or_create_persistent(viewer, true, now_ms);
        };
        Self::open_or_create_persistent(viewer, writable, now_ms)
    }

    pub fn project_id(&self) -> ProjectId {
        self.project_id
    }

    pub fn database_path(&self) -> Option<&Path> {
        self.database_path.as_deref()
    }

    pub fn is_persistent(&self) -> bool {
        self.database_path.is_some()
    }

    pub fn is_writable(&self) -> bool {
        self.writable
    }

    fn open_or_create_persistent(
        viewer: PathBuf,
        writable: bool,
        now_ms: i64,
    ) -> Result<Self, PortableMetadataError> {
        validate_viewer_directory(&viewer)?;
        let manifest_path = viewer.join("project.json");
        let manifest = if manifest_path.exists() {
            read_manifest(&manifest_path)?
        } else if writable {
            let manifest = ProjectManifest {
                schema_version: MANIFEST_SCHEMA_VERSION,
                project_id: ProjectId::new().to_string(),
                created_at_ms: now_ms,
            };
            write_manifest(&manifest_path, &manifest)?;
            manifest
        } else {
            return Err(PortableMetadataError::Incomplete);
        };
        if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
            return Err(PortableMetadataError::UnsupportedManifestVersion(
                manifest.schema_version,
            ));
        }
        let project_id = ProjectId::from_str(&manifest.project_id)
            .map_err(|_| PortableMetadataError::InvalidManifest)?;
        let database_path = viewer.join("metadata.sqlite");
        let connection = open_database(&database_path, writable)?;
        let database_id = connection
            .query_row(
                "SELECT project_id FROM project_metadata WHERE singleton = 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        match database_id {
            Some(value) if value != project_id.to_string() => {
                return Err(PortableMetadataError::ProjectIdentityMismatch);
            }
            Some(_) => {}
            None if writable => {
                connection.execute(
                    "INSERT INTO project_metadata(singleton, project_id, created_at_ms)
                     VALUES (1, ?1, ?2)",
                    params![project_id.to_string(), manifest.created_at_ms],
                )?;
            }
            None => return Err(PortableMetadataError::Incomplete),
        }
        Ok(Self {
            project_id,
            database_path: Some(database_path),
            writable,
        })
    }
}

fn validate_root(root: &Path) -> Result<(), PortableMetadataError> {
    let metadata = fs::symlink_metadata(root).map_err(|_| PortableMetadataError::UnsafePath)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(PortableMetadataError::UnsafePath);
    }
    Ok(())
}

fn locate_viewer_directory(root: &Path) -> Result<Option<PathBuf>, PortableMetadataError> {
    let mut found = None;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.eq_ignore_ascii_case(".viewer") {
            if name.as_ref() != ".viewer" || found.is_some() {
                return Err(PortableMetadataError::UnsafePath);
            }
            found = Some(entry.path());
        }
    }
    Ok(found)
}

fn validate_viewer_directory(path: &Path) -> Result<(), PortableMetadataError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| PortableMetadataError::UnsafePath)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(PortableMetadataError::UnsafePath);
    }
    Ok(())
}

fn read_manifest(path: &Path) -> Result<ProjectManifest, PortableMetadataError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| PortableMetadataError::InvalidManifest)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(PortableMetadataError::UnsafePath);
    }
    let manifest: ProjectManifest = serde_json::from_slice(&fs::read(path)?)
        .map_err(|_| PortableMetadataError::InvalidManifest)?;
    if manifest.created_at_ms < 0 {
        return Err(PortableMetadataError::InvalidManifest);
    }
    Ok(manifest)
}

fn write_manifest(path: &Path, manifest: &ProjectManifest) -> Result<(), PortableMetadataError> {
    let temporary = path.with_file_name("project.json.tmp");
    let bytes =
        serde_json::to_vec_pretty(manifest).map_err(|_| PortableMetadataError::InvalidManifest)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| -> io::Result<()> {
        file.write_all(&bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        sync_directory(path.parent().expect("manifest always has a parent"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(Into::into)
}

fn sync_directory(path: &Path) -> io::Result<()> {
    fs::File::open(path)?.sync_all()
}
