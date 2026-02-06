mod backend;
mod incremental;
mod matcher;
mod storage;
mod types;
mod ui;

use backend::{Command, DataEvent, StatusEvent, start_backend};
use std::env;
use std::path::PathBuf;
use types::load_config;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = load_config();
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("scrape") => {
            let root = args
                .next()
                .map(PathBuf::from)
                .ok_or("missing media root path")?;
            run_scrape_cmd(config, root)?;
        }
        Some("gui") => {
            ui::run_gui(config)?;
        }
        Some("ipc") => {
            ui::run_ipc(config)?;
        }
        Some("help") | None => print_help(),
        Some(cmd) => {
            println!("unknown command: {cmd}");
            print_help();
        }
    }
    Ok(())
}

fn run_scrape_cmd(
    config: types::Config,
    root: PathBuf,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let backend = start_backend(config);
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
                    println!("作品缓存已更新: {id}");
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
    println!("  anifrz gui");
    println!("  anifrz ipc");
}
