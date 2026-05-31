use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use qingqi_plugin::app::AppEntry;
use qingqi_plugin::database::{DatabaseService, SqlitePool};

pub const LOAD_LAST_SCAN: &str = "SELECT value FROM app_index_meta WHERE key = 'last_scan'";

pub const LOAD_APPS: &str = "
SELECT name, path, bundle_id, icon_path, aliases_json, icon_letter
FROM app_index_entries
ORDER BY sort_name ASC, name ASC
";

pub const DELETE_ENTRIES: &str = "DELETE FROM app_index_entries";

pub const INSERT_ENTRY: &str = "
INSERT INTO app_index_entries
    (path, name, bundle_id, icon_path, aliases_json, icon_letter, sort_name, search_text)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
";

pub const UPSERT_LAST_SCAN: &str = "
INSERT INTO app_index_meta (key, value)
VALUES ('last_scan', ?1)
ON CONFLICT(key) DO UPDATE SET value = excluded.value
";

pub const DELETE_LAST_SCAN: &str = "DELETE FROM app_index_meta WHERE key = 'last_scan'";

pub const RECORD_LAUNCH: &str = "
INSERT INTO command_usage (command_key, use_count, last_used_at)
VALUES (?1, 1, strftime('%s', 'now'))
ON CONFLICT(command_key) DO UPDATE SET
    use_count = use_count + 1,
    last_used_at = excluded.last_used_at
";

pub const LOAD_USAGE: &str = "SELECT command_key, use_count, last_used_at FROM command_usage";

pub const SCHEMA: &str = "
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
";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppIndexCache {
    pub apps: Vec<AppEntry>,
    pub last_scan: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppLaunchUsage {
    pub use_count: i64,
    pub last_used_at: i64,
}

#[derive(Clone, Debug)]
pub struct AppIndexStore {
    pool: SqlitePool,
}

impl AppIndexStore {
    pub fn new(database: Arc<DatabaseService>, key: &str) -> Self {
        Self {
            pool: database
                .pool(key)
                .expect("app index database should be registered"),
        }
    }

    pub fn load(&self) -> Result<AppIndexCache> {
        let conn = self.connection()?;
        ensure_schema(&conn)?;
        let last_scan = self.load_last_scan(&conn)?;
        let mut stmt = conn.prepare(LOAD_APPS)?;
        let rows = stmt.query_map([], map_app)?;
        let mut apps = Vec::new();
        for row in rows {
            apps.push(row?);
        }
        Ok(AppIndexCache { apps, last_scan })
    }

    pub fn save(&self, cache: &AppIndexCache) -> Result<()> {
        let conn = self.connection()?;
        ensure_schema(&conn)?;
        let tx = conn.unchecked_transaction()?;
        tx.execute(DELETE_ENTRIES, [])?;

        {
            let mut stmt = tx.prepare(INSERT_ENTRY)?;
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
                tx.execute(UPSERT_LAST_SCAN, params![last_scan])?;
            }
            None => {
                tx.execute(DELETE_LAST_SCAN, [])?;
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
    ) -> Result<qingqi_plugin::page::Page<AppEntry>> {
        let conn = self.connection()?;
        ensure_schema(&conn)?;
        let terms = query_terms(query);
        let (where_sql, params) = search_where_clause(&terms);
        let total = count_matches(&conn, &where_sql, &params)?;
        let rows = select_matches(&conn, &where_sql, &params, offset, limit, terms.is_empty())?;
        Ok(qingqi_plugin::page::Page {
            rows,
            total,
            offset,
            limit,
        })
    }

    pub fn record_launch(&self, app_path: &str) -> Result<()> {
        let conn = self.connection()?;
        ensure_schema(&conn)?;
        conn.execute(RECORD_LAUNCH, params![format!("app:{app_path}")])?;
        Ok(())
    }

    pub fn usage_map(&self) -> Result<HashMap<String, AppLaunchUsage>> {
        let conn = self.connection()?;
        ensure_schema(&conn)?;
        let mut stmt = conn.prepare(LOAD_USAGE)?;
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

    fn connection(&self) -> Result<qingqi_plugin::database::PooledConnection> {
        self.pool.get().map_err(Into::into)
    }

    fn load_last_scan(&self, conn: &rusqlite::Connection) -> Result<Option<String>> {
        conn.query_row(LOAD_LAST_SCAN, [], |row| row.get(0))
            .optional()
            .map_err(Into::into)
    }
}

fn ensure_schema(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)?;
    Ok(())
}

#[cfg(test)]
fn count_matches(conn: &rusqlite::Connection, where_sql: &str, values: &[String]) -> Result<usize> {
    let sql = format!("SELECT COUNT(*) FROM app_index_entries {where_sql}");
    let total: i64 = conn.query_row(&sql, rusqlite::params_from_iter(values.iter()), |row| {
        row.get(0)
    })?;
    Ok(total.max(0) as usize)
}

#[cfg(test)]
fn select_matches(
    conn: &rusqlite::Connection,
    where_sql: &str,
    values: &[String],
    offset: usize,
    limit: usize,
    recommend: bool,
) -> Result<Vec<AppEntry>> {
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

pub(super) fn search_text(app: &AppEntry) -> String {
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

fn map_app(row: &rusqlite::Row<'_>) -> rusqlite::Result<AppEntry> {
    let aliases_json: String = row.get(4)?;
    let aliases = serde_json::from_str(&aliases_json).unwrap_or_default();
    Ok(AppEntry {
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
    use std::{
        fs,
        path::PathBuf,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use qingqi_plugin::{database::DatabaseService, storage::AppPaths};

    fn temp_file(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-app-index-{nanos}"));
        let _ = fs::create_dir_all(&dir);
        dir.join(name)
    }

    fn store_for_path(path: &PathBuf) -> AppIndexStore {
        let paths = AppPaths::for_test(path.parent().unwrap().to_path_buf());
        let database = Arc::new(DatabaseService::new(paths));
        database
            .register_database(qingqi_plugin::database::DatabaseSpec::path(
                "app-launcher/index",
                path.clone(),
            ))
            .unwrap();
        AppIndexStore::new(database, "app-launcher/index")
    }

    fn sample_cache() -> AppIndexCache {
        AppIndexCache {
            apps: vec![
                AppEntry {
                    name: String::from("Arc"),
                    path: String::from("/Applications/Arc.app"),
                    bundle_id: Some(String::from("company.thebrowser.Browser")),
                    icon_path: None,
                    aliases: vec![String::from("Arc Browser")],
                    icon_letter: String::from("A"),
                },
                AppEntry {
                    name: String::from("Safari"),
                    path: String::from("/Applications/Safari.app"),
                    bundle_id: Some(String::from("com.apple.Safari")),
                    icon_path: Some(String::from("/tmp/safari.png")),
                    aliases: vec![String::from("Browser")],
                    icon_letter: String::from("S"),
                },
                AppEntry {
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
                AppEntry {
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
                AppEntry {
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
        let store = store_for_path(&path);
        let cache = store.load().expect("missing cache should load");
        assert!(cache.apps.is_empty());
        assert!(cache.last_scan.is_none());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let path = temp_file("index.db");
        let store = store_for_path(&path);
        let cache = sample_cache();

        store.save(&cache).expect("cache should save");
        let loaded = store.load().expect("cache should reload");
        assert_eq!(loaded, cache);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn search_page_matches_cached_database() {
        let path = temp_file("search.db");
        let store = store_for_path(&path);
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
        let store = store_for_path(&path);
        store
            .save(&cache_with_expanded_aliases())
            .expect("cache should save");

        let page = store
            .search_page("vscode", 0, 10)
            .expect("search should work");
        assert_eq!(page.total, 1);
        assert_eq!(page.rows[0].name, "Visual Studio Code");

        let page = store
            .search_page("commicrosoftvscode", 0, 10)
            .expect("bundle id search should work");
        assert_eq!(page.total, 1);
        assert_eq!(page.rows[0].name, "Visual Studio Code");

        let page = store
            .search_page("comapplesafari", 0, 10)
            .expect("safari bundle id search should work");
        assert_eq!(page.total, 1);
        assert_eq!(page.rows[0].name, "Safari");

        let _ = fs::remove_file(path);
    }
}
