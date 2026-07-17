use std::{fs, sync::Arc};
use tauri::Url;
use viewer_application::{
    ScanPort,
    scan::{ScanEvent, ScanRequest},
};
use viewer_desktop::{
    image_protocol::{ImageProtocolResolver, ProtocolError},
    is_allowed_navigation, sanitize_markdown_html,
};
use viewer_domain::{EntityId, RelativePath, SessionId, search::Generation};
use viewer_infrastructure::{image_cache::ImageArtifactRegistry, scan::walker::ProjectWalker};

#[test]
fn project_paths_reject_lexical_and_symlink_escape_boundaries() {
    for rejected in [
        "/absolute/front.jpg",
        "../outside.jpg",
        "products/../outside.jpg",
        ".viewer/metadata.sqlite",
        "products/.VIEWER/metadata.sqlite",
    ] {
        assert!(
            RelativePath::parse(rejected).is_err(),
            "accepted {rejected}"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn project_scan_never_publishes_a_symlink_target() {
    use std::os::unix::fs::symlink;

    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.jpg"), b"secret").unwrap();
    symlink(
        outside.path().join("secret.jpg"),
        project.path().join("escaped.jpg"),
    )
    .unwrap();
    fs::write(project.path().join("safe.jpg"), b"safe").unwrap();

    let (sink, mut events) = tokio::sync::mpsc::channel(8);
    ProjectWalker
        .scan(
            ScanRequest {
                session_id: SessionId::new(),
                generation: Generation::new(1),
                root: project.path().to_path_buf(),
            },
            sink,
        )
        .await
        .unwrap();

    let mut paths = Vec::new();
    while let Some(event) = events.recv().await {
        if let ScanEvent::Files { nodes, .. } = event {
            paths.extend(
                nodes
                    .into_iter()
                    .map(|node| node.relative_path.as_str().to_owned()),
            );
        }
    }
    assert_eq!(paths, ["safe.jpg"]);
}

#[test]
fn image_protocol_rejects_unknown_cross_session_non_image_and_traversal_inputs() {
    let registry = Arc::new(ImageArtifactRegistry::default());
    let session_id = SessionId::new();
    let resolver = ImageProtocolResolver::new(session_id, Arc::clone(&registry));
    let directory = tempfile::tempdir().unwrap();
    let artifact = directory.path().join("artifact");
    fs::write(&artifact, b"not an image").unwrap();
    let token = registry
        .insert(session_id, EntityId::new(), artifact, "text/plain")
        .unwrap();

    assert_eq!(
        resolver.resolve("/../secret"),
        Err(ProtocolError::BadRequest)
    );
    assert_eq!(
        resolver.resolve(&format!("/{session_id}/unknown")),
        Err(ProtocolError::NotFound)
    );
    assert_eq!(
        resolver.resolve(&format!("/{}/{}", SessionId::new(), token.as_str())),
        Err(ProtocolError::Forbidden)
    );
    assert_eq!(
        resolver.resolve(&format!("/{session_id}/{}", token.as_str())),
        Err(ProtocolError::UnsupportedMediaType)
    );
}

#[test]
fn markdown_sanitizer_removes_active_and_remote_content() {
    let unsafe_html = r#"
        <h1 onclick="steal()">Title</h1>
        <script>alert(1)</script>
        <iframe src="https://attacker.example/frame"></iframe>
        <link rel="stylesheet" href="https://attacker.example/style.css">
        <img src="https://attacker.example/pixel.png" onerror="steal()">
        <p>Safe text</p>
    "#;

    let sanitized = sanitize_markdown_html(unsafe_html);
    assert!(sanitized.contains("<h1>Title</h1>"));
    assert!(sanitized.contains("<p>Safe text</p>"));
    for rejected in [
        "<script",
        "<iframe",
        "<link",
        "onclick",
        "onerror",
        "attacker.example",
    ] {
        assert!(
            !sanitized.contains(rejected),
            "retained {rejected}: {sanitized}"
        );
    }
}

#[test]
fn capability_and_csp_are_exact_local_allowlists() {
    let capability: serde_json::Value =
        serde_json::from_str(include_str!("../src-tauri/capabilities/main.json")).unwrap();
    let permissions = capability["permissions"].as_array().unwrap();
    let allowed = ["core:default", "dialog:allow-open", "opener:allow-open-url"];
    for permission in permissions {
        let identifier = permission
            .as_str()
            .or_else(|| permission["identifier"].as_str())
            .expect("permission identifier");
        assert!(
            allowed.contains(&identifier),
            "unexpected permission {identifier}"
        );
        for forbidden in [
            "fs:",
            "shell:",
            "sql:",
            "http:",
            "updater:",
            "websocket:",
            "upload:",
        ] {
            assert!(!identifier.starts_with(forbidden));
        }
    }
    assert!(capability.get("remote").is_none());

    let configuration: serde_json::Value =
        serde_json::from_str(include_str!("../src-tauri/tauri.conf.json")).unwrap();
    let csp = configuration["app"]["security"]["csp"].as_str().unwrap();
    assert!(!csp.contains('*'));
    assert!(!csp.contains("'unsafe-eval'"));
    assert!(!csp.contains("https:"));
    let connect = directive(csp, "connect-src");
    assert_eq!(connect, ["'self'", "ipc:", "http://ipc.localhost"]);
    for directive_name in ["default-src", "script-src", "connect-src", "style-src"] {
        assert!(!directive(csp, directive_name).contains(&"viewer-image:"));
    }
    assert!(directive(csp, "img-src").contains(&"viewer-image:"));
}

#[test]
fn navigation_policy_rejects_remote_and_credentialed_origins() {
    assert!(is_allowed_navigation(
        &Url::parse("tauri://localhost/").unwrap()
    ));
    for rejected in [
        "https://example.com/",
        "http://user@localhost:5173/",
        "file:///tmp/project.html",
        "data:text/html,viewer",
    ] {
        assert!(!is_allowed_navigation(&Url::parse(rejected).unwrap()));
    }
}

fn directive<'a>(csp: &'a str, name: &str) -> Vec<&'a str> {
    csp.split(';')
        .map(str::trim)
        .find_map(|value| {
            let mut parts = value.split_whitespace();
            (parts.next() == Some(name)).then(|| parts.collect())
        })
        .unwrap_or_default()
}
