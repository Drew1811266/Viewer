use async_trait::async_trait;
use std::path::Path;
use viewer_application::{FileOperationError, TrashPort};

#[derive(Clone, Copy, Debug, Default)]
pub struct MacTrashPort;

#[async_trait]
impl TrashPort for MacTrashPort {
    async fn trash(&self, path: &Path) -> Result<(), FileOperationError> {
        let path = path.to_path_buf();
        let error_path = path.clone();
        tokio::task::spawn_blocking(move || {
            ::trash::delete(&path).map_err(|error| FileOperationError::Io {
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
}

#[cfg(test)]
mod tests {
    use super::MacTrashPort;
    use viewer_application::TrashPort;

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
