use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::platform::apps::InstalledApp;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppIndexCache {
    pub apps: Vec<InstalledApp>,
    pub last_scan: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppLaunchUsage {
    pub use_count: i64,
    pub last_used_at: i64,
}

#[derive(Clone, Debug)]
pub struct AppIndexStore {
    path: PathBuf,
}

impl AppIndexStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn load(&self) -> Result<AppIndexCache> {
        let conn = self.open_connection()?;
        let last_scan = self.load_last_scan(&conn)?;
        let mut stmt = conn.prepare(
            "
            SELECT name, path, bundle_id, icon_path, aliases_json, icon_letter
            FROM app_index_entries
            ORDER BY sort_name ASC, name ASC
            ",
        )?;
        let rows = stmt.query_map([], map_app)?;
        let mut apps = Vec::new();
        for row in rows {
            apps.push(row?);
        }

        Ok(AppIndexCache { apps, last_scan })
    }

    pub fn save(&self, cache: &AppIndexCache) -> Result<()> {
        let mut conn = self.open_connection()?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM app_index_entries", [])?;

        {
            let mut stmt = tx.prepare(
                "
                INSERT INTO app_index_entries
                    (path, name, bundle_id, icon_path, aliases_json, icon_letter, sort_name, search_text)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ",
            )?;

            for app in &cache.apps {
                stmt.execute(params![
                    app.path,
                    app.name,
                    app.bundle_id,
                    app.icon_path,
                    serde_json::to_string(&app.aliases)?,
                    app.icon_letter,
                    app.name.to_lowercase(),
                    search_text(app),
                ])?;
            }
        }

        match cache.last_scan.as_ref() {
            Some(last_scan) => {
                tx.execute(
                    "
                    INSERT INTO app_index_meta (key, value)
                    VALUES ('last_scan', ?1)
                    ON CONFLICT(key) DO UPDATE SET value = excluded.value
                    ",
                    params![last_scan],
                )?;
            }
            None => {
                tx.execute("DELETE FROM app_index_meta WHERE key = 'last_scan'", [])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    #[cfg(test)]
    pub fn search_page(
        &self,
        query: &str,
        offset: usize,
        limit: usize,
    ) -> Result<crate::core::page::Page<InstalledApp>> {
        let conn = self.open_connection()?;
        let terms = query_terms(query);
        let (where_sql, params) = search_where_clause(&terms);

        let total = count_matches(&conn, &where_sql, &params)?;
        let rows = select_matches(&conn, &where_sql, &params, offset, limit, terms.is_empty())?;

        Ok(crate::core::page::Page {
            rows,
            total,
            offset,
            limit,
        })
    }

    pub fn record_launch(&self, app_path: &str) -> Result<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "
            INSERT INTO command_usage (command_key, use_count, last_used_at)
            VALUES (?1, 1, strftime('%s', 'now'))
            ON CONFLICT(command_key) DO UPDATE SET
                use_count = use_count + 1,
                last_used_at = excluded.last_used_at
            ",
            params![format!("app:{app_path}")],
        )?;
        Ok(())
    }

    pub fn usage_map(&self) -> Result<HashMap<String, AppLaunchUsage>> {
        let conn = self.open_connection()?;
        let mut stmt =
            conn.prepare("SELECT command_key, use_count, last_used_at FROM command_usage")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                AppLaunchUsage {
                    use_count: row.get(1)?,
                    last_used_at: row.get(2)?,
                },
            ))
        })?;

        let mut usage = HashMap::new();
        for row in rows {
            let (key, value) = row?;
            usage.insert(key, value);
        }
        Ok(usage)
    }

    fn open_connection(&self) -> Result<Connection> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("无法创建应用索引数据库目录 {}", parent.display()))?;
        }

        let conn = Connection::open(&self.path)
            .with_context(|| format!("无法打开应用索引数据库 {}", self.path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        ensure_schema(&conn)?;
        Ok(conn)
    }

    fn load_last_scan(&self, conn: &Connection) -> Result<Option<String>> {
        conn.query_row(
            "SELECT value FROM app_index_meta WHERE key = 'last_scan'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }
}

fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS app_index_entries (
            path TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            bundle_id TEXT,
            icon_path TEXT,
            aliases_json TEXT NOT NULL DEFAULT '[]',
            icon_letter TEXT NOT NULL DEFAULT 'A',
            sort_name TEXT NOT NULL,
            search_text TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_app_index_entries_sort
            ON app_index_entries(sort_name, name);

        CREATE INDEX IF NOT EXISTS idx_app_index_entries_search
            ON app_index_entries(search_text);

        CREATE TABLE IF NOT EXISTS app_index_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS command_usage (
            command_key TEXT PRIMARY KEY,
            use_count INTEGER NOT NULL DEFAULT 0,
            last_used_at INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_app_command_usage_recent
            ON command_usage(use_count DESC, last_used_at DESC);
        ",
    )?;
    Ok(())
}

#[cfg(test)]
fn count_matches(conn: &Connection, where_sql: &str, values: &[String]) -> Result<usize> {
    let sql = format!("SELECT COUNT(*) FROM app_index_entries {where_sql}");
    let total: i64 = conn.query_row(&sql, rusqlite::params_from_iter(values.iter()), |row| {
        row.get(0)
    })?;
    Ok(total.max(0) as usize)
}

#[cfg(test)]
fn select_matches(
    conn: &Connection,
    where_sql: &str,
    values: &[String],
    offset: usize,
    limit: usize,
    recommend: bool,
) -> Result<Vec<InstalledApp>> {
    let limit = limit.min(i64::MAX as usize);
    let offset = offset.min(i64::MAX as usize);
    let order_sql = if recommend {
        "
        ORDER BY COALESCE(usage.use_count, 0) DESC,
                 COALESCE(usage.last_used_at, 0) DESC,
                 sort_name ASC,
                 name ASC
        "
    } else {
        "ORDER BY sort_name ASC, name ASC"
    };
    let sql = format!(
        "
        SELECT name, path, bundle_id, icon_path, aliases_json, icon_letter
        FROM app_index_entries
        LEFT JOIN command_usage AS usage
            ON usage.command_key = 'app:' || app_index_entries.path
        {where_sql}
        {order_sql}
        LIMIT {limit} OFFSET {offset}
        "
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(values.iter()), map_app)?;
    let mut apps = Vec::new();
    for row in rows {
        apps.push(row?);
    }
    Ok(apps)
}

#[cfg(test)]
fn search_where_clause(terms: &[String]) -> (String, Vec<String>) {
    if terms.is_empty() {
        return (String::new(), Vec::new());
    }

    let clauses = terms
        .iter()
        .map(|_| "search_text LIKE ? ESCAPE '\\'")
        .collect::<Vec<_>>()
        .join(" AND ");
    let values = terms
        .iter()
        .map(|term| format!("%{}%", escape_like(term)))
        .collect();

    (format!("WHERE {clauses}"), values)
}

pub(super) fn query_terms(query: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let trimmed = query.trim().to_lowercase();
    if !trimmed.is_empty() {
        terms.push(trimmed);
    }

    let normalized = normalize_query(query);
    if !normalized.is_empty() && !terms.iter().any(|term| term == &normalized) {
        terms.push(normalized);
    }
    terms
}

pub(super) fn search_text(app: &InstalledApp) -> String {
    let mut values = vec![
        app.name.clone(),
        app.path.clone(),
        app.bundle_id.clone().unwrap_or_default(),
        normalize_query(&app.name),
        normalize_query(&app.path),
    ];
    if let Some(bundle_id) = app.bundle_id.as_ref() {
        values.push(normalize_query(bundle_id));
    }
    for alias in &app.aliases {
        values.push(alias.clone());
        values.push(normalize_query(alias));
    }
    values.join("\n").to_lowercase()
}

pub(super) fn normalize_query(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '-' | '_' | '.' | '/' | '\\'))
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn map_app(row: &rusqlite::Row<'_>) -> rusqlite::Result<InstalledApp> {
    let aliases_json: String = row.get(4)?;
    let aliases = serde_json::from_str(&aliases_json).unwrap_or_default();
    Ok(InstalledApp {
        name: row.get(0)?,
        path: row.get(1)?,
        bundle_id: row.get(2)?,
        icon_path: row.get(3)?,
        aliases,
        icon_letter: row.get(5)?,
    })
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-app-index-{nanos}"));
        let _ = fs::create_dir_all(&dir);
        dir.join(name)
    }

    fn sample_cache() -> AppIndexCache {
        AppIndexCache {
            apps: vec![
                InstalledApp {
                    name: String::from("Arc"),
                    path: String::from("/Applications/Arc.app"),
                    bundle_id: Some(String::from("company.thebrowser.Browser")),
                    icon_path: None,
                    aliases: vec![String::from("Arc Browser")],
                    icon_letter: String::from("A"),
                },
                InstalledApp {
                    name: String::from("Safari"),
                    path: String::from("/Applications/Safari.app"),
                    bundle_id: Some(String::from("com.apple.Safari")),
                    icon_path: Some(String::from("/tmp/safari.png")),
                    aliases: vec![String::from("Browser")],
                    icon_letter: String::from("S"),
                },
                InstalledApp {
                    name: String::from("Visual Studio Code"),
                    path: String::from("/Applications/Visual Studio Code.app"),
                    bundle_id: Some(String::from("com.microsoft.VSCode")),
                    icon_path: None,
                    aliases: vec![String::from("VS Code"), String::from("Code")],
                    icon_letter: String::from("V"),
                },
            ],
            last_scan: Some(String::from("2026-05-25 20:00:00")),
        }
    }

    fn cache_with_expanded_aliases() -> AppIndexCache {
        AppIndexCache {
            apps: vec![
                InstalledApp {
                    name: String::from("Visual Studio Code"),
                    path: String::from("/Applications/Visual Studio Code.app"),
                    bundle_id: Some(String::from("com.microsoft.VSCode")),
                    icon_path: None,
                    aliases: vec![
                        String::from("VS Code"),
                        String::from("Code"),
                        String::from("vscode"),
                        String::from("com.microsoft.VSCode"),
                        String::from("com.microsoft.vscode"),
                        String::from("VSCode"),
                    ],
                    icon_letter: String::from("V"),
                },
                InstalledApp {
                    name: String::from("Safari"),
                    path: String::from("/Applications/Safari.app"),
                    bundle_id: Some(String::from("com.apple.Safari")),
                    icon_path: None,
                    aliases: vec![
                        String::from("com.apple.Safari"),
                        String::from("comapplesafari"),
                        String::from("Safari"),
                    ],
                    icon_letter: String::from("S"),
                },
            ],
            last_scan: Some(String::from("2026-05-26 10:00:00")),
        }
    }

    #[test]
    fn load_missing_cache_as_default() {
        let path = temp_file("missing.db");
        let store = AppIndexStore::new(&path);
        let cache = store.load().expect("missing cache should load");
        assert!(cache.apps.is_empty());
        assert!(cache.last_scan.is_none());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let path = temp_file("index.db");
        let store = AppIndexStore::new(&path);
        let cache = sample_cache();

        store.save(&cache).expect("cache should save");
        let loaded = store.load().expect("cache should reload");
        assert_eq!(loaded, cache);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn search_page_matches_cached_database() {
        let path = temp_file("search.db");
        let store = AppIndexStore::new(&path);
        store.save(&sample_cache()).expect("cache should save");

        let page = store
            .search_page("browser", 1, 1)
            .expect("search should query database");
        assert_eq!(page.total, 2);
        assert_eq!(page.offset, 1);
        assert_eq!(page.limit, 1);
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0].name, "Safari");

        let normalized = store
            .search_page("vscode", 0, 10)
            .expect("normalized search should query database");
        assert_eq!(normalized.total, 1);
        assert_eq!(normalized.rows[0].name, "Visual Studio Code");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn search_finds_expanded_aliases() {
        let path = temp_file("expanded.db");
        let store = AppIndexStore::new(&path);
        store
            .save(&cache_with_expanded_aliases())
            .expect("cache should save");

        // Normalized bundle-id suffix should match
        let page = store
            .search_page("vscode", 0, 10)
            .expect("search should work");
        assert_eq!(page.total, 1);
        assert_eq!(page.rows[0].name, "Visual Studio Code");

        // Normalized full bundle id should match
        let page = store
            .search_page("commicrosoftvscode", 0, 10)
            .expect("bundle id search should work");
        assert_eq!(page.total, 1);
        assert_eq!(page.rows[0].name, "Visual Studio Code");

        // Normalized Safari bundle id should match
        let page = store
            .search_page("comapplesafari", 0, 10)
            .expect("safari bundle id search should work");
        assert_eq!(page.total, 1);
        assert_eq!(page.rows[0].name, "Safari");

        let _ = fs::remove_file(path);
    }
}
