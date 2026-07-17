#[cfg(target_os = "macos")]
pub mod files;
#[cfg(target_os = "macos")]
pub mod image;

use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::path::Path;
use uuid::Uuid;
use viewer_application::{
    ProjectAccess, ProjectProbeError, ProjectProbeOperation, ProjectProbePort,
};

pub struct MacProjectProbe;

fn is_read_only_write_error(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem
    )
}

impl ProjectProbePort for MacProjectProbe {
    fn probe(&self, root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
        let metadata = fs::metadata(root).map_err(|error| {
            ProjectProbeError::io(ProjectProbeOperation::ReadMetadata, root, &error)
        })?;
        if !metadata.is_dir() {
            return Err(ProjectProbeError::NotDirectory {
                path: root.to_path_buf(),
            });
        }

        let entries = fs::read_dir(root).map_err(|error| {
            ProjectProbeError::io(ProjectProbeOperation::ReadDirectory, root, &error)
        })?;
        drop(entries);

        let probe_path = root.join(format!(".viewer-write-probe-{}", Uuid::new_v4()));
        let probe_file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe_path)
        {
            Ok(file) => file,
            Err(error) if is_read_only_write_error(&error) => {
                return Ok(ProjectAccess::ReadOnly);
            }
            Err(error) => {
                return Err(ProjectProbeError::io(
                    ProjectProbeOperation::CreateWriteProbe,
                    &probe_path,
                    &error,
                ));
            }
        };
        drop(probe_file);

        fs::remove_file(&probe_path).map_err(|error| {
            ProjectProbeError::io(ProjectProbeOperation::RemoveWriteProbe, &probe_path, &error)
        })?;

        Ok(ProjectAccess::ReadWrite)
    }
}

#[cfg(test)]
mod tests {
    use super::MacProjectProbe;
    use std::fs;
    use std::io::ErrorKind;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;
    use viewer_application::{ProjectAccess, ProjectProbePort};

    #[test]
    fn writable_root_is_read_write_without_touching_viewer_metadata() {
        let root = tempdir().expect("create isolated project root");
        let viewer_directory = root.path().join(".viewer");
        fs::create_dir(&viewer_directory).expect("create empty .viewer directory");

        let result = MacProjectProbe.probe(root.path());

        assert_eq!(result, Ok(ProjectAccess::ReadWrite));
        let root_entries = fs::read_dir(root.path())
            .expect("read project root after probe")
            .map(|entry| entry.expect("read project root entry").file_name())
            .collect::<Vec<_>>();
        assert_eq!(root_entries, [".viewer"]);
        assert_eq!(
            fs::read_dir(&viewer_directory)
                .expect("read .viewer after probe")
                .count(),
            0
        );
    }

    #[test]
    fn readable_non_writable_root_is_read_only() {
        let root = tempdir().expect("create isolated project root");
        let original_permissions = fs::metadata(root.path())
            .expect("read original root metadata")
            .permissions();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o555))
            .expect("make project root read-only");

        let result = MacProjectProbe.probe(root.path());
        fs::set_permissions(root.path(), original_permissions)
            .expect("restore project root permissions before assertions");

        assert_eq!(result, Ok(ProjectAccess::ReadOnly));
    }

    #[test]
    fn unreadable_root_returns_an_error() {
        let root = tempdir().expect("create isolated project root");
        let original_permissions = fs::metadata(root.path())
            .expect("read original root metadata")
            .permissions();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o000))
            .expect("make project root unreadable");

        let result = MacProjectProbe.probe(root.path());
        fs::set_permissions(root.path(), original_permissions)
            .expect("restore project root permissions before assertions");

        assert!(
            result.is_err(),
            "expected unreadable root error, got {result:?}"
        );
    }

    #[test]
    fn write_probe_classifies_permission_and_read_only_filesystems() {
        for kind in [ErrorKind::PermissionDenied, ErrorKind::ReadOnlyFilesystem] {
            let error = std::io::Error::from(kind);
            assert!(super::is_read_only_write_error(&error));
        }

        let other = std::io::Error::from(ErrorKind::Other);
        assert!(!super::is_read_only_write_error(&other));
    }
}
