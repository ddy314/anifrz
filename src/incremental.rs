use std::collections::{HashMap, HashSet};

use crate::types::{CachedFileMatch, InputItem, MediaFile};

pub struct IncrementalPlan {
    pub unchanged: Vec<CachedFileMatch>,
    pub to_process: Vec<InputItem>,
    pub removed: Vec<CachedFileMatch>,
    pub replaced: Vec<CachedFileMatch>,
    pub affected_series_ids: HashSet<i64>,
}

pub fn build_incremental_plan(
    media_files: &[MediaFile],
    existing_rows: &[CachedFileMatch],
) -> IncrementalPlan {
    let mut existing_by_path: HashMap<String, CachedFileMatch> = HashMap::new();
    for row in existing_rows.iter().cloned() {
        existing_by_path.insert(row.file_path.clone(), row);
    }

    let mut unchanged = Vec::new();
    let mut to_process = Vec::new();
    let mut replaced = Vec::new();
    let mut affected_series_ids = HashSet::new();

    for media in media_files.iter().cloned() {
        let path = media.path.to_string_lossy().to_string();
        if let Some(prev) = existing_by_path.remove(&path) {
            if prev.file_fingerprint == media.fingerprint {
                unchanged.push(prev);
                continue;
            }
            if let Some(id) = prev.bgm.id {
                affected_series_ids.insert(id);
            }
            replaced.push(prev);
        }
        to_process.push(InputItem { file: media });
    }

    let mut removed = Vec::new();
    for row in existing_by_path.into_values() {
        if let Some(id) = row.bgm.id {
            affected_series_ids.insert(id);
        }
        removed.push(row);
    }

    IncrementalPlan {
        unchanged,
        to_process,
        removed,
        replaced,
        affected_series_ids,
    }
}
