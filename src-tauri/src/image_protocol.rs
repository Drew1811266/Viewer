use std::{
    io::Read,
    path::Path,
    sync::{Arc, RwLock},
};
use tauri::http::{Method, Request, Response, StatusCode, header};
use viewer_domain::SessionId;
use viewer_infrastructure::image_cache::{
    ImageArtifactLookup, ImageArtifactRegistry, RegisteredImageArtifact,
};

#[derive(Clone, Default)]
pub struct ActiveImageSession(Arc<RwLock<Option<SessionId>>>);

impl ActiveImageSession {
    pub fn set(&self, session_id: Option<SessionId>) {
        *self
            .0
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = session_id;
    }

    pub fn get(&self) -> Option<SessionId> {
        *self
            .0
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Clone)]
pub struct ImageProtocolResolver {
    active_session: ActiveImageSession,
    registry: Arc<ImageArtifactRegistry>,
}

impl ImageProtocolResolver {
    pub fn new(active_session: ActiveImageSession, registry: Arc<ImageArtifactRegistry>) -> Self {
        Self {
            active_session,
            registry,
        }
    }

    #[cfg(test)]
    pub fn session_id(&self) -> SessionId {
        self.active_session
            .get()
            .expect("test session must be active")
    }

    pub fn resolve(&self, path: &str) -> Result<ProtocolImage, ProtocolError> {
        if path.contains('%') {
            return Err(ProtocolError::BadRequest);
        }
        let path = path.strip_prefix('/').ok_or(ProtocolError::BadRequest)?;
        let mut segments = path.split('/');
        let session = segments.next().filter(|segment| !segment.is_empty());
        let token = segments.next().filter(|segment| !segment.is_empty());
        let (Some(session), Some(token)) = (session, token) else {
            return Err(ProtocolError::BadRequest);
        };
        if segments.next().is_some() {
            return Err(ProtocolError::BadRequest);
        }
        let session_id = session
            .parse::<SessionId>()
            .map_err(|_| ProtocolError::BadRequest)?;
        if self.active_session.get() != Some(session_id) {
            return Err(ProtocolError::Forbidden);
        }

        match self.registry.lookup(session_id, token) {
            ImageArtifactLookup::Found(artifact) => protocol_image(artifact),
            ImageArtifactLookup::WrongSession => Err(ProtocolError::Forbidden),
            ImageArtifactLookup::NotFound => Err(ProtocolError::NotFound),
        }
    }
}

fn protocol_image(artifact: RegisteredImageArtifact) -> Result<ProtocolImage, ProtocolError> {
    if !matches!(artifact.mime(), "image/jpeg" | "image/png") {
        return Err(ProtocolError::UnsupportedMediaType);
    }
    Ok(ProtocolImage {
        cache_path: artifact.cache_path().to_path_buf(),
        mime: artifact.mime().to_owned(),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolImage {
    cache_path: std::path::PathBuf,
    mime: String,
}

impl ProtocolImage {
    pub fn cache_path(&self) -> &Path {
        &self.cache_path
    }

    pub fn mime(&self) -> &str {
        &self.mime
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    BadRequest,
    Forbidden,
    NotFound,
    UnsupportedMediaType,
    MethodNotAllowed,
    Internal,
}

impl ProtocolError {
    fn status(self) -> StatusCode {
        match self {
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn body(self) -> &'static [u8] {
        match self {
            Self::BadRequest => b"bad request",
            Self::Forbidden => b"forbidden",
            Self::NotFound => b"not found",
            Self::UnsupportedMediaType => b"unsupported media type",
            Self::MethodNotAllowed => b"method not allowed",
            Self::Internal => b"internal error",
        }
    }
}

pub fn handle_request(
    resolver: Arc<ImageProtocolResolver>,
    request: Request<Vec<u8>>,
    responder: tauri::UriSchemeResponder,
) {
    tauri::async_runtime::spawn_blocking(move || {
        responder.respond(response_for_request(&resolver, &request));
    });
}

fn response_for_request(
    resolver: &ImageProtocolResolver,
    request: &Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    if request.method() != Method::GET {
        return error_response(ProtocolError::MethodNotAllowed);
    }
    if request.uri().authority().map(|value| value.as_str()) != Some("localhost") {
        return error_response(ProtocolError::BadRequest);
    }

    match resolver.resolve(request.uri().path()) {
        Ok(image) => match read_image_bytes(image.cache_path()) {
            Ok(bytes) => response(StatusCode::OK, image.mime(), bytes),
            Err(_) => error_response(ProtocolError::Internal),
        },
        Err(error) => error_response(error),
    }
}

fn read_image_bytes(path: &Path) -> std::io::Result<Vec<u8>> {
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "image artifact is not a regular file",
        ));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn error_response(error: ProtocolError) -> Response<Vec<u8>> {
    response(
        error.status(),
        "text/plain; charset=utf-8",
        error.body().to_vec(),
    )
}

fn response(status: StatusCode, content_type: &str, body: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-store")
        .header("X-Content-Type-Options", "nosniff")
        .body(body)
        .expect("static image protocol response headers must be valid")
}

#[cfg(test)]
mod tests {
    use super::{ActiveImageSession, ImageProtocolResolver, ProtocolError, response_for_request};
    use std::{fs, sync::Arc};
    use tauri::http::{Method, Request, StatusCode, header};
    use viewer_domain::{EntityId, SessionId};
    use viewer_infrastructure::image_cache::{ImageArtifactRegistry, ImageArtifactToken};

    struct TestResolver {
        resolver: ImageProtocolResolver,
        registry: Arc<ImageArtifactRegistry>,
        directory: tempfile::TempDir,
    }

    impl TestResolver {
        fn new() -> Self {
            let registry = Arc::new(ImageArtifactRegistry::default());
            let session_id = SessionId::new();
            let active_session = ActiveImageSession::default();
            active_session.set(Some(session_id));
            Self {
                resolver: ImageProtocolResolver::new(active_session, Arc::clone(&registry)),
                registry,
                directory: tempfile::tempdir().unwrap(),
            }
        }

        fn session_id(&self) -> SessionId {
            self.resolver.session_id()
        }

        fn insert(&self, mime: &str) -> ImageArtifactToken {
            let artifact = self.directory.path().join(EntityId::new().to_string());
            fs::write(&artifact, b"fixture artifact").unwrap();
            self.registry
                .insert(self.session_id(), EntityId::new(), artifact, mime)
                .unwrap()
        }

        fn path(&self, session_id: SessionId, token: &ImageArtifactToken) -> String {
            format!("/{session_id}/{}", token.as_str())
        }
    }

    #[test]
    fn protocol_rejects_path_traversal_and_unknown_tokens() {
        let resolver = TestResolver::new();
        assert_eq!(
            resolver.resolver.resolve("/../secret"),
            Err(ProtocolError::BadRequest)
        );
        assert_eq!(
            resolver
                .resolver
                .resolve(&format!("/{}/unknown", resolver.session_id())),
            Err(ProtocolError::NotFound)
        );
    }

    #[test]
    fn protocol_rejects_a_token_from_another_session() {
        let resolver = TestResolver::new();
        let token = resolver.insert("image/png");
        assert_eq!(
            resolver
                .resolver
                .resolve(&resolver.path(SessionId::new(), &token)),
            Err(ProtocolError::Forbidden)
        );
    }

    #[test]
    fn protocol_returns_only_image_mime_types() {
        let resolver = TestResolver::new();
        for mime in ["image/png", "image/jpeg"] {
            let token = resolver.insert(mime);
            assert_eq!(
                resolver
                    .resolver
                    .resolve(&resolver.path(resolver.session_id(), &token))
                    .unwrap()
                    .mime(),
                mime
            );
        }

        let token = resolver.insert("text/plain");
        assert_eq!(
            resolver
                .resolver
                .resolve(&resolver.path(resolver.session_id(), &token)),
            Err(ProtocolError::UnsupportedMediaType)
        );
    }

    #[test]
    fn protocol_requires_exactly_two_normalized_segments() {
        let resolver = TestResolver::new();
        for path in ["/", "//token", "/a/b/c", "/./token", "/%2e%2e/token"] {
            assert_eq!(
                resolver.resolver.resolve(path),
                Err(ProtocolError::BadRequest),
                "accepted {path}"
            );
        }
    }

    #[test]
    fn protocol_response_has_restricted_image_headers() {
        let resolver = TestResolver::new();
        let token = resolver.insert("image/png");
        let request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "viewer-image://localhost{}",
                resolver.path(resolver.session_id(), &token)
            ))
            .body(Vec::new())
            .unwrap();

        let response = response_for_request(&resolver.resolver, &request);
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(response.headers()["X-Content-Type-Options"], "nosniff");
        assert_eq!(response.body(), b"fixture artifact");
    }

    #[test]
    fn protocol_response_rejects_methods_and_authority_bypasses() {
        let resolver = TestResolver::new();
        for (method, uri, expected) in [
            (
                Method::POST,
                "viewer-image://localhost/ignored/ignored",
                StatusCode::METHOD_NOT_ALLOWED,
            ),
            (
                Method::GET,
                "viewer-image://evil.example/ignored/ignored",
                StatusCode::BAD_REQUEST,
            ),
            (
                Method::GET,
                "viewer-image://localhost:80/ignored/ignored",
                StatusCode::BAD_REQUEST,
            ),
        ] {
            let request = Request::builder()
                .method(method)
                .uri(uri)
                .body(Vec::new())
                .unwrap();
            assert_eq!(
                response_for_request(&resolver.resolver, &request).status(),
                expected
            );
        }
    }

    #[test]
    fn protocol_internal_errors_do_not_disclose_cache_paths() {
        let resolver = TestResolver::new();
        let token = resolver.insert("image/png");
        let cache_path = resolver.directory.path().to_path_buf();
        fs::remove_dir_all(&cache_path).unwrap();
        let request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "viewer-image://localhost{}",
                resolver.path(resolver.session_id(), &token)
            ))
            .body(Vec::new())
            .unwrap();

        let response = response_for_request(&resolver.resolver, &request);
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = String::from_utf8(response.into_body()).unwrap();
        assert_eq!(body, "internal error");
        assert!(!body.contains(cache_path.to_str().unwrap()));
    }

    #[cfg(unix)]
    #[test]
    fn protocol_refuses_an_artifact_replaced_by_a_symlink() {
        let resolver = TestResolver::new();
        let token = resolver.insert("image/png");
        let artifact = fs::read_dir(resolver.directory.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let secret_directory = tempfile::tempdir().unwrap();
        let secret = secret_directory.path().join("secret.txt");
        fs::write(&secret, b"secret bytes").unwrap();
        fs::remove_file(&artifact).unwrap();
        std::os::unix::fs::symlink(&secret, &artifact).unwrap();
        let request = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "viewer-image://localhost{}",
                resolver.path(resolver.session_id(), &token)
            ))
            .body(Vec::new())
            .unwrap();

        let response = response_for_request(&resolver.resolver, &request);

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.body(), b"internal error");
    }

    #[test]
    fn protocol_tracks_the_current_session_across_close_and_reopen() {
        let active = ActiveImageSession::default();
        let registry = Arc::new(ImageArtifactRegistry::default());
        let directory = tempfile::tempdir().unwrap();
        let artifact = directory.path().join("preview.png");
        fs::write(&artifact, b"png").unwrap();
        let first_session = SessionId::new();
        let token = registry
            .insert(first_session, EntityId::new(), artifact, "image/png")
            .unwrap();
        let resolver = ImageProtocolResolver::new(active.clone(), registry);

        active.set(Some(first_session));
        assert!(
            resolver
                .resolve(&format!("/{first_session}/{}", token.as_str()))
                .is_ok()
        );
        active.set(Some(SessionId::new()));
        assert_eq!(
            resolver.resolve(&format!("/{first_session}/{}", token.as_str())),
            Err(ProtocolError::Forbidden)
        );
    }
}
