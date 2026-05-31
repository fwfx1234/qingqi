use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

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

    pub fn get_json(&self, namespace: &str, key: &str) -> Result<Option<Value>> {
        self.database.with_connection(self.path.clone(), |conn| {
            ensure_schema(conn)?;
            conn.query_row(
                &format!("SELECT value_json FROM {TABLE_NAME} WHERE namespace = ?1 AND key = ?2"),
                rusqlite::params![namespace, key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(Into::into)
            .and_then(|raw: Option<String>| match raw {
                Some(value) => serde_json::from_str(&value)
                    .with_context(|| format!("invalid json value for {namespace}:{key}"))
                    .map(Some),
                None => Ok(None),
            })
        })
    }

    pub fn get<T: DeserializeOwned>(&self, namespace: &str, key: &str) -> Result<Option<T>> {
        self.get_json(namespace, key)?
            .map(serde_json::from_value)
            .transpose()
            .with_context(|| format!("cannot decode value for {namespace}:{key}"))
    }

    pub fn set_json(&self, namespace: &str, key: &str, value: &Value) -> Result<()> {
        let raw = serde_json::to_string(value).context("cannot encode dict json value")?;
        self.database.with_connection(self.path.clone(), |conn| {
            ensure_schema(conn)?;
            conn.execute(
                &format!(
                    "
                    INSERT INTO {TABLE_NAME} (namespace, key, value_json, updated_at)
                    VALUES (?1, ?2, ?3, strftime('%s', 'now'))
                    ON CONFLICT(namespace, key) DO UPDATE SET
                        value_json = excluded.value_json,
                        updated_at = excluded.updated_at
                    "
                ),
                rusqlite::params![namespace, key, raw],
            )?;
            Ok(())
        })
    }

    pub fn set<T: Serialize>(&self, namespace: &str, key: &str, value: &T) -> Result<()> {
        let json = serde_json::to_value(value).context("cannot convert dict value to json")?;
        self.set_json(namespace, key, &json)
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
}

fn ensure_schema(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(&format!(
        "
        CREATE TABLE IF NOT EXISTS {TABLE_NAME} (
            namespace   TEXT NOT NULL,
            key         TEXT NOT NULL,
            value_json  TEXT NOT NULL,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{database::DatabaseService, storage::AppPaths};
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct DemoValue {
        name: String,
        count: i32,
    }

    fn temp_paths(label: &str) -> AppPaths {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-dict-store-{label}-{nanos}"));
        fs::create_dir_all(&dir).expect("temp dir");
        AppPaths::for_test(dir)
    }

    #[test]
    fn supports_json_roundtrip_and_namespace_isolation() {
        let db = DatabaseService::new(temp_paths("json-roundtrip"));
        let store = PluginDictStore::for_database(db, "dict.db");

        store
            .set(
                "plugin-a",
                "config",
                &DemoValue {
                    name: String::from("demo"),
                    count: 3,
                },
            )
            .expect("set struct");
        store
            .set_json("plugin-a", "title", &json!("hello"))
            .expect("set string");
        store
            .set_json("plugin-b", "title", &json!(42))
            .expect("set number");

        let value: DemoValue = store
            .get("plugin-a", "config")
            .expect("get struct")
            .expect("struct value");
        assert_eq!(
            value,
            DemoValue {
                name: String::from("demo"),
                count: 3
            }
        );
        assert_eq!(
            store.get_json("plugin-a", "title").expect("json title"),
            Some(json!("hello"))
        );
        assert_eq!(
            store.get_json("plugin-b", "title").expect("json number"),
            Some(json!(42))
        );
        assert_eq!(
            store.list_keys("plugin-a").expect("keys"),
            vec![String::from("config"), String::from("title")]
        );
    }

    #[test]
    fn remove_and_clear_namespace_work() {
        let db = DatabaseService::new(temp_paths("remove-clear"));
        let store = PluginDictStore::for_database(db, "dict.db");

        store
            .set_json("plugin-a", "a", &json!({"ok": true}))
            .unwrap();
        store.set_json("plugin-a", "b", &json!("value")).unwrap();
        store.set_json("plugin-b", "a", &json!(1)).unwrap();

        assert!(store.remove("plugin-a", "a").expect("remove"));
        assert_eq!(store.get_json("plugin-a", "a").unwrap(), None);
        assert_eq!(store.clear_namespace("plugin-a").expect("clear"), 1);
        assert_eq!(store.list_keys("plugin-a").unwrap(), Vec::<String>::new());
        assert_eq!(store.get_json("plugin-b", "a").unwrap(), Some(json!(1)));
    }
}
