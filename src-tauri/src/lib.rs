use serde::Serialize;

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

fn is_allowed_navigation(url: &tauri::Url) -> bool {
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
    tauri::Builder::default()
        .plugin(navigation_guard())
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
    fn main_capability_grants_no_core_or_plugin_permissions() {
        let capability: serde_json::Value = serde_json::from_str(include_str!(
            "../capabilities/main.json"
        ))
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
}
