use crate::types::{FinalMatch, SeriesRecord, now_ts};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fs;
use std::path::{Path, PathBuf};

pub struct LibraryDb {
    conn: Connection,
    pub path: PathBuf,
}

impl LibraryDb {
    pub fn open(library_path: &Path) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let db_path = resolve_db_path(library_path);
        if let Some(parent) = db_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open(&db_path)?;
        let db = Self {
            conn,
            path: db_path,
        };
        db.init_schema()?;
        Ok(db)
    }

    pub fn clear_root_matches(
        &self,
        root: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.conn.execute(
            "DELETE FROM file_matches WHERE root_path = ?1",
            params![root],
        )?;
        Ok(())
    }

    pub fn upsert_file_match(
        &self,
        root: &str,
        item: &FinalMatch,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let llm_episode_json = item
            .llm_episode
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        self.conn.execute(
            "INSERT INTO file_matches(
                root_path, file_path, input, file_size, media_kind, llm_title, llm_episode_json,
                episode_number, bgm_id, bgm_name, bgm_name_cn, bgm_date, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(file_path) DO UPDATE SET
                root_path = excluded.root_path,
                input = excluded.input,
                file_size = excluded.file_size,
                media_kind = excluded.media_kind,
                llm_title = excluded.llm_title,
                llm_episode_json = excluded.llm_episode_json,
                episode_number = excluded.episode_number,
                bgm_id = excluded.bgm_id,
                bgm_name = excluded.bgm_name,
                bgm_name_cn = excluded.bgm_name_cn,
                bgm_date = excluded.bgm_date,
                updated_at = excluded.updated_at",
            params![
                root,
                item.file_path,
                item.input,
                item.file_size as i64,
                item.media_kind.as_str(),
                item.llm_title,
                llm_episode_json,
                item.episode_number.map(|v| v as i64),
                item.bgm.id,
                item.bgm.name,
                item.bgm.name_cn,
                item.bgm.date,
                now_ts(),
            ],
        )?;
        Ok(())
    }

    pub fn upsert_series(
        &self,
        record: &SeriesRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.conn.execute(
            "INSERT INTO series_records(
                id, name, name_cn, summary, tags_json, air_date, rating_json, episodes_json,
                local_json, cover_url, cover_local_path, updated_at, rating_updated_at,
                episodes_updated_at, cover_updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                name_cn = excluded.name_cn,
                summary = excluded.summary,
                tags_json = excluded.tags_json,
                air_date = excluded.air_date,
                rating_json = excluded.rating_json,
                episodes_json = excluded.episodes_json,
                local_json = excluded.local_json,
                cover_url = excluded.cover_url,
                cover_local_path = excluded.cover_local_path,
                updated_at = excluded.updated_at,
                rating_updated_at = excluded.rating_updated_at,
                episodes_updated_at = excluded.episodes_updated_at,
                cover_updated_at = excluded.cover_updated_at",
            params![
                record.id,
                record.name,
                record.name_cn,
                record.summary,
                to_json(&record.tags)?,
                record.air_date,
                record
                    .rating
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
                to_json(&record.episodes)?,
                to_json(&record.local)?,
                record.cover_url.clone(),
                record.cover_local_path.clone(),
                record.updated_at,
                record.rating_updated_at,
                record.episodes_updated_at,
                record.cover_updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn load_series(
        &self,
        id: i64,
    ) -> Result<Option<SeriesRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let mut stmt = self.conn.prepare(
            "SELECT
                id, name, name_cn, summary, tags_json, air_date, rating_json, episodes_json,
                local_json, cover_url, cover_local_path, updated_at, rating_updated_at,
                episodes_updated_at, cover_updated_at
            FROM series_records
            WHERE id = ?1",
        )?;
        let row = stmt
            .query_row(params![id], |row| {
                let tags_json: String = row.get(4)?;
                let rating_json: Option<String> = row.get(6)?;
                let episodes_json: String = row.get(7)?;
                let local_json: String = row.get(8)?;
                Ok(SeriesRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    name_cn: row.get(2)?,
                    summary: row.get(3)?,
                    tags: from_json(&tags_json).map_err(to_sql_err)?,
                    air_date: row.get(5)?,
                    rating: rating_json
                        .as_deref()
                        .map(from_json)
                        .transpose()
                        .map_err(to_sql_err)?,
                    episodes: from_json(&episodes_json).map_err(to_sql_err)?,
                    local: from_json(&local_json).map_err(to_sql_err)?,
                    cover_url: row.get(9)?,
                    cover_local_path: row.get(10)?,
                    updated_at: row.get(11)?,
                    rating_updated_at: row.get(12)?,
                    episodes_updated_at: row.get(13)?,
                    cover_updated_at: row.get(14)?,
                })
            })
            .optional()?;
        Ok(row)
    }

    #[cfg(feature = "tauri-ui")]
    pub fn list_series(
        &self,
        limit: usize,
    ) -> Result<Vec<SeriesRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let mut stmt = self.conn.prepare(
            "SELECT
                id, name, name_cn, summary, tags_json, air_date, rating_json, episodes_json,
                local_json, cover_url, cover_local_path, updated_at, rating_updated_at,
                episodes_updated_at, cover_updated_at
            FROM series_records
            ORDER BY updated_at DESC
            LIMIT ?1",
        )?;

        let mut rows = stmt.query(params![limit as i64])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            let tags_json: String = row.get(4)?;
            let rating_json: Option<String> = row.get(6)?;
            let episodes_json: String = row.get(7)?;
            let local_json: String = row.get(8)?;
            out.push(SeriesRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                name_cn: row.get(2)?,
                summary: row.get(3)?,
                tags: from_json(&tags_json)?,
                air_date: row.get(5)?,
                rating: rating_json.as_deref().map(from_json).transpose()?,
                episodes: from_json(&episodes_json)?,
                local: from_json(&local_json)?,
                cover_url: row.get(9)?,
                cover_local_path: row.get(10)?,
                updated_at: row.get(11)?,
                rating_updated_at: row.get(12)?,
                episodes_updated_at: row.get(13)?,
                cover_updated_at: row.get(14)?,
            });
        }
        Ok(out)
    }

    pub fn save_report(
        &self,
        root: &str,
        report_json: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.conn.execute(
            "INSERT INTO scrape_reports(root_path, report_json, created_at) VALUES (?1, ?2, ?3)",
            params![root, report_json, now_ts()],
        )?;
        Ok(())
    }

    fn init_schema(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;

             CREATE TABLE IF NOT EXISTS series_records (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                name_cn TEXT NOT NULL,
                summary TEXT NOT NULL,
                tags_json TEXT NOT NULL,
                air_date TEXT,
                rating_json TEXT,
                episodes_json TEXT NOT NULL,
                local_json TEXT NOT NULL,
                cover_url TEXT,
                cover_local_path TEXT,
                updated_at INTEGER NOT NULL,
                rating_updated_at INTEGER NOT NULL,
                episodes_updated_at INTEGER NOT NULL,
                cover_updated_at INTEGER NOT NULL DEFAULT 0
             );

             CREATE TABLE IF NOT EXISTS file_matches (
                file_path TEXT PRIMARY KEY,
                root_path TEXT NOT NULL,
                input TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                media_kind TEXT NOT NULL,
                llm_title TEXT NOT NULL,
                llm_episode_json TEXT,
                episode_number INTEGER,
                bgm_id INTEGER,
                bgm_name TEXT,
                bgm_name_cn TEXT,
                bgm_date TEXT,
                updated_at INTEGER NOT NULL
             );

             CREATE INDEX IF NOT EXISTS idx_file_matches_root ON file_matches(root_path);
             CREATE INDEX IF NOT EXISTS idx_file_matches_bgm_id ON file_matches(bgm_id);

             CREATE TABLE IF NOT EXISTS scrape_reports (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                root_path TEXT NOT NULL,
                report_json TEXT NOT NULL,
                created_at INTEGER NOT NULL
             );",
        )?;
        self.ensure_series_column("cover_url", "TEXT")?;
        self.ensure_series_column("cover_local_path", "TEXT")?;
        self.ensure_series_column("cover_updated_at", "INTEGER NOT NULL DEFAULT 0")?;
        Ok(())
    }

    fn ensure_series_column(
        &self,
        column: &str,
        ddl: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut stmt = self.conn.prepare("PRAGMA table_info(series_records)")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            if name == column {
                return Ok(());
            }
        }
        let sql = format!("ALTER TABLE series_records ADD COLUMN {column} {ddl}");
        self.conn.execute(&sql, [])?;
        Ok(())
    }
}

pub fn resolve_db_path(library_path: &Path) -> PathBuf {
    if library_path
        .extension()
        .and_then(|v| v.to_str())
        .map(|v| v.eq_ignore_ascii_case("db"))
        .unwrap_or(false)
    {
        library_path.to_path_buf()
    } else {
        library_path.join("anifrz.db")
    }
}

fn to_json<T: Serialize>(value: &T) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    Ok(serde_json::to_string(value)?)
}

fn from_json<T: DeserializeOwned>(
    text: &str,
) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
    Ok(serde_json::from_str(text)?)
}

fn to_sql_err(err: Box<dyn std::error::Error + Send + Sync>) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            err.to_string(),
        )),
    )
}
