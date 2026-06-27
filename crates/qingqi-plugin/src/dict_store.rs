use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::database::DatabaseService;

const TABLE_NAME: &str = "plugin_dict_entries";

#[derive(Clone)]
pub struct PluginDictStore {
    database: DatabaseService,
    path: PathBuf,
}

impl PluginDictStore {
    pub fn new(database: DatabaseService, path: impl Into<PathBuf>) -> Self {
        Self {
            database,
            path: path.into(),
        }
    }

    pub fn for_database(database: DatabaseService, name: &str) -> Self {
        let path = database.paths().database(name);
        Self::new(database, path)
    }

    pub fn get_bool(&self, namespace: &str, key: &str) -> Result<Option<bool>> {
        self.get_typed(namespace, key)?
            .map(|(kind, value)| decode_bool(&kind, &value))
            .transpose()
    }

    pub fn set_bool(&self, namespace: &str, key: &str, value: bool) -> Result<()> {
        self.set_typed(namespace, key, "bool", if value { "1" } else { "0" })
    }

    pub fn get_i64(&self, namespace: &str, key: &str) -> Result<Option<i64>> {
        self.get_typed(namespace, key)?
            .map(|(kind, value)| decode_i64(&kind, &value))
            .transpose()
    }

    pub fn set_i64(&self, namespace: &str, key: &str, value: i64) -> Result<()> {
        self.set_typed(namespace, key, "i64", &value.to_string())
    }

    pub fn get_u64(&self, namespace: &str, key: &str) -> Result<Option<u64>> {
        self.get_typed(namespace, key)?
            .map(|(kind, value)| decode_u64(&kind, &value))
            .transpose()
    }

    pub fn set_u64(&self, namespace: &str, key: &str, value: u64) -> Result<()> {
        self.set_typed(namespace, key, "u64", &value.to_string())
    }

    pub fn get_string(&self, namespace: &str, key: &str) -> Result<Option<String>> {
        self.get_typed(namespace, key)?
            .map(|(kind, value)| decode_string(&kind, &value))
            .transpose()
    }

    pub fn set_string(&self, namespace: &str, key: &str, value: &str) -> Result<()> {
        self.set_typed(namespace, key, "string", value)
    }

    pub fn remove(&self, namespace: &str, key: &str) -> Result<bool> {
        self.database.with_connection(self.path.clone(), |conn| {
            ensure_schema(conn)?;
            let affected = conn.execute(
                &format!("DELETE FROM {TABLE_NAME} WHERE namespace = ?1 AND key = ?2"),
                rusqlite::params![namespace, key],
            )?;
            Ok(affected > 0)
        })
    }

    pub fn list_keys(&self, namespace: &str) -> Result<Vec<String>> {
        self.database.with_connection(self.path.clone(), |conn| {
            ensure_schema(conn)?;
            let mut stmt = conn.prepare(&format!(
                "SELECT key FROM {TABLE_NAME} WHERE namespace = ?1 ORDER BY key ASC"
            ))?;
            let rows = stmt.query_map(rusqlite::params![namespace], |row| row.get(0))?;
            let mut keys = Vec::new();
            for row in rows {
                keys.push(row?);
            }
            Ok(keys)
        })
    }

    pub fn clear_namespace(&self, namespace: &str) -> Result<usize> {
        self.database.with_connection(self.path.clone(), |conn| {
            ensure_schema(conn)?;
            conn.execute(
                &format!("DELETE FROM {TABLE_NAME} WHERE namespace = ?1"),
                rusqlite::params![namespace],
            )
            .map_err(Into::into)
        })
    }

    fn get_typed(&self, namespace: &str, key: &str) -> Result<Option<(String, String)>> {
        self.database.with_connection(self.path.clone(), |conn| {
            ensure_schema(conn)?;
            conn.query_row(
                &format!("SELECT value_kind, value_text FROM {TABLE_NAME} WHERE namespace = ?1 AND key = ?2"),
                rusqlite::params![namespace, key],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(Into::into)
        })
    }

    fn set_typed(&self, namespace: &str, key: &str, kind: &str, value: &str) -> Result<()> {
        self.database.with_connection(self.path.clone(), |conn| {
            ensure_schema(conn)?;
            conn.execute(
                &format!(
                    "
                    INSERT INTO {TABLE_NAME} (namespace, key, value_kind, value_text, updated_at)
                    VALUES (?1, ?2, ?3, ?4, strftime('%s', 'now'))
                    ON CONFLICT(namespace, key) DO UPDATE SET
                        value_kind = excluded.value_kind,
                        value_text = excluded.value_text,
                        updated_at = excluded.updated_at
                    "
                ),
                rusqlite::params![namespace, key, kind, value],
            )?;
            Ok(())
        })
    }
}

fn ensure_schema(conn: &rusqlite::Connection) -> Result<()> {
    reset_legacy_json_schema(conn)?;
    conn.execute_batch(&format!(
        "
        CREATE TABLE IF NOT EXISTS {TABLE_NAME} (
            namespace   TEXT NOT NULL,
            key         TEXT NOT NULL,
            value_kind  TEXT NOT NULL,
            value_text  TEXT NOT NULL,
            updated_at  INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (namespace, key)
        );
        CREATE INDEX IF NOT EXISTS idx_plugin_dict_namespace
            ON {TABLE_NAME}(namespace, updated_at DESC);
        "
    ))?;
    Ok(())
}

use rusqlite::OptionalExtension;

fn reset_legacy_json_schema(conn: &rusqlite::Connection) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({TABLE_NAME})"))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if !columns.iter().any(|column| column == "value_json") {
        return Ok(());
    }
    conn.execute_batch(&format!(
        "
        DROP TABLE {TABLE_NAME};
        "
    ))?;
    Ok(())
}

fn decode_bool(kind: &str, value: &str) -> Result<bool> {
    match kind {
        "bool" => Ok(value == "1" || value.eq_ignore_ascii_case("true")),
        other => anyhow::bail!("dict value is {other}, expected bool"),
    }
}

fn decode_i64(kind: &str, value: &str) -> Result<i64> {
    match kind {
        "i64" | "u64" => value.parse().context("cannot decode i64 dict value"),
        other => anyhow::bail!("dict value is {other}, expected i64"),
    }
}

fn decode_u64(kind: &str, value: &str) -> Result<u64> {
    match kind {
        "u64" | "i64" => value.parse().context("cannot decode u64 dict value"),
        other => anyhow::bail!("dict value is {other}, expected u64"),
    }
}

fn decode_string(kind: &str, value: &str) -> Result<String> {
    match kind {
        "string" => Ok(value.to_string()),
        other => anyhow::bail!("dict value is {other}, expected string"),
    }
}
