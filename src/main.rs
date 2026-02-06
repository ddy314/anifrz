mod backend;
mod matcher;
mod storage;
mod types;
mod ui;

use backend::{Command, DataEvent, StatusEvent, start_backend};
use matcher::{bgm_search, build_report, llm_parse_list, read_samples};
use std::env;
use std::path::PathBuf;
use types::{
    InputItem, MatchOptions, MediaFile, MediaKind, get_string_config, get_u64_env, load_config,
    resolve_llm_settings,
};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = load_config();
    let mut args = env::args().skip(1);
    let rt = tokio::runtime::Runtime::new()?;
    match args.next().as_deref() {
        Some("scrape") => {
            let root = args
                .next()
                .map(PathBuf::from)
                .ok_or("missing media root path")?;
            run_scrape_cmd(&config, root)?;
        }
        Some("bgm") => {
            let title = args.next().ok_or("missing title")?;
            let base_url =
                get_string_config("BGM_BASE_URL", &config.bgm.base_url, "https://api.bgm.tv");
            let token = env::var("BGM_TOKEN")
                .ok()
                .or_else(|| config.bgm.token.clone())
                .ok_or("missing BGM_TOKEN (set in config.toml or environment)")?;
            let bgm_retries = get_u64_env("BGM_RETRY", config.bgm.retries as u64) as usize;
            let result = rt.block_on(bgm_search(
                &base_url,
                &token,
                &title,
                config.bgm.limit,
                bgm_retries,
            ))?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Some("report") => {
            let input_path = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("test.txt"));
            let output_path = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("report.json"));

            let samples = read_samples(&input_path)?;
            if samples.is_empty() {
                return Err("no samples found".into());
            }

            let llm_settings = resolve_llm_settings(&config)?;

            let llm_items = rt.block_on(llm_parse_list(
                llm_settings.provider,
                &llm_settings.base_url,
                llm_settings.token.as_deref(),
                &llm_settings.model,
                &samples,
                config.llm.batch_size,
            ))?;

            let inputs: Vec<InputItem> = samples
                .iter()
                .map(|name| InputItem {
                    file: MediaFile {
                        path: PathBuf::from(name),
                        name: name.clone(),
                        size_bytes: 100 * 1024 * 1024,
                        kind: MediaKind::Video,
                    },
                })
                .collect();

            let bgm_base =
                get_string_config("BGM_BASE_URL", &config.bgm.base_url, "https://api.bgm.tv");
            let bgm_token = env::var("BGM_TOKEN")
                .ok()
                .or_else(|| config.bgm.token.clone())
                .ok_or("missing BGM_TOKEN (set in config.toml or environment)")?;
            let bgm_limit = env::var("BGM_LIMIT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(config.bgm.limit);
            let bgm_retries = get_u64_env("BGM_RETRY", config.bgm.retries as u64) as usize;

            let report = rt.block_on(build_report(
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
                &MatchOptions::from_config(&config),
                config.llm.match_concurrency,
                None,
                None,
            ))?;

            std::fs::write(&output_path, serde_json::to_string_pretty(&report)?)?;
            println!("report saved to {}", output_path.display());
        }
        Some("gui") => {
            ui::run_gui(config.clone())?;
        }
        Some("help") | None => {
            print_help();
        }
        Some(cmd) => {
            println!("unknown command: {cmd}");
            print_help();
        }
    }
    Ok(())
}

fn run_scrape_cmd(
    config: &types::Config,
    root: PathBuf,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let backend = start_backend(config.clone());
    backend.send(Command::Scrape { root })?;

    loop {
        if let Ok(status) = backend.status_rx.recv() {
            match status {
                StatusEvent::Started { root } => {
                    println!("开始扫描: {root}");
                }
                StatusEvent::Scanned { total_files } => {
                    println!("发现媒体文件: {total_files}");
                }
                StatusEvent::LlmParsing { total_files } => {
                    println!("LLM 解析文件名: {total_files}");
                }
                StatusEvent::Matching { current, total } => {
                    println!("匹配进度: {current}/{total}");
                }
                StatusEvent::Persisting { current, total } => {
                    println!("写入库: {current}/{total}");
                }
                StatusEvent::Finished { summary } => {
                    println!(
                        "完成: 总数={} 匹配={} 跳过={} 未匹配={} 作品数={}",
                        summary.total_files,
                        summary.matched_files,
                        summary.skipped_files,
                        summary.unmatched_files,
                        summary.series_count
                    );
                    break;
                }
                StatusEvent::Error { message } => {
                    return Err(message.into());
                }
            }
        }

        while let Ok(data) = backend.data_rx.try_recv() {
            match data {
                DataEvent::DatabaseReady { path } => {
                    println!("数据库: {path}");
                }
                DataEvent::MatchSaved {
                    bgm_id,
                    file_path,
                    matched,
                    processed,
                    total,
                } => {
                    println!(
                        "匹配已入库: bgm={} file={} ({}/{}, matched={})",
                        bgm_id, file_path, processed, total, matched
                    );
                }
                DataEvent::SeriesSaved { id } => {
                    println!("已更新作品: {id}");
                }
            }
        }
    }

    backend.stop();
    Ok(())
}

fn print_help() {
    println!("Usage:");
    println!("  anifrz scrape <media_dir>");
    println!("  anifrz bgm <title>");
    println!("  anifrz report [input.txt] [report.json]");
    println!("  anifrz gui");
    println!();
    println!("Configuration:");
    println!("  优先级: 环境变量 > config.toml > 默认值");
}
