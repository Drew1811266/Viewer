use crate::{
    commands::{browse::parse_entity_id, search::parse_session_id},
    dto::{BeginFinderDragRequestDto, FinderDragReceiptDto},
    error::{CommandError, ErrorCategory},
    state::DesktopRuntime,
};
use std::sync::Arc;
use tauri::{State, WebviewWindow};
use viewer_domain::search::Generation;

#[tauri::command]
pub async fn begin_finder_drag(
    runtime: State<'_, Arc<DesktopRuntime>>,
    window: WebviewWindow,
    request: BeginFinderDragRequestDto,
) -> Result<FinderDragReceiptDto, CommandError> {
    let session_id = parse_session_id(&request.session_id)?;
    let generation = Generation::new(request.generation);
    let entity_ids = request
        .entity_ids
        .iter()
        .map(|value| parse_entity_id(value))
        .collect::<Result<Vec<_>, _>>()?;
    let prepared = runtime
        .prepare_finder_drag(session_id, generation, &entity_ids)
        .await?;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let runtime = Arc::clone(runtime.inner());
    window
        .with_webview(move |webview| {
            let result = runtime
                .run_if_project_current(session_id, generation, || {
                    // SAFETY: Tauri documents `inner()` as WKWebView on macOS. WKWebView
                    // inherits NSView, and this closure runs on the AppKit main thread.
                    unsafe {
                        let view: &objc2_app_kit::NSView = &*webview.inner().cast();
                        viewer_platform_macos::files::MacFinderDragPort::new(view).and_then(
                            |port| viewer_application::begin_finder_drag(&port, &prepared),
                        )
                    }
                })
                .and_then(|result| result.map_err(CommandError::from));
            let _ = sender.send(result);
        })
        .map_err(|_| native_drag_unavailable())?;
    let receipt = receiver.await.map_err(|_| native_drag_unavailable())??;
    Ok(FinderDragReceiptDto {
        file_count: receipt.file_count,
    })
}

fn native_drag_unavailable() -> CommandError {
    CommandError::new(
        "finder_drag_unavailable",
        ErrorCategory::Environment,
        "当前无法启动 Finder 拖动，请重试。",
        true,
    )
}
