pub const APP_NAME: &str = "Viewer";

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run Viewer");
}

#[cfg(test)]
mod tests {
    #[test]
    fn app_name_is_viewer() {
        assert_eq!(super::APP_NAME, "Viewer");
    }
}
