use std::path::{Path, PathBuf};

use anyhow::Result;
use regex::RegexBuilder;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, macros::format_description};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClipboardItemKind {
    Text,
    Image,
    Files,
}

impl ClipboardItemKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Files => "files",
        }
    }

    fn from_db(value: &str) -> Self {
        match value {
            "image" => Self::Image,
            "files" => Self::Files,
            _ => Self::Text,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipboardRecord {
    pub id: i64,
    pub kind: ClipboardItemKind,
    pub content: String,
    pub preview: String,
    pub pinned: bool,
    pub created_at: String,
    pub badge: String,
}

/// Classify text content into a primary badge label for filtering.
/// Returns one of: "链接", "邮箱", "JSON", "多行", or "".
pub fn classify_text(text: &str) -> &'static str {
    let value = text.trim();
    if value.is_empty() {
        return "";
    }
    if is_url(value) {
        "链接"
    } else if is_email(value) {
        "邮箱"
    } else if looks_like_json(value) {
        "JSON"
    } else if value.contains('\n') {
        "多行"
    } else {
        ""
    }
}

/// Returns all text badges for display.
pub fn text_badges(text: &str) -> Vec<&'static str> {
    let value = text.trim();
    if value.is_empty() {
        return vec![];
    }
    let mut badges: Vec<&'static str> = Vec::new();
    let lowered = value.to_lowercase();
    if is_url(value) {
        badges.push("链接");
    } else if is_email(value) {
        badges.push("邮箱");
    } else if looks_like_json(value) {
        badges.push("JSON");
    } else if value.contains('\n') {
        badges.push("多行");
    }
    if any_sensitive(&lowered) {
        badges.push("敏感");
    }
    badges.truncate(3);
    badges
}

/// Returns a display string for text statistics (char count, line count).
pub fn text_stats(text: &str) -> String {
    let chars = text.chars().count();
    let lines = if text.is_empty() {
        0
    } else {
        text.lines().count()
    };
    if lines > 1 {
        format!("{chars} 字符 · {lines} 行")
    } else {
        format!("{chars} 字符")
    }
}

fn is_url(text: &str) -> bool {
    text.starts_with("http://") || text.starts_with("https://")
}

fn is_email(text: &str) -> bool {
    if let Some(at_pos) = text.find('@') {
        at_pos > 0 && at_pos < text.len() - 1 && text[at_pos + 1..].contains('.')
    } else {
        false
    }
}

fn looks_like_json(text: &str) -> bool {
    let value = text.trim();
    if !((value.starts_with('{') && value.ends_with('}'))
        || (value.starts_with('[') && value.ends_with(']')))
    {
        return false;
    }
    serde_json::from_str::<serde_json::Value>(value).is_ok()
}

fn any_sensitive(lowered: &str) -> bool {
    for token in &["password", "token", "secret", "apikey", "api_key"] {
        if lowered.contains(token) {
            return true;
        }
    }
    false
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClipboardConfig {
    pub capture_text: bool,
    pub capture_image: bool,
    pub capture_files: bool,
    pub max_text_chars: usize,
    pub ignore_patterns: Vec<String>,
    pub hotkey: String,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            capture_text: true,
            capture_image: true,
            capture_files: true,
            max_text_chars: 20_000,
            ignore_patterns: Vec::new(),
            hotkey: String::from("Alt+V"),
        }
    }
}

pub struct ClipboardHistoryStore {
    conn: Connection,
}

impl ClipboardHistoryStore {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let store = Self { conn };
        store.ensure_schema()?;
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
        if !config.capture_image {
            return Ok(false);
        }
        if image_path.trim().is_empty() {
            return Ok(false);
        }
        self.add_item(ClipboardItemKind::Image, image_path, preview, badge)
    }

    /// Store a file list record. `paths` are serialized as a JSON array into
    /// the `content` column. The preview is generated from file names.
    pub fn add_files(&self, paths: &[String], config: &ClipboardConfig) -> Result<bool> {
        if !config.capture_files {
            return Ok(false);
        }
        if paths.is_empty() {
            return Ok(false);
        }
        let content = serde_json::to_string(paths)?;
        let preview = file_list_preview(paths);
        self.add_item(ClipboardItemKind::Files, &content, &preview, "文件")
    }

    pub fn load_config(&self) -> Result<ClipboardConfig> {
        let config = self
            .conn
            .query_row(
                "SELECT capture_text, capture_image, capture_files, max_text_chars, ignore_patterns_json, hotkey
                 FROM clipboard_config WHERE id = 1",
                [],
                |row| {
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
                },
            )
            .optional()?;

        Ok(config.unwrap_or_default())
    }

    pub fn save_config(&self, config: &ClipboardConfig) -> Result<()> {
        self.conn.execute(
            "INSERT INTO clipboard_config
                (id, capture_text, capture_image, capture_files, max_text_chars, ignore_patterns_json, hotkey, updated_at)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
               capture_text = excluded.capture_text,
               capture_image = excluded.capture_image,
               capture_files = excluded.capture_files,
               max_text_chars = excluded.max_text_chars,
               ignore_patterns_json = excluded.ignore_patterns_json,
               hotkey = excluded.hotkey,
               updated_at = excluded.updated_at",
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

    pub fn add_item(
        &self,
        kind: ClipboardItemKind,
        content: &str,
        preview: &str,
        badge: &str,
    ) -> Result<bool> {
        let latest: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT item_type, content FROM clipboard_history ORDER BY id DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        if latest
            .as_ref()
            .is_some_and(|(latest_kind, latest_content)| {
                latest_kind == kind.as_str() && latest_content == content
            })
        {
            return Ok(false);
        }

        let existing_pinned = self
            .conn
            .query_row(
                "SELECT pinned FROM clipboard_history WHERE item_type = ?1 AND content = ?2 ORDER BY id DESC LIMIT 1",
                params![kind.as_str(), content],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0);

        self.conn.execute(
            "DELETE FROM clipboard_history WHERE item_type = ?1 AND content = ?2",
            params![kind.as_str(), content],
        )?;

        self.conn.execute(
            "INSERT INTO clipboard_history (item_type, content, preview, pinned, created_at, badge) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                kind.as_str(),
                content,
                preview,
                existing_pinned,
                now_label(),
                badge,
            ],
        )?;
        Ok(true)
    }

    pub fn search(
        &self,
        query: &str,
        filter: crate::features::clipboard::service::ClipboardFilter,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<ClipboardRecord>> {
        let q = query.trim();
        let kind_filter = filter.kind();
        let badge_filter = filter.badge_filter();
        let pinned_only = filter.pinned_only();

        let mut conditions: Vec<String> = Vec::new();
        let mut base_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(kind) = kind_filter {
            conditions.push("item_type = ?".into());
            base_params.push(Box::new(kind.as_str().to_string()));
        }

        if pinned_only {
            conditions.push("pinned = 1".into());
        }

        if let Some(badge) = badge_filter {
            conditions.push("badge = ?".into());
            base_params.push(Box::new(badge.to_string()));
        }

        if !q.is_empty() {
            conditions.push("(content LIKE ? OR preview LIKE ?)".into());
            let q_pattern = format!("%{q}%");
            base_params.push(Box::new(q_pattern.clone()));
            base_params.push(Box::new(q_pattern));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT id, item_type, content, preview, pinned, created_at, badge
             FROM clipboard_history{where_clause}
             ORDER BY pinned DESC, id DESC
             LIMIT ? OFFSET ?"
        );

        let mut all_params: Vec<Box<dyn rusqlite::types::ToSql>> = base_params;
        all_params.push(Box::new(limit as i64));
        all_params.push(Box::new(offset as i64));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            all_params.iter().map(|p| p.as_ref()).collect();
        self.query_records_dyn(&sql, &param_refs)
    }

    pub fn search_all(
        &self,
        query: &str,
        filter: crate::features::clipboard::service::ClipboardFilter,
    ) -> Result<Vec<ClipboardRecord>> {
        let q = query.trim();
        let kind_filter = filter.kind();
        let badge_filter = filter.badge_filter();
        let pinned_only = filter.pinned_only();

        let mut conditions: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(kind) = kind_filter {
            conditions.push("item_type = ?".into());
            params.push(Box::new(kind.as_str().to_string()));
        }

        if pinned_only {
            conditions.push("pinned = 1".into());
        }

        if let Some(badge) = badge_filter {
            conditions.push("badge = ?".into());
            params.push(Box::new(badge.to_string()));
        }

        if !q.is_empty() {
            conditions.push("(content LIKE ? OR preview LIKE ?)".into());
            let q_pattern = format!("%{q}%");
            params.push(Box::new(q_pattern.clone()));
            params.push(Box::new(q_pattern));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT id, item_type, content, preview, pinned, created_at, badge
             FROM clipboard_history{where_clause}
             ORDER BY pinned DESC, id DESC"
        );

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        self.query_records_dyn(&sql, &param_refs)
    }

    pub fn latest(&self) -> Result<Option<ClipboardRecord>> {
        let rows = self.query_records_dyn(
            "SELECT id, item_type, content, preview, pinned, created_at, badge
             FROM clipboard_history
             ORDER BY id DESC
             LIMIT 1",
            &[],
        )?;
        Ok(rows.into_iter().next())
    }

    fn query_records_dyn(
        &self,
        sql: &str,
        params: &[&dyn rusqlite::types::ToSql],
    ) -> Result<Vec<ClipboardRecord>> {
        let mut stmt = self.conn.prepare(sql)?;
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

    pub fn toggle_pin(&self, id: i64) -> Result<Option<bool>> {
        let pinned = self
            .conn
            .query_row(
                "SELECT pinned FROM clipboard_history WHERE id = ?1",
                params![id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        let Some(pinned) = pinned else {
            return Ok(None);
        };
        let next = if pinned == 0 { 1 } else { 0 };
        self.conn.execute(
            "UPDATE clipboard_history SET pinned = ?1 WHERE id = ?2",
            params![next, id],
        )?;
        Ok(Some(next == 1))
    }

    pub fn delete(&self, id: i64) -> Result<bool> {
        let affected = self
            .conn
            .execute("DELETE FROM clipboard_history WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn clear_all(&self) -> Result<usize> {
        self.conn
            .execute("DELETE FROM clipboard_history", [])
            .map_err(Into::into)
    }

    pub fn clear_unpinned(&self) -> Result<usize> {
        self.conn
            .execute("DELETE FROM clipboard_history WHERE pinned = 0", [])
            .map_err(Into::into)
    }

    fn ensure_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "
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
            ",
        )?;
        let _ = self.conn.execute(
            "ALTER TABLE clipboard_history ADD COLUMN badge TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE clipboard_config ADD COLUMN capture_image INTEGER NOT NULL DEFAULT 1",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE clipboard_config ADD COLUMN capture_files INTEGER NOT NULL DEFAULT 1",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE clipboard_config ADD COLUMN ignore_patterns_json TEXT NOT NULL DEFAULT '[]'",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE clipboard_config ADD COLUMN hotkey TEXT NOT NULL DEFAULT 'Alt+V'",
            [],
        );
        Ok(())
    }
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

pub fn compact_preview(text: &str) -> String {
    let preview = text.split_whitespace().collect::<Vec<_>>().join(" ");
    const LIMIT: usize = 160;
    if preview.chars().count() <= LIMIT {
        return preview;
    }
    let mut out = preview
        .chars()
        .take(LIMIT.saturating_sub(3))
        .collect::<String>();
    out.push_str("...");
    out
}

/// Generate a preview string for a file list: show up to 3 file names, with count.
pub fn file_list_preview(paths: &[String]) -> String {
    let names: Vec<&str> = paths
        .iter()
        .filter_map(|p| Path::new(p).file_name().and_then(|n| n.to_str()))
        .take(3)
        .collect();
    if names.is_empty() {
        return format!("{} 个文件", paths.len());
    }
    let mut preview = names.join(", ");
    if paths.len() > 3 {
        preview.push_str(&format!(" ... (+{})", paths.len() - 3));
    }
    preview
}

/// Parse file paths from a JSON array string stored in clipboard content.
pub fn parse_file_paths(content: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(content).unwrap_or_default()
}

/// Find the first directory that can be opened from a list of file paths.
///
/// For each path, returns the existing file's parent directory (if the file
/// exists), or the parent directory if it exists even though the file was
/// moved. Returns `None` when no path's parent directory is accessible.
pub fn find_first_actionable_path(paths: &[String]) -> Option<PathBuf> {
    for raw in paths {
        let path = Path::new(raw);
        if path.is_dir() && path.exists() {
            return Some(path.to_path_buf());
        }
        if path.exists() {
            // File exists — return its parent directory so the user can see it
            return path.parent().filter(|p| p.exists()).map(Path::to_path_buf);
        }
        if let Some(parent) = path.parent() {
            if parent.exists() {
                return Some(parent.to_path_buf());
            }
        }
    }
    None
}

/// Find the first path in a file list that still exists on disk.
pub fn find_first_existing_path(paths: &[String]) -> Option<PathBuf> {
    paths.iter().find_map(|raw| {
        let path = Path::new(raw);
        path.exists().then(|| path.to_path_buf())
    })
}

/// Per-path inspection result used by the clipboard detail pane to render
/// truthful file rows. Touches the filesystem once per path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePathState {
    pub path: String,
    pub display_name: String,
    pub exists: bool,
    pub is_dir: bool,
    pub parent_exists: bool,
}

impl FilePathState {
    /// Returns true if this entry can be revealed in Finder (i.e. the file or
    /// directory still exists on disk).
    pub fn can_reveal(&self) -> bool {
        self.exists
    }

    /// Returns true if there is *some* meaningful action: either the entry
    /// itself or at least its parent directory still exists.
    pub fn has_actionable_target(&self) -> bool {
        self.exists || self.parent_exists
    }
}

/// Inspect each path in a file list for existence/kind/parent info so the UI
/// can render truthful per-row state without scattering `Path::new(...)` calls
/// across `view/`.
pub fn file_path_states(paths: &[String]) -> Vec<FilePathState> {
    paths
        .iter()
        .map(|raw| {
            let path = Path::new(raw);
            let exists = path.exists();
            let is_dir = exists && path.is_dir();
            let parent_exists = path.parent().is_some_and(Path::exists);
            let display_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .map(String::from)
                .unwrap_or_else(|| raw.clone());
            FilePathState {
                path: raw.clone(),
                display_name,
                exists,
                is_dir,
                parent_exists,
            }
        })
        .collect()
}

fn now_label() -> String {
    let fmt = format_description!("[month]-[day] [hour]:[minute]:[second]");
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .format(&fmt)
        .unwrap_or_else(|_| String::from("-- --:--:--"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_db(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-clipboard-store-{nanos}"));
        let _ = fs::create_dir_all(&dir);
        dir.join(name)
    }

    #[test]
    fn config_roundtrip_persists_values() {
        let path = temp_db("config.db");
        let store = ClipboardHistoryStore::open(&path).expect("store should open");
        let config = ClipboardConfig {
            capture_text: false,
            capture_image: false,
            capture_files: false,
            max_text_chars: 512,
            ignore_patterns: vec![String::from("secret"), String::from("^token:")],
            hotkey: String::from("Ctrl+Shift+V"),
        };

        store.save_config(&config).expect("config should save");
        let loaded = store.load_config().expect("config should load");
        assert_eq!(loaded, config);
    }

    #[test]
    fn add_text_respects_capture_settings() {
        let path = temp_db("capture.db");
        let store = ClipboardHistoryStore::open(&path).expect("store should open");

        let disabled = ClipboardConfig {
            capture_text: false,
            capture_image: true,
            capture_files: true,
            max_text_chars: 200,
            ignore_patterns: Vec::new(),
            hotkey: String::from("Alt+V"),
        };
        assert!(
            !store
                .add_text("hello", &disabled)
                .expect("disabled capture should not insert")
        );

        let limited = ClipboardConfig {
            capture_text: true,
            capture_image: true,
            capture_files: true,
            max_text_chars: 4,
            ignore_patterns: Vec::new(),
            hotkey: String::from("Alt+V"),
        };
        assert!(
            !store
                .add_text("hello", &limited)
                .expect("oversized text should be skipped")
        );

        let enabled = ClipboardConfig {
            capture_text: true,
            capture_image: true,
            capture_files: true,
            max_text_chars: 32,
            ignore_patterns: Vec::new(),
            hotkey: String::from("Alt+V"),
        };
        assert!(
            store
                .add_text("hello", &enabled)
                .expect("enabled capture should insert")
        );
        let rows = store
            .search(
                "",
                crate::features::clipboard::service::ClipboardFilter::All,
                0,
                10,
            )
            .expect("rows should load");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].content, "hello");
    }

    #[test]
    fn add_text_respects_ignore_patterns() {
        let path = temp_db("ignore.db");
        let store = ClipboardHistoryStore::open(&path).expect("store should open");

        let config = ClipboardConfig {
            capture_text: true,
            capture_image: true,
            capture_files: true,
            max_text_chars: 200,
            ignore_patterns: vec![String::from("secret"), String::from("^token:")],
            hotkey: String::from("Alt+V"),
        };

        assert!(
            !store
                .add_text("contains secret data", &config)
                .expect("substring ignore should skip")
        );
        assert!(
            !store
                .add_text("token: abc", &config)
                .expect("regex ignore should skip")
        );
        assert!(
            store
                .add_text("harmless", &config)
                .expect("non-matching text should insert")
        );
    }

    #[test]
    fn pinned_filter_only_returns_pinned_rows() {
        let path = temp_db("pinned-filter.db");
        let store = ClipboardHistoryStore::open(&path).expect("store should open");
        let config = ClipboardConfig::default();

        assert!(
            store
                .add_text("first item", &config)
                .expect("first row should insert")
        );
        assert!(
            store
                .add_text("second item", &config)
                .expect("second row should insert")
        );

        let all_rows = store
            .search(
                "",
                crate::features::clipboard::service::ClipboardFilter::All,
                0,
                10,
            )
            .expect("all rows should load");
        assert_eq!(all_rows.len(), 2);

        let toggled = store
            .toggle_pin(all_rows[0].id)
            .expect("pin toggle should succeed");
        assert_eq!(toggled, Some(true));

        let pinned_rows = store
            .search(
                "",
                crate::features::clipboard::service::ClipboardFilter::Pinned,
                0,
                10,
            )
            .expect("pinned rows should load");
        assert_eq!(pinned_rows.len(), 1);
        assert!(pinned_rows[0].pinned);
        assert_eq!(pinned_rows[0].content, all_rows[0].content);
    }

    #[test]
    fn latest_uses_insertion_order_not_pin_order() {
        let path = temp_db("latest.db");
        let store = ClipboardHistoryStore::open(&path).expect("store should open");
        let config = ClipboardConfig::default();

        assert!(
            store
                .add_text("first item", &config)
                .expect("first row should insert")
        );
        let first = store
            .latest()
            .expect("latest should load")
            .expect("first latest should exist");
        store.toggle_pin(first.id).expect("pin should toggle");

        assert!(
            store
                .add_text("second item", &config)
                .expect("second row should insert")
        );

        let latest = store
            .latest()
            .expect("latest should load")
            .expect("latest should exist");
        assert_eq!(latest.content, "second item");
    }

    #[test]
    fn file_list_preview_shows_names_and_count() {
        assert_eq!(
            file_list_preview(&[
                String::from("/Users/me/Documents/report.pdf"),
                String::from("/Users/me/Desktop/notes.txt"),
            ]),
            "report.pdf, notes.txt"
        );
        assert_eq!(
            file_list_preview(&[
                String::from("/a/x.txt"),
                String::from("/b/y.txt"),
                String::from("/c/z.txt"),
                String::from("/d/w.txt"),
            ]),
            "x.txt, y.txt, z.txt ... (+1)"
        );
        assert_eq!(file_list_preview(&[]), "0 个文件");
    }

    #[test]
    fn parse_file_paths_roundtrips_json() {
        let paths = vec![
            String::from("/Users/me/file1.txt"),
            String::from("/Users/me/file2.png"),
        ];
        let json = serde_json::to_string(&paths).unwrap();
        let parsed = parse_file_paths(&json);
        assert_eq!(parsed, paths);
    }

    #[test]
    fn parse_file_paths_handles_invalid_input() {
        assert!(parse_file_paths("not json").is_empty());
        assert!(parse_file_paths("\"just a string\"").is_empty());
        assert!(parse_file_paths("[1, 2, 3]").is_empty());
    }

    #[test]
    fn add_files_stores_and_searches() {
        let path = temp_db("files.db");
        let store = ClipboardHistoryStore::open(&path).expect("store should open");
        let config = ClipboardConfig::default();

        let paths = vec![
            String::from("/Users/me/Documents/report.pdf"),
            String::from("/Users/me/Desktop/notes.txt"),
        ];
        assert!(
            store
                .add_files(&paths, &config)
                .expect("add_files should insert")
        );

        let rows = store
            .search(
                "",
                crate::features::clipboard::service::ClipboardFilter::Files,
                0,
                10,
            )
            .expect("rows should load");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, ClipboardItemKind::Files);
        assert_eq!(rows[0].badge, "文件");
        assert!(rows[0].preview.contains("report.pdf"));
        assert!(rows[0].preview.contains("notes.txt"));

        // Content should be valid JSON array
        let parsed = parse_file_paths(&rows[0].content);
        assert_eq!(parsed, paths);
    }

    #[test]
    fn add_files_respects_capture_settings() {
        let path = temp_db("files-disabled.db");
        let store = ClipboardHistoryStore::open(&path).expect("store should open");
        let disabled = ClipboardConfig {
            capture_files: false,
            ..Default::default()
        };
        assert!(
            !store
                .add_files(&[String::from("/tmp/test.txt")], &disabled)
                .expect("disabled should not insert")
        );
    }

    #[test]
    fn add_files_deduplicates_same_paths() {
        let path = temp_db("files-dedupe.db");
        let store = ClipboardHistoryStore::open(&path).expect("store should open");
        let config = ClipboardConfig::default();

        let paths = vec![String::from("/tmp/file.txt")];
        assert!(store.add_files(&paths, &config).expect("first insert"));
        // Same content should be deduped
        assert!(
            !store
                .add_files(&paths, &config)
                .expect("duplicate should skip")
        );
    }

    #[test]
    fn find_first_actionable_path_returns_dir_for_existing_file() {
        let test_dir =
            std::env::temp_dir().join(format!("qingqi-clipboard-fap-{}", std::process::id()));
        fs::create_dir_all(&test_dir).unwrap();
        let file = test_dir.join("real.txt");
        fs::write(&file, b"data").unwrap();

        let paths = vec![file.to_string_lossy().to_string()];
        let result = find_first_actionable_path(&paths);
        assert_eq!(result, Some(test_dir.clone()));

        fs::remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn find_first_actionable_path_finds_parent_when_file_missing() {
        let test_dir =
            std::env::temp_dir().join(format!("qingqi-clipboard-fap2-{}", std::process::id()));
        fs::create_dir_all(&test_dir).unwrap();
        let missing = test_dir.join("gone.txt");

        let paths = vec![missing.to_string_lossy().to_string()];
        let result = find_first_actionable_path(&paths);
        assert_eq!(result, Some(test_dir.clone()));

        fs::remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn find_first_actionable_path_returns_none_when_nothing_exists() {
        let paths = vec![String::from("/nonexistent/qingqi/test/file.xyz")];
        assert!(find_first_actionable_path(&paths).is_none());
    }

    #[test]
    fn find_first_actionable_path_returns_first_actionable() {
        let test_dir =
            std::env::temp_dir().join(format!("qingqi-clipboard-fap3-{}", std::process::id()));
        fs::create_dir_all(&test_dir).unwrap();
        let missing = test_dir.join("gone.txt");
        let real = test_dir.join("real.txt");
        fs::write(&real, b"data").unwrap();

        // First path is missing, second exists
        let paths = vec![
            missing.to_string_lossy().to_string(),
            real.to_string_lossy().to_string(),
        ];
        let result = find_first_actionable_path(&paths);
        // Should return test_dir (parent of the first actionable file)
        assert_eq!(result, Some(test_dir.clone()));

        fs::remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn find_first_existing_path_returns_existing_file() {
        let test_dir =
            std::env::temp_dir().join(format!("qingqi-clipboard-fep-{}", std::process::id()));
        fs::create_dir_all(&test_dir).unwrap();
        let real = test_dir.join("real.txt");
        fs::write(&real, b"data").unwrap();

        let paths = vec![
            String::from("/nonexistent/file.txt"),
            real.to_string_lossy().to_string(),
        ];
        let result = find_first_existing_path(&paths);
        assert_eq!(result, Some(real.clone()));

        fs::remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn find_first_existing_path_returns_none_when_all_missing() {
        let paths = vec![
            String::from("/nonexistent/a.txt"),
            String::from("/nonexistent/b.txt"),
        ];
        assert!(find_first_existing_path(&paths).is_none());
    }

    #[test]
    fn file_path_states_marks_missing_paths_honestly() {
        let test_dir =
            std::env::temp_dir().join(format!("qingqi-clipboard-fps1-{}", std::process::id()));
        fs::create_dir_all(&test_dir).unwrap();
        let existing_file = test_dir.join("present.txt");
        fs::write(&existing_file, b"x").unwrap();
        let missing_in_existing_dir = test_dir.join("gone.txt");

        let paths = vec![
            existing_file.to_string_lossy().to_string(),
            missing_in_existing_dir.to_string_lossy().to_string(),
            String::from("/nonexistent/parent/orphan.txt"),
        ];
        let states = file_path_states(&paths);
        assert_eq!(states.len(), 3);

        assert!(states[0].exists);
        assert!(!states[0].is_dir);
        assert!(states[0].parent_exists);
        assert_eq!(states[0].display_name, "present.txt");
        assert!(states[0].can_reveal());
        assert!(states[0].has_actionable_target());

        assert!(!states[1].exists);
        assert!(!states[1].is_dir);
        assert!(states[1].parent_exists);
        assert!(!states[1].can_reveal());
        assert!(states[1].has_actionable_target());

        assert!(!states[2].exists);
        assert!(!states[2].parent_exists);
        assert!(!states[2].can_reveal());
        assert!(!states[2].has_actionable_target());

        fs::remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn file_path_states_recognizes_directories() {
        let test_dir =
            std::env::temp_dir().join(format!("qingqi-clipboard-fps2-{}", std::process::id()));
        fs::create_dir_all(&test_dir).unwrap();

        let paths = vec![test_dir.to_string_lossy().to_string()];
        let states = file_path_states(&paths);
        assert_eq!(states.len(), 1);
        assert!(states[0].exists);
        assert!(states[0].is_dir);
        assert!(states[0].parent_exists);

        fs::remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn file_path_states_falls_back_to_raw_for_unnameable_path() {
        let paths = vec![String::from("/")];
        let states = file_path_states(&paths);
        assert_eq!(states.len(), 1);
        // root path has no `file_name`, so display_name falls back to the raw value
        assert_eq!(states[0].display_name, "/");
    }
}
