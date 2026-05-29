use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use rusqlite::params;
use time::OffsetDateTime;

use crate::core::database::DatabaseService;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandUsage {
    pub use_count: i64,
    pub last_used_at: i64,
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
        })
    }

    pub fn record_launch(&self, command_key: &str) -> Result<()> {
        if command_key.trim().is_empty() {
            return Ok(());
        }

        self.database.with_registered_connection(&self.key, |conn| {
            ensure_schema(conn)?;
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
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::core::{database::DatabaseService, storage::AppPaths};

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
            .register_database(crate::core::database::DatabaseSpec::app(
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
    }
}
