use crate::backend::{
    BackendHandle, Command as BackendCommand, DataEvent, StatusEvent, start_backend,
};
use crate::storage::{LibraryDb, resolve_db_path};
use crate::types::{
    Config, EpisodeInfo, LocalEpisode, SeriesRecord, get_string_config, now_ts,
    save_config as persist_config,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::Mutex;
use std::sync::mpsc as std_mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};

const MAX_LOGS: usize = 1200;
const MIN_WATCH_INTERVAL_SECS: u64 = 2;

struct AppState {
    config: Mutex<Config>,
    backend: Mutex<Option<BackendHandle>>,
    logs: Mutex<LogState>,
    log_file: Mutex<PathBuf>,
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
    cover_url: Option<String>,
    cover_local_path: Option<String>,
    episode_count: usize,
    total_episode_count: usize,
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
    missing: bool,
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
    cover_url: Option<String>,
    cover_local_path: Option<String>,
    root: String,
    episodes: Vec<SeriesDetailEpisode>,
    missing_episodes: Vec<u32>,
    unmatched_files: Vec<String>,
    updated_at: i64,
}

#[derive(Clone, serde::Serialize)]
struct RuntimeState {
    scraping: bool,
}

#[derive(Clone, serde::Serialize)]
struct BackendStatusPayload {
    kind: String,
    message: String,
    scraping: bool,
}

#[tauri::command]
fn get_app_config(state: tauri::State<AppState>) -> Result<Config, String> {
    let config = state
        .config
        .lock()
        .map_err(|_| "config mutex poisoned".to_string())?;
    Ok(config.clone())
}

#[tauri::command]
fn save_app_config(mut config: Config, state: tauri::State<AppState>) -> Result<(), String> {
    if config.library.watch_interval_secs < MIN_WATCH_INTERVAL_SECS {
        config.library.watch_interval_secs = MIN_WATCH_INTERVAL_SECS;
    }
    persist_config(&config).map_err(|e| e.to_string())?;

    {
        let mut guard = state
            .config
            .lock()
            .map_err(|_| "config mutex poisoned".to_string())?;
        *guard = config.clone();
    }
    {
        let mut path = state
            .log_file
            .lock()
            .map_err(|_| "log_file mutex poisoned".to_string())?;
        *path = runtime_log_file_from_config(&config);
    }
    {
        let mut backend = state
            .backend
            .lock()
            .map_err(|_| "backend mutex poisoned".to_string())?;
        if let Some(handle) = backend.take() {
            handle.stop();
        }
    }

    let mut logs = state
        .logs
        .lock()
        .map_err(|_| "logs mutex poisoned".to_string())?;
    push_log_with_state(&mut logs, &state, "info", "配置已保存");
    Ok(())
}

#[tauri::command]
fn start_scrape(root: String, state: tauri::State<AppState>) -> Result<(), String> {
    let resolved = if root.trim().is_empty() {
        config_snapshot(&state)?
            .library
            .media_root
            .trim()
            .to_string()
    } else {
        root.trim().to_string()
    };
    if resolved.is_empty() {
        return Err("root path is empty".to_string());
    }
    start_scrape_internal(&resolved, &state)
}

#[tauri::command]
fn get_runtime_state(state: tauri::State<AppState>) -> Result<RuntimeState, String> {
    drain_backend_events(&state)?;
    let scraping = state
        .logs
        .lock()
        .map_err(|_| "logs mutex poisoned".to_string())?
        .scraping;
    Ok(RuntimeState { scraping })
}

fn start_scrape_internal(root: &str, state: &AppState) -> Result<(), String> {
    let _ = drain_backend_events(state);
    {
        let logs = state
            .logs
            .lock()
            .map_err(|_| "logs mutex poisoned".to_string())?;
        if logs.scraping {
            return Ok(());
        }
    }
    {
        let mut guard = state
            .backend
            .lock()
            .map_err(|_| "backend mutex poisoned".to_string())?;
        if guard.is_none() {
            *guard = Some(start_backend(config_snapshot(state)?));
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
    push_log_with_state(&mut logs, state, "info", format!("开始刮削: {root}"));
    Ok(())
}

#[tauri::command]
async fn rematch_series(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    if id <= 0 {
        return Err("invalid series id".to_string());
    }

    let config = config_snapshot(&state)?;
    let (root, files_count) = run_blocking(move || {
        let db = open_db(&config)?;
        let record = db
            .load_series(id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("series not found: {id}"))?;
        let root = record.local.root.trim().to_string();
        if root.is_empty() {
            return Err("series local root is empty".to_string());
        }

        let mut file_paths: HashSet<String> = HashSet::new();
        for ep in record.local.episodes {
            for file in ep.files {
                let abs = resolve_local_record_path(&root, &file);
                if !abs.is_empty() {
                    file_paths.insert(abs);
                }
            }
        }
        for file in record.local.unmatched_files {
            let abs = resolve_local_record_path(&root, &file);
            if !abs.is_empty() {
                file_paths.insert(abs);
            }
        }
        let file_paths: Vec<String> = file_paths.into_iter().collect();

        db.clear_series_match_records(&root, id, &file_paths)
            .map_err(|e| e.to_string())?;
        db.clear_series(id).map_err(|e| e.to_string())?;

        Ok::<(String, usize), String>((root, file_paths.len()))
    })
    .await?;

    start_scrape_internal(&root, &state)?;
    let mut logs = state
        .logs
        .lock()
        .map_err(|_| "logs mutex poisoned".to_string())?;
    push_log_with_state(
        &mut logs,
        &state,
        "info",
        format!("已清理作品匹配并触发重匹配: id={id}, files={files_count}"),
    );
    Ok(())
}

fn config_snapshot(state: &AppState) -> Result<Config, String> {
    let guard = state
        .config
        .lock()
        .map_err(|_| "config mutex poisoned".to_string())?;
    Ok(guard.clone())
}

fn runtime_log_file_from_config(config: &Config) -> PathBuf {
    let library_dir = get_string_config("LIBRARY_DIR", &config.library.dir, "library");
    let db_path = resolve_db_path(Path::new(&library_dir));
    let base_dir = db_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    base_dir.join("logs").join("runtime.log")
}

fn append_runtime_log_line(state: &AppState, level: &str, message: &str) {
    let path = match state.log_file.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => return,
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut file = match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(v) => v,
        Err(_) => return,
    };
    let ts = now_ts();
    let _ = writeln!(file, "[{ts}] [{level}] {message}");
}

fn push_log_with_state(
    logs: &mut LogState,
    state: &AppState,
    level: &str,
    message: impl Into<String>,
) {
    let message = message.into();
    logs.push(level, &message);
    append_runtime_log_line(state, level, &message);
}

async fn run_blocking<T, F>(job: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tokio::task::spawn_blocking(job)
        .await
        .map_err(|e| format!("blocking task join error: {e}"))?
}

#[tauri::command]
fn read_logs(
    after_id: Option<u64>,
    limit: Option<usize>,
    state: tauri::State<AppState>,
) -> Result<Vec<AppLog>, String> {
    drain_backend_events_with_emit(&state, None)?;
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
async fn list_wall(
    limit: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SeriesWallItem>, String> {
    let config = config_snapshot(&state)?;
    let max = limit.unwrap_or(120);
    run_blocking(move || {
        let db = open_db(&config)?;
        let records = db.list_series(max).map_err(|e| e.to_string())?;
        Ok(records.iter().map(to_wall_item).collect())
    })
    .await
}

#[tauri::command]
async fn get_series_detail(
    id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<Option<SeriesDetail>, String> {
    let config = config_snapshot(&state)?;
    run_blocking(move || {
        let db = open_db(&config)?;
        let record = db.load_series(id).map_err(|e| e.to_string())?;
        Ok(record.map(to_series_detail))
    })
    .await
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
    push_log_with_state(
        &mut logs,
        &state,
        "info",
        format!("唤起播放器: {}", target.display()),
    );
    Ok(())
}

#[tauri::command]
async fn get_cover_data_url(path: String) -> Result<Option<String>, String> {
    run_blocking(move || get_cover_data_url_blocking(path)).await
}

fn get_cover_data_url_blocking(path: String) -> Result<Option<String>, String> {
    let input = path.trim();
    if input.is_empty() {
        return Ok(None);
    }

    let path_buf = if Path::new(input).is_absolute() {
        PathBuf::from(input)
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(input)
    };

    if !path_buf.exists() {
        return Ok(None);
    }

    let bytes = fs::read(&path_buf).map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Ok(None);
    }

    let mime = sniff_image_mime(&bytes).unwrap_or("application/octet-stream");

    let encoded = BASE64_STANDARD.encode(bytes);
    Ok(Some(format!("data:{mime};base64,{encoded}")))
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
    push_log_with_state(&mut logs, &state, "info", "后台任务已停止");
    Ok(())
}

fn drain_backend_events(state: &AppState) -> Result<(), String> {
    drain_backend_events_with_emit(state, None)
}

fn drain_backend_events_with_emit(
    state: &AppState,
    app: Option<&tauri::AppHandle>,
) -> Result<(), String> {
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
            StatusEvent::Started { root } => {
                let message = format!("开始扫描目录: {root}");
                push_log_with_state(&mut logs, state, "info", &message);
                emit_backend_status(app, "started", message, true);
            }
            StatusEvent::Scanned { total_files } => {
                let message = format!("扫描完成，媒体文件 {total_files} 个");
                push_log_with_state(&mut logs, state, "info", &message);
                emit_backend_status(app, "scanned", message, true);
            }
            StatusEvent::LlmParsing { total_files } => {
                let message = format!("LLM 解析文件名: {total_files} 个");
                push_log_with_state(&mut logs, state, "info", &message);
                emit_backend_status(app, "llm_parsing", message, true);
            }
            StatusEvent::Matching { current, total } => {
                let should_emit = current <= 3 || current == total || current % 10 == 0;
                if should_emit {
                    let message = format!("匹配中: {current}/{total}");
                    push_log_with_state(&mut logs, state, "debug", &message);
                    emit_backend_status(app, "matching", message, true);
                }
            }
            StatusEvent::Persisting { current, total } => {
                let should_emit = current == 0 || current == total || current % 5 == 0;
                if should_emit {
                    let message = format!("入库中: {current}/{total}");
                    push_log_with_state(&mut logs, state, "debug", &message);
                    emit_backend_status(app, "persisting", message, true);
                }
            }
            StatusEvent::Finished { summary } => {
                logs.scraping = false;
                let message = format!(
                    "刮削完成: 总数={} 匹配={} 跳过={} 未匹配={} 作品={}",
                    summary.total_files,
                    summary.matched_files,
                    summary.skipped_files,
                    summary.unmatched_files,
                    summary.series_count
                );
                push_log_with_state(&mut logs, state, "info", &message);
                emit_backend_status(app, "finished", message, false);
            }
            StatusEvent::Error { message } => {
                logs.scraping = false;
                let err_message = format!("刮削失败: {message}");
                push_log_with_state(&mut logs, state, "error", &err_message);
                emit_backend_status(app, "error", err_message, false);
            }
        }
    }

    while let Ok(evt) = handle.data_rx.try_recv() {
        match evt {
            DataEvent::DatabaseReady { path } => {
                push_log_with_state(&mut logs, state, "info", format!("数据库就绪: {path}"));
            }
            DataEvent::MatchSaved {
                bgm_id,
                file_path,
                matched,
                processed,
                total,
            } => {
                push_log_with_state(
                    &mut logs,
                    state,
                    "debug",
                    format!(
                        "匹配入库: bgm={} file={} ({}/{}, matched={})",
                        bgm_id, file_path, processed, total, matched
                    ),
                );
                let should_emit = processed <= 3 || processed == total || processed % 12 == 0;
                if should_emit {
                    emit_backend_status(
                        app,
                        "match_saved",
                        format!("匹配入库进度: {processed}/{total}"),
                        true,
                    );
                }
            }
            DataEvent::SeriesSaved { id } => {
                push_log_with_state(&mut logs, state, "debug", format!("作品缓存已更新: {id}"));
                emit_backend_status(app, "series_saved", format!("作品已写入缓存: {id}"), true);
            }
        }
    }

    Ok(())
}

fn open_db(config: &Config) -> Result<LibraryDb, String> {
    let library_dir = get_string_config("LIBRARY_DIR", &config.library.dir, "library");
    LibraryDb::open(&PathBuf::from(library_dir)).map_err(|e| e.to_string())
}

fn to_wall_item(record: &SeriesRecord) -> SeriesWallItem {
    let total_episode_count = count_main_episodes(record);
    let episode_count = record.local.episodes.len();
    let missing_count = total_episode_count.saturating_sub(episode_count);
    SeriesWallItem {
        id: record.id,
        title: pick_title(record),
        subtitle: record.name.clone(),
        cover_url: record.cover_url.clone(),
        cover_local_path: normalize_cover_path(record.cover_local_path.clone()),
        episode_count,
        total_episode_count,
        missing_count,
        updated_at: record.updated_at,
    }
}

fn count_main_episodes(record: &SeriesRecord) -> usize {
    let mut episodes = std::collections::BTreeSet::new();
    for ep in record.episodes.iter().filter(|ep| ep.ep_type == 0) {
        if let Some(num) = episode_from_sort(ep) {
            episodes.insert(num);
        }
    }
    if !episodes.is_empty() {
        return episodes.len();
    }
    if !record.local.episodes.is_empty() {
        return record.local.episodes.len();
    }
    0
}

fn to_series_detail(record: SeriesRecord) -> SeriesDetail {
    let mut local_map: HashMap<u32, LocalEpisode> = HashMap::new();
    for ep in &record.local.episodes {
        local_map.insert(ep.episode, ep.clone());
    }

    let mut episodes = Vec::new();
    for ep in &record.episodes {
        if let Some(episode_num) = episode_from_sort(ep) {
            let local = local_map.remove(&episode_num);
            let files = local
                .as_ref()
                .map(|v| v.files.clone())
                .unwrap_or_else(Vec::new);
            episodes.push(SeriesDetailEpisode {
                episode: episode_num,
                name: if ep.name.is_empty() {
                    format!("Episode {episode_num}")
                } else {
                    ep.name.clone()
                },
                name_cn: ep.name_cn.clone(),
                ep_type: ep.ep_type,
                missing: files.is_empty(),
                files,
            });
        }
    }

    for local in local_map.into_values() {
        episodes.push(SeriesDetailEpisode {
            episode: local.episode,
            name: local.name,
            name_cn: local.name_cn,
            ep_type: local.ep_type,
            missing: false,
            files: local.files,
        });
    }
    episodes.sort_by_key(|ep| (ep.ep_type, ep.episode));

    let missing_episodes = episodes
        .iter()
        .filter(|ep| ep.ep_type == 0 && ep.missing)
        .map(|ep| ep.episode)
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
        cover_url: record.cover_url,
        cover_local_path: normalize_cover_path(record.cover_local_path),
        root: record.local.root,
        episodes,
        missing_episodes,
        unmatched_files: record.local.unmatched_files,
        updated_at: record.updated_at,
    }
}

fn episode_from_sort(ep: &EpisodeInfo) -> Option<u32> {
    let rounded = ep.sort.round();
    if rounded >= 1.0 && (ep.sort - rounded).abs() <= 0.01 {
        return Some(rounded as u32);
    }
    None
}

fn normalize_cover_path(raw: Option<String>) -> Option<String> {
    let input = raw?;
    if input.trim().is_empty() {
        return None;
    }

    let path = PathBuf::from(input.trim());
    if path.is_absolute() {
        return Some(path.to_string_lossy().to_string());
    }

    let cwd = std::env::current_dir().ok()?;
    let joined = cwd.join(path);
    if let Ok(canonical) = joined.canonicalize() {
        return Some(canonical.to_string_lossy().to_string());
    }
    Some(joined.to_string_lossy().to_string())
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

fn resolve_local_record_path(root: &str, file: &str) -> String {
    let file = file.trim();
    if file.is_empty() {
        return String::new();
    }
    let path = PathBuf::from(file);
    if path.is_absolute() {
        return path.to_string_lossy().to_string();
    }
    PathBuf::from(root).join(path).to_string_lossy().to_string()
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

fn sniff_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return Some("image/jpeg");
    }
    if bytes.len() >= 8
        && bytes[0] == 0x89
        && bytes[1] == 0x50
        && bytes[2] == 0x4E
        && bytes[3] == 0x47
        && bytes[4] == 0x0D
        && bytes[5] == 0x0A
        && bytes[6] == 0x1A
        && bytes[7] == 0x0A
    {
        return Some("image/png");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    if bytes.len() >= 6 && (&bytes[0..6] == b"GIF87a" || &bytes[0..6] == b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() >= 2 && bytes[0] == b'B' && bytes[1] == b'M' {
        return Some("image/bmp");
    }
    None
}

fn is_media_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "mkv"
            | "mp4"
            | "avi"
            | "mov"
            | "flv"
            | "wmv"
            | "mpg"
            | "mpeg"
            | "m2ts"
            | "ts"
            | "webm"
            | "flac"
            | "mp3"
            | "aac"
            | "ogg"
            | "opus"
            | "wav"
            | "m4a"
            | "ape"
            | "alac"
    )
}

fn should_skip_dir(path: &Path) -> bool {
    let name = match path.file_name() {
        Some(v) => v.to_string_lossy(),
        None => return false,
    };
    let cleaned: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect();
    matches!(
        cleaned.as_str(),
        "sp" | "sps" | "special" | "specials" | "cd" | "cds"
    )
}

fn emit_backend_status(
    app: Option<&tauri::AppHandle>,
    kind: &str,
    message: String,
    scraping: bool,
) {
    let Some(app) = app else {
        return;
    };
    let payload = BackendStatusPayload {
        kind: kind.to_string(),
        message,
        scraping,
    };
    let _ = app.emit("backend://status", payload);
}

fn spawn_runtime_worker(app_handle: tauri::AppHandle) {
    thread::spawn(move || {
        let (notify_tx, notify_rx) = std_mpsc::channel::<Result<notify::Event, notify::Error>>();
        let mut _watcher: Option<RecommendedWatcher> = None;
        let mut watched_root: Option<PathBuf> = None;
        let mut pending_change = false;
        let mut last_change = Instant::now();
        let debounce = Duration::from_secs(2);

        loop {
            let state = app_handle.state::<AppState>();
            let _ = drain_backend_events_with_emit(&state, Some(&app_handle));

            let desired_root = desired_watch_root(&state);
            if desired_root != watched_root {
                _watcher = None;
                watched_root = None;
                pending_change = false;

                if let Some(root) = desired_root {
                    let tx = notify_tx.clone();
                    match notify::recommended_watcher(move |res| {
                        let _ = tx.send(res);
                    }) {
                        Ok(mut created) => {
                            let _ = created.configure(notify::Config::default());
                            if created.watch(&root, RecursiveMode::Recursive).is_ok() {
                                _watcher = Some(created);
                                watched_root = Some(root.clone());
                                emit_backend_status(
                                    Some(&app_handle),
                                    "watching",
                                    format!("目录监控已启用: {}", root.display()),
                                    false,
                                );
                            } else {
                                emit_backend_status(
                                    Some(&app_handle),
                                    "error",
                                    format!("目录监控失败: {}", root.display()),
                                    false,
                                );
                            }
                        }
                        Err(err) => {
                            emit_backend_status(
                                Some(&app_handle),
                                "error",
                                format!("notify 初始化失败: {err}"),
                                false,
                            );
                        }
                    }
                }
            }

            while let Ok(event) = notify_rx.try_recv() {
                match event {
                    Ok(evt) => {
                        if should_trigger_on_notify_event(&evt) {
                            pending_change = true;
                            last_change = Instant::now();
                        }
                    }
                    Err(err) => emit_backend_status(
                        Some(&app_handle),
                        "error",
                        format!("目录监控事件异常: {err}"),
                        false,
                    ),
                }
            }

            if pending_change && last_change.elapsed() >= debounce {
                if let Some(root) = watched_root.as_ref() {
                    let state = app_handle.state::<AppState>();
                    let _ = start_scrape_internal(root.to_string_lossy().as_ref(), &state);
                }
                pending_change = false;
            }

            thread::sleep(Duration::from_millis(180));
        }
    });
}

fn desired_watch_root(state: &AppState) -> Option<PathBuf> {
    let config = config_snapshot(state).ok()?;
    if !config.library.auto_watch {
        return None;
    }
    let root = config.library.media_root.trim();
    if root.is_empty() {
        return None;
    }
    let path = PathBuf::from(root);
    if !path.exists() {
        return None;
    }
    Some(path.canonicalize().unwrap_or(path))
}

fn should_trigger_on_notify_event(event: &notify::Event) -> bool {
    use notify::event::{CreateKind, ModifyKind, RemoveKind, RenameMode};
    let mut interesting = false;
    match &event.kind {
        notify::EventKind::Create(CreateKind::Any)
        | notify::EventKind::Create(CreateKind::File)
        | notify::EventKind::Create(CreateKind::Folder)
        | notify::EventKind::Modify(ModifyKind::Any)
        | notify::EventKind::Modify(ModifyKind::Data(_))
        | notify::EventKind::Modify(ModifyKind::Name(RenameMode::Any))
        | notify::EventKind::Modify(ModifyKind::Name(RenameMode::Both))
        | notify::EventKind::Modify(ModifyKind::Name(RenameMode::From))
        | notify::EventKind::Modify(ModifyKind::Name(RenameMode::To))
        | notify::EventKind::Remove(RemoveKind::Any)
        | notify::EventKind::Remove(RemoveKind::File)
        | notify::EventKind::Remove(RemoveKind::Folder) => {
            interesting = true;
        }
        _ => {}
    }
    if !interesting {
        return false;
    }

    event.paths.iter().any(|path| {
        if path.is_dir() {
            return !should_skip_dir(path);
        }
        is_media_file(path)
    })
}

pub fn run_gui(config: Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let log_file = runtime_log_file_from_config(&config);
    tauri::Builder::default()
        .manage(AppState {
            config: Mutex::new(config),
            backend: Mutex::new(None),
            logs: Mutex::new(LogState {
                next_id: 0,
                scraping: false,
                items: VecDeque::new(),
            }),
            log_file: Mutex::new(log_file),
        })
        .setup(|app| {
            spawn_runtime_worker(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_config,
            save_app_config,
            start_scrape,
            rematch_series,
            get_runtime_state,
            read_logs,
            list_wall,
            get_series_detail,
            play_episode,
            get_cover_data_url,
            stop_backend
        ])
        .run(tauri::generate_context!("tauri.conf.json"))
        .map_err(|e| format!("failed to run tauri app: {e}"))?;
    Ok(())
}
