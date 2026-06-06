use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use rusqlite::params;
use time::OffsetDateTime;

use qingqi_plugin::database::DatabaseService;

/// 半衰期 7 天：DECAY_PER_DAY = 0.5^(1/7)。每过一周 frecency 减半，
/// 久未使用的高频项会比 28 天半衰期下沉得更快。
const DECAY_PER_DAY: f64 = 0.905_723_664_263_435_5;
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

    /// 排序时的“有效” frecency：在存储值基础上，对“上次使用至今”再衰减一次。
    ///
    /// 存储的 frecency 只反映上次启动那一刻的值，不会随时间下降；若直接用它排序，
    /// 一个很久前高频使用、之后再没碰过的项会长期霸榜。这里按 `λ^Δdays` 把它衰减
    /// 到当前时刻，等价于“假如现在启动会得到 effective + 1”，从而让最近常用项浮上来。
    ///
    /// `effective = frecency × λ^Δdays`，`Δdays = max(0, (now - last_used_at) / 86400)`。
    /// 无记录（`last_used_at <= 0`）时直接返回存储的 frecency（通常为 0.0）。
    pub fn effective_frecency(&self, now_unix: i64) -> f64 {
        if self.last_used_at <= 0 {
            return self.frecency;
        }
        let days = ((now_unix as f64 - self.last_used_at as f64) / SECONDS_PER_DAY).max(0.0);
        self.frecency * DECAY_PER_DAY.powf(days)
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

    const DAY: i64 = 86_400;
    const NOW: i64 = 1_700_000_000;

    fn usage(frecency: f64, last_used_at: i64) -> CommandUsage {
        CommandUsage {
            use_count: 1,
            last_used_at,
            frecency,
        }
    }

    #[test]
    fn effective_frecency_no_record_returns_raw() {
        assert_eq!(usage(0.0, 0).effective_frecency(NOW), 0.0);
        assert_eq!(usage(5.0, 0).effective_frecency(NOW), 5.0);
    }

    #[test]
    fn effective_frecency_halves_each_week() {
        let one_week = usage(10.0, NOW - 7 * DAY).effective_frecency(NOW);
        assert!((one_week - 5.0).abs() < 1e-6, "got {one_week}");
        let two_weeks = usage(10.0, NOW - 14 * DAY).effective_frecency(NOW);
        assert!((two_weeks - 2.5).abs() < 1e-6, "got {two_weeks}");
    }

    #[test]
    fn effective_frecency_recent_does_not_decay() {
        let value = usage(7.0, NOW).effective_frecency(NOW);
        assert!((value - 7.0).abs() < 1e-6, "got {value}");
    }

    #[test]
    fn effective_frecency_clamps_clock_skew() {
        // last_used_at 在未来（时钟回拨）时 Δdays 钳为 0，不放大评分。
        let value = usage(7.0, NOW + DAY).effective_frecency(NOW);
        assert!((value - 7.0).abs() < 1e-6, "got {value}");
    }

    #[test]
    fn recent_use_beats_old_high_frequency() {
        // 需求核心：很久前高频（frecency 40，21 天前）应低于近期低频（frecency 8，刚用过）。
        let recent = usage(8.0, NOW).effective_frecency(NOW);
        let old_heavy = usage(40.0, NOW - 21 * DAY).effective_frecency(NOW); // 40 × 0.5^3 = 5.0
        assert!(
            recent > old_heavy,
            "recent {recent} should outrank old-heavy {old_heavy}"
        );
    }
}
