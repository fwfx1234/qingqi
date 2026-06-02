use std::sync::Arc;

use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params};
use time::{OffsetDateTime, macros::format_description};

use crate::mock_model::MockRule;
use qingqi_plugin::database::{DatabaseService, PooledConnection, SqlitePool};

pub const SCHEMA_VERSION: i64 = 1;
pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS schema_info (
    version INTEGER PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS mock_rules (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    name                TEXT NOT NULL DEFAULT '',
    enabled             INTEGER NOT NULL DEFAULT 1,
    match_url_pattern   TEXT NOT NULL DEFAULT '*',
    match_method        TEXT NOT NULL DEFAULT '',
    match_headers_json  TEXT NOT NULL DEFAULT '[]',
    action_status_code  INTEGER NOT NULL DEFAULT 200,
    action_headers_json TEXT NOT NULL DEFAULT '[]',
    action_body         TEXT NOT NULL DEFAULT '',
    action_delay_ms     INTEGER NOT NULL DEFAULT 0,
    sort_order          INTEGER NOT NULL DEFAULT 0,
    created_at          TEXT NOT NULL DEFAULT ''
);
";

const READ_SCHEMA_VERSION: &str = "SELECT COALESCE(MAX(version), 0) FROM schema_info";
const UPSERT_SCHEMA_VERSION: &str = "INSERT OR REPLACE INTO schema_info (version) VALUES (?1)";

const INSERT_RULE: &str = "
INSERT INTO mock_rules
    (name, enabled, match_url_pattern, match_method, match_headers_json,
     action_status_code, action_headers_json, action_body, action_delay_ms,
     sort_order, created_at)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
";

const UPDATE_RULE: &str = "
UPDATE mock_rules SET
    name = ?2, enabled = ?3, match_url_pattern = ?4, match_method = ?5,
    match_headers_json = ?6, action_status_code = ?7, action_headers_json = ?8,
    action_body = ?9, action_delay_ms = ?10, sort_order = ?11
WHERE id = ?1
";

const DELETE_RULE: &str = "DELETE FROM mock_rules WHERE id = ?1";
const GET_BY_ID: &str = "SELECT id, name, enabled, match_url_pattern, match_method, match_headers_json, action_status_code, action_headers_json, action_body, action_delay_ms, sort_order, created_at FROM mock_rules WHERE id = ?1";
const LIST_ENABLED: &str = "SELECT id, name, enabled, match_url_pattern, match_method, match_headers_json, action_status_code, action_headers_json, action_body, action_delay_ms, sort_order, created_at FROM mock_rules WHERE enabled = 1 ORDER BY sort_order ASC, id ASC";
const LIST_ALL: &str = "SELECT id, name, enabled, match_url_pattern, match_method, match_headers_json, action_status_code, action_headers_json, action_body, action_delay_ms, sort_order, created_at FROM mock_rules ORDER BY sort_order ASC, id ASC";
const SET_ENABLED: &str = "UPDATE mock_rules SET enabled = ?2 WHERE id = ?1";

fn map_rule(row: &rusqlite::Row) -> std::result::Result<MockRule, rusqlite::Error> {
    Ok(MockRule {
        id: row.get(0)?,
        name: row.get(1)?,
        enabled: row.get::<_, i64>(2)? != 0,
        match_url_pattern: row.get(3)?,
        match_method: row.get(4)?,
        match_headers_json: row.get(5)?,
        action_status_code: row.get(6)?,
        action_headers_json: row.get(7)?,
        action_body: row.get(8)?,
        action_delay_ms: row.get(9)?,
        sort_order: row.get(10)?,
        created_at: row.get(11)?,
    })
}

fn now_label() -> String {
    let fmt = format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .format(&fmt)
        .unwrap_or_else(|_| String::from("1970-01-01 00:00:00"))
}

pub struct MockStore {
    pool: SqlitePool,
}

impl MockStore {
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

    /// 插入新规则，返回 id。
    pub fn insert(&self, rule: &MockRule) -> Result<i64> {
        let conn = self.connection()?;
        let now = now_label();
        conn.execute(
            INSERT_RULE,
            params![
                rule.name,
                rule.enabled as i64,
                rule.match_url_pattern,
                rule.match_method,
                rule.match_headers_json,
                rule.action_status_code,
                rule.action_headers_json,
                rule.action_body,
                rule.action_delay_ms,
                rule.sort_order,
                now,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// 更新规则。
    pub fn update(&self, rule: &MockRule) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            UPDATE_RULE,
            params![
                rule.id,
                rule.name,
                rule.enabled as i64,
                rule.match_url_pattern,
                rule.match_method,
                rule.match_headers_json,
                rule.action_status_code,
                rule.action_headers_json,
                rule.action_body,
                rule.action_delay_ms,
                rule.sort_order,
            ],
        )?;
        Ok(())
    }

    /// 删除规则。
    pub fn delete(&self, id: i64) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(DELETE_RULE, params![id])?;
        Ok(())
    }

    /// 按 id 获取单条规则。
    pub fn get_by_id(&self, id: i64) -> Result<Option<MockRule>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(GET_BY_ID)?;
        let result = stmt
            .query_row(params![id], map_rule)
            .optional()
            .map_err(Into::into);
        result
    }

    /// 列出所有启用的规则（按 sort_order 排序）。
    pub fn list_enabled(&self) -> Result<Vec<MockRule>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(LIST_ENABLED)?;
        let rows = stmt.query_map([], map_rule)?;
        let mut rules = Vec::new();
        for row in rows {
            rules.push(row?);
        }
        Ok(rules)
    }

    /// 列出所有规则。
    pub fn list_all(&self) -> Result<Vec<MockRule>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(LIST_ALL)?;
        let rows = stmt.query_map([], map_rule)?;
        let mut rules = Vec::new();
        for row in rows {
            rules.push(row?);
        }
        Ok(rules)
    }

    /// 启用/禁用规则。
    pub fn set_enabled(&self, id: i64, enabled: bool) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(SET_ENABLED, params![id, enabled as i64])?;
        Ok(())
    }

    fn connection(&self) -> Result<PooledConnection> {
        self.pool
            .get()
            .context("无法获取 mock 数据库连接")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qingqi_plugin::{database::DatabaseService, storage::AppPaths};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_db() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-mock-{nanos}"));
        let _ = fs::create_dir_all(&dir);
        dir.join("test_mock.db")
    }

    fn open_store() -> MockStore {
        let path = temp_db();
        let database = Arc::new(DatabaseService::new(AppPaths::for_test(
            path.parent().unwrap().to_path_buf(),
        )));
        let key = qingqi_plugin::database::feature_database_key("http-capture", "mock");
        database
            .register_database(qingqi_plugin::database::DatabaseSpec::path(&key, path))
            .unwrap();
        MockStore::open(database, &key).unwrap()
    }

    #[test]
    fn insert_and_list() {
        let store = open_store();
        let rule = MockRule::new("测试规则", "*/api/*");
        let id = store.insert(&rule).unwrap();
        assert!(id > 0);

        let all = store.list_all().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "测试规则");
        assert!(all[0].enabled);
    }

    #[test]
    fn update_rule() {
        let store = open_store();
        let mut rule = MockRule::new("原始", "*");
        let id = store.insert(&rule).unwrap();
        rule.id = id;
        rule.name = "更新后".to_string();
        rule.action_status_code = 404;
        store.update(&rule).unwrap();

        let fetched = store.get_by_id(id).unwrap().unwrap();
        assert_eq!(fetched.name, "更新后");
        assert_eq!(fetched.action_status_code, 404);
    }

    #[test]
    fn delete_rule() {
        let store = open_store();
        let rule = MockRule::new("待删除", "*");
        let id = store.insert(&rule).unwrap();
        store.delete(id).unwrap();
        assert!(store.get_by_id(id).unwrap().is_none());
    }

    #[test]
    fn set_enabled() {
        let store = open_store();
        let rule = MockRule::new("切换", "*");
        let id = store.insert(&rule).unwrap();

        store.set_enabled(id, false).unwrap();
        let disabled = store.get_by_id(id).unwrap().unwrap();
        assert!(!disabled.enabled);

        // list_enabled 不应包含禁用的
        assert!(store.list_enabled().unwrap().is_empty());
    }

    #[test]
    fn list_enabled_sorted() {
        let store = open_store();

        let mut r1 = MockRule::new("第二个", "*/b/*");
        r1.sort_order = 2;
        store.insert(&r1).unwrap();

        let mut r2 = MockRule::new("第一个", "*/a/*");
        r2.sort_order = 1;
        store.insert(&r2).unwrap();

        let enabled = store.list_enabled().unwrap();
        assert_eq!(enabled.len(), 2);
        assert_eq!(enabled[0].name, "第一个"); // 按 sort_order
        assert_eq!(enabled[1].name, "第二个");
    }
}
