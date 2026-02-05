use crate::types::SeriesRecord;
use std::fs;
use std::path::{Path, PathBuf};

pub fn ensure_library_dir(path: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

pub fn series_path(library_dir: &Path, id: i64, name: &str) -> PathBuf {
    let safe = sanitize_filename(name);
    if safe.is_empty() {
        library_dir.join(format!("{id}.json"))
    } else {
        library_dir.join(format!("{id}_{safe}.json"))
    }
}

pub fn load_series(path: &Path) -> Result<Option<SeriesRecord>, Box<dyn std::error::Error + Send + Sync>> {
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)?;
    let record = serde_json::from_str::<SeriesRecord>(&text)?;
    Ok(Some(record))
}

pub fn save_series(path: &Path, record: &SeriesRecord) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    let text = serde_json::to_string_pretty(record)?;
    fs::write(path, text)?;
    Ok(())
}

fn sanitize_filename(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if ch == ' ' || ch == '-' || ch == '_' {
            if !out.ends_with('_') {
                out.push('_');
            }
        }
    }
    out.trim_matches('_').to_string()
}
