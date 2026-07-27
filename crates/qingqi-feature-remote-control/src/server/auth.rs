//! Token 认证存储 - SQLite 实现

use std::sync::Arc;

use rusqlite::params;
use uuid::Uuid;

use qingqi_plugin::database::{DatabaseService, PooledConnection, SqlitePool};

/// Token 信息
#[derive(Clone, Debug)]
pub struct TokenInfo {
    pub device_name: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub permanent: bool,
}

/// Token 存储
pub struct TokenStore {
    pool: SqlitePool,
}

const DB_KEY: &str = "remote-control/data";

impl TokenStore {
    /// 创建 TokenStore
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
            CREATE TABLE IF NOT EXISTS tokens (
                token TEXT PRIMARY KEY,
                device_name TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                permanent INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_tokens_device ON tokens(device_name);
            "#,
        )
        .expect("创建 tokens 表失败");
    }

    /// 获取数据库连接
    fn connection(&self) -> Result<PooledConnection, anyhow::Error> {
        self.pool.get().map_err(|e| anyhow::anyhow!(e))
    }

    /// 创建 Token
    pub fn create_token(&self, device_name: &str, ttl_seconds: i64) -> String {
        let permanent = ttl_seconds <= 0;
        let token = Uuid::new_v4().to_string();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let expires_at = if permanent { i64::MAX } else { now + ttl_seconds };

        if let Ok(conn) = self.connection() {
            let _ = conn.execute(
                "INSERT INTO tokens (token, device_name, created_at, expires_at, permanent) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![&token, device_name, now, expires_at, if permanent { 1 } else { 0 }],
            );
        }
        token
    }

    /// 创建永久 Token
    pub fn create_permanent_token(&self, device_name: &str) -> String {
        self.create_token(device_name, 0)
    }

    /// 验证 Token
    pub fn validate(&self, token: &str) -> bool {
        let conn = match self.connection() {
            Ok(c) => c,
            Err(_) => return false,
        };

        let result: Result<(i64, i64), _> = conn.query_row(
            "SELECT expires_at, permanent FROM tokens WHERE token = ?1",
            [token],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );

        match result {
            Ok((expires_at, permanent)) => {
                if permanent == 1 {
                    true
                } else {
                    let now = time::OffsetDateTime::now_utc().unix_timestamp();
                    expires_at > now
                }
            }
            Err(_) => false,
        }
    }

    /// 撤销 Token
    pub fn revoke(&self, token: &str) -> bool {
        if let Ok(conn) = self.connection() {
            let affected = conn
                .execute("DELETE FROM tokens WHERE token = ?1", [token])
                .unwrap_or(0);
            affected > 0
        } else {
            false
        }
    }

    /// 清理过期 Token
    pub fn cleanup(&self) {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        if let Ok(conn) = self.connection() {
            let _ = conn.execute(
                "DELETE FROM tokens WHERE permanent = 0 AND expires_at < ?1",
                [now],
            );
        }
    }

    /// 列出所有有效 Token
    pub fn list_active(&self) -> Vec<(String, TokenInfo)> {
        let conn = match self.connection() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let mut stmt = match conn.prepare(
            "SELECT token, device_name, created_at, expires_at, permanent FROM tokens WHERE permanent = 1 OR expires_at > ?1",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let rows = match stmt.query_map([now], |row| {
            Ok((
                row.get::<_, String>(0)?,
                TokenInfo {
                    device_name: row.get(1)?,
                    created_at: row.get(2)?,
                    expires_at: row.get(3)?,
                    permanent: row.get::<_, i64>(4)? == 1,
                },
            ))
        }) {
            Ok(rows) => rows,
            Err(_) => return Vec::new(),
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// 按设备名撤销 Token
    pub fn revoke_by_name(&self, device_name: &str) -> bool {
        if let Ok(conn) = self.connection() {
            let affected = conn
                .execute("DELETE FROM tokens WHERE device_name = ?1", [device_name])
                .unwrap_or(0);
            affected > 0
        } else {
            false
        }
    }
}
