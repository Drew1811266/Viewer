pub(crate) mod project;

use crate::{APP_NAME, HealthResponse};

#[tauri::command]
pub fn health() -> HealthResponse {
    HealthResponse {
        app_name: APP_NAME,
        version: env!("CARGO_PKG_VERSION"),
    }
}
