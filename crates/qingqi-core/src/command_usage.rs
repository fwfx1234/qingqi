use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use rusqlite::params;
use time::OffsetDateTime;

use qingqi_plugin::database::DatabaseService;

/// 半衰期约 28 天（Firefox Places 同款）。
const DECAY_PER_DAY: f64 = 0.975;
const SECONDS_PER_DAY: f64 = 86_400.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommandUsage {
    pub use_count: i64,
    pub last_used_at: i64,
    /// 预计算的衰减评分，每次写入时更新。
    /// 公式：new = old × λ^Δdays + 1.0
    pub frecency: f64,
}

impl CommandUsage {
    /// 基于上次写入的时间和当前时间，计算新的 frecency 值。
    pub fn next_frecency(old_frecency: f64, old_last_used_at: i64) -> f64 {
        let now = time::OffsetDateTime::now_utc().unix_timestamp() as f64;
        let days = ((now - old_last_used_at as f64) / SECONDS_PER_DAY).max(0.0);
        old_frecency * DECAY_PER_DAY.powf(days) + 1.0
    }
}

#[derive(Clone, Debug)]
pub struct CommandUsageStore {
    database: Arc<DatabaseService>,
    key: String,
}

impl CommandUsageStore {
    pub fn new(database: Arc<DatabaseService>, key: impl Into<String>) -> Self {
        Self {
            database,
            key: key.into(),
        }
    }

    pub fn usage_map(&self) -> Result<HashMap<String, CommandUsage>> {
        self.database.with_registered_connection(&self.key, |conn| {
            ensure_schema(conn)?;
            let mut stmt = conn.prepare(
                "SELECT command_key, use_count, last_used_at, frecency FROM command_usage",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    CommandUsage {
                        use_count: row.get(1)?,
                        last_used_at: row.get(2)?,
                        frecency: row.get::<_, f64>(3).unwrap_or(0.0),
                    },
                ))
            })?;

            let mut usage = HashMap::new();
            for row in rows {
                let (key, value) = row?;
                usage.insert(key, value);
            }
            Ok(usage)
        })
    }

    pub fn record_launch(&self, command_key: &str) -> Result<()> {
        if command_key.trim().is_empty() {
            return Ok(());
        }

        self.database.with_registered_connection(&self.key, |conn| {
            ensure_schema(conn)?;
            let now = OffsetDateTime::now_utc().unix_timestamp();
            // Read existing values to compute new frecency
            let (old_frecency, old_last_used_at) = conn
                .query_row(
                    "SELECT frecency, last_used_at FROM command_usage WHERE command_key = ?1",
                    params![command_key],
                    |row| {
                        Ok((
                            row.get::<_, f64>(0).unwrap_or(0.0),
                            row.get::<_, i64>(1).unwrap_or(0),
                        ))
                    },
                )
                .unwrap_or((0.0, 0));
            let new_frecency = CommandUsage::next_frecency(old_frecency, old_last_used_at);
            conn.execute(
                "INSERT INTO command_usage (command_key, use_count, last_used_at, frecency)
                 VALUES (?1, 1, ?2, ?3)
                 ON CONFLICT(command_key) DO UPDATE SET
                     use_count = use_count + 1,
                     last_used_at = excluded.last_used_at,
                     frecency = excluded.frecency",
                params![command_key, now, new_frecency],
            )?;
            Ok(())
        })
    }
}

fn ensure_schema(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS command_usage (
            command_key TEXT PRIMARY KEY,
            use_count INTEGER NOT NULL DEFAULT 0,
            last_used_at INTEGER NOT NULL DEFAULT 0
        );
        ",
    )?;
    ensure_frecency_column(conn)?;
    backfill_frecency(conn)?;
    conn.execute_batch(
        "
        CREATE INDEX IF NOT EXISTS idx_command_usage_frecency
            ON command_usage(frecency DESC);
        ",
    )?;
    Ok(())
}

fn ensure_frecency_column(conn: &rusqlite::Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(command_usage)")?;
    let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "frecency" {
            return Ok(());
        }
    }

    conn.execute_batch("ALTER TABLE command_usage ADD COLUMN frecency REAL NOT NULL DEFAULT 0.0;")?;
    Ok(())
}

fn backfill_frecency(conn: &rusqlite::Connection) -> Result<()> {
    let now = OffsetDateTime::now_utc().unix_timestamp() as f64;
    let rows = {
        let mut stmt = conn.prepare(
            "SELECT command_key, use_count, last_used_at
             FROM command_usage
             WHERE frecency <= 0.0 AND use_count > 0",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        let mut collected = Vec::new();
        for row in rows {
            collected.push(row?);
        }
        collected
    };

    for (command_key, use_count, last_used_at) in rows {
        let frecency = if last_used_at > 0 {
            let days = ((now - last_used_at as f64) / SECONDS_PER_DAY).max(0.0);
            use_count as f64 * DECAY_PER_DAY.powf(days)
        } else {
            use_count as f64
        };
        conn.execute(
            "UPDATE command_usage SET frecency = ?2 WHERE command_key = ?1",
            params![command_key, frecency],
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use qingqi_plugin::{database::DatabaseService, storage::AppPaths};

    fn temp_paths() -> AppPaths {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-command-usage-{nanos}"));
        let _ = fs::create_dir_all(&dir);
        AppPaths::for_test(dir)
    }

    #[test]
    fn record_launch_accumulates_usage() {
        let paths = temp_paths();
        let database = Arc::new(DatabaseService::new(paths.clone()));
        database
            .register_database(qingqi_plugin::database::DatabaseSpec::app(
                "command-usage",
                "usage.db",
            ))
            .unwrap();
        let store = CommandUsageStore::new(database, "command-usage");

        store.record_launch("plugin:json-parser").unwrap();
        store.record_launch("plugin:json-parser").unwrap();

        let usage = store.usage_map().unwrap();
        let stats = usage.get("plugin:json-parser").unwrap();
        assert_eq!(stats.use_count, 2);
        assert!(stats.last_used_at > 0);
        assert!(stats.frecency > 0.0, "frecency should be computed on write");
    }

    #[test]
    fn legacy_usage_rows_are_backfilled_with_frecency() {
        let paths = temp_paths();
        let database = Arc::new(DatabaseService::new(paths.clone()));
        database
            .register_database(qingqi_plugin::database::DatabaseSpec::app(
                "command-usage",
                "usage.db",
            ))
            .unwrap();
        let now = OffsetDateTime::now_utc().unix_timestamp();
        database
            .with_registered_connection("command-usage", |conn| {
                conn.execute_batch(
                    "
                    CREATE TABLE command_usage (
                        command_key TEXT PRIMARY KEY,
                        use_count INTEGER NOT NULL DEFAULT 0,
                        last_used_at INTEGER NOT NULL DEFAULT 0
                    );
                    ",
                )?;
                conn.execute(
                    "INSERT INTO command_usage (command_key, use_count, last_used_at)
                     VALUES (?1, ?2, ?3)",
                    params!["plugin:legacy", 5_i64, now],
                )?;
                Ok(())
            })
            .unwrap();
        let store = CommandUsageStore::new(database, "command-usage");

        let usage = store.usage_map().unwrap();

        let stats = usage.get("plugin:legacy").unwrap();
        assert_eq!(stats.use_count, 5);
        assert!(stats.frecency > 4.9);
    }
}
