use std::{path::Path, sync::Arc};

use anyhow::{Context, Result};
use regex::RegexBuilder;
use rusqlite::{OptionalExtension, params};
use time::{OffsetDateTime, macros::format_description};

use qingqi_plugin::database::{DatabaseService, PooledConnection, SqlitePool};

use super::{
    history_store::{
        ClipboardConfig, ClipboardItemKind, ClipboardRecord, classify_text, compact_preview,
        file_list_preview, parse_file_paths,
    },
    service::ClipboardFilter,
};

pub struct ClipboardDataSource {
    pool: SqlitePool,
}

const MAX_HISTORY_ITEMS: i64 = 5_000;

const LOAD_CONFIG_SQL: &str = "
SELECT capture_text, capture_image, capture_files, max_text_chars, ignore_patterns_json, hotkey
FROM clipboard_config WHERE id = 1
";
const UPSERT_CONFIG_SQL: &str = "
INSERT INTO clipboard_config
    (id, capture_text, capture_image, capture_files, max_text_chars, ignore_patterns_json, hotkey, updated_at)
VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)
ON CONFLICT(id) DO UPDATE SET
    capture_text = excluded.capture_text,
    capture_image = excluded.capture_image,
    capture_files = excluded.capture_files,
    max_text_chars = excluded.max_text_chars,
    ignore_patterns_json = excluded.ignore_patterns_json,
    hotkey = excluded.hotkey,
    updated_at = excluded.updated_at
";
const LATEST_ITEM_SQL: &str =
    "SELECT item_type, content FROM clipboard_history ORDER BY id DESC LIMIT 1";
const LOAD_EXISTING_PINNED_SQL: &str = "
SELECT pinned FROM clipboard_history
WHERE item_type = ?1 AND content = ?2
ORDER BY id DESC LIMIT 1
";
const DELETE_DUPLICATE_FTS_SQL: &str = "
DELETE FROM clipboard_history_fts
WHERE rowid IN (
    SELECT id FROM clipboard_history WHERE item_type = ?1 AND content = ?2
)
";
const DELETE_DUPLICATE_HISTORY_SQL: &str =
    "DELETE FROM clipboard_history WHERE item_type = ?1 AND content = ?2";
const INSERT_HISTORY_ITEM_SQL: &str = "
INSERT INTO clipboard_history (item_type, content, preview, pinned, created_at, badge)
VALUES (?1, ?2, ?3, ?4, ?5, ?6)
";
const SELECT_RECORDS_BASE_SQL: &str = "
SELECT id, item_type, content, preview, pinned, created_at, badge
FROM clipboard_history
";
const LATEST_RECORD_SQL: &str = "
SELECT id, item_type, content, preview, pinned, created_at, badge
FROM clipboard_history
ORDER BY id DESC
LIMIT 1
";
const SELECT_PINNED_BY_ID_SQL: &str = "SELECT pinned FROM clipboard_history WHERE id = ?1";
const UPDATE_PINNED_SQL: &str = "UPDATE clipboard_history SET pinned = ?1 WHERE id = ?2";
const DELETE_FTS_BY_ROWID_SQL: &str = "DELETE FROM clipboard_history_fts WHERE rowid = ?1";
const DELETE_HISTORY_BY_ID_SQL: &str = "DELETE FROM clipboard_history WHERE id = ?1";
const CLEAR_FTS_SQL: &str = "DELETE FROM clipboard_history_fts";
const CLEAR_HISTORY_SQL: &str = "DELETE FROM clipboard_history";
const CLEAR_UNPINNED_FTS_SQL: &str = "
DELETE FROM clipboard_history_fts
WHERE rowid IN (SELECT id FROM clipboard_history WHERE pinned = 0)
";
const CLEAR_UNPINNED_HISTORY_SQL: &str = "DELETE FROM clipboard_history WHERE pinned = 0";
const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS clipboard_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_type TEXT NOT NULL DEFAULT 'text',
    content TEXT NOT NULL DEFAULT '',
    preview TEXT NOT NULL DEFAULT '',
    pinned INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    badge TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_clipboard_order
    ON clipboard_history(pinned DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_clipboard_type
    ON clipboard_history(item_type);
CREATE VIRTUAL TABLE IF NOT EXISTS clipboard_history_fts
    USING fts5(search_text, content='', contentless_delete=1);
CREATE TABLE IF NOT EXISTS clipboard_config (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    capture_text INTEGER NOT NULL DEFAULT 1,
    capture_image INTEGER NOT NULL DEFAULT 1,
    capture_files INTEGER NOT NULL DEFAULT 1,
    max_text_chars INTEGER NOT NULL DEFAULT 20000,
    ignore_patterns_json TEXT NOT NULL DEFAULT '[]',
    hotkey TEXT NOT NULL DEFAULT 'Alt+V',
    updated_at TEXT NOT NULL
);
";
const SELECT_FTS_RECORD_SQL: &str = "
SELECT id, preview, badge FROM clipboard_history
WHERE item_type = ?1 AND content = ?2
ORDER BY id DESC LIMIT 1
";
const UPSERT_FTS_ROW_SQL: &str =
    "INSERT OR REPLACE INTO clipboard_history_fts(rowid, search_text) VALUES (?1, ?2)";
const COUNT_HISTORY_OVERFLOW_SQL: &str = "SELECT COUNT(*) - ?1 FROM clipboard_history";
const PRUNE_FTS_SQL: &str = "
DELETE FROM clipboard_history_fts
WHERE rowid IN (
    SELECT id
    FROM clipboard_history
    WHERE pinned = 0
    ORDER BY id ASC
    LIMIT ?1
)
";
const PRUNE_HISTORY_SQL: &str = "
DELETE FROM clipboard_history
WHERE id IN (
    SELECT id
    FROM clipboard_history
    WHERE pinned = 0
    ORDER BY id ASC
    LIMIT ?1
)
";

impl ClipboardDataSource {
    pub fn open(database: Arc<DatabaseService>, key: &str) -> Result<Self> {
        let pool = database.pool(key)?;
        let store = Self { pool };
        store
            .ensure_schema()
            .context("cannot initialize clipboard database schema")?;
        Ok(store)
    }

    pub fn add_text(&self, text: &str, config: &ClipboardConfig) -> Result<bool> {
        if !config.capture_text {
            return Ok(false);
        }
        let value = text.trim_end_matches('\0');
        if value.trim().is_empty() {
            return Ok(false);
        }
        if config.max_text_chars > 0 && value.chars().count() > config.max_text_chars {
            return Ok(false);
        }
        let preview = compact_preview(value);
        if should_ignore_text(value, &preview, &config.ignore_patterns) {
            return Ok(false);
        }
        let badge = classify_text(value);
        self.add_item(ClipboardItemKind::Text, value, &preview, badge)
    }

    pub fn add_image(
        &self,
        image_path: &str,
        preview: &str,
        badge: &str,
        config: &ClipboardConfig,
    ) -> Result<bool> {
        if !config.capture_image || image_path.trim().is_empty() {
            return Ok(false);
        }
        self.add_item(ClipboardItemKind::Image, image_path, preview, badge)
    }

    pub fn add_files(&self, paths: &[String], config: &ClipboardConfig) -> Result<bool> {
        if !config.capture_files || paths.is_empty() {
            return Ok(false);
        }
        let content = serde_json::to_string(paths)?;
        let preview = file_list_preview(paths);
        self.add_item(ClipboardItemKind::Files, &content, &preview, "文件")
    }

    pub fn load_config(&self) -> Result<ClipboardConfig> {
        let conn = self.connection()?;
        let config = conn
            .query_row(LOAD_CONFIG_SQL, [], |row| {
                let ignore_patterns_json: String = row.get(4)?;
                Ok(ClipboardConfig {
                    capture_text: row.get::<_, i64>(0)? != 0,
                    capture_image: row.get::<_, i64>(1)? != 0,
                    capture_files: row.get::<_, i64>(2)? != 0,
                    max_text_chars: row.get::<_, i64>(3)? as usize,
                    ignore_patterns: serde_json::from_str(&ignore_patterns_json)
                        .unwrap_or_default(),
                    hotkey: row.get::<_, String>(5)?,
                })
            })
            .optional()?;
        Ok(config.unwrap_or_default())
    }

    pub fn save_config(&self, config: &ClipboardConfig) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            UPSERT_CONFIG_SQL,
            params![
                if config.capture_text { 1 } else { 0 },
                if config.capture_image { 1 } else { 0 },
                if config.capture_files { 1 } else { 0 },
                config.max_text_chars as i64,
                serde_json::to_string(&config.ignore_patterns)?,
                config.hotkey.as_str(),
                now_label(),
            ],
        )?;
        Ok(())
    }

    pub fn search(
        &self,
        query: &str,
        filter: ClipboardFilter,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<ClipboardRecord>> {
        let q = query.trim();
        let mut conditions = Vec::new();
        let mut base_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(kind) = filter.kind() {
            conditions.push("item_type = ?".to_string());
            base_params.push(Box::new(kind.as_str().to_string()));
        }
        if filter.pinned_only() {
            conditions.push("pinned = 1".to_string());
        }
        if let Some(badge) = filter.badge_filter() {
            conditions.push("badge = ?".to_string());
            base_params.push(Box::new(badge.to_string()));
        }
        if !q.is_empty() {
            conditions.push(
                "id IN (
                    SELECT rowid
                    FROM clipboard_history_fts
                    WHERE clipboard_history_fts MATCH ?
                )"
                .to_string(),
            );
            base_params.push(Box::new(fts_query(q)));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "{SELECT_RECORDS_BASE_SQL}{where_clause}
             ORDER BY pinned DESC, id DESC
             LIMIT ? OFFSET ?"
        );

        let mut all_params = base_params;
        all_params.push(Box::new(limit as i64));
        all_params.push(Box::new(offset as i64));
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            all_params.iter().map(|p| p.as_ref()).collect();
        self.query_records_dyn(&sql, &param_refs)
    }

    pub fn search_all(&self, query: &str, filter: ClipboardFilter) -> Result<Vec<ClipboardRecord>> {
        let q = query.trim();
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(kind) = filter.kind() {
            conditions.push("item_type = ?".to_string());
            params.push(Box::new(kind.as_str().to_string()));
        }
        if filter.pinned_only() {
            conditions.push("pinned = 1".to_string());
        }
        if let Some(badge) = filter.badge_filter() {
            conditions.push("badge = ?".to_string());
            params.push(Box::new(badge.to_string()));
        }
        if !q.is_empty() {
            conditions.push(
                "id IN (
                    SELECT rowid
                    FROM clipboard_history_fts
                    WHERE clipboard_history_fts MATCH ?
                )"
                .to_string(),
            );
            params.push(Box::new(fts_query(q)));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "{SELECT_RECORDS_BASE_SQL}{where_clause}
             ORDER BY pinned DESC, id DESC"
        );
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        self.query_records_dyn(&sql, &param_refs)
    }

    pub fn latest(&self) -> Result<Option<ClipboardRecord>> {
        let rows = self.query_records_dyn(LATEST_RECORD_SQL, &[])?;
        Ok(rows.into_iter().next())
    }

    pub fn toggle_pin(&self, id: i64) -> Result<Option<bool>> {
        let conn = self.connection()?;
        let pinned = self.query_optional(&conn, SELECT_PINNED_BY_ID_SQL, params![id], |row| {
            row.get::<_, i64>(0)
        })?;
        let Some(pinned) = pinned else {
            return Ok(None);
        };
        let next = if pinned == 0 { 1 } else { 0 };
        conn.execute(UPDATE_PINNED_SQL, params![next, id])?;
        Ok(Some(next == 1))
    }

    pub fn delete(&self, id: i64) -> Result<bool> {
        let conn = self.connection()?;
        conn.execute(DELETE_FTS_BY_ROWID_SQL, params![id])?;
        Ok(conn.execute(DELETE_HISTORY_BY_ID_SQL, params![id])? > 0)
    }

    pub fn clear_all(&self) -> Result<usize> {
        let conn = self.connection()?;
        conn.execute(CLEAR_FTS_SQL, [])?;
        conn.execute(CLEAR_HISTORY_SQL, []).map_err(Into::into)
    }

    pub fn clear_unpinned(&self) -> Result<usize> {
        let conn = self.connection()?;
        conn.execute(CLEAR_UNPINNED_FTS_SQL, [])?;
        conn.execute(CLEAR_UNPINNED_HISTORY_SQL, [])
            .map_err(Into::into)
    }

    fn add_item(
        &self,
        kind: ClipboardItemKind,
        content: &str,
        preview: &str,
        badge: &str,
    ) -> Result<bool> {
        let conn = self.connection()?;
        let latest = self.query_optional(&conn, LATEST_ITEM_SQL, [], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        if latest
            .as_ref()
            .is_some_and(|(latest_kind, latest_content)| {
                latest_kind == kind.as_str() && latest_content == content
            })
        {
            return Ok(false);
        }

        let existing_pinned = self
            .query_optional(
                &conn,
                LOAD_EXISTING_PINNED_SQL,
                params![kind.as_str(), content],
                |row| row.get::<_, i64>(0),
            )?
            .unwrap_or(0);

        conn.execute(DELETE_DUPLICATE_FTS_SQL, params![kind.as_str(), content])?;
        conn.execute(
            DELETE_DUPLICATE_HISTORY_SQL,
            params![kind.as_str(), content],
        )?;
        conn.execute(
            INSERT_HISTORY_ITEM_SQL,
            params![
                kind.as_str(),
                content,
                preview,
                existing_pinned,
                now_label(),
                badge,
            ],
        )?;
        self.sync_fts_for_content(&conn, kind, content)?;
        self.prune_history(&conn)?;
        Ok(true)
    }

    fn query_records_dyn(
        &self,
        sql: &str,
        params: &[&dyn rusqlite::types::ToSql],
    ) -> Result<Vec<ClipboardRecord>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params, |row| {
            let kind_str: String = row.get(1)?;
            Ok(ClipboardRecord {
                id: row.get(0)?,
                kind: ClipboardItemKind::from_db(&kind_str),
                content: row.get(2)?,
                preview: row.get(3)?,
                pinned: row.get::<_, i64>(4)? != 0,
                created_at: row.get(5)?,
                badge: row.get(6)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    fn ensure_schema(&self) -> Result<()> {
        let conn = self.connection()?;
        conn.execute_batch(SCHEMA_SQL)?;

        // 迁移：已有 FTS 表缺少 contentless_delete=1 选项，不支持 DELETE，
        // 需要重建 FTS 表以启用该选项。
        let fts_ddl: String = conn.query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='clipboard_history_fts'",
            [],
            |row| row.get(0),
        )?;
        if !fts_ddl.contains("contentless_delete") {
            conn.execute_batch("DROP TABLE IF EXISTS clipboard_history_fts")?;
            conn.execute_batch(
                "CREATE VIRTUAL TABLE clipboard_history_fts
                 USING fts5(search_text, content='', contentless_delete=1)",
            )?;
            // 从主表重建 FTS 索引
            let mut stmt =
                conn.prepare("SELECT id, item_type, content, preview, badge FROM clipboard_history")?;
            let rows: Vec<(i64, String, String, String, String)> = stmt
                .query_map([], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                })?
                .filter_map(|r| r.ok())
                .collect();

            let mut insert = conn.prepare(
                "INSERT INTO clipboard_history_fts(rowid, search_text) VALUES (?1, ?2)",
            )?;
            for (id, item_type, content, preview, badge) in &rows {
                let kind = match item_type.as_str() {
                    "image" => ClipboardItemKind::Image,
                    "files" => ClipboardItemKind::Files,
                    _ => ClipboardItemKind::Text,
                };
                let search_text =
                    search_text_for_record(kind, content, preview, badge);
                insert.execute(params![id, search_text])?;
            }
        }
        Ok(())
    }

    fn sync_fts_for_content(
        &self,
        conn: &rusqlite::Connection,
        kind: ClipboardItemKind,
        content: &str,
    ) -> Result<()> {
        let record = self.query_optional(
            conn,
            SELECT_FTS_RECORD_SQL,
            params![kind.as_str(), content],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )?;
        let Some((id, preview, badge)) = record else {
            return Ok(());
        };
        let search_text = search_text_for_record(kind, content, &preview, &badge);
        conn.execute(UPSERT_FTS_ROW_SQL, params![id, search_text])?;
        Ok(())
    }

    fn prune_history(&self, conn: &rusqlite::Connection) -> Result<()> {
        let overflow = conn
            .query_row(
                COUNT_HISTORY_OVERFLOW_SQL,
                params![MAX_HISTORY_ITEMS],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0);
        if overflow <= 0 {
            return Ok(());
        }
        conn.execute(PRUNE_FTS_SQL, params![overflow])?;
        conn.execute(PRUNE_HISTORY_SQL, params![overflow])?;
        Ok(())
    }

    fn connection(&self) -> Result<PooledConnection> {
        self.pool
            .get()
            .context("cannot get clipboard pooled connection")
    }

    fn query_optional<T, P, F>(
        &self,
        conn: &rusqlite::Connection,
        sql: &str,
        params: P,
        f: F,
    ) -> Result<Option<T>>
    where
        P: rusqlite::Params,
        F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    {
        conn.query_row(sql, params, f)
            .optional()
            .map_err(Into::into)
    }
}

fn search_text_for_record(
    kind: ClipboardItemKind,
    content: &str,
    preview: &str,
    badge: &str,
) -> String {
    match kind {
        ClipboardItemKind::Files => {
            let names = parse_file_paths(content)
                .into_iter()
                .map(|path| {
                    Path::new(&path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(str::to_string)
                        .unwrap_or(path)
                })
                .collect::<Vec<_>>()
                .join(" ");
            format!("{preview} {names} {badge}")
        }
        _ => format!("{preview} {content} {badge}"),
    }
}

fn fts_query(query: &str) -> String {
    query
        .split_whitespace()
        .map(|term| {
            let escaped = term.replace('"', "\"\"");
            format!("\"{escaped}\"*")
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn should_ignore_text(content: &str, preview: &str, patterns: &[String]) -> bool {
    if patterns.is_empty() {
        return false;
    }

    let haystack = format!("{preview}\n{content}");
    let lowered = haystack.to_lowercase();
    for pattern in patterns {
        let pattern = pattern.trim();
        if pattern.is_empty() {
            continue;
        }
        match RegexBuilder::new(pattern).case_insensitive(true).build() {
            Ok(regex) => {
                if regex.is_match(&haystack) {
                    return true;
                }
            }
            Err(_) => {
                if lowered.contains(&pattern.to_lowercase()) {
                    return true;
                }
            }
        }
    }
    false
}

fn now_label() -> String {
    let fmt = format_description!("[month]-[day] [hour]:[minute]:[second]");
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .format(&fmt)
        .unwrap_or_else(|_| String::from("-- --:--:--"))
}
