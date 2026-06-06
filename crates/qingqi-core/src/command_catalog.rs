use std::sync::Arc;

use anyhow::Result;
use qingqi_plugin::{command::Command, database::DatabaseService};
use rusqlite::{params, types::Type};

pub const COMMAND_CATALOG_KEY: &str = "command-catalog";

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS command_catalog_entries (
    id TEXT PRIMARY KEY,
    plugin_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    command_json TEXT NOT NULL,
    search_text TEXT NOT NULL,
    sort_title TEXT NOT NULL,
    updated_at INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_command_catalog_sort
    ON command_catalog_entries(sort_title, id);

CREATE INDEX IF NOT EXISTS idx_command_catalog_search
    ON command_catalog_entries(search_text);
";

const DELETE_ENTRIES: &str = "DELETE FROM command_catalog_entries";

const INSERT_ENTRY: &str = "
INSERT INTO command_catalog_entries
    (id, plugin_id, kind, command_json, search_text, sort_title, updated_at)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%s', 'now'))
";

const LOAD_COMMANDS: &str = "
SELECT command_json
FROM command_catalog_entries
ORDER BY sort_title ASC, id ASC
";

#[derive(Clone, Debug)]
pub struct CommandCatalogStore {
    database: Arc<DatabaseService>,
    key: String,
}

impl CommandCatalogStore {
    pub fn new(database: Arc<DatabaseService>, key: impl Into<String>) -> Self {
        Self {
            database,
            key: key.into(),
        }
    }

    pub fn save_commands(&self, commands: &[Command]) -> Result<()> {
        self.database.with_registered_connection(&self.key, |conn| {
            ensure_schema(conn)?;
            let tx = conn.unchecked_transaction()?;
            tx.execute(DELETE_ENTRIES, [])?;
            {
                let mut stmt = tx.prepare(INSERT_ENTRY)?;
                for command in commands {
                    stmt.execute(params![
                        command.id,
                        command.plugin_id,
                        format!("{:?}", command.kind),
                        serde_json::to_string(command)?,
                        command_search_text(command),
                        command.title.to_lowercase(),
                    ])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
    }

    pub fn load_commands(&self) -> Result<Vec<Command>> {
        self.database.with_registered_connection(&self.key, |conn| {
            ensure_schema(conn)?;
            let mut stmt = conn.prepare(LOAD_COMMANDS)?;
            let rows = stmt.query_map([], map_command)?;
            let mut commands = Vec::new();
            for row in rows {
                commands.push(row?);
            }
            Ok(commands)
        })
    }
}

fn ensure_schema(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)?;
    Ok(())
}

fn map_command(row: &rusqlite::Row<'_>) -> rusqlite::Result<Command> {
    let command_json: String = row.get(0)?;
    serde_json::from_str(&command_json)
        .map_err(|error| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(error)))
}

fn command_search_text(command: &Command) -> String {
    let mut values = vec![
        command.id.clone(),
        command.plugin_id.clone(),
        command.title.clone(),
        command.subtitle.clone(),
    ];
    values.extend(command.keywords.clone());
    values.extend(command.prefixes.clone());
    values.join("\n").to_lowercase()
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::Arc, time::SystemTime};

    use qingqi_plugin::{
        command::Command,
        database::{DatabaseService, DatabaseSpec},
        storage::AppPaths,
    };

    use super::*;

    fn temp_paths() -> AppPaths {
        let nanos = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-command-catalog-{nanos}"));
        let _ = fs::create_dir_all(&dir);
        AppPaths::for_test(dir)
    }

    fn store() -> CommandCatalogStore {
        let database = Arc::new(DatabaseService::new(temp_paths()));
        database
            .register_database(DatabaseSpec::app(COMMAND_CATALOG_KEY, "catalog.db"))
            .unwrap();
        CommandCatalogStore::new(database, COMMAND_CATALOG_KEY)
    }

    #[test]
    fn save_and_load_commands_roundtrip() {
        let store = store();
        let commands = vec![Command::plugin_open(
            "json-parser",
            "JSON 解析",
            "格式化 JSON",
            ["json"],
            ["json"],
            "icons/json.svg",
        )];

        store.save_commands(&commands).unwrap();
        let loaded = store.load_commands().unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, commands[0].id);
        assert_eq!(loaded[0].title, commands[0].title);
    }
}
