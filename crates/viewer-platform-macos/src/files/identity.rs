use std::{ffi::CString, path::Path};
use viewer_application::{FileOperationError, VolumePort};

#[derive(Clone, Copy, Debug, Default)]
pub struct MacVolumePort;

impl VolumePort for MacVolumePort {
    fn volume_id(&self, path: &Path) -> Result<u64, FileOperationError> {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(path)
            .map(|metadata| metadata.dev())
            .map_err(|error| FileOperationError::io("read volume identity", path, &error))
    }

    fn is_case_sensitive(&self, path: &Path) -> Result<bool, FileOperationError> {
        let (canonical, encoded) = encoded_canonical_path(path, "query volume case sensitivity")?;
        // SAFETY: `encoded` is a valid NUL-terminated path and Darwin defines
        // `_PC_CASE_SENSITIVE` as a read-only pathconf selector.
        let value = unsafe { libc::pathconf(encoded.as_ptr(), libc::_PC_CASE_SENSITIVE) };
        match value {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(FileOperationError::Io {
                action: "query volume case sensitivity",
                path: canonical,
                message: std::io::Error::last_os_error().to_string(),
            }),
        }
    }

    fn name_max(&self, path: &Path) -> Result<usize, FileOperationError> {
        let (canonical, encoded) = encoded_canonical_path(path, "query volume name limit")?;
        // SAFETY: `encoded` is a valid NUL-terminated path and `_PC_NAME_MAX`
        // is a read-only POSIX pathconf selector.
        let value = unsafe { libc::pathconf(encoded.as_ptr(), libc::_PC_NAME_MAX) };
        usize::try_from(value).map_err(|_| FileOperationError::Io {
            action: "query volume name limit",
            path: canonical,
            message: std::io::Error::last_os_error().to_string(),
        })
    }
}

fn encoded_canonical_path(
    path: &Path,
    action: &'static str,
) -> Result<(std::path::PathBuf, CString), FileOperationError> {
    use std::os::unix::ffi::OsStrExt;
    let canonical = std::fs::canonicalize(path).map_err(|error| {
        FileOperationError::io("canonicalize volume capability path", path, &error)
    })?;
    let encoded =
        CString::new(canonical.as_os_str().as_bytes()).map_err(|_| FileOperationError::Io {
            action,
            path: canonical.clone(),
            message: "path contains a NUL byte".into(),
        })?;
    Ok((canonical, encoded))
}

#[cfg(test)]
mod tests {
    use super::MacVolumePort;
    use std::os::unix::fs::MetadataExt;
    use viewer_application::VolumePort;

    #[test]
    fn mac_volume_port_reports_identity_and_case_behavior() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(
            MacVolumePort.volume_id(directory.path()).unwrap(),
            std::fs::metadata(directory.path()).unwrap().dev()
        );
        MacVolumePort
            .is_case_sensitive(directory.path())
            .expect("standard test volume exposes case-sensitivity capability");
        assert!(MacVolumePort.name_max(directory.path()).unwrap() >= 255);
    }
}
