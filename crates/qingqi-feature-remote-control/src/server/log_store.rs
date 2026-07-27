//! 日志存储 - SQLite 实现

use qingqi_plugin::database::{DatabaseService, PooledConnection, SqlitePool};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 日志条目
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: i64,
    pub level: String,
    pub message: String,
    pub device: Option<String>,
}

/// 日志存储
pub struct LogStore {
    pool: SqlitePool,
}

const DB_KEY: &str = "remote-control/data";

impl LogStore {
    /// 创建日志存储
    pub fn new(database: Arc<DatabaseService>) -> Self {
        let pool = database.pool(DB_KEY).expect("无法获取数据库连接池");
        let store = Self { pool };
        store.ensure_schema();
        store
    }

    /// 确保表结构存在
    fn ensure_schema(&self) {
        let conn = self.connection().expect("无法获取数据库连接");
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                level TEXT NOT NULL,
                message TEXT NOT NULL,
                device TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_logs_timestamp ON logs(timestamp);
            "#,
        )
        .expect("创建 logs 表失败");
    }

    /// 获取数据库连接
    fn connection(&self) -> Result<PooledConnection, anyhow::Error> {
        self.pool.get().map_err(|e| anyhow::anyhow!(e))
    }

    /// 添加日志
    pub fn log(&self, level: &str, message: &str, device: Option<&str>) {
        if let Ok(conn) = self.connection() {
            let now = time::OffsetDateTime::now_utc().unix_timestamp();
            let _ = conn.execute(
                "INSERT INTO logs (timestamp, level, message, device) VALUES (?1, ?2, ?3, ?4)",
                params![now, level, message, device],
            );
        }
    }

    /// 信息日志
    pub fn info(&self, message: &str, device: Option<&str>) {
        self.log("info", message, device);
    }

    /// 警告日志
    pub fn warn(&self, message: &str, device: Option<&str>) {
        self.log("warn", message, device);
    }

    /// 错误日志
    pub fn error(&self, message: &str, device: Option<&str>) {
        self.log("error", message, device);
    }

    /// 获取日志列表
    pub fn list(&self) -> Vec<LogEntry> {
        let conn = match self.connection() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut stmt = match conn.prepare(
            "SELECT timestamp, level, message, device FROM logs ORDER BY timestamp DESC LIMIT 200",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let rows = match stmt.query_map([], |row| {
            Ok(LogEntry {
                timestamp: row.get(0)?,
                level: row.get(1)?,
                message: row.get(2)?,
                device: row.get(3)?,
            })
        }) {
            Ok(rows) => rows,
            Err(_) => return Vec::new(),
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// 清空日志
    pub fn clear(&self) {
        if let Ok(conn) = self.connection() {
            let _ = conn.execute("DELETE FROM logs", []);
        }
    }
}
