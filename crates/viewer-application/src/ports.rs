use crate::{FileOperationError, FileSnapshot, ImageArtifact, ImageError, ImageRequest};
use async_trait::async_trait;
use std::fmt;
use std::path::{Path, PathBuf};
use viewer_domain::{SessionId, image::ImageProbe};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectAccess {
    ReadWrite,
    ReadOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectProbeOperation {
    ReadMetadata,
    ReadDirectory,
    CreateWriteProbe,
    RemoveWriteProbe,
}

impl fmt::Display for ProjectProbeOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::ReadMetadata => "read project root metadata",
            Self::ReadDirectory => "read project root directory",
            Self::CreateWriteProbe => "create project write probe",
            Self::RemoveWriteProbe => "remove project write probe",
        };
        formatter.write_str(label)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProjectProbeError {
    #[error("project root is not a directory: {path}")]
    NotDirectory { path: PathBuf },
    #[error("failed to {operation} at {path}: {message}")]
    Io {
        operation: ProjectProbeOperation,
        path: PathBuf,
        message: String,
    },
}

impl ProjectProbeError {
    pub fn io(
        operation: ProjectProbeOperation,
        path: impl Into<PathBuf>,
        error: &std::io::Error,
    ) -> Self {
        Self::Io {
            operation,
            path: path.into(),
            message: error.to_string(),
        }
    }
}

pub trait ProjectProbePort: Send + Sync {
    fn probe(&self, root: &Path) -> Result<ProjectAccess, ProjectProbeError>;
}

pub trait ClockPort: Send + Sync {
    fn unix_millis(&self) -> i64;
}

#[async_trait]
pub trait FileMutationPort: Send + Sync {
    async fn snapshot(&self, path: &Path) -> Result<FileSnapshot, FileOperationError>;

    async fn copy_and_hash(
        &self,
        source: &Path,
        temporary: &Path,
    ) -> Result<(u64, [u8; 32]), FileOperationError>;

    async fn rename(&self, source: &Path, destination: &Path) -> Result<(), FileOperationError>;

    async fn remove_registered_temporary(&self, path: &Path) -> Result<(), FileOperationError>;
}

#[async_trait]
pub trait TrashPort: Send + Sync {
    async fn trash(&self, path: &Path) -> Result<(), FileOperationError>;
}

pub trait VolumePort: Send + Sync {
    fn volume_id(&self, path: &Path) -> Result<u64, FileOperationError>;
    fn is_case_sensitive(&self, path: &Path) -> Result<bool, FileOperationError>;
}

#[async_trait]
pub trait ImagePort: Send + Sync {
    async fn probe(&self, source: &Path) -> Result<ImageProbe, ImageError>;
    async fn render(&self, request: ImageRequest) -> Result<ImageArtifact, ImageError>;
    async fn cancel_session(&self, session_id: SessionId);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    struct TypedProbe;

    impl ProjectProbePort for TypedProbe {
        fn probe(&self, root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
            Err(ProjectProbeError::NotDirectory {
                path: root.to_path_buf(),
            })
        }
    }

    #[test]
    fn project_probe_port_exposes_a_synchronous_typed_error() {
        let result: Result<ProjectAccess, ProjectProbeError> =
            TypedProbe.probe(Path::new("not-a-directory"));
        assert_eq!(
            result,
            Err(ProjectProbeError::NotDirectory {
                path: PathBuf::from("not-a-directory"),
            })
        );
    }
}
