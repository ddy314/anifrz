use crate::backend::{BackendHandle, Command, DataEvent, StatusEvent, start_backend};
use crate::storage::LibraryDb;
use crate::types::{Config, SeriesRecord, get_string_config};
use std::path::PathBuf;
use std::sync::Mutex;

struct AppState {
    config: Config,
    backend: Mutex<Option<BackendHandle>>,
}

#[derive(serde::Serialize)]
struct PollResponse {
    running: bool,
    statuses: Vec<StatusEvent>,
    data_events: Vec<DataEvent>,
}

#[tauri::command]
fn start_scrape(root: String, state: tauri::State<AppState>) -> Result<(), String> {
    let mut guard = state
        .backend
        .lock()
        .map_err(|_| "backend mutex poisoned".to_string())?;
    if guard.is_none() {
        *guard = Some(start_backend(state.config.clone()));
    }
    let handle = guard
        .as_ref()
        .ok_or_else(|| "backend init failed".to_string())?;
    handle
        .send(Command::Scrape {
            root: PathBuf::from(root),
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn poll_events(state: tauri::State<AppState>) -> Result<PollResponse, String> {
    let mut statuses = Vec::new();
    let mut data_events = Vec::new();

    let mut guard = state
        .backend
        .lock()
        .map_err(|_| "backend mutex poisoned".to_string())?;
    let running = guard.is_some();
    if let Some(handle) = guard.as_mut() {
        while let Ok(evt) = handle.status_rx.try_recv() {
            statuses.push(evt);
        }
        while let Ok(evt) = handle.data_rx.try_recv() {
            data_events.push(evt);
        }
    }

    Ok(PollResponse {
        running,
        statuses,
        data_events,
    })
}

#[tauri::command]
fn list_series(
    limit: Option<usize>,
    state: tauri::State<AppState>,
) -> Result<Vec<SeriesRecord>, String> {
    let library_dir = get_string_config("LIBRARY_DIR", &state.config.library.dir, "library");
    let db = LibraryDb::open(&PathBuf::from(library_dir)).map_err(|e| e.to_string())?;
    db.list_series(limit.unwrap_or(100))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn stop_backend(state: tauri::State<AppState>) -> Result<(), String> {
    let mut guard = state
        .backend
        .lock()
        .map_err(|_| "backend mutex poisoned".to_string())?;
    if let Some(handle) = guard.take() {
        handle.stop();
    }
    Ok(())
}

pub fn run_gui(config: Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tauri::Builder::default()
        .manage(AppState {
            config,
            backend: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            start_scrape,
            poll_events,
            list_series,
            stop_backend
        ])
        .run(tauri::generate_context!("tauri.conf.json"))
        .map_err(|e| format!("failed to run tauri app: {e}"))?;
    Ok(())
}
