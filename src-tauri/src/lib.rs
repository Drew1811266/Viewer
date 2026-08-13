use serde::Serialize;
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use tauri::{Emitter, Manager};
use viewer_application::ViewerSettingsService;
use viewer_infrastructure::{
    image_cache::ImageArtifactRegistry, settings::JsonViewerSettingsStore,
};

pub mod commands;
pub mod dto;
pub mod error;
pub mod image_protocol;
pub mod markdown;
pub mod operation_runtime;
pub mod state;
pub mod video_events;
#[cfg(all(target_os = "macos", feature = "video-feasibility"))]
pub mod video_feasibility;
pub mod video_runtime;
pub mod watcher_runtime;

pub const APP_NAME: &str = "Viewer";
const EMBEDDED_VIDEO_RUNTIME_SCHEMA_VERSION: &str = env!("VIEWER_VIDEO_RUNTIME_SCHEMA_VERSION");
const PROJECT_CLOSED_EVENT: &str = "viewer://project-closed";
const TERMINAL_CLOSE_CACHE_CLEANUP_FAILURE: &str = "project_closed_cache_cleanup_failed";
const TERMINAL_VIDEO_CLOSE_FAILURE: &str = "video_close_failed";

fn validate_embedded_video_runtime_schema() -> Result<(), String> {
    let embedded = EMBEDDED_VIDEO_RUNTIME_SCHEMA_VERSION
        .parse::<u32>()
        .map_err(|_| "embedded video runtime schema is invalid".to_string())?;
    if embedded != viewer_video_mpv::runtime_manifest::RUNTIME_MANIFEST_SCHEMA_VERSION {
        return Err("embedded video runtime schema does not match the loader schema".to_string());
    }
    Ok(())
}

pub(crate) fn is_terminal_close_cleanup_failure(error: &error::CommandError) -> bool {
    matches!(
        error.code.as_str(),
        TERMINAL_CLOSE_CACHE_CLEANUP_FAILURE | TERMINAL_VIDEO_CLOSE_FAILURE
    )
}

fn close_completion_after_attempt(
    attempt: &Result<state::CloseRequestOutcome, error::CommandError>,
    target: state::CloseTarget,
) -> Option<state::CloseCompletionAction> {
    let outcome = match attempt {
        Ok(outcome) => *outcome,
        Err(error) if is_terminal_close_cleanup_failure(error) => {
            state::CloseRequestOutcome::Closed
        }
        Err(_) => return None,
    };
    match outcome.completion_action(target) {
        state::CloseCompletionAction::KeepOpen => None,
        action => Some(action),
    }
}

#[derive(Default)]
pub struct ExitGate(AtomicBool);

impl ExitGate {
    pub(crate) fn allow_exit(&self) {
        self.0.store(true, Ordering::Release);
    }

    fn exit_is_allowed(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

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
        target: state::CloseTarget,
    ) {
        if let Some(app) = self.0.get() {
            let _ = app.emit(
                "viewer://close-blocked",
                dto::CloseBlockedDto {
                    session_id: session_id.to_string(),
                    generation: generation.get(),
                    batch_id: batch_id.to_string(),
                    target: match target {
                        state::CloseTarget::Project => dto::CloseTargetDto::Project,
                        state::CloseTarget::Window => dto::CloseTargetDto::Window,
                        state::CloseTarget::Application => dto::CloseTargetDto::Application,
                    },
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
            commands::project::reveal_project_in_file_manager,
            commands::project::cancel_task,
            commands::browse::folder_tree,
            commands::browse::query_folder,
            commands::browse::request_image_representation,
            commands::browse::cancel_image_request,
            commands::preview::preview_text,
            commands::preview::open_external_link,
            commands::search::search_project,
            commands::search::search_text_snippet,
            commands::markers::set_review_state,
            commands::markers::toggle_favorite,
            commands::markers::selection_info,
            commands::operations::preview_rename,
            commands::operations::preflight_file_command,
            commands::operations::execute_file_command,
            commands::operations::operation_status,
            commands::operations::operation_results,
            commands::operations::cancel_operation,
            commands::operations::undo_last_operation,
            commands::operations::open_permission_settings,
            commands::finder_drag::begin_finder_drag,
            commands::settings::get_viewer_settings,
            commands::settings::update_viewer_settings,
            commands::video::video_open,
            commands::video::video_cancel_open,
            commands::video::video_close,
            commands::video::video_play,
            commands::video::video_pause,
            commands::video::video_seek,
            commands::video::video_step,
            commands::video::video_set_volume,
            commands::video::video_set_muted,
            commands::video::video_set_rate,
            commands::video::video_set_surface_rect,
            commands::video::video_set_fullscreen,
            commands::video::video_request_thumbnail,
            commands::video::video_cache_stats,
            commands::video::video_cache_clear,
            #[cfg(all(target_os = "macos", feature = "video-feasibility"))]
            video_feasibility::run_video_feasibility
        ])
        .setup(move |app| {
            validate_embedded_video_runtime_schema()?;
            let settings_directory = app.path().app_config_dir()?;
            let settings_service = Arc::new(ViewerSettingsService::new(Arc::new(
                JsonViewerSettingsStore::new(settings_directory),
            )));
            let cache_base = app.path().app_cache_dir()?.join("sessions");
            let _ = viewer_infrastructure::session_cache::SessionCache::cleanup_stale(
                &cache_base,
                None,
            );
            let event_sink = Arc::new(TauriEventSink::default());
            event_sink.attach(app.handle().clone());
            let event_port: Arc<dyn state::DesktopEventSink> = event_sink;
            let video_probe: Arc<dyn viewer_infrastructure::video_probe::VideoMetadataProbe> = app
                .path()
                .resource_dir()
                .ok()
                .and_then(|resources| {
                    viewer_infrastructure::video_probe::VideoProbe::from_bundle_root(&resources)
                        .ok()
                })
                .map(|probe| {
                    Arc::new(probe)
                        as Arc<dyn viewer_infrastructure::video_probe::VideoMetadataProbe>
                })
                .unwrap_or_else(|| {
                    Arc::new(viewer_infrastructure::video_probe::UnavailableVideoProbe)
                });
            let runtime = Arc::new(state::DesktopRuntime::new_with_media_services(
                cache_base,
                Arc::new(viewer_platform_macos::MacProjectProbe),
                Arc::new(viewer_infrastructure::scan::walker::ProjectWalker),
                event_port,
                Arc::new(state::MacDesktopImageFactory),
                Arc::clone(&runtime_registry),
                runtime_active_image_session.clone(),
                video_probe,
            ));
            let video_events: Arc<dyn video_runtime::VideoEventPort> = Arc::new(
                video_events::TauriVideoEventEmitter::new(app.handle().clone()),
            );
            let video_engine = Arc::new(viewer_platform_macos::video::MacOsLibmpvAdapter::new(
                app.path().resource_dir().unwrap_or_default(),
            ));
            let app_cache_root = app.path().app_cache_dir()?;
            let video_cache =
                viewer_infrastructure::video_cache::VideoCache::initialize(&app_cache_root)
                    .map_err(|error| format!("failed to initialize video cache: {error}"))?;
            let runtime_layout =
                viewer_video_mpv::runtime_manifest::RuntimeLayout::from_bundle_root(
                    &app.path().resource_dir()?,
                )
                .map_err(|error| format!("failed to validate video runtime: {error}"))?;
            let media_tools = viewer_video_mpv::BundledMediaTools::from_layout(&runtime_layout)
                .map_err(|error| format!("failed to initialize media tools: {error}"))?;
            let thumbnail_bridge = Arc::new(video_runtime::NativeTimelineThumbnailBridge::new(
                media_tools,
                video_cache.clone(),
                Arc::clone(&runtime_registry),
            ));
            let timeline_thumbnails: Arc<dyn video_runtime::TimelineThumbnailPort> =
                thumbnail_bridge.clone();
            let playback_activity: Arc<dyn video_runtime::PlaybackActivityPort> = thumbnail_bridge;
            let video_runtime = Arc::new(video_runtime::VideoRuntime::with_native_bridges(
                Arc::clone(&video_engine),
                video_cache,
                video_events,
                timeline_thumbnails,
                playback_activity,
            ));
            let weak_video_runtime = Arc::downgrade(&video_runtime);
            let (engine_event_tx, mut engine_event_rx) = tokio::sync::mpsc::unbounded_channel();
            video_engine.set_event_sink(Arc::new(move |generation, event| {
                let _ = engine_event_tx.send((generation, event));
            }));
            tauri::async_runtime::spawn(async move {
                while let Some((generation, event)) = engine_event_rx.recv().await {
                    let Some(video_runtime) = weak_video_runtime.upgrade() else {
                        break;
                    };
                    video_runtime.handle_engine_event(generation, event).await;
                }
            });
            let video_lifecycle: Arc<dyn state::VideoClosePort> = video_runtime.clone();
            runtime.register_video_lifecycle(video_lifecycle);
            app.manage(runtime);
            app.manage(video_runtime);
            app.manage(settings_service);
            app.manage(ExitGate::default());

            if let Some(window) = app.get_webview_window("main") {
                let app_handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let app_handle = app_handle.clone();
                        tauri::async_runtime::spawn(async move {
                            let runtime = app_handle.state::<Arc<state::DesktopRuntime>>();
                            let outcome = runtime
                                .request_close_for(None, state::CloseTarget::Window)
                                .await;
                            if let Err(error) = &outcome
                                && is_terminal_close_cleanup_failure(error)
                            {
                                eprintln!(
                                    "Viewer window-close cache cleanup failed: {}",
                                    error.code
                                );
                                if let Err(retry_error) =
                                    runtime.cleanup_session_caches_for_process_exit()
                                {
                                    eprintln!(
                                        "Viewer window-close cache cleanup retry failed: {}",
                                        retry_error.code
                                    );
                                }
                            }
                            if close_completion_after_attempt(&outcome, state::CloseTarget::Window)
                                == Some(state::CloseCompletionAction::HideWindow)
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
        tauri::RunEvent::ExitRequested { api, .. } => {
            let exit_gate = app.state::<ExitGate>();
            if exit_gate.exit_is_allowed() {
                return;
            }
            api.prevent_exit();
            let runtime = app.state::<Arc<state::DesktopRuntime>>();
            let outcome = tauri::async_runtime::block_on(
                runtime.request_close_for(None, state::CloseTarget::Application),
            );
            if let Err(error) = &outcome
                && is_terminal_close_cleanup_failure(error)
            {
                let _ = app.emit(PROJECT_CLOSED_EVENT, ());
                if let Err(retry_error) = runtime.cleanup_session_caches_for_process_exit() {
                    eprintln!(
                        "Viewer application-close cache cleanup retry failed: {}",
                        retry_error.code
                    );
                }
            }
            if close_completion_after_attempt(&outcome, state::CloseTarget::Application)
                == Some(state::CloseCompletionAction::ExitApplication)
            {
                exit_gate.allow_exit();
                app.exit(0);
            }
        }
        tauri::RunEvent::Reopen { .. } => {
            let _ = app.emit(PROJECT_CLOSED_EVENT, ());
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        tauri::RunEvent::Exit => {
            // On macOS a native Quit can reach the final Exit event without a
            // preventable ExitRequested event. Finish session teardown here
            // and synchronously sweep Viewer-owned session directories before
            // the operating system terminates the process.
            let runtime = app.state::<Arc<state::DesktopRuntime>>();
            if let Err(error) = tauri::async_runtime::block_on(runtime.finalize_process_exit()) {
                eprintln!("Viewer process-exit cleanup failed: {}", error.code);
            }
        }
        _ => {}
    });
}

#[cfg(test)]
mod tests {
    use crate::{
        error::{CommandError, ErrorCategory},
        state::{CloseCompletionAction, CloseRequestOutcome, CloseTarget},
    };

    #[test]
    fn exit_gate_changes_only_after_a_committed_application_close() {
        let gate = super::ExitGate::default();
        assert!(!gate.exit_is_allowed());
        gate.allow_exit();
        assert!(gate.exit_is_allowed());
    }

    #[test]
    fn native_close_treats_terminal_cache_cleanup_failure_as_committed() {
        let error = CommandError::new(
            "project_closed_cache_cleanup_failed",
            ErrorCategory::Environment,
            "cache cleanup failed",
            true,
        );

        assert_eq!(
            super::close_completion_after_attempt(&Err(error.clone()), CloseTarget::Window),
            Some(CloseCompletionAction::HideWindow)
        );
        assert_eq!(
            super::close_completion_after_attempt(&Err(error), CloseTarget::Application),
            Some(CloseCompletionAction::ExitApplication)
        );

        let video_error = CommandError::new(
            "video_close_failed",
            ErrorCategory::Environment,
            "video close failed",
            true,
        );
        assert_eq!(
            super::close_completion_after_attempt(&Err(video_error), CloseTarget::Window),
            Some(CloseCompletionAction::HideWindow)
        );
    }

    #[test]
    fn native_close_does_not_commit_stayed_or_unrelated_error_attempts() {
        assert_eq!(
            super::close_completion_after_attempt(
                &Ok(CloseRequestOutcome::Stayed),
                CloseTarget::Window
            ),
            None
        );
        let error = CommandError::new(
            "internal_error",
            ErrorCategory::Internal,
            "unexpected",
            false,
        );
        assert_eq!(
            super::close_completion_after_attempt(&Err(error), CloseTarget::Application),
            None
        );
    }

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
