use crate::backend::{
    BackendHandle, Command as BackendCommand, DataEvent, StatusEvent, start_backend,
};
use crate::storage::LibraryDb;
use crate::types::{Config, SeriesRecord, get_string_config, now_ts};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::Mutex;

const MAX_LOGS: usize = 1200;

struct AppState {
    config: Config,
    backend: Mutex<Option<BackendHandle>>,
    logs: Mutex<LogState>,
}

struct LogState {
    next_id: u64,
    scraping: bool,
    items: VecDeque<AppLog>,
}

impl LogState {
    fn push(&mut self, level: &str, message: impl Into<String>) {
        self.next_id = self.next_id.saturating_add(1);
        self.items.push_back(AppLog {
            id: self.next_id,
            ts: now_ts(),
            level: level.to_string(),
            message: message.into(),
        });
        while self.items.len() > MAX_LOGS {
            self.items.pop_front();
        }
    }
}

#[derive(Clone, serde::Serialize)]
struct AppLog {
    id: u64,
    ts: i64,
    level: String,
    message: String,
}

#[derive(serde::Serialize)]
struct SeriesWallItem {
    id: i64,
    title: String,
    subtitle: String,
    cover_local_path: Option<String>,
    episode_count: usize,
    missing_count: usize,
    updated_at: i64,
}

#[derive(serde::Serialize)]
struct SeriesDetailEpisode {
    episode: u32,
    name: String,
    name_cn: String,
    ep_type: u8,
    files: Vec<String>,
}

#[derive(serde::Serialize)]
struct SeriesDetail {
    id: i64,
    title: String,
    subtitle: String,
    summary: String,
    tags: Vec<String>,
    air_date: Option<String>,
    rating_score: Option<f64>,
    rating_total: Option<u64>,
    cover_local_path: Option<String>,
    root: String,
    episodes: Vec<SeriesDetailEpisode>,
    missing_episodes: Vec<u32>,
    unmatched_files: Vec<String>,
    updated_at: i64,
}

#[tauri::command]
fn start_scrape(root: String, state: tauri::State<AppState>) -> Result<(), String> {
    let root = root.trim();
    if root.is_empty() {
        return Err("root path is empty".to_string());
    }
    {
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
            .send(BackendCommand::Scrape {
                root: PathBuf::from(root),
            })
            .map_err(|e| e.to_string())?;
    }

    let mut logs = state
        .logs
        .lock()
        .map_err(|_| "logs mutex poisoned".to_string())?;
    logs.scraping = true;
    logs.push("info", format!("开始刮削: {root}"));
    Ok(())
}

#[tauri::command]
fn read_logs(
    after_id: Option<u64>,
    limit: Option<usize>,
    state: tauri::State<AppState>,
) -> Result<Vec<AppLog>, String> {
    drain_backend_events(&state)?;
    let max = limit.unwrap_or(300).clamp(1, 500);
    let logs = state
        .logs
        .lock()
        .map_err(|_| "logs mutex poisoned".to_string())?;
    let mut out: Vec<AppLog> = logs
        .items
        .iter()
        .filter(|item| after_id.map(|id| item.id > id).unwrap_or(true))
        .cloned()
        .collect();
    if out.len() > max {
        out = out[out.len().saturating_sub(max)..].to_vec();
    }
    Ok(out)
}

#[tauri::command]
fn list_wall(
    limit: Option<usize>,
    state: tauri::State<AppState>,
) -> Result<Vec<SeriesWallItem>, String> {
    let db = open_db(&state.config)?;
    let records = db
        .list_series(limit.unwrap_or(120))
        .map_err(|e| e.to_string())?;
    Ok(records.iter().map(to_wall_item).collect())
}

#[tauri::command]
fn get_series_detail(
    id: i64,
    state: tauri::State<AppState>,
) -> Result<Option<SeriesDetail>, String> {
    let db = open_db(&state.config)?;
    let record = db.load_series(id).map_err(|e| e.to_string())?;
    Ok(record.map(to_series_detail))
}

#[tauri::command]
fn play_episode(file_path: String, state: tauri::State<AppState>) -> Result<(), String> {
    let path = file_path.trim();
    if path.is_empty() {
        return Err("file path is empty".to_string());
    }
    let target = PathBuf::from(path);
    if !target.exists() {
        return Err(format!("file not found: {}", target.display()));
    }

    open_with_system_player(&target).map_err(|e| e.to_string())?;
    let mut logs = state
        .logs
        .lock()
        .map_err(|_| "logs mutex poisoned".to_string())?;
    logs.push("info", format!("唤起播放器: {}", target.display()));
    Ok(())
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
    let mut logs = state
        .logs
        .lock()
        .map_err(|_| "logs mutex poisoned".to_string())?;
    logs.scraping = false;
    logs.push("info", "后台任务已停止");
    Ok(())
}

fn drain_backend_events(state: &AppState) -> Result<(), String> {
    let mut backend = state
        .backend
        .lock()
        .map_err(|_| "backend mutex poisoned".to_string())?;
    let mut logs = state
        .logs
        .lock()
        .map_err(|_| "logs mutex poisoned".to_string())?;

    let Some(handle) = backend.as_mut() else {
        return Ok(());
    };

    while let Ok(evt) = handle.status_rx.try_recv() {
        match evt {
            StatusEvent::Started { root } => logs.push("info", format!("开始扫描目录: {root}")),
            StatusEvent::Scanned { total_files } => {
                logs.push("info", format!("扫描完成，媒体文件 {total_files} 个"))
            }
            StatusEvent::LlmParsing { total_files } => {
                logs.push("info", format!("LLM 解析文件名: {total_files} 个"))
            }
            StatusEvent::Matching { current, total } => {
                logs.push("debug", format!("匹配进度: {current}/{total}"))
            }
            StatusEvent::Persisting { current, total } => {
                logs.push("debug", format!("写入作品缓存: {current}/{total}"))
            }
            StatusEvent::Finished { summary } => {
                logs.scraping = false;
                logs.push(
                    "info",
                    format!(
                        "刮削完成: 总数={} 匹配={} 跳过={} 未匹配={} 作品={}",
                        summary.total_files,
                        summary.matched_files,
                        summary.skipped_files,
                        summary.unmatched_files,
                        summary.series_count
                    ),
                );
            }
            StatusEvent::Error { message } => {
                logs.scraping = false;
                logs.push("error", format!("刮削失败: {message}"));
            }
        }
    }

    while let Ok(evt) = handle.data_rx.try_recv() {
        match evt {
            DataEvent::DatabaseReady { path } => logs.push("info", format!("数据库就绪: {path}")),
            DataEvent::MatchSaved {
                bgm_id,
                file_path,
                matched,
                processed,
                total,
            } => logs.push(
                "debug",
                format!(
                    "匹配入库: bgm={} file={} ({}/{}, matched={})",
                    bgm_id, file_path, processed, total, matched
                ),
            ),
            DataEvent::SeriesSaved { id } => logs.push("debug", format!("作品缓存已更新: {id}")),
        }
    }

    Ok(())
}

fn open_db(config: &Config) -> Result<LibraryDb, String> {
    let library_dir = get_string_config("LIBRARY_DIR", &config.library.dir, "library");
    LibraryDb::open(&PathBuf::from(library_dir)).map_err(|e| e.to_string())
}

fn to_wall_item(record: &SeriesRecord) -> SeriesWallItem {
    SeriesWallItem {
        id: record.id,
        title: pick_title(record),
        subtitle: record.name.clone(),
        cover_local_path: record.cover_local_path.clone(),
        episode_count: record.local.episodes.len(),
        missing_count: record.local.missing_episodes.len(),
        updated_at: record.updated_at,
    }
}

fn to_series_detail(record: SeriesRecord) -> SeriesDetail {
    let episodes = record
        .local
        .episodes
        .iter()
        .map(|ep| SeriesDetailEpisode {
            episode: ep.episode,
            name: ep.name.clone(),
            name_cn: ep.name_cn.clone(),
            ep_type: ep.ep_type,
            files: ep.files.clone(),
        })
        .collect();

    SeriesDetail {
        id: record.id,
        title: pick_title(&record),
        subtitle: record.name,
        summary: record.summary,
        tags: record.tags,
        air_date: record.air_date,
        rating_score: record.rating.as_ref().and_then(|r| r.score),
        rating_total: record.rating.as_ref().and_then(|r| r.total),
        cover_local_path: record.cover_local_path,
        root: record.local.root,
        episodes,
        missing_episodes: record.local.missing_episodes,
        unmatched_files: record.local.unmatched_files,
        updated_at: record.updated_at,
    }
}

fn pick_title(record: &SeriesRecord) -> String {
    if !record.name_cn.trim().is_empty() {
        return record.name_cn.clone();
    }
    if !record.name.trim().is_empty() {
        return record.name.clone();
    }
    format!("#{}", record.id)
}

fn open_with_system_player(path: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    #[cfg(target_os = "linux")]
    {
        ProcessCommand::new("xdg-open").arg(path).spawn()?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        ProcessCommand::new("open").arg(path).spawn()?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        ProcessCommand::new("cmd")
            .args(["/C", "start", ""])
            .arg(path)
            .spawn()?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("unsupported OS for auto player launch".into())
}

pub fn run_gui(config: Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tauri::Builder::default()
        .manage(AppState {
            config,
            backend: Mutex::new(None),
            logs: Mutex::new(LogState {
                next_id: 0,
                scraping: false,
                items: VecDeque::new(),
            }),
        })
        .invoke_handler(tauri::generate_handler![
            start_scrape,
            read_logs,
            list_wall,
            get_series_detail,
            play_episode,
            stop_backend
        ])
        .run(tauri::generate_context!("tauri.conf.json"))
        .map_err(|e| format!("failed to run tauri app: {e}"))?;
    Ok(())
}
