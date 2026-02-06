use crate::matcher::{build_report, fetch_subject_details, llm_parse_list};
use crate::storage::LibraryDb;
use crate::types::{
    Config, FinalMatch, InputItem, LocalEpisode, LocalInfo, MatchOptions, MediaFile, MediaKind,
    ScrapeSummary, SeriesRecord, get_string_config, get_u64_env, now_ts, resolve_llm_settings,
    to_rel_string,
};
use reqwest::Client;
use serde::Serialize;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

#[derive(Debug)]
pub enum Command {
    Scrape { root: PathBuf },
    Stop,
}

#[derive(Debug, Clone, Serialize)]
pub enum StatusEvent {
    Started { root: String },
    Scanned { total_files: usize },
    LlmParsing { total_files: usize },
    Matching { current: usize, total: usize },
    Persisting { current: usize, total: usize },
    Finished { summary: ScrapeSummary },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize)]
pub enum DataEvent {
    DatabaseReady {
        path: String,
    },
    MatchSaved {
        bgm_id: i64,
        file_path: String,
        matched: usize,
        processed: usize,
        total: usize,
    },
    SeriesSaved {
        id: i64,
    },
}

pub struct BackendHandle {
    cmd_tx: Sender<Command>,
    pub status_rx: Receiver<StatusEvent>,
    pub data_rx: Receiver<DataEvent>,
    join: Option<thread::JoinHandle<()>>,
}

impl BackendHandle {
    pub fn send(&self, cmd: Command) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.cmd_tx.send(cmd)?;
        Ok(())
    }

    pub fn stop(mut self) {
        let _ = self.cmd_tx.send(Command::Stop);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

pub fn start_backend(config: Config) -> BackendHandle {
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (status_tx, status_rx) = mpsc::channel();
    let (data_tx, data_rx) = mpsc::channel();

    let join = thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(err) => {
                let _ = status_tx.send(StatusEvent::Error {
                    message: format!("failed to create runtime: {err}"),
                });
                return;
            }
        };

        for cmd in cmd_rx {
            match cmd {
                Command::Scrape { root } => {
                    let status_tx_run = status_tx.clone();
                    let data_tx_run = data_tx.clone();
                    let cfg = config.clone();
                    let res = rt.block_on(async move {
                        run_scrape(&cfg, &root, &status_tx_run, &data_tx_run).await
                    });
                    if let Err(err) = res {
                        let _ = status_tx.send(StatusEvent::Error {
                            message: err.to_string(),
                        });
                    }
                }
                Command::Stop => break,
            }
        }
    });

    BackendHandle {
        cmd_tx,
        status_rx,
        data_rx,
        join: Some(join),
    }
}

async fn run_scrape(
    config: &Config,
    root: &Path,
    status_tx: &Sender<StatusEvent>,
    data_tx: &Sender<DataEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let root_str = root.to_string_lossy().to_string();
    let _ = status_tx.send(StatusEvent::Started {
        root: root_str.clone(),
    });

    let library_dir = get_string_config("LIBRARY_DIR", &config.library.dir, "library");
    let db = LibraryDb::open(&PathBuf::from(library_dir))?;
    let _ = data_tx.send(DataEvent::DatabaseReady {
        path: db.path.to_string_lossy().to_string(),
    });
    let cover_cache_dir = db
        .path
        .parent()
        .map(|p| p.join("covers"))
        .unwrap_or_else(|| PathBuf::from("library/covers"));
    if !cover_cache_dir.exists() {
        fs::create_dir_all(&cover_cache_dir)?;
    }
    db.clear_root_matches(&root_str)?;

    let match_opts = MatchOptions::from_config(config);
    let media_files = scan_media_files(&root)?;
    let _ = status_tx.send(StatusEvent::Scanned {
        total_files: media_files.len(),
    });

    if media_files.is_empty() {
        let summary = ScrapeSummary {
            total_files: 0,
            matched_files: 0,
            skipped_files: 0,
            unmatched_files: 0,
            series_count: 0,
        };
        let _ = status_tx.send(StatusEvent::Finished { summary });
        return Ok(());
    }

    let inputs: Vec<InputItem> = media_files
        .iter()
        .cloned()
        .map(|file| InputItem { file })
        .collect();
    let samples: Vec<String> = inputs.iter().map(|i| i.file.name.clone()).collect();

    let _ = status_tx.send(StatusEvent::LlmParsing {
        total_files: samples.len(),
    });

    let llm_settings = resolve_llm_settings(config)?;
    let llm_items = llm_parse_list(
        llm_settings.provider,
        &llm_settings.base_url,
        llm_settings.token.as_deref(),
        &llm_settings.model,
        &samples,
        config.llm.batch_size,
    )
    .await?;

    let bgm_base = get_string_config("BGM_BASE_URL", &config.bgm.base_url, "https://api.bgm.tv");
    let bgm_token = std::env::var("BGM_TOKEN")
        .ok()
        .or_else(|| config.bgm.token.clone())
        .ok_or("missing BGM_TOKEN (set in config.toml or environment)")?;
    let bgm_limit = get_u64_env("BGM_LIMIT", config.bgm.limit as u64) as usize;
    let bgm_retries = get_u64_env("BGM_RETRY", config.bgm.retries as u64) as usize;

    let mut progress = |current: usize, total: usize| {
        let _ = status_tx.send(StatusEvent::Matching { current, total });
    };

    let mut matched_saved = 0usize;
    let mut on_match_error: Option<String> = None;
    let root_for_match = root_str.clone();
    let mut on_match = |processed: usize, total: usize, result: Option<&FinalMatch>| {
        let matched = match result {
            Some(v) if v.bgm.id.is_some() => v,
            _ => return,
        };
        if on_match_error.is_some() {
            return;
        }
        let bgm_id = match matched.bgm.id {
            Some(v) => v,
            None => return,
        };
        if let Err(err) = db.upsert_file_match(&root_for_match, matched) {
            on_match_error = Some(err.to_string());
            return;
        }
        matched_saved += 1;
        let _ = data_tx.send(DataEvent::MatchSaved {
            bgm_id,
            file_path: matched.file_path.clone(),
            matched: matched_saved,
            processed,
            total,
        });
    };

    let report = build_report(
        llm_settings.provider,
        &llm_settings.base_url,
        llm_settings.token.as_deref(),
        &llm_settings.model,
        &bgm_base,
        &bgm_token,
        bgm_limit,
        bgm_retries,
        &inputs,
        &llm_items,
        &match_opts,
        config.llm.match_concurrency,
        Some(&mut progress),
        Some(&mut on_match),
    )
    .await?;

    if let Some(err) = on_match_error.take() {
        return Err(format!("failed to persist match: {err}").into());
    }

    db.save_report(&root_str, &serde_json::to_string(&report)?)?;
    for item in report.final_matches.iter() {
        if item.bgm.id.is_some() {
            db.upsert_file_match(&root_str, item)?;
        }
    }

    let mut matched_files = 0usize;
    let mut skipped_files = 0usize;
    let mut unmatched_files = 0usize;
    for item in &report.items {
        if item.status == "matched" {
            matched_files += 1;
            continue;
        }
        if item.status.starts_with("skipped_") {
            skipped_files += 1;
        } else {
            unmatched_files += 1;
        }
    }

    let mut grouped: HashMap<i64, Vec<FinalMatch>> = HashMap::new();
    for matched in report.final_matches.iter().cloned() {
        if let Some(id) = matched.bgm.id {
            grouped.entry(id).or_default().push(matched);
        } else {
            unmatched_files += 1;
        }
    }

    let total_series = grouped.len();
    let mut series_done = 0usize;

    for (id, matches) in grouped {
        series_done += 1;
        let _ = status_tx.send(StatusEvent::Persisting {
            current: series_done,
            total: total_series,
        });

        let record = db.load_series(id)?;
        let now = now_ts();
        let refresh_days = get_u64_env("REFRESH_DAYS", config.library.refresh_days);
        let refresh_secs = refresh_days.saturating_mul(86_400) as i64;

        let needs_rating = record
            .as_ref()
            .map(|r| now.saturating_sub(r.rating_updated_at) >= refresh_secs)
            .unwrap_or(true);
        let needs_episodes = record
            .as_ref()
            .map(|r| now.saturating_sub(r.episodes_updated_at) >= refresh_secs)
            .unwrap_or(true);
        let needs_details = record
            .as_ref()
            .map(|r| {
                r.name.is_empty()
                    || r.summary.is_empty()
                    || r.tags.is_empty()
                    || r.air_date.is_none()
            })
            .unwrap_or(true);

        let details = if needs_rating || needs_episodes || needs_details {
            Some(fetch_subject_details(&bgm_base, Some(&bgm_token), id).await?)
        } else {
            None
        };

        let (name, name_cn, summary, tags, air_date, rating, episodes, cover_url) =
            match (&details, &record) {
                (Some(d), _) => (
                    d.name.clone(),
                    d.name_cn.clone(),
                    d.summary.clone(),
                    d.tags.clone(),
                    d.air_date.clone(),
                    d.rating.clone(),
                    d.episodes.clone(),
                    d.cover_url.clone(),
                ),
                (None, Some(r)) => (
                    r.name.clone(),
                    r.name_cn.clone(),
                    r.summary.clone(),
                    r.tags.clone(),
                    r.air_date.clone(),
                    r.rating.clone(),
                    r.episodes.clone(),
                    r.cover_url.clone(),
                ),
                (None, None) => (
                    String::new(),
                    String::new(),
                    String::new(),
                    Vec::new(),
                    None,
                    None,
                    Vec::new(),
                    None,
                ),
            };

        let mut local_eps_map: HashMap<u32, LocalEpisode> = HashMap::new();
        let mut local_eps = HashSet::new();
        let mut series_unmatched = Vec::new();
        let has_special_eps = episodes.iter().any(|e| e.ep_type != 0);
        let mut episode_info_map = HashMap::new();
        for ep in &episodes {
            let rounded = ep.sort.round();
            if (ep.sort - rounded).abs() <= 0.01 && rounded >= 1.0 {
                episode_info_map.insert(rounded as u32, ep);
            }
        }

        for m in &matches {
            let needs_special = matches!(m.media_kind, MediaKind::Audio)
                || m.file_size < match_opts.min_media_size_bytes;
            if needs_special && !has_special_eps {
                unmatched_files += 1;
                if matched_files > 0 {
                    matched_files -= 1;
                }
                series_unmatched.push(m.file_path.clone());
                continue;
            }

            let episode_num = match m.episode_number {
                Some(n) => n,
                None => {
                    series_unmatched.push(m.file_path.clone());
                    continue;
                }
            };
            local_eps.insert(episode_num);
            let rel_path = to_rel_string(&root, Path::new(&m.file_path));
            let info = episode_info_map.get(&episode_num);
            let entry = local_eps_map
                .entry(episode_num)
                .or_insert_with(|| LocalEpisode {
                    episode: episode_num,
                    name: info
                        .map(|e| e.name.clone())
                        .unwrap_or_else(|| format!("Episode {episode_num}")),
                    name_cn: info.map(|e| e.name_cn.clone()).unwrap_or_default(),
                    ep_type: info.map(|e| e.ep_type).unwrap_or(0),
                    files: Vec::new(),
                });
            entry.files.push(rel_path);
        }

        let mut missing_eps = Vec::new();
        let mut main_eps = BTreeSet::new();
        for ep in episodes.iter().filter(|e| e.ep_type == 0) {
            let rounded = ep.sort.round();
            if (ep.sort - rounded).abs() <= 0.01 && rounded >= 1.0 {
                main_eps.insert(rounded as u32);
            }
        }
        for ep in main_eps {
            if !local_eps.contains(&ep) {
                missing_eps.push(ep);
            }
        }

        let mut local_episodes: Vec<LocalEpisode> = local_eps_map.into_values().collect();
        local_episodes.sort_by_key(|e| e.episode);
        let local = LocalInfo {
            root: root_str.clone(),
            episodes: local_episodes,
            missing_episodes: missing_eps,
            unmatched_files: series_unmatched,
        };

        let rating_updated_at = if details.is_some() || record.is_none() {
            now
        } else {
            record.as_ref().map(|r| r.rating_updated_at).unwrap_or(now)
        };
        let episodes_updated_at = if details.is_some() || record.is_none() {
            now
        } else {
            record
                .as_ref()
                .map(|r| r.episodes_updated_at)
                .unwrap_or(now)
        };
        let existing_cover_local_path = record.as_ref().and_then(|r| r.cover_local_path.clone());
        let existing_cover_updated_at = record.as_ref().map(|r| r.cover_updated_at).unwrap_or(0);
        let cover_missing = existing_cover_local_path
            .as_deref()
            .map(|v| !Path::new(v).exists())
            .unwrap_or(true);
        let needs_cover = record
            .as_ref()
            .map(|r| now.saturating_sub(r.cover_updated_at) >= refresh_secs)
            .unwrap_or(true);
        let mut cover_local_path = existing_cover_local_path.clone();
        let mut cover_updated_at = existing_cover_updated_at;
        if let Some(url) = cover_url.as_deref() {
            if needs_cover || cover_missing {
                if let Ok(path) = cache_cover_image(&cover_cache_dir, id, url).await {
                    cover_local_path = Some(path.to_string_lossy().to_string());
                    cover_updated_at = now;
                }
            }
            if cover_local_path.is_some() && cover_updated_at == 0 {
                cover_updated_at = now;
            }
        }

        let record = SeriesRecord {
            id,
            name,
            name_cn,
            summary,
            tags,
            air_date,
            rating,
            episodes,
            local,
            cover_url,
            cover_local_path,
            updated_at: now,
            rating_updated_at,
            episodes_updated_at,
            cover_updated_at,
        };
        db.upsert_series(&record)?;
        let _ = data_tx.send(DataEvent::SeriesSaved { id });
    }

    let summary = ScrapeSummary {
        total_files: report.summary.total,
        matched_files,
        skipped_files,
        unmatched_files,
        series_count: total_series,
    };

    let _ = status_tx.send(StatusEvent::Finished { summary });
    Ok(())
}

fn scan_media_files(
    root: &Path,
) -> Result<Vec<MediaFile>, Box<dyn std::error::Error + Send + Sync>> {
    let mut out = Vec::new();
    scan_dir(root, &mut out)?;
    Ok(out)
}

fn scan_dir(
    root: &Path,
    out: &mut Vec<MediaFile>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if !root.exists() {
        return Ok(());
    }
    if is_skip_dir(root) {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = match entry {
            Ok(v) => v,
            Err(_) => continue,
        };
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            scan_dir(&path, out)?;
        } else if file_type.is_file() {
            if let Some(kind) = classify_media(&path) {
                let name = entry.file_name().to_string_lossy().to_string();
                let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                out.push(MediaFile {
                    path,
                    name,
                    size_bytes,
                    kind,
                });
            }
        }
    }
    Ok(())
}

fn classify_media(path: &Path) -> Option<MediaKind> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    let video_exts = [
        "mkv", "mp4", "avi", "mov", "flv", "wmv", "mpg", "mpeg", "m2ts", "ts", "webm",
    ];
    let audio_exts = [
        "flac", "mp3", "aac", "ogg", "opus", "wav", "m4a", "ape", "alac",
    ];
    if video_exts.iter().any(|e| *e == ext) {
        Some(MediaKind::Video)
    } else if audio_exts.iter().any(|e| *e == ext) {
        Some(MediaKind::Audio)
    } else {
        None
    }
}

fn is_skip_dir(path: &Path) -> bool {
    let name = match path.file_name() {
        Some(n) => n.to_string_lossy(),
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

async fn cache_cover_image(
    cache_dir: &Path,
    subject_id: i64,
    source_url: &str,
) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    let mut url = source_url.trim().to_string();
    if url.starts_with("//") {
        url = format!("https:{url}");
    }
    if !cache_dir.exists() {
        fs::create_dir_all(cache_dir)?;
    }

    let ext = cover_ext_from_url(&url);
    let file_path = cache_dir.join(format!("{subject_id}.{ext}"));

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let bytes = client
        .get(&url)
        .header("User-Agent", "anifrz/0.1")
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    if bytes.is_empty() {
        return Err("cover image is empty".into());
    }
    fs::write(&file_path, &bytes)?;
    Ok(file_path)
}

fn cover_ext_from_url(url: &str) -> String {
    let no_query = url.split('?').next().unwrap_or(url);
    if let Some(name) = no_query.rsplit('/').next()
        && let Some(ext) = name.rsplit('.').next()
    {
        let ext = ext.to_ascii_lowercase();
        if matches!(
            ext.as_str(),
            "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp"
        ) {
            return ext;
        }
    }
    "jpg".to_string()
}
