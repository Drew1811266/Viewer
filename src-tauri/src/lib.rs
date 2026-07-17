use serde::Serialize;
use std::sync::Arc;
use viewer_domain::SessionId;
use viewer_infrastructure::image_cache::ImageArtifactRegistry;

pub mod dto;
pub mod error;
pub mod image_protocol;
pub mod state;

pub const APP_NAME: &str = "Viewer";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    app_name: &'static str,
    version: &'static str,
}

mod commands {
    use super::{APP_NAME, HealthResponse};

    #[tauri::command]
    pub fn health() -> HealthResponse {
        HealthResponse {
            app_name: APP_NAME,
            version: env!("CARGO_PKG_VERSION"),
        }
    }
}

pub use commands::health;

pub fn sanitize_markdown_html(input: &str) -> String {
    let mut builder = ammonia::Builder::default();
    builder
        .url_relative(ammonia::UrlRelative::Deny)
        .add_url_schemes(&["viewer-image"])
        .attribute_filter(|element, attribute, value| match (element, attribute) {
            ("img", "src") if value.starts_with("viewer-image:") => Some(value.into()),
            ("img", "src") => None,
            _ => Some(value.into()),
        });
    builder.clean(input).to_string()
}

pub fn is_allowed_navigation(url: &tauri::Url) -> bool {
    if !url.username().is_empty() || url.password().is_some() {
        return false;
    }

    match (url.scheme(), url.host_str(), url.port()) {
        ("tauri", Some("localhost"), None) => true,
        ("http", Some("localhost"), Some(5173)) if cfg!(debug_assertions) => true,
        _ => false,
    }
}

fn navigation_guard<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("viewer-navigation-guard")
        .on_navigation(|_, url| is_allowed_navigation(url))
        .build()
}

pub fn run() {
    let registry = Arc::new(ImageArtifactRegistry::default());
    let image_resolver = Arc::new(image_protocol::ImageProtocolResolver::new(
        SessionId::new(),
        registry,
    ));
    tauri::Builder::default()
        .plugin(navigation_guard())
        .manage(Arc::clone(&image_resolver))
        .register_asynchronous_uri_scheme_protocol(
            "viewer-image",
            move |_context, request, responder| {
                image_protocol::handle_request(Arc::clone(&image_resolver), request, responder);
            },
        )
        .invoke_handler(tauri::generate_handler![health])
        .run(tauri::generate_context!())
        .expect("failed to run Viewer");
}

#[cfg(test)]
mod tests {
    #[test]
    fn app_name_is_viewer() {
        assert_eq!(super::APP_NAME, "Viewer");
    }

    #[test]
    fn health_returns_app_identity() {
        let response = super::health();
        assert_eq!(response.app_name, "Viewer");
        assert_eq!(response.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn health_serializes_the_exact_ipc_shape() {
        let value = serde_json::to_value(super::health()).expect("serialize health response");
        assert_eq!(
            value,
            serde_json::json!({
                "appName": "Viewer",
                "version": env!("CARGO_PKG_VERSION"),
            })
        );
    }

    #[test]
    fn main_capability_grants_no_core_or_plugin_permissions() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/main.json"))
                .expect("main capability must be valid JSON");

        assert_eq!(capability["windows"], serde_json::json!(["main"]));
        assert_eq!(capability["permissions"], serde_json::json!([]));
        assert!(capability.get("remote").is_none());
    }

    #[test]
    fn navigation_policy_allows_only_viewer_origins() {
        for allowed in [
            "tauri://localhost/",
            "tauri://localhost/folder?id=1#preview",
        ] {
            let url = tauri::Url::parse(allowed).expect("valid test URL");
            assert!(super::is_allowed_navigation(&url), "rejected {allowed}");
        }

        let development =
            tauri::Url::parse("http://localhost:5173/").expect("valid development URL");
        assert_eq!(
            super::is_allowed_navigation(&development),
            cfg!(debug_assertions)
        );
    }

    #[test]
    fn navigation_policy_rejects_remote_and_origin_bypasses() {
        for rejected in [
            "https://example.com/",
            "http://localhost:5174/",
            "http://127.0.0.1:5173/",
            "http://user@localhost:5173/",
            "file:///tmp/project.html",
            "data:text/html,viewer",
        ] {
            let url = tauri::Url::parse(rejected).expect("valid test URL");
            assert!(!super::is_allowed_navigation(&url), "accepted {rejected}");
        }
    }

    #[test]
    fn csp_allows_only_the_explicit_viewer_image_origins() {
        let configuration: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json"))
                .expect("tauri configuration must be valid JSON");
        let csp = configuration["app"]["security"]["csp"]
            .as_str()
            .expect("CSP must be a string");
        let image_directive = csp
            .split(';')
            .map(str::trim)
            .find(|directive| directive.starts_with("img-src "))
            .expect("CSP must define img-src");
        assert_eq!(
            image_directive,
            "img-src 'self' asset: data: viewer-image: http://viewer-image.localhost"
        );
        assert!(!csp.contains('*'));
    }
}
