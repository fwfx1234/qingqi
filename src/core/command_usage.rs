use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use time::OffsetDateTime;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandUsage {
    pub use_count: i64,
    pub last_used_at: i64,
}

#[derive(Clone, Debug)]
pub struct CommandUsageStore {
    path: PathBuf,
}

impl CommandUsageStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn usage_map(&self) -> Result<HashMap<String, CommandUsage>> {
        let conn = self.open_connection()?;
        let mut stmt =
            conn.prepare("SELECT command_key, use_count, last_used_at FROM command_usage")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                CommandUsage {
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

    pub fn record_launch(&self, command_key: &str) -> Result<()> {
        if command_key.trim().is_empty() {
            return Ok(());
        }

        let conn = self.open_connection()?;
        conn.execute(
            "
            INSERT INTO command_usage (command_key, use_count, last_used_at)
            VALUES (?1, 1, ?2)
            ON CONFLICT(command_key) DO UPDATE SET
                use_count = use_count + 1,
                last_used_at = excluded.last_used_at
            ",
            params![command_key, OffsetDateTime::now_utc().unix_timestamp()],
        )?;
        Ok(())
    }

    fn open_connection(&self) -> Result<Connection> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("无法创建命令使用记录数据库目录 {}", parent.display()))?;
        }

        let conn = Connection::open(&self.path)
            .with_context(|| format!("无法打开命令使用记录数据库 {}", self.path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        ensure_schema(&conn)?;
        Ok(conn)
    }
}

fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS command_usage (
            command_key TEXT PRIMARY KEY,
            use_count INTEGER NOT NULL DEFAULT 0,
            last_used_at INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_command_usage_recent
            ON command_usage(use_count DESC, last_used_at DESC);
        ",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-command-usage-{nanos}"));
        let _ = fs::create_dir_all(&dir);
        dir.join(name)
    }

    #[test]
    fn record_launch_accumulates_usage() {
        let path = temp_file("usage.db");
        let store = CommandUsageStore::new(&path);

        store.record_launch("plugin:json-parser").unwrap();
        store.record_launch("plugin:json-parser").unwrap();

        let usage = store.usage_map().unwrap();
        let stats = usage.get("plugin:json-parser").unwrap();
        assert_eq!(stats.use_count, 2);
        assert!(stats.last_used_at > 0);

        let _ = fs::remove_file(path);
    }
}
