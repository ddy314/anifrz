use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub bgm: BgmConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub library: LibraryConfig,
    #[serde(default)]
    pub media: MediaConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BgmConfig {
    #[serde(default = "default_bgm_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default = "default_bgm_limit")]
    pub limit: usize,
    #[serde(default = "default_bgm_retries")]
    pub retries: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LlmConfig {
    #[serde(default = "default_llm_url")]
    pub url: String,
    #[serde(default = "default_llm_provider")]
    pub provider: String,
    #[serde(default = "default_llm_remote_url")]
    pub remote_url: String,
    #[serde(default = "default_llm_remote_token")]
    pub remote_token: String,
    #[serde(default = "default_llm_model")]
    pub model: String,
    #[serde(default = "default_llm_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_llm_match_concurrency")]
    pub match_concurrency: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LibraryConfig {
    #[serde(default = "default_library_dir")]
    pub dir: String,
    #[serde(default = "default_refresh_days")]
    pub refresh_days: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MediaConfig {
    #[serde(default = "default_min_media_size_mb")]
    pub min_media_size_mb: u64,
}

impl Default for BgmConfig {
    fn default() -> Self {
        Self {
            base_url: default_bgm_base_url(),
            token: None,
            limit: default_bgm_limit(),
            retries: default_bgm_retries(),
        }
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            url: default_llm_url(),
            provider: default_llm_provider(),
            remote_url: default_llm_remote_url(),
            remote_token: default_llm_remote_token(),
            model: default_llm_model(),
            batch_size: default_llm_batch_size(),
            match_concurrency: default_llm_match_concurrency(),
        }
    }
}

impl Default for LibraryConfig {
    fn default() -> Self {
        Self {
            dir: default_library_dir(),
            refresh_days: default_refresh_days(),
        }
    }
}

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            min_media_size_mb: default_min_media_size_mb(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bgm: BgmConfig::default(),
            llm: LlmConfig::default(),
            library: LibraryConfig::default(),
            media: MediaConfig::default(),
        }
    }
}

fn default_bgm_base_url() -> String {
    "https://api.bgm.tv".to_string()
}

fn default_bgm_limit() -> usize {
    20
}

fn default_bgm_retries() -> usize {
    2
}

fn default_llm_url() -> String {
    "http://127.0.0.1:11434".to_string()
}

fn default_llm_model() -> String {
    "qwen3:4b".to_string()
}

fn default_llm_batch_size() -> usize {
    15
}

fn default_llm_remote_url() -> String {
    "".to_string()
}

fn default_llm_remote_token() -> String {
    "".to_string()
}

fn default_llm_provider() -> String {
    "ollama".to_string()
}

fn default_llm_match_concurrency() -> usize {
    4
}

fn default_library_dir() -> String {
    "library".to_string()
}

fn default_refresh_days() -> u64 {
    7
}

fn default_min_media_size_mb() -> u64 {
    30
}

pub fn load_config() -> Config {
    let config_path = PathBuf::from("config.toml");
    if let Ok(content) = fs::read_to_string(&config_path) {
        if let Ok(config) = toml::from_str::<Config>(&content) {
            return config;
        }
    }
    Config::default()
}

pub fn get_string_config(env_key: &str, config_value: &str, default: &str) -> String {
    env::var(env_key).unwrap_or_else(|_| {
        if !config_value.is_empty() {
            config_value.to_string()
        } else {
            default.to_string()
        }
    })
}

pub fn get_u64_env(env_key: &str, fallback: u64) -> u64 {
    env::var(env_key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(fallback)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmProvider {
    Ollama,
    OpenAi,
}

impl LlmProvider {
    pub fn from_str(s: &str) -> Self {
        if s.eq_ignore_ascii_case("openai") {
            LlmProvider::OpenAi
        } else {
            LlmProvider::Ollama
        }
    }
}

#[derive(Debug, Clone)]
pub struct LlmSettings {
    pub provider: LlmProvider,
    pub base_url: String,
    pub token: Option<String>,
    pub model: String,
}

pub fn resolve_llm_settings(
    config: &Config,
) -> Result<LlmSettings, Box<dyn std::error::Error + Send + Sync>> {
    let provider = LlmProvider::from_str(&config.llm.provider);
    if provider == LlmProvider::OpenAi {
        if config.llm.remote_url.trim().is_empty() {
            return Err("llm.remote_url missing (openai mode)".into());
        }
        if config.llm.remote_token.trim().is_empty() {
            return Err("llm.remote_token missing (openai mode)".into());
        }
        return Ok(LlmSettings {
            provider,
            base_url: config.llm.remote_url.clone(),
            token: Some(config.llm.remote_token.clone()),
            model: config.llm.model.clone(),
        });
    }

    Ok(LlmSettings {
        provider,
        base_url: config.llm.url.clone(),
        token: None,
        model: config.llm.model.clone(),
    })
}

pub fn now_ts() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Video,
    Audio,
}

impl MediaKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            MediaKind::Video => "video",
            MediaKind::Audio => "audio",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MediaFile {
    pub path: PathBuf,
    pub name: String,
    pub size_bytes: u64,
    pub kind: MediaKind,
}

#[derive(Debug, Clone)]
pub struct InputItem {
    pub file: MediaFile,
}

#[derive(Debug, Clone)]
pub struct MatchOptions {
    pub min_media_size_bytes: u64,
}

impl MatchOptions {
    pub fn from_config(config: &Config) -> Self {
        let min_mb = get_u64_env("MIN_MEDIA_SIZE_MB", config.media.min_media_size_mb);
        Self {
            min_media_size_bytes: min_mb.saturating_mul(1024 * 1024),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rating {
    pub score: Option<f64>,
    pub total: Option<u64>,
    pub count: Option<std::collections::BTreeMap<String, u64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeInfo {
    pub id: i64,
    pub sort: f64,
    #[serde(rename = "type")]
    pub ep_type: u8,
    pub name: String,
    pub name_cn: String,
}

#[derive(Debug, Clone)]
pub struct SubjectDetails {
    pub name: String,
    pub name_cn: String,
    pub summary: String,
    pub tags: Vec<String>,
    pub air_date: Option<String>,
    pub rating: Option<Rating>,
    pub episodes: Vec<EpisodeInfo>,
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalInfo {
    pub root: String,
    #[serde(default)]
    pub episodes: Vec<LocalEpisode>,
    #[serde(default)]
    pub missing_episodes: Vec<u32>,
    #[serde(default)]
    pub unmatched_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalEpisode {
    pub episode: u32,
    pub name: String,
    pub name_cn: String,
    pub ep_type: u8,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesRecord {
    pub id: i64,
    pub name: String,
    pub name_cn: String,
    pub summary: String,
    pub tags: Vec<String>,
    pub air_date: Option<String>,
    pub rating: Option<Rating>,
    pub episodes: Vec<EpisodeInfo>,
    pub local: LocalInfo,
    pub cover_url: Option<String>,
    pub cover_local_path: Option<String>,
    pub updated_at: i64,
    pub rating_updated_at: i64,
    pub episodes_updated_at: i64,
    pub cover_updated_at: i64,
}

#[derive(Serialize, Clone)]
pub struct BgmMatch {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub name_cn: Option<String>,
    pub date: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct FinalMatch {
    pub input: String,
    pub file_path: String,
    pub file_size: u64,
    pub media_kind: MediaKind,
    pub llm_title: String,
    pub llm_episode: Option<Value>,
    pub episode_number: Option<u32>,
    pub bgm: BgmMatch,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScrapeSummary {
    pub total_files: usize,
    pub matched_files: usize,
    pub skipped_files: usize,
    pub unmatched_files: usize,
    pub series_count: usize,
}

pub fn to_rel_string(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.to_string_lossy().replace('\\', "/")
}
