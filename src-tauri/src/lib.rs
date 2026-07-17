use serde::Serialize;
use std::sync::{Arc, OnceLock};
use tauri::{Emitter, Manager};
use viewer_infrastructure::image_cache::ImageArtifactRegistry;

pub mod commands;
pub mod dto;
pub mod error;
pub mod image_protocol;
pub mod markdown;
pub mod operation_runtime;
pub mod state;
pub mod watcher_runtime;

pub const APP_NAME: &str = "Viewer";
const PROJECT_CLOSED_EVENT: &str = "viewer://project-closed";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    app_name: &'static str,
    version: &'static str,
}

pub use commands::health;

#[derive(Default)]
struct TauriEventSink(OnceLock<tauri::AppHandle>);

impl TauriEventSink {
    fn attach(&self, app: tauri::AppHandle) {
        let _ = self.0.set(app);
    }
}

impl state::DesktopEventSink for TauriEventSink {
    fn emit_scan(&self, event: dto::ScanEventDto) {
        if let Some(app) = self.0.get() {
            let _ = app.emit("viewer://scan-progress", event);
        }
    }

    fn emit_index(&self, event: dto::IndexProgressDto) {
        if let Some(app) = self.0.get() {
            let _ = app.emit("viewer://index-progress", event);
        }
    }

    fn emit_operation(
        &self,
        session_id: viewer_domain::SessionId,
        generation: viewer_domain::search::Generation,
        event: viewer_application::file_commands::BatchProgress,
    ) {
        if let Some(app) = self.0.get() {
            let _ = app.emit(
                "viewer://operation-progress",
                dto::OperationProgressDto::from_progress(session_id, generation.get(), event),
            );
        }
    }

    fn emit_project_changed(
        &self,
        session_id: viewer_domain::SessionId,
        generation: viewer_domain::search::Generation,
        summary: viewer_application::watcher::ReconcileSummary,
    ) {
        if let Some(app) = self.0.get() {
            let _ = app.emit(
                "viewer://project-changed",
                dto::ProjectChangedDto::from_summary(session_id, generation.get(), summary),
            );
        }
    }

    fn emit_close_blocked(
        &self,
        session_id: viewer_domain::SessionId,
        generation: viewer_domain::search::Generation,
        batch_id: viewer_domain::operation::BatchId,
    ) {
        if let Some(app) = self.0.get() {
            let _ = app.emit(
                "viewer://close-blocked",
                dto::CloseBlockedDto {
                    session_id: session_id.to_string(),
                    generation: generation.get(),
                    batch_id: batch_id.to_string(),
                },
            );
        }
    }
}

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
    let active_image_session = image_protocol::ActiveImageSession::default();
    let image_resolver = Arc::new(image_protocol::ImageProtocolResolver::new(
        active_image_session.clone(),
        Arc::clone(&registry),
    ));
    let runtime_registry = Arc::clone(&registry);
    let runtime_active_image_session = active_image_session.clone();
    let app = tauri::Builder::default()
        .plugin(navigation_guard())
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::clone(&image_resolver))
        .register_asynchronous_uri_scheme_protocol(
            "viewer-image",
            move |_context, request, responder| {
                image_protocol::handle_request(Arc::clone(&image_resolver), request, responder);
            },
        )
        .invoke_handler(tauri::generate_handler![
            health,
            commands::project::open_project,
            commands::project::close_project,
            commands::project::project_snapshot,
            commands::project::cancel_task,
            commands::browse::folder_tree,
            commands::browse::query_folder,
            commands::browse::request_image_representation,
            commands::preview::preview_text,
            commands::preview::open_external_link,
            commands::search::search_project,
            commands::search::search_text_snippet,
            commands::markers::set_review_state,
            commands::markers::toggle_favorite,
            commands::markers::selection_info,
            commands::operations::preview_rename,
            commands::operations::execute_file_command,
            commands::operations::operation_status,
            commands::operations::operation_results,
            commands::operations::cancel_operation,
            commands::operations::undo_last_operation,
            commands::operations::open_permission_settings
        ])
        .setup(move |app| {
            let cache_base = app.path().app_cache_dir()?.join("sessions");
            let _ = viewer_infrastructure::session_cache::SessionCache::cleanup_stale(
                &cache_base,
                None,
            );
            let event_sink = Arc::new(TauriEventSink::default());
            event_sink.attach(app.handle().clone());
            let event_port: Arc<dyn state::DesktopEventSink> = event_sink;
            let runtime = Arc::new(state::DesktopRuntime::new_with_image_services(
                cache_base,
                Arc::new(viewer_platform_macos::MacProjectProbe),
                Arc::new(viewer_infrastructure::scan::walker::ProjectWalker),
                event_port,
                Arc::new(state::MacDesktopImageFactory),
                Arc::clone(&runtime_registry),
                runtime_active_image_session.clone(),
            ));
            app.manage(runtime);

            if let Some(window) = app.get_webview_window("main") {
                let app_handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let app_handle = app_handle.clone();
                        tauri::async_runtime::spawn(async move {
                            let runtime = app_handle.state::<Arc<state::DesktopRuntime>>();
                            if runtime.request_close(None).await
                                == Ok(state::CloseRequestOutcome::Closed)
                            {
                                let _ = app_handle.emit(PROJECT_CLOSED_EVENT, ());
                                if let Some(window) = app_handle.get_webview_window("main") {
                                    let _ = window.hide();
                                }
                            }
                        });
                    }
                });
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build Viewer");

    app.run(|app, event| match event {
        tauri::RunEvent::ExitRequested { .. } => {
            let runtime = app.state::<Arc<state::DesktopRuntime>>();
            let _ = tauri::async_runtime::block_on(runtime.close_project());
        }
        tauri::RunEvent::Reopen { .. } => {
            let _ = app.emit(PROJECT_CLOSED_EVENT, ());
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        _ => {}
    });
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
    fn main_capability_grants_only_dialog_open_and_event_subscription() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/main.json"))
                .expect("main capability must be valid JSON");

        assert_eq!(capability["windows"], serde_json::json!(["main"]));
        assert_eq!(
            capability["permissions"],
            serde_json::json!([
                "dialog:allow-open",
                "core:event:allow-listen",
                "core:event:allow-unlisten"
            ])
        );
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
