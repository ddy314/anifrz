mod electron_ipc;

pub use electron_ipc::run_ipc;

#[cfg(feature = "tauri-ui")]
mod tauri_app;

#[cfg(feature = "tauri-ui")]
pub use tauri_app::run_gui;

#[cfg(not(feature = "tauri-ui"))]
pub fn run_gui(
    _config: crate::types::Config,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Err("GUI is disabled in this build. Use Electron frontend under src/ui/frontend/.".into())
}
