use std::{fs, path::Path};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use time::{OffsetDateTime, macros::format_description};

use crate::features::http_capture::model::{CapturedExchange, FilterState};

const SCHEMA_VERSION: i64 = 1;

pub struct CaptureStore {
    conn: Connection,
}

impl CaptureStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("无法创建抓包数据目录 {}", parent.display()))?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let store = Self { conn };
        store.ensure_schema()?;
        Ok(store)
    }

    fn ensure_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "
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
            ",
        )?;

        let version: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_info",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if version < SCHEMA_VERSION {
            self.conn.execute(
                "INSERT OR REPLACE INTO schema_info (version) VALUES (?1)",
                params![SCHEMA_VERSION],
            )?;
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
        let now = now_label();
        self.conn.execute(
            "INSERT INTO captured_exchanges
                 (method, url, host, status, protocol, duration_ms,
                  request_size, response_size, request_headers_json,
                  response_headers_json, request_body, response_body,
                  timestamp, is_https)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
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
        Ok(self.conn.last_insert_rowid())
    }

    pub fn query(
        &self,
        filter: &FilterState,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<CapturedExchange>> {
        let (where_clause, method_val, host_val, status_val, search_val) =
            Self::build_filter(filter);

        let sql = format!(
            "SELECT id, method, url, host, status, protocol, duration_ms,
                    request_size, response_size, request_headers_json,
                    response_headers_json, request_body, response_body,
                    timestamp, is_https
               FROM captured_exchanges
              {where_clause}
              ORDER BY id DESC
              LIMIT ? OFFSET ?"
        );

        let mut stmt = self.conn.prepare(&sql)?;
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
        let (where_clause, method_val, host_val, status_val, search_val) =
            Self::build_filter(filter);

        let sql = format!("SELECT COUNT(*) FROM captured_exchanges {where_clause}");

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

        let count: i64 = self
            .conn
            .query_row(&sql, param_refs.as_slice(), |row| row.get(0))?;
        Ok(count)
    }

    pub fn get_by_id(&self, id: i64) -> Result<Option<CapturedExchange>> {
        self.conn
            .query_row(
                "SELECT id, method, url, host, status, protocol, duration_ms,
                        request_size, response_size, request_headers_json,
                        response_headers_json, request_body, response_body,
                        timestamp, is_https
                   FROM captured_exchanges WHERE id = ?1",
                params![id],
                map_exchange,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn clear(&self) -> Result<usize> {
        let affected = self.conn.execute("DELETE FROM captured_exchanges", [])?;
        Ok(affected)
    }

    pub fn total_count(&self) -> Result<i64> {
        let count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM captured_exchanges", [], |row| {
                    row.get(0)
                })?;
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
    use std::{
        fs,
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
        CaptureStore::open(&temp_db()).unwrap()
    }

    #[test]
    fn schema_creates_table() {
        let store = open_store();
        let tables: Vec<String> = store
            .conn
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
                "GET",
                "/c",
                "other.com",
                404,
                "HTTP/1.1",
                30,
                0,
                0,
                "[]",
                "[]",
                "",
                "",
                true,
            )
            .unwrap();

        let all = store.query(&FilterState::default(), 0, 100).unwrap();
        assert_eq!(all.len(), 3);

        let get_only = store
            .query(
                &FilterState {
                    method: "GET".to_string(),
                    ..Default::default()
                },
                0,
                100,
            )
            .unwrap();
        assert_eq!(get_only.len(), 2);

        let host_filter = store
            .query(
                &FilterState {
                    host: "other".to_string(),
                    ..Default::default()
                },
                0,
                100,
            )
            .unwrap();
        assert_eq!(host_filter.len(), 1);
        assert_eq!(host_filter[0].host, "other.com");

        let https_only = store
            .query(
                &FilterState {
                    https_only: true,
                    ..Default::default()
                },
                0,
                100,
            )
            .unwrap();
        assert_eq!(https_only.len(), 1);
    }

    #[test]
    fn pagination() {
        let store = open_store();
        for i in 0..10 {
            store
                .insert(
                    "GET",
                    &format!("/{i}"),
                    "example.com",
                    200,
                    "HTTP/1.1",
                    0,
                    0,
                    0,
                    "[]",
                    "[]",
                    "",
                    "",
                    false,
                )
                .unwrap();
        }

        let page1 = store.query(&FilterState::default(), 0, 3).unwrap();
        assert_eq!(page1.len(), 3);

        let page2 = store.query(&FilterState::default(), 3, 3).unwrap();
        assert_eq!(page2.len(), 3);

        let page4 = store.query(&FilterState::default(), 9, 3).unwrap();
        assert_eq!(page4.len(), 1);
    }

    #[test]
    fn count_and_total_count() {
        let store = open_store();
        store
            .insert(
                "GET",
                "/a",
                "example.com",
                200,
                "HTTP/1.1",
                0,
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
                500,
                "HTTP/1.1",
                0,
                0,
                0,
                "[]",
                "[]",
                "",
                "",
                false,
            )
            .unwrap();

        assert_eq!(store.total_count().unwrap(), 2);
        assert_eq!(store.count(&FilterState::default()).unwrap(), 2);

        let error_filter = FilterState {
            error_only: true,
            ..Default::default()
        };
        assert_eq!(store.count(&error_filter).unwrap(), 1);
    }

    #[test]
    fn clear_removes_all() {
        let store = open_store();
        store
            .insert(
                "GET",
                "/a",
                "example.com",
                200,
                "HTTP/1.1",
                0,
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
                "GET",
                "/b",
                "example.com",
                200,
                "HTTP/1.1",
                0,
                0,
                0,
                "[]",
                "[]",
                "",
                "",
                false,
            )
            .unwrap();
        assert_eq!(store.total_count().unwrap(), 2);
        let cleared = store.clear().unwrap();
        assert_eq!(cleared, 2);
        assert_eq!(store.total_count().unwrap(), 0);
    }

    #[test]
    fn query_ordered_by_id_desc() {
        let store = open_store();
        store
            .insert(
                "GET", "/first", "a.com", 200, "HTTP/1.1", 0, 0, 0, "[]", "[]", "", "", false,
            )
            .unwrap();
        store
            .insert(
                "GET", "/second", "a.com", 200, "HTTP/1.1", 0, 0, 0, "[]", "[]", "", "", false,
            )
            .unwrap();

        let results = store.query(&FilterState::default(), 0, 10).unwrap();
        assert_eq!(results.len(), 2);
        // Most recent first (highest id)
        assert_eq!(results[0].url, "/second");
        assert_eq!(results[1].url, "/first");
    }
}
