use std::sync::Arc;

use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params};
use time::{OffsetDateTime, macros::format_description};

use crate::core::database::{DatabaseService, PooledConnection, SqlitePool};
use crate::features::http_capture::model::{CapturedExchange, FilterState};

pub const SCHEMA_VERSION: i64 = 1;
pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS schema_info (
    version INTEGER PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS captured_exchanges (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    method              TEXT NOT NULL DEFAULT '',
    url                 TEXT NOT NULL DEFAULT '',
    host                TEXT NOT NULL DEFAULT '',
    status              INTEGER NOT NULL DEFAULT 0,
    protocol            TEXT NOT NULL DEFAULT '',
    duration_ms         INTEGER NOT NULL DEFAULT 0,
    request_size        INTEGER NOT NULL DEFAULT 0,
    response_size       INTEGER NOT NULL DEFAULT 0,
    request_headers_json TEXT NOT NULL DEFAULT '[]',
    response_headers_json TEXT NOT NULL DEFAULT '[]',
    request_body        TEXT NOT NULL DEFAULT '',
    response_body       TEXT NOT NULL DEFAULT '',
    timestamp           TEXT NOT NULL DEFAULT '',
    is_https            INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_exchanges_timestamp
    ON captured_exchanges(timestamp);
CREATE INDEX IF NOT EXISTS idx_exchanges_host
    ON captured_exchanges(host);
CREATE INDEX IF NOT EXISTS idx_exchanges_method
    ON captured_exchanges(method);
CREATE INDEX IF NOT EXISTS idx_exchanges_status
    ON captured_exchanges(status);
";
pub const READ_SCHEMA_VERSION: &str = "SELECT COALESCE(MAX(version), 0) FROM schema_info";
pub const UPSERT_SCHEMA_VERSION: &str = "INSERT OR REPLACE INTO schema_info (version) VALUES (?1)";
pub const INSERT_EXCHANGE: &str = "
INSERT INTO captured_exchanges
     (method, url, host, status, protocol, duration_ms,
      request_size, response_size, request_headers_json,
      response_headers_json, request_body, response_body,
      timestamp, is_https)
 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
";
pub const GET_BY_ID: &str = "
SELECT id, method, url, host, status, protocol, duration_ms,
       request_size, response_size, request_headers_json,
       response_headers_json, request_body, response_body,
       timestamp, is_https
  FROM captured_exchanges WHERE id = ?1
";
pub const CLEAR_ALL: &str = "DELETE FROM captured_exchanges";
pub const TOTAL_COUNT: &str = "SELECT COUNT(*) FROM captured_exchanges";

pub fn query_exchanges_sql(where_clause: &str) -> String {
    format!(
        "SELECT id, method, url, host, status, protocol, duration_ms,
                request_size, response_size, request_headers_json,
                response_headers_json, request_body, response_body,
                timestamp, is_https
           FROM captured_exchanges
          {where_clause}
          ORDER BY id DESC
          LIMIT ? OFFSET ?"
    )
}

pub fn count_exchanges_sql(where_clause: &str) -> String {
    format!("SELECT COUNT(*) FROM captured_exchanges {where_clause}")
}

pub struct CaptureStore {
    pool: SqlitePool,
}

impl CaptureStore {
    pub fn open(database: Arc<DatabaseService>, key: &str) -> Result<Self> {
        let pool = database.pool(key)?;
        let store = Self { pool };
        store.ensure_schema()?;
        Ok(store)
    }

    fn ensure_schema(&self) -> Result<()> {
        let conn = self.connection()?;
        conn.execute_batch(SCHEMA)?;

        let version: i64 = conn.query_row(READ_SCHEMA_VERSION, [], |row| row.get(0))?;
        if version < SCHEMA_VERSION {
            conn.execute(UPSERT_SCHEMA_VERSION, params![SCHEMA_VERSION])?;
        }
        Ok(())
    }

    pub fn insert(
        &self,
        method: &str,
        url: &str,
        host: &str,
        status: i64,
        protocol: &str,
        duration_ms: i64,
        request_size: i64,
        response_size: i64,
        request_headers_json: &str,
        response_headers_json: &str,
        request_body: &str,
        response_body: &str,
        is_https: bool,
    ) -> Result<i64> {
        let conn = self.connection()?;
        let now = now_label();
        conn.execute(
            INSERT_EXCHANGE,
            params![
                method,
                url,
                host,
                status,
                protocol,
                duration_ms,
                request_size,
                response_size,
                request_headers_json,
                response_headers_json,
                request_body,
                response_body,
                now,
                if is_https { 1 } else { 0 },
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn query(
        &self,
        filter: &FilterState,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<CapturedExchange>> {
        let conn = self.connection()?;
        let (where_clause, method_val, host_val, status_val, search_val) =
            Self::build_filter(filter);

        let sql = query_exchanges_sql(&where_clause);
        let mut stmt = conn.prepare(&sql)?;
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref v) = method_val {
            params_vec.push(Box::new(v.clone()));
        }
        if let Some(ref v) = host_val {
            params_vec.push(Box::new(v.clone()));
        }
        if let Some(ref v) = status_val {
            params_vec.push(Box::new(*v));
        }
        if let Some(ref v) = search_val {
            params_vec.push(Box::new(v.clone()));
        }
        params_vec.push(Box::new(limit));
        params_vec.push(Box::new(offset));

        let param_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();
        let rows = stmt.query_map(param_refs.as_slice(), map_exchange)?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn count(&self, filter: &FilterState) -> Result<i64> {
        let conn = self.connection()?;
        let (where_clause, method_val, host_val, status_val, search_val) =
            Self::build_filter(filter);

        let sql = count_exchanges_sql(&where_clause);
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(ref v) = method_val {
            params_vec.push(Box::new(v.clone()));
        }
        if let Some(ref v) = host_val {
            params_vec.push(Box::new(v.clone()));
        }
        if let Some(ref v) = status_val {
            params_vec.push(Box::new(*v));
        }
        if let Some(ref v) = search_val {
            params_vec.push(Box::new(v.clone()));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();
        let count: i64 = conn.query_row(&sql, param_refs.as_slice(), |row| row.get(0))?;
        Ok(count)
    }

    pub fn get_by_id(&self, id: i64) -> Result<Option<CapturedExchange>> {
        let conn = self.connection()?;
        conn.query_row(GET_BY_ID, params![id], map_exchange)
            .optional()
            .map_err(Into::into)
    }

    pub fn clear(&self) -> Result<usize> {
        let conn = self.connection()?;
        let affected = conn.execute(CLEAR_ALL, [])?;
        Ok(affected)
    }

    pub fn total_count(&self) -> Result<i64> {
        let conn = self.connection()?;
        let count: i64 = conn.query_row(TOTAL_COUNT, [], |row| row.get(0))?;
        Ok(count)
    }

    fn build_filter(
        filter: &FilterState,
    ) -> (
        String,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<String>,
    ) {
        let mut conditions = Vec::new();
        let mut method_val = None;
        let mut host_val = None;
        let mut status_val = None;
        let mut search_val = None;

        if !filter.method.is_empty() && filter.method != "ALL" {
            conditions.push("method = ?".to_string());
            method_val = Some(filter.method.to_uppercase());
        }
        if !filter.host.is_empty() {
            conditions.push("host LIKE ?".to_string());
            host_val = Some(format!("%{}%", filter.host));
        }
        if !filter.status.is_empty() {
            if let Ok(num) = filter.status.parse::<i64>() {
                conditions.push("status = ?".to_string());
                status_val = Some(num);
            }
        }
        if !filter.search.is_empty() {
            conditions.push("url LIKE ?".to_string());
            search_val = Some(format!("%{}%", filter.search));
        }
        if filter.https_only {
            conditions.push("is_https = 1".to_string());
        }
        if filter.error_only {
            conditions.push("status >= 400".to_string());
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        (where_clause, method_val, host_val, status_val, search_val)
    }

    fn connection(&self) -> Result<PooledConnection> {
        self.pool
            .get()
            .context("cannot get capture pooled connection")
    }
}

fn map_exchange(row: &rusqlite::Row) -> std::result::Result<CapturedExchange, rusqlite::Error> {
    Ok(CapturedExchange {
        id: row.get(0)?,
        method: row.get(1)?,
        url: row.get(2)?,
        host: row.get(3)?,
        status: row.get(4)?,
        protocol: row.get(5)?,
        duration_ms: row.get(6)?,
        request_size: row.get(7)?,
        response_size: row.get(8)?,
        request_headers_json: row.get(9)?,
        response_headers_json: row.get(10)?,
        request_body: row.get(11)?,
        response_body: row.get(12)?,
        timestamp: row.get(13)?,
        is_https: row.get::<_, i64>(14)? != 0,
    })
}

fn now_label() -> String {
    let fmt = format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .format(&fmt)
        .unwrap_or_else(|_| String::from("1970-01-01 00:00:00"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{database::DatabaseService, storage::AppPaths};
    use std::{
        fs,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_db() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-capture-{nanos}"));
        let _ = fs::create_dir_all(&dir);
        dir.join("test.db")
    }

    fn open_store() -> CaptureStore {
        let path = temp_db();
        let database = Arc::new(DatabaseService::new(AppPaths::for_test(
            path.parent().unwrap().to_path_buf(),
        )));
        database
            .register_database(crate::core::database::DatabaseSpec::path(
                crate::core::database::feature_database_key("http-capture", "capture"),
                path,
            ))
            .unwrap();
        CaptureStore::open(
            database,
            &crate::core::database::feature_database_key("http-capture", "capture"),
        )
        .unwrap()
    }

    #[test]
    fn schema_creates_table() {
        let store = open_store();
        let conn = store.connection().unwrap();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert!(tables.contains(&"captured_exchanges".into()));
        assert!(tables.contains(&"schema_info".into()));
    }

    #[test]
    fn insert_and_get_by_id() {
        let store = open_store();
        let id = store
            .insert(
                "GET",
                "/api/users",
                "example.com",
                200,
                "HTTP/1.1",
                42,
                256,
                1024,
                r#"[["Accept","application/json"]]"#,
                r#"[["Content-Type","application/json"]]"#,
                "",
                r#"{"users":[]}"#,
                true,
            )
            .unwrap();
        assert!(id > 0);

        let exchange = store.get_by_id(id).unwrap().unwrap();
        assert_eq!(exchange.method, "GET");
        assert_eq!(exchange.url, "/api/users");
        assert_eq!(exchange.host, "example.com");
        assert_eq!(exchange.status, 200);
        assert_eq!(exchange.duration_ms, 42);
        assert_eq!(exchange.response_size, 1024);
        assert!(exchange.is_https);
    }

    #[test]
    fn query_with_filter() {
        let store = open_store();
        store
            .insert(
                "GET",
                "/a",
                "example.com",
                200,
                "HTTP/1.1",
                10,
                0,
                0,
                "[]",
                "[]",
                "",
                "",
                false,
            )
            .unwrap();
        store
            .insert(
                "POST",
                "/b",
                "example.com",
                201,
                "HTTP/1.1",
                20,
                0,
                0,
                "[]",
                "[]",
                "",
                "",
                false,
            )
            .unwrap();
        store
            .insert(
                "GET", "/err", "api.test", 500, "HTTP/2", 30, 0, 0, "[]", "[]", "", "", true,
            )
            .unwrap();

        let mut filter = FilterState::default();
        filter.method = String::from("GET");
        let rows = store.query(&filter, 0, 50).unwrap();
        assert_eq!(rows.len(), 2);

        filter.host = String::from("api.test");
        let rows = store.query(&filter, 0, 50).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, 500);

        filter.error_only = true;
        let count = store.count(&filter).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn clear_and_total_count() {
        let store = open_store();
        store
            .insert(
                "GET", "/one", "a", 200, "HTTP/1.1", 1, 1, 1, "[]", "[]", "", "", false,
            )
            .unwrap();
        store
            .insert(
                "GET", "/two", "a", 200, "HTTP/1.1", 1, 1, 1, "[]", "[]", "", "", false,
            )
            .unwrap();
        assert_eq!(store.total_count().unwrap(), 2);
        assert_eq!(store.clear().unwrap(), 2);
        assert_eq!(store.total_count().unwrap(), 0);
    }
}
