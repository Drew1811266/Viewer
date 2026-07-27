pub(crate) mod browse;
pub(crate) mod finder_drag;
pub(crate) mod markers;
pub(crate) mod operations;
pub(crate) mod preview;
pub(crate) mod project;
pub(crate) mod search;
pub(crate) mod settings;

use crate::{APP_NAME, HealthResponse};

#[tauri::command]
pub fn health() -> HealthResponse {
    HealthResponse {
        app_name: APP_NAME,
        version: env!("CARGO_PKG_VERSION"),
    }
}
