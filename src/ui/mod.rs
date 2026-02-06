#[cfg(feature = "tauri-ui")]
mod tauri_app;

#[cfg(feature = "tauri-ui")]
pub use tauri_app::run_gui;

#[cfg(not(feature = "tauri-ui"))]
pub fn run_gui(
    _config: crate::types::Config,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Err("GUI is disabled. Rebuild with `--features tauri-ui`.".into())
}
