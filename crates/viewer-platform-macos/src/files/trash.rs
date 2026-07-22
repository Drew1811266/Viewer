use async_trait::async_trait;
use objc2_foundation::{NSFileManager, NSURL};
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
            trash_verified_sync_with_sink(
                &path,
                &expected,
                expected_parent,
                || {},
                |reference| {
                    NSFileManager::defaultManager()
                        .trashItemAtURL_resultingItemURL_error(reference, None)
                        .map_err(|error| FileOperationError::Io {
                            action: "move identity-bound item to macOS Trash",
                            path: path.clone(),
                            message: error.to_string(),
                        })
                },
            )
        })
        .await
        .map_err(|error| FileOperationError::Io {
            action: "verified macOS Trash worker",
            path: error_path,
            message: error.to_string(),
        })?
    }
}

fn trash_verified_sync_with_sink<BeforeSink, Sink>(
    path: &Path,
    expected: &FileSnapshot,
    expected_parent: FileIdentity,
    before_sink: BeforeSink,
    sink: Sink,
) -> Result<(), FileOperationError>
where
    BeforeSink: FnOnce(),
    Sink: FnOnce(&NSURL) -> Result<(), FileOperationError>,
{
    let (_parent, leaf) = open_bound_trash_leaf(path, expected, expected_parent)?;
    let reference = bound_file_reference(path, &leaf)?;
    before_sink();
    sink(&reference)
}

fn bound_file_reference(
    path: &Path,
    leaf: &std::fs::File,
) -> Result<objc2::rc::Retained<NSURL>, FileOperationError> {
    use std::{
        ffi::{CStr, CString},
        os::{fd::FromRawFd, unix::ffi::OsStrExt},
        ptr::NonNull,
    };
    let encoded = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| FileOperationError::OutsideProject)?;
    let pointer = NonNull::new(encoded.as_ptr().cast_mut()).expect("CString pointer is non-null");
    let path_url = unsafe {
        NSURL::fileURLWithFileSystemRepresentation_isDirectory_relativeToURL(pointer, false, None)
    };
    let reference = path_url
        .fileReferenceURL()
        .ok_or(FileOperationError::IdentityChanged)?;
    let resolved = reference
        .filePathURL()
        .ok_or(FileOperationError::IdentityChanged)?;
    let resolved_path = unsafe { CStr::from_ptr(resolved.fileSystemRepresentation().as_ptr()) };
    let resolved_fd = unsafe {
        libc::open(
            resolved_path.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW_ANY,
        )
    };
    if resolved_fd < 0 {
        return Err(FileOperationError::io(
            "open identity-bound Trash reference",
            path,
            &std::io::Error::last_os_error(),
        ));
    }
    let resolved_file = unsafe { std::fs::File::from_raw_fd(resolved_fd) };
    use std::os::unix::fs::MetadataExt;
    let expected_metadata = leaf
        .metadata()
        .map_err(|error| FileOperationError::io("inspect verified Trash leaf", path, &error))?;
    let resolved_metadata = resolved_file.metadata().map_err(|error| {
        FileOperationError::io("inspect identity-bound Trash reference", path, &error)
    })?;
    if expected_metadata.dev() != resolved_metadata.dev()
        || expected_metadata.ino() != resolved_metadata.ino()
    {
        return Err(FileOperationError::IdentityChanged);
    }
    Ok(reference)
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
    use super::{MacTrashPort, mac_trash_context, trash_verified_sync_with_sink};
    use std::{ffi::CStr, fs, os::unix::ffi::OsStrExt, path::PathBuf};
    use trash::macos::{DeleteMethod, TrashContextExtMacos};
    use viewer_application::{FileOperationError, FileSnapshot, TrashPort, watcher::FileIdentity};

    fn snapshot(path: &std::path::Path) -> FileSnapshot {
        use std::os::unix::fs::MetadataExt;
        let metadata = fs::metadata(path).unwrap();
        FileSnapshot {
            len: metadata.len(),
            volume_id: metadata.dev(),
            file_id: Some(u128::from(metadata.ino())),
            modified_ns: Some(
                i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec()),
            ),
            changed_ns: Some(
                i128::from(metadata.ctime()) * 1_000_000_000 + i128::from(metadata.ctime_nsec()),
            ),
        }
    }

    fn parent_identity(path: &std::path::Path) -> FileIdentity {
        use std::os::unix::fs::MetadataExt;
        let metadata = fs::metadata(path).unwrap();
        FileIdentity {
            volume: metadata.dev(),
            file: metadata.ino(),
        }
    }

    fn fake_trash_reference(
        reference: &objc2_foundation::NSURL,
        destination: &std::path::Path,
    ) -> Result<(), FileOperationError> {
        let path_url = reference
            .filePathURL()
            .ok_or(FileOperationError::IdentityChanged)?;
        let resolved = unsafe { CStr::from_ptr(path_url.fileSystemRepresentation().as_ptr()) };
        fs::rename(
            PathBuf::from(std::ffi::OsStr::from_bytes(resolved.to_bytes())),
            destination,
        )
        .map_err(|error| FileOperationError::io("fake Trash bound reference", destination, &error))
    }

    #[test]
    fn trash_uses_ns_file_manager_without_finder_automation() {
        assert!(matches!(
            mac_trash_context().delete_method(),
            DeleteMethod::NsFileManager
        ));
    }

    #[test]
    fn verified_trash_sink_keeps_the_bound_leaf_when_the_name_is_swapped() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let selected = root.join("selected.bin");
        let replacement = root.join("replacement.bin");
        let parked = root.join("parked.bin");
        let trashed = root.join("trashed.bin");
        fs::write(&selected, b"selected identity").unwrap();
        fs::write(&replacement, b"replacement identity").unwrap();
        let expected = snapshot(&selected);

        trash_verified_sync_with_sink(
            &selected,
            &expected,
            parent_identity(&root),
            || {
                fs::rename(&selected, &parked).unwrap();
                fs::rename(&replacement, &selected).unwrap();
            },
            |reference| fake_trash_reference(reference, &trashed),
        )
        .unwrap();

        assert_eq!(fs::read(&selected).unwrap(), b"replacement identity");
        assert_eq!(fs::read(&trashed).unwrap(), b"selected identity");
        assert!(!parked.exists());
    }

    #[test]
    fn verified_trash_sink_keeps_the_bound_leaf_when_its_parent_is_reparented() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_root = fs::canonicalize(outside.path()).unwrap();
        let parent = root.join("parent");
        let moved_parent = outside_root.join("moved-parent");
        fs::create_dir(&parent).unwrap();
        let selected = parent.join("selected.bin");
        let trashed = outside_root.join("trashed.bin");
        fs::write(&selected, b"selected identity").unwrap();
        let expected = snapshot(&selected);

        trash_verified_sync_with_sink(
            &selected,
            &expected,
            parent_identity(&parent),
            || {
                fs::rename(&parent, &moved_parent).unwrap();
                fs::create_dir(&parent).unwrap();
                fs::write(parent.join("selected.bin"), b"replacement identity").unwrap();
            },
            |reference| fake_trash_reference(reference, &trashed),
        )
        .unwrap();

        assert_eq!(fs::read(&selected).unwrap(), b"replacement identity");
        assert_eq!(fs::read(&trashed).unwrap(), b"selected identity");
        assert!(!moved_parent.join("selected.bin").exists());
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
