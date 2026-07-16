use async_trait::async_trait;
use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::path::Path;
use uuid::Uuid;
use viewer_application::{ProjectAccess, ProjectProbePort};

pub struct MacProjectProbe;

#[async_trait]
impl ProjectProbePort for MacProjectProbe {
    async fn probe(&self, root: &Path) -> Result<ProjectAccess, String> {
        let metadata = fs::metadata(root).map_err(|error| {
            format!(
                "failed to read project root metadata at {}: {error}",
                root.display()
            )
        })?;
        if !metadata.is_dir() {
            return Err(format!(
                "project root is not a directory: {}",
                root.display()
            ));
        }

        let entries = fs::read_dir(root).map_err(|error| {
            format!("failed to read project root at {}: {error}", root.display())
        })?;
        drop(entries);

        let probe_path = root.join(format!(".viewer-write-probe-{}", Uuid::new_v4()));
        let probe_file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::PermissionDenied => {
                return Ok(ProjectAccess::ReadOnly);
            }
            Err(error) => {
                return Err(format!(
                    "failed to create write probe at {}: {error}",
                    probe_path.display()
                ));
            }
        };
        drop(probe_file);

        fs::remove_file(&probe_path).map_err(|error| {
            format!(
                "failed to remove write probe at {}: {error}",
                probe_path.display()
            )
        })?;

        Ok(ProjectAccess::ReadWrite)
    }
}

#[cfg(test)]
mod tests {
    use super::MacProjectProbe;
    use futures::executor::block_on;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;
    use viewer_application::{ProjectAccess, ProjectProbePort};

    #[test]
    fn writable_root_is_read_write_without_touching_viewer_metadata() {
        let root = tempdir().expect("create isolated project root");
        let viewer_directory = root.path().join(".viewer");
        fs::create_dir(&viewer_directory).expect("create empty .viewer directory");

        let result = block_on(MacProjectProbe.probe(root.path()));

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

        let result = block_on(MacProjectProbe.probe(root.path()));
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

        let result = block_on(MacProjectProbe.probe(root.path()));
        fs::set_permissions(root.path(), original_permissions)
            .expect("restore project root permissions before assertions");

        assert!(
            result.is_err(),
            "expected unreadable root error, got {result:?}"
        );
    }
}
