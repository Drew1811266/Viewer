use async_trait::async_trait;
use std::path::Path;
use trash::{
    TrashContext,
    macos::{DeleteMethod, TrashContextExtMacos},
};
use viewer_application::{FileOperationError, TrashPort};

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
