use async_trait::async_trait;
use std::path::Path;
use trash::{
    TrashContext,
    macos::{DeleteMethod, TrashContextExtMacos},
};
use viewer_application::{FileOperationError, FileSnapshot, TrashPort, watcher::FileIdentity};

#[derive(Clone, Copy, Debug, Default)]
pub struct MacTrashPort;

fn mac_trash_context() -> TrashContext {
    let mut context = TrashContext::new();
    context.set_delete_method(DeleteMethod::NsFileManager);
    context
}

#[async_trait]
impl TrashPort for MacTrashPort {
    async fn trash(&self, path: &Path) -> Result<(), FileOperationError> {
        let path = path.to_path_buf();
        let error_path = path.clone();
        tokio::task::spawn_blocking(move || {
            mac_trash_context()
                .delete(&path)
                .map_err(|error| FileOperationError::Io {
                    action: "move item to macOS Trash",
                    path,
                    message: error.to_string(),
                })
        })
        .await
        .map_err(|error| FileOperationError::Io {
            action: "macOS Trash worker",
            path: error_path,
            message: error.to_string(),
        })?
    }

    async fn trash_verified(
        &self,
        path: &Path,
        expected: &FileSnapshot,
        expected_parent: FileIdentity,
    ) -> Result<(), FileOperationError> {
        let path = path.to_path_buf();
        let expected = expected.clone();
        let error_path = path.clone();
        tokio::task::spawn_blocking(move || {
            let (_parent, _leaf) = open_bound_trash_leaf(&path, &expected, expected_parent)?;
            mac_trash_context()
                .delete(&path)
                .map_err(|error| FileOperationError::Io {
                    action: "move verified item to macOS Trash",
                    path,
                    message: error.to_string(),
                })
        })
        .await
        .map_err(|error| FileOperationError::Io {
            action: "verified macOS Trash worker",
            path: error_path,
            message: error.to_string(),
        })?
    }
}

fn open_bound_trash_leaf(
    path: &Path,
    expected: &FileSnapshot,
    expected_parent: FileIdentity,
) -> Result<(std::fs::File, std::fs::File), FileOperationError> {
    use std::{
        ffi::CString,
        os::{fd::FromRawFd, unix::ffi::OsStrExt, unix::fs::MetadataExt},
    };
    let parent_path = path.parent().ok_or(FileOperationError::OutsideProject)?;
    let parent_name = CString::new(parent_path.as_os_str().as_bytes())
        .map_err(|_| FileOperationError::OutsideProject)?;
    let parent_fd = unsafe {
        libc::open(
            parent_name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW_ANY,
        )
    };
    if parent_fd < 0 {
        return Err(FileOperationError::io(
            "open verified Trash parent",
            parent_path,
            &std::io::Error::last_os_error(),
        ));
    }
    let parent = unsafe { std::fs::File::from_raw_fd(parent_fd) };
    let parent_metadata = parent.metadata().map_err(|error| {
        FileOperationError::io("inspect verified Trash parent", parent_path, &error)
    })?;
    if parent_metadata.dev() != expected_parent.volume
        || parent_metadata.ino() != expected_parent.file
    {
        return Err(FileOperationError::IdentityChanged);
    }
    let leaf_name = CString::new(
        path.file_name()
            .ok_or(FileOperationError::OutsideProject)?
            .as_bytes(),
    )
    .map_err(|_| FileOperationError::OutsideProject)?;
    use std::os::fd::AsRawFd;
    let leaf_fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            leaf_name.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if leaf_fd < 0 {
        return Err(FileOperationError::io(
            "open verified Trash leaf",
            path,
            &std::io::Error::last_os_error(),
        ));
    }
    let leaf = unsafe { std::fs::File::from_raw_fd(leaf_fd) };
    let metadata = leaf
        .metadata()
        .map_err(|error| FileOperationError::io("inspect verified Trash leaf", path, &error))?;
    let modified_ns =
        i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec());
    if !metadata.is_file()
        || metadata.len() != expected.len
        || metadata.dev() != expected.volume_id
        || Some(u128::from(metadata.ino())) != expected.file_id
        || Some(modified_ns) != expected.modified_ns
    {
        return Err(FileOperationError::IdentityChanged);
    }
    Ok((parent, leaf))
}

#[cfg(test)]
mod tests {
    use super::{MacTrashPort, mac_trash_context};
    use trash::macos::{DeleteMethod, TrashContextExtMacos};
    use viewer_application::TrashPort;

    #[test]
    fn trash_uses_ns_file_manager_without_finder_automation() {
        assert!(matches!(
            mac_trash_context().delete_method(),
            DeleteMethod::NsFileManager
        ));
    }

    #[tokio::test]
    async fn real_trash_smoke_test_requires_explicit_opt_in() {
        if std::env::var("VIEWER_ALLOW_REAL_TRASH_TEST").as_deref() != Ok("1") {
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("viewer-real-trash-smoke.txt");
        std::fs::write(&path, b"Viewer disposable Trash smoke test").unwrap();
        MacTrashPort.trash(&path).await.unwrap();
        assert!(!path.exists());
    }
}
