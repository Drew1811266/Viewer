//! `plan-ref://` 只服务当前项目 `.viewer/refs/<sha256>` 下的参考图。
//!
//! 参考图不在项目索引里，因此不能复用 `viewer-image://`；这里自己维护一份
//! "当前项目根"，并在每次请求时重新做规范化与包含校验。

use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};
use tauri::http::{Request, Response, StatusCode, header};

#[derive(Clone, Default)]
pub struct ActivePlanRoot(Arc<RwLock<Option<PathBuf>>>);

impl ActivePlanRoot {
    pub fn set(&self, root: Option<PathBuf>) {
        *self
            .0
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = root;
    }

    pub fn get(&self) -> Option<PathBuf> {
        self.0
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

const MAX_REF_BYTES: u64 = 32 * 1024 * 1024;

pub fn handle_request(
    root: &ActivePlanRoot,
    request: &Request<Vec<u8>>,
    responder: tauri::UriSchemeResponder,
) {
    let response = match resolve(root, request) {
        Ok(response) => response,
        Err(status) => Response::builder()
            .status(status)
            .body(Vec::new())
            .unwrap_or_else(|_| Response::new(Vec::new())),
    };
    responder.respond(response);
}

fn resolve(
    root: &ActivePlanRoot,
    request: &Request<Vec<u8>>,
) -> Result<Response<Vec<u8>>, StatusCode> {
    if request.method() != tauri::http::Method::GET {
        return Err(StatusCode::METHOD_NOT_ALLOWED);
    }
    let asset = asset_id(request.uri().path().trim_start_matches('/'))?;
    let project = root.get().ok_or(StatusCode::NOT_FOUND)?;
    let refs = project.join(".viewer").join("refs");
    let path = refs.join(&asset);
    let canonical = path.canonicalize().map_err(|_| StatusCode::NOT_FOUND)?;
    let canonical_refs = refs.canonicalize().map_err(|_| StatusCode::NOT_FOUND)?;
    if !canonical.starts_with(&canonical_refs) {
        return Err(StatusCode::FORBIDDEN);
    }
    let metadata = std::fs::metadata(&canonical).map_err(|_| StatusCode::NOT_FOUND)?;
    if !metadata.is_file() || metadata.len() > MAX_REF_BYTES {
        return Err(StatusCode::FORBIDDEN);
    }
    let bytes = std::fs::read(&canonical).map_err(|_| StatusCode::NOT_FOUND)?;
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime_for(&bytes))
        .header(
            header::CACHE_CONTROL,
            "private, max-age=31536000, immutable",
        )
        .body(bytes)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn asset_id(path: &str) -> Result<String, StatusCode> {
    let value = path.strip_prefix('/').unwrap_or(path);
    if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(value.to_ascii_lowercase())
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

fn mime_for(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        "image/tiff"
    } else {
        "application/octet-stream"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(path: &str) -> Request<Vec<u8>> {
        Request::builder()
            .uri(format!("plan-ref://localhost{path}"))
            .body(Vec::new())
            .unwrap()
    }

    fn project_with_ref(asset: &str) -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        let refs = directory.path().join(".viewer/refs");
        std::fs::create_dir_all(&refs).unwrap();
        std::fs::write(refs.join(asset), b"\x89PNG\r\n\x1a\npayload").unwrap();
        directory
    }

    #[test]
    fn serves_a_reference_image_from_the_project_store() {
        let asset = "a".repeat(64);
        let directory = project_with_ref(&asset);
        let root = ActivePlanRoot::default();
        root.set(Some(directory.path().to_path_buf()));
        let response = resolve(&root, &request(&format!("/{asset}"))).unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
    }

    #[test]
    fn rejects_paths_that_are_not_plain_asset_ids() {
        let directory = project_with_ref(&"b".repeat(64));
        let root = ActivePlanRoot::default();
        root.set(Some(directory.path().to_path_buf()));
        for path in [
            "/../../etc/passwd",
            "/short",
            &format!("/{}", "z".repeat(64)),
        ] {
            assert!(resolve(&root, &request(path)).is_err(), "accepted {path}");
        }
    }

    #[test]
    fn rejects_everything_when_no_project_is_open() {
        let root = ActivePlanRoot::default();
        assert!(resolve(&root, &request(&format!("/{}", "c".repeat(64)))).is_err());
    }

    #[test]
    fn rejects_a_symlink_that_escapes_the_reference_store() {
        let directory = tempfile::tempdir().unwrap();
        let outside = directory.path().join("outside.png");
        std::fs::write(&outside, b"\x89PNG\r\n\x1a\nsecret").unwrap();
        let refs = directory.path().join(".viewer/refs");
        std::fs::create_dir_all(&refs).unwrap();
        let asset = "d".repeat(64);
        std::os::unix::fs::symlink(&outside, refs.join(&asset)).unwrap();

        let root = ActivePlanRoot::default();
        root.set(Some(directory.path().to_path_buf()));
        assert!(resolve(&root, &request(&format!("/{asset}"))).is_err());
    }
}
