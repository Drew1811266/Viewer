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

pub fn run() {
    tauri::Builder::default()
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
}
