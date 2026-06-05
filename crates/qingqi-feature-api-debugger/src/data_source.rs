use std::sync::Arc;

use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params};
use time::{OffsetDateTime, macros::format_description};

use crate::model::{
    ApiVariable, CollectionNode, EnvHeader, EnvVariable, Environment, EnvironmentFull, HttpHistory,
    HttpTab, NodeKind, RequestSnapshot, Script, ScriptCategory, VariableScope,
};
use qingqi_plugin::database::{DatabaseService, PooledConnection, SqlitePool};

const SCHEMA_VERSION: i64 = 1;

pub struct ApiDebuggerDataSource {
    pool: SqlitePool,
}

impl ApiDebuggerDataSource {
    pub fn open(database: Arc<DatabaseService>, key: &str) -> Result<Self> {
        let pool = database.pool(key)?;
        let store = Self { pool };
        store.ensure_schema()?;
        Ok(store)
    }

    fn ensure_schema(&self) -> Result<()> {
        let conn = self.connection()?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS schema_info (
                version INTEGER PRIMARY KEY
            );

            CREATE TABLE IF NOT EXISTS collection_nodes (
                id          TEXT PRIMARY KEY,
                parent_id   TEXT,
                kind        TEXT NOT NULL CHECK(kind IN ('folder','endpoint','case')),
                name        TEXT NOT NULL DEFAULT '',
                method      TEXT NOT NULL DEFAULT 'GET',
                url         TEXT NOT NULL DEFAULT '',
                request_json TEXT NOT NULL DEFAULT '{}',
                sort_order  INTEGER NOT NULL DEFAULT 0,
                expanded    INTEGER NOT NULL DEFAULT 0,
                created_at  TEXT NOT NULL DEFAULT '',
                updated_at  TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_collection_parent
                ON collection_nodes(parent_id, sort_order);
            CREATE INDEX IF NOT EXISTS idx_collection_kind
                ON collection_nodes(kind);

            CREATE TABLE IF NOT EXISTS environments (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL DEFAULT '',
                base_url    TEXT NOT NULL DEFAULT '',
                sort_order  INTEGER NOT NULL DEFAULT 0,
                created_at  TEXT NOT NULL DEFAULT '',
                updated_at  TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS environment_variables (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                environment_id  TEXT NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
                enabled         INTEGER NOT NULL DEFAULT 1,
                var_key         TEXT NOT NULL DEFAULT '',
                var_value       TEXT NOT NULL DEFAULT '',
                sort_order      INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_env_var_env
                ON environment_variables(environment_id);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_env_var_key
                ON environment_variables(environment_id, var_key);

            CREATE TABLE IF NOT EXISTS environment_headers (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                environment_id  TEXT NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
                enabled         INTEGER NOT NULL DEFAULT 1,
                header_key      TEXT NOT NULL DEFAULT '',
                header_value    TEXT NOT NULL DEFAULT '',
                sort_order      INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_env_hdr_env
                ON environment_headers(environment_id);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_env_hdr_key
                ON environment_headers(environment_id, header_key);

            CREATE TABLE IF NOT EXISTS http_tabs (
                id                  TEXT PRIMARY KEY,
                name                TEXT NOT NULL DEFAULT '',
                method              TEXT NOT NULL DEFAULT 'GET',
                url                 TEXT NOT NULL DEFAULT '',
                request_mode        TEXT NOT NULL DEFAULT 'rest',
                body_mode           TEXT NOT NULL DEFAULT 'none',
                auth_type           TEXT NOT NULL DEFAULT '',
                auth_value          TEXT NOT NULL DEFAULT '',
                headers_text        TEXT NOT NULL DEFAULT '',
                cookies_text        TEXT NOT NULL DEFAULT '',
                body_text           TEXT NOT NULL DEFAULT '',
                params_text         TEXT NOT NULL DEFAULT '',
                path_params_text    TEXT NOT NULL DEFAULT '',
                pre_ops_text        TEXT NOT NULL DEFAULT '',
                post_ops_text       TEXT NOT NULL DEFAULT '',
                node_id             TEXT NOT NULL DEFAULT '',
                active_request_tab  INTEGER NOT NULL DEFAULT 0,
                updated_at          TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS http_history (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                tab_id      TEXT NOT NULL DEFAULT '',
                method      TEXT NOT NULL DEFAULT '',
                url         TEXT NOT NULL DEFAULT '',
                status      INTEGER NOT NULL DEFAULT 0,
                title       TEXT NOT NULL DEFAULT '',
                response    TEXT NOT NULL DEFAULT '',
                created_at  TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_history_tab
                ON http_history(tab_id, id DESC);

            CREATE TABLE IF NOT EXISTS api_variables (
                scope       TEXT NOT NULL DEFAULT 'global',
                env_name    TEXT NOT NULL DEFAULT '',
                var_key     TEXT NOT NULL DEFAULT '',
                var_value   TEXT NOT NULL DEFAULT '',
                updated_at  TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (scope, env_name, var_key)
            );

            CREATE TABLE IF NOT EXISTS scripts (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL DEFAULT '',
                category    TEXT NOT NULL CHECK(category IN ('pre','post','common')),
                content     TEXT NOT NULL DEFAULT '',
                sort_order  INTEGER NOT NULL DEFAULT 0,
                created_at  TEXT NOT NULL DEFAULT '',
                updated_at  TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_scripts_category
                ON scripts(category, sort_order);
            ",
        )?;

        let version: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_info",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if version < SCHEMA_VERSION {
            conn.execute(
                "INSERT OR REPLACE INTO schema_info (version) VALUES (?1)",
                params![SCHEMA_VERSION],
            )?;
        }

        // Seed inline (avoid re-locking)
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM environments", [], |row| row.get(0))?;
        if count == 0 {
            let now = now_label();
            conn.execute(
                "INSERT INTO environments (id, name, base_url, sort_order, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "env-default",
                    "默认环境",
                    "http://127.0.0.1:8000",
                    0,
                    now,
                    now
                ],
            )?;
            for (i, (key, value)) in [
                ("BASE_URL", "http://127.0.0.1:8000"),
                ("API_KEY", ""),
                ("AUTH_TOKEN", ""),
            ]
            .iter()
            .enumerate()
            {
                conn.execute(
                    "INSERT INTO environment_variables
                         (environment_id, enabled, var_key, var_value, sort_order)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params!["env-default", 1, key, value, i as i64],
                )?;
            }
        }

        Ok(())
    }

    fn connection(&self) -> Result<PooledConnection> {
        self.pool.get().context("无法获取 API 调试器数据库连接")
    }

    // ── Collection CRUD ──

    pub fn list_collection_nodes(&self) -> Result<Vec<CollectionNode>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, parent_id, kind, name, method, url, request_json,
                    sort_order, expanded, created_at, updated_at
               FROM collection_nodes
              ORDER BY sort_order ASC, created_at ASC",
        )?;
        let rows = stmt.query_map([], map_collection_node)?;
        let mut nodes = Vec::new();
        for row in rows {
            nodes.push(row?);
        }
        Ok(nodes)
    }

    pub fn get_collection_node(&self, id: &str) -> Result<Option<CollectionNode>> {
        let conn = self.connection()?;
        conn.query_row(
            "SELECT id, parent_id, kind, name, method, url, request_json,
                    sort_order, expanded, created_at, updated_at
               FROM collection_nodes WHERE id = ?1",
            params![id],
            map_collection_node,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn create_collection_node(
        &self,
        id: &str,
        parent_id: Option<&str>,
        kind: NodeKind,
        name: &str,
        method: &str,
        url: &str,
        request: &RequestSnapshot,
    ) -> Result<CollectionNode> {
        let now = now_label();
        let conn = self.connection()?;
        let sort_order = {
            let max: Option<i64> = conn
                .query_row(
                    "SELECT COALESCE(MAX(sort_order), -1) FROM collection_nodes WHERE parent_id IS ?1",
                    params![parent_id],
                    |row| row.get(0),
                )
                .optional()?;
            max.unwrap_or(-1) + 1
        };
        conn.execute(
            "INSERT INTO collection_nodes
                 (id, parent_id, kind, name, method, url, request_json,
                  sort_order, expanded, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, ?10)",
            params![
                id,
                parent_id,
                kind.as_str(),
                name.trim(),
                method,
                url,
                request.to_json(),
                sort_order,
                now,
                now,
            ],
        )?;
        drop(conn);
        self.get_collection_node(id)?
            .context("创建集合节点后无法读取")
    }

    pub fn update_collection_node(
        &self,
        id: &str,
        name: &str,
        method: &str,
        url: &str,
        request: &RequestSnapshot,
    ) -> Result<()> {
        let now = now_label();
        let conn = self.connection()?;
        conn.execute(
            "UPDATE collection_nodes
                SET name = ?2, method = ?3, url = ?4, request_json = ?5, updated_at = ?6
              WHERE id = ?1",
            params![id, name.trim(), method, url, request.to_json(), now],
        )?;
        Ok(())
    }

    pub fn delete_collection_node(&self, id: &str) -> Result<bool> {
        let conn = self.connection()?;
        let affected = conn.execute("DELETE FROM collection_nodes WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn delete_collection_node_recursive(&self, id: &str) -> Result<usize> {
        let nodes = self.list_collection_nodes()?;
        let mut to_delete = vec![id.to_string()];
        let mut queue = vec![id.to_string()];
        while let Some(current_id) = queue.pop() {
            for node in &nodes {
                if node.parent_id.as_deref() == Some(current_id.as_str()) {
                    to_delete.push(node.id.clone());
                    queue.push(node.id.clone());
                }
            }
        }
        let mut total = 0usize;
        for node_id in &to_delete {
            if self.delete_collection_node(node_id)? {
                total += 1;
            }
        }
        Ok(total)
    }

    pub fn set_collection_node_expanded(&self, id: &str, expanded: bool) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE collection_nodes SET expanded = ?2 WHERE id = ?1",
            params![id, if expanded { 1 } else { 0 }],
        )?;
        Ok(())
    }

    // ── Environment CRUD ──

    pub fn list_environments(&self) -> Result<Vec<EnvironmentFull>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, base_url, sort_order, created_at, updated_at
               FROM environments ORDER BY sort_order ASC, created_at ASC",
        )?;
        let env_rows = stmt.query_map([], map_environment)?;
        let mut result = Vec::new();
        for row in env_rows {
            let env = row?;
            let variables = self.list_env_variables_locked(&conn, &env.id)?;
            let headers = self.list_env_headers_locked(&conn, &env.id)?;
            result.push(EnvironmentFull {
                env,
                variables,
                headers,
            });
        }
        Ok(result)
    }

    pub fn get_environment(&self, id: &str) -> Result<Option<Environment>> {
        let conn = self.connection()?;
        conn.query_row(
            "SELECT id, name, base_url, sort_order, created_at, updated_at
               FROM environments WHERE id = ?1",
            params![id],
            map_environment,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn create_environment(&self, id: &str, name: &str, base_url: &str) -> Result<Environment> {
        let now = now_label();
        let conn = self.connection()?;
        let sort_order: i64 = conn.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM environments",
            [],
            |row| row.get(0),
        )?;
        conn.execute(
            "INSERT INTO environments (id, name, base_url, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, name.trim(), base_url, sort_order, now, now],
        )?;
        drop(conn);
        self.get_environment(id)?.context("创建环境后无法读取")
    }

    pub fn update_environment(&self, id: &str, name: &str, base_url: &str) -> Result<()> {
        let now = now_label();
        let conn = self.connection()?;
        conn.execute(
            "UPDATE environments SET name = ?2, base_url = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, name.trim(), base_url, now],
        )?;
        Ok(())
    }

    pub fn delete_environment(&self, id: &str) -> Result<bool> {
        let conn = self.connection()?;
        let affected = conn.execute("DELETE FROM environments WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    // ── Environment Variables ──

    fn list_env_variables_locked(
        &self,
        conn: &rusqlite::Connection,
        env_id: &str,
    ) -> Result<Vec<EnvVariable>> {
        let mut stmt = conn.prepare(
            "SELECT id, environment_id, enabled, var_key, var_value, sort_order
               FROM environment_variables WHERE environment_id = ?1
              ORDER BY sort_order ASC, id ASC",
        )?;
        let rows = stmt.query_map(params![env_id], map_env_variable)?;
        let mut vars = Vec::new();
        for row in rows {
            vars.push(row?);
        }
        Ok(vars)
    }

    pub fn list_env_variables(&self, env_id: &str) -> Result<Vec<EnvVariable>> {
        let conn = self.connection()?;
        self.list_env_variables_locked(&conn, env_id)
    }

    pub fn upsert_env_variable(
        &self,
        env_id: &str,
        enabled: bool,
        key: &str,
        value: &str,
    ) -> Result<()> {
        let conn = self.connection()?;
        let sort_order: i64 = conn.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1
               FROM environment_variables WHERE environment_id = ?1",
            params![env_id],
            |row| row.get(0),
        )?;
        conn.execute(
            "INSERT INTO environment_variables
                 (environment_id, enabled, var_key, var_value, sort_order)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(environment_id, var_key) DO UPDATE SET
                var_value = excluded.var_value,
                enabled = excluded.enabled",
            params![
                env_id,
                if enabled { 1 } else { 0 },
                key.trim(),
                value,
                sort_order
            ],
        )?;
        Ok(())
    }

    pub fn delete_env_variable(&self, id: i64) -> Result<bool> {
        let conn = self.connection()?;
        let affected = conn.execute(
            "DELETE FROM environment_variables WHERE id = ?1",
            params![id],
        )?;
        Ok(affected > 0)
    }

    pub fn replace_env_variables(
        &self,
        env_id: &str,
        rows: &[(bool, String, String)],
    ) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "DELETE FROM environment_variables WHERE environment_id = ?1",
            params![env_id],
        )?;
        for (i, (enabled, key, value)) in rows.iter().enumerate() {
            conn.execute(
                "INSERT INTO environment_variables
                     (environment_id, enabled, var_key, var_value, sort_order)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    env_id,
                    if *enabled { 1 } else { 0 },
                    key.trim(),
                    value,
                    i as i64
                ],
            )?;
        }
        Ok(())
    }

    // ── Environment Headers ──

    fn list_env_headers_locked(
        &self,
        conn: &rusqlite::Connection,
        env_id: &str,
    ) -> Result<Vec<EnvHeader>> {
        let mut stmt = conn.prepare(
            "SELECT id, environment_id, enabled, header_key, header_value, sort_order
               FROM environment_headers WHERE environment_id = ?1
              ORDER BY sort_order ASC, id ASC",
        )?;
        let rows = stmt.query_map(params![env_id], map_env_header)?;
        let mut headers = Vec::new();
        for row in rows {
            headers.push(row?);
        }
        Ok(headers)
    }

    pub fn list_env_headers(&self, env_id: &str) -> Result<Vec<EnvHeader>> {
        let conn = self.connection()?;
        self.list_env_headers_locked(&conn, env_id)
    }

    pub fn replace_env_headers(&self, env_id: &str, rows: &[(bool, String, String)]) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "DELETE FROM environment_headers WHERE environment_id = ?1",
            params![env_id],
        )?;
        for (i, (enabled, key, value)) in rows.iter().enumerate() {
            conn.execute(
                "INSERT INTO environment_headers
                     (environment_id, enabled, header_key, header_value, sort_order)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    env_id,
                    if *enabled { 1 } else { 0 },
                    key.trim(),
                    value,
                    i as i64
                ],
            )?;
        }
        Ok(())
    }

    // ── Tabs ──

    pub fn list_tabs(&self) -> Result<Vec<HttpTab>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, method, url, request_mode, body_mode,
                    auth_type, auth_value, headers_text, cookies_text,
                    body_text, params_text, path_params_text,
                    pre_ops_text, post_ops_text, node_id,
                    active_request_tab, updated_at
               FROM http_tabs ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], map_http_tab)?;
        let mut tabs = Vec::new();
        for row in rows {
            tabs.push(row?);
        }
        Ok(tabs)
    }

    pub fn save_tab(&self, tab: &HttpTab) -> Result<()> {
        let now = now_label();
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO http_tabs
                 (id, name, method, url, request_mode, body_mode,
                  auth_type, auth_value, headers_text, cookies_text,
                  body_text, params_text, path_params_text,
                  pre_ops_text, post_ops_text, node_id,
                  active_request_tab, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                     ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name, method = excluded.method,
                url = excluded.url, request_mode = excluded.request_mode,
                body_mode = excluded.body_mode, auth_type = excluded.auth_type,
                auth_value = excluded.auth_value, headers_text = excluded.headers_text,
                cookies_text = excluded.cookies_text, body_text = excluded.body_text,
                params_text = excluded.params_text,
                path_params_text = excluded.path_params_text,
                pre_ops_text = excluded.pre_ops_text,
                post_ops_text = excluded.post_ops_text,
                node_id = excluded.node_id,
                active_request_tab = excluded.active_request_tab,
                updated_at = ?18",
            params![
                tab.id,
                tab.name,
                tab.method,
                tab.url,
                tab.request_mode,
                tab.body_mode,
                tab.auth_type,
                tab.auth_value,
                tab.headers_text,
                tab.cookies_text,
                tab.body_text,
                tab.params_text,
                tab.path_params_text,
                tab.pre_ops_text,
                tab.post_ops_text,
                tab.node_id,
                tab.active_request_tab,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn delete_tab(&self, id: &str) -> Result<bool> {
        let conn = self.connection()?;
        let affected = conn.execute("DELETE FROM http_tabs WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    // ── History ──

    pub fn insert_history(
        &self,
        tab_id: &str,
        method: &str,
        url: &str,
        status: i64,
        title: &str,
        response: &str,
    ) -> Result<i64> {
        let now = now_label();
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO http_history (tab_id, method, url, status, title, response, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![tab_id, method, url, status, title, response, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_history(&self, tab_id: &str, limit: i64) -> Result<Vec<HttpHistory>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, tab_id, method, url, status, title, response, created_at
               FROM http_history WHERE tab_id = ?1
              ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![tab_id, limit], map_http_history)?;
        let mut items = Vec::new();
        for row in rows {
            items.push(row?);
        }
        Ok(items)
    }

    pub fn clear_history(&self, tab_id: &str) -> Result<usize> {
        let conn = self.connection()?;
        let affected = conn.execute(
            "DELETE FROM http_history WHERE tab_id = ?1",
            params![tab_id],
        )?;
        Ok(affected)
    }

    // ── Scripts ──

    pub fn list_scripts(&self, category: Option<ScriptCategory>) -> Result<Vec<Script>> {
        let conn = self.connection()?;
        let (sql, params_vec): (String, Vec<String>) = if let Some(cat) = category {
            (
                "SELECT id, name, category, content, sort_order, created_at, updated_at
                 FROM scripts WHERE category = ?1 ORDER BY sort_order, name"
                    .into(),
                vec![cat.as_str().to_string()],
            )
        } else {
            (
                "SELECT id, name, category, content, sort_order, created_at, updated_at
                 FROM scripts ORDER BY category, sort_order, name"
                    .into(),
                vec![],
            )
        };
        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec
            .iter()
            .map(|s| s as &dyn rusqlite::types::ToSql)
            .collect();
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(Script {
                id: row.get(0)?,
                name: row.get(1)?,
                category: ScriptCategory::from_db(&row.get::<_, String>(2)?),
                content: row.get(3)?,
                sort_order: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn get_script(&self, id: &str) -> Result<Option<Script>> {
        let conn = self.connection()?;
        conn.query_row(
            "SELECT id, name, category, content, sort_order, created_at, updated_at
             FROM scripts WHERE id = ?1",
            params![id],
            |row| {
                Ok(Script {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    category: ScriptCategory::from_db(&row.get::<_, String>(2)?),
                    content: row.get(3)?,
                    sort_order: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn save_script(&self, script: &Script) -> Result<()> {
        let now = now_label();
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO scripts (id, name, category, content, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                category = excluded.category,
                content = excluded.content,
                sort_order = excluded.sort_order,
                updated_at = ?7",
            params![
                script.id,
                script.name,
                script.category.as_str(),
                script.content,
                script.sort_order,
                script.created_at,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn delete_script(&self, id: &str) -> Result<bool> {
        let conn = self.connection()?;
        let affected = conn.execute("DELETE FROM scripts WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    // ── Scoped Variables ──

    pub fn get_variable(
        &self,
        scope: VariableScope,
        env_name: &str,
        key: &str,
    ) -> Result<Option<String>> {
        let conn = self.connection()?;
        conn.query_row(
            "SELECT var_value FROM api_variables
              WHERE scope = ?1 AND env_name = ?2 AND var_key = ?3",
            params![scope.as_str(), env_name, key],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn upsert_variable(
        &self,
        scope: VariableScope,
        env_name: &str,
        key: &str,
        value: &str,
    ) -> Result<()> {
        let now = now_label();
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO api_variables (scope, env_name, var_key, var_value, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(scope, env_name, var_key) DO UPDATE SET
                var_value = excluded.var_value,
                updated_at = excluded.updated_at",
            params![scope.as_str(), env_name, key.trim(), value, now],
        )?;
        Ok(())
    }

    pub fn list_variables(&self, scope: VariableScope, env_name: &str) -> Result<Vec<ApiVariable>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT scope, env_name, var_key, var_value, updated_at
               FROM api_variables WHERE scope = ?1 AND env_name = ?2
              ORDER BY var_key ASC",
        )?;
        let rows = stmt.query_map(params![scope.as_str(), env_name], map_api_variable)?;
        let mut vars = Vec::new();
        for row in rows {
            vars.push(row?);
        }
        Ok(vars)
    }

    pub fn delete_variable(&self, scope: VariableScope, env_name: &str, key: &str) -> Result<bool> {
        let conn = self.connection()?;
        let affected = conn.execute(
            "DELETE FROM api_variables WHERE scope = ?1 AND env_name = ?2 AND var_key = ?3",
            params![scope.as_str(), env_name, key],
        )?;
        Ok(affected > 0)
    }

    // ── Bulk operations ──

    pub fn save_environments_full(&self, envs: &[EnvironmentFull]) -> Result<()> {
        let conn = self.connection()?;
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(
            "DELETE FROM environment_headers; DELETE FROM environment_variables; DELETE FROM environments;",
        )?;
        for (i, full) in envs.iter().enumerate() {
            let now = now_label();
            tx.execute(
                "INSERT INTO environments (id, name, base_url, sort_order, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    full.env.id,
                    full.env.name,
                    full.env.base_url,
                    i as i64,
                    now,
                    now,
                ],
            )?;
            for (j, var) in full.variables.iter().enumerate() {
                tx.execute(
                    "INSERT INTO environment_variables
                         (environment_id, enabled, var_key, var_value, sort_order)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        full.env.id,
                        if var.enabled { 1 } else { 0 },
                        var.var_key,
                        var.var_value,
                        j as i64,
                    ],
                )?;
            }
            for (j, hdr) in full.headers.iter().enumerate() {
                tx.execute(
                    "INSERT INTO environment_headers
                         (environment_id, enabled, header_key, header_value, sort_order)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        full.env.id,
                        if hdr.enabled { 1 } else { 0 },
                        hdr.header_key,
                        hdr.header_value,
                        j as i64,
                    ],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

// ── Row mappers ──

fn map_collection_node(
    row: &rusqlite::Row,
) -> std::result::Result<CollectionNode, rusqlite::Error> {
    Ok(CollectionNode {
        id: row.get(0)?,
        parent_id: row.get(1)?,
        kind: NodeKind::from_db(&row.get::<_, String>(2)?),
        name: row.get(3)?,
        method: row.get(4)?,
        url: row.get(5)?,
        request_json: row.get(6)?,
        sort_order: row.get(7)?,
        expanded: row.get::<_, i64>(8)? != 0,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn map_environment(row: &rusqlite::Row) -> std::result::Result<Environment, rusqlite::Error> {
    Ok(Environment {
        id: row.get(0)?,
        name: row.get(1)?,
        base_url: row.get(2)?,
        sort_order: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn map_env_variable(row: &rusqlite::Row) -> std::result::Result<EnvVariable, rusqlite::Error> {
    Ok(EnvVariable {
        id: row.get(0)?,
        environment_id: row.get(1)?,
        enabled: row.get::<_, i64>(2)? != 0,
        var_key: row.get(3)?,
        var_value: row.get(4)?,
        sort_order: row.get(5)?,
    })
}

fn map_env_header(row: &rusqlite::Row) -> std::result::Result<EnvHeader, rusqlite::Error> {
    Ok(EnvHeader {
        id: row.get(0)?,
        environment_id: row.get(1)?,
        enabled: row.get::<_, i64>(2)? != 0,
        header_key: row.get(3)?,
        header_value: row.get(4)?,
        sort_order: row.get(5)?,
    })
}

fn map_http_tab(row: &rusqlite::Row) -> std::result::Result<HttpTab, rusqlite::Error> {
    Ok(HttpTab {
        id: row.get(0)?,
        name: row.get(1)?,
        method: row.get(2)?,
        url: row.get(3)?,
        request_mode: row.get(4)?,
        body_mode: row.get(5)?,
        auth_type: row.get(6)?,
        auth_value: row.get(7)?,
        headers_text: row.get(8)?,
        cookies_text: row.get(9)?,
        body_text: row.get(10)?,
        params_text: row.get(11)?,
        path_params_text: row.get(12)?,
        pre_ops_text: row.get(13)?,
        post_ops_text: row.get(14)?,
        node_id: row.get(15)?,
        active_request_tab: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

fn map_http_history(row: &rusqlite::Row) -> std::result::Result<HttpHistory, rusqlite::Error> {
    Ok(HttpHistory {
        id: row.get(0)?,
        tab_id: row.get(1)?,
        method: row.get(2)?,
        url: row.get(3)?,
        status: row.get(4)?,
        title: row.get(5)?,
        response: row.get(6)?,
        created_at: row.get(7)?,
    })
}

fn map_api_variable(row: &rusqlite::Row) -> std::result::Result<ApiVariable, rusqlite::Error> {
    Ok(ApiVariable {
        scope: VariableScope::from_db(&row.get::<_, String>(0)?),
        env_name: row.get(1)?,
        var_key: row.get(2)?,
        var_value: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn now_label() -> String {
    let fmt = format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .format(&fmt)
        .unwrap_or_else(|_| String::from("1970-01-01 00:00:00"))
}

// Also keep backward-compatible workspace types for JSON import
use crate::model::{ApiEnvironment, ApiGroup};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ApiWorkspace {
    pub groups: Vec<ApiGroup>,
    pub environments: Vec<ApiEnvironment>,
}

impl ApiWorkspace {
    pub fn new(groups: Vec<ApiGroup>, environments: Vec<ApiEnvironment>) -> Self {
        Self {
            groups,
            environments,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ApiWorkspaceStore {
    path: std::path::PathBuf,
}

impl ApiWorkspaceStore {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> anyhow::Result<Option<ApiWorkspace>> {
        if !self.path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&self.path)
            .with_context(|| format!("无法读取 API 工作区 {}", self.path.display()))?;
        let workspace = serde_json::from_str::<ApiWorkspace>(&raw)
            .with_context(|| format!("无法解析 API 工作区 {}", self.path.display()))?;
        Ok(Some(workspace))
    }

    pub fn save(&self, workspace: &ApiWorkspace) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("无法创建 API 工作区目录 {}", parent.display()))?;
        }
        let raw = serde_json::to_string_pretty(workspace)?;
        std::fs::write(&self.path, raw)
            .with_context(|| format!("无法写入 API 工作区 {}", self.path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qingqi_plugin::{database::DatabaseService, storage::AppPaths};
    use std::{
        fs,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_db() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-api-debugger-{nanos}"));
        let _ = fs::create_dir_all(&dir);
        dir.join("test.db")
    }

    fn open_store() -> ApiDebuggerDataSource {
        let path = temp_db();
        let database = Arc::new(DatabaseService::new(AppPaths::for_test(
            path.parent().unwrap().to_path_buf(),
        )));
        database
            .register_database(qingqi_plugin::database::DatabaseSpec::path(
                "api_debugger/main",
                path,
            ))
            .unwrap();
        ApiDebuggerDataSource::open(database, "api_debugger/main").unwrap()
    }

    #[test]
    fn schema_creates_tables() {
        let store = open_store();
        let conn = store.connection().unwrap();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert!(tables.contains(&"collection_nodes".into()));
        assert!(tables.contains(&"environments".into()));
        assert!(tables.contains(&"environment_variables".into()));
        assert!(tables.contains(&"environment_headers".into()));
        assert!(tables.contains(&"http_tabs".into()));
        assert!(tables.contains(&"http_history".into()));
        assert!(tables.contains(&"api_variables".into()));
        assert!(tables.contains(&"schema_info".into()));
    }

    #[test]
    fn seeds_default_environment() {
        let store = open_store();
        let envs = store.list_environments().unwrap();
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].env.name, "默认环境");
        assert_eq!(envs[0].env.base_url, "http://127.0.0.1:8000");
        assert_eq!(envs[0].variables.len(), 3);
        assert_eq!(envs[0].variables[0].var_key, "BASE_URL");
    }

    #[test]
    fn collection_node_crud() {
        let store = open_store();

        let folder = store
            .create_collection_node(
                "node-folder-1",
                None,
                NodeKind::Folder,
                "用户模块",
                "",
                "",
                &RequestSnapshot::default(),
            )
            .unwrap();
        assert_eq!(folder.name, "用户模块");
        assert_eq!(folder.kind, NodeKind::Folder);

        let endpoint = store
            .create_collection_node(
                "node-ep-1",
                Some("node-folder-1"),
                NodeKind::Endpoint,
                "/user/info",
                "GET",
                "/api/v1/user/info",
                &RequestSnapshot {
                    method: "GET".into(),
                    url: "/api/v1/user/info".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(endpoint.parent_id, Some("node-folder-1".into()));
        assert_eq!(endpoint.method, "GET");

        let nodes = store.list_collection_nodes().unwrap();
        assert_eq!(nodes.len(), 2);

        store
            .update_collection_node(
                "node-ep-1",
                "/user/info v2",
                "POST",
                "/api/v2/user/info",
                &RequestSnapshot {
                    method: "POST".into(),
                    url: "/api/v2/user/info".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let updated = store.get_collection_node("node-ep-1").unwrap().unwrap();
        assert_eq!(updated.name, "/user/info v2");
        assert_eq!(updated.method, "POST");

        assert!(store.delete_collection_node("node-ep-1").unwrap());
        assert!(!store.delete_collection_node("node-ep-1").unwrap());
        assert_eq!(store.list_collection_nodes().unwrap().len(), 1);
    }

    #[test]
    fn environment_crud_with_children() {
        let store = open_store();

        let env = store
            .create_environment("env-test", "测试环境", "http://test.api.com")
            .unwrap();
        assert_eq!(env.name, "测试环境");

        store
            .upsert_env_variable("env-test", true, "API_KEY", "test-key-123")
            .unwrap();
        store
            .upsert_env_variable("env-test", true, "TOKEN", "abc")
            .unwrap();
        let vars = store.list_env_variables("env-test").unwrap();
        assert_eq!(vars.len(), 2);

        store
            .upsert_env_variable("env-test", true, "API_KEY", "new-key-456")
            .unwrap();
        let vars = store.list_env_variables("env-test").unwrap();
        assert_eq!(vars.len(), 2);
        let api_key = vars.iter().find(|v| v.var_key == "API_KEY").unwrap();
        assert_eq!(api_key.var_value, "new-key-456");

        store
            .replace_env_headers(
                "env-test",
                &[
                    (true, "Content-Type".into(), "application/json".into()),
                    (true, "X-Env".into(), "test".into()),
                ],
            )
            .unwrap();
        let headers = store.list_env_headers("env-test").unwrap();
        assert_eq!(headers.len(), 2);

        let envs = store.list_environments().unwrap();
        assert_eq!(envs.len(), 2);
        let test_env = envs.iter().find(|e| e.env.id == "env-test").unwrap();
        assert_eq!(test_env.variables.len(), 2);
        assert_eq!(test_env.headers.len(), 2);

        assert!(store.delete_environment("env-test").unwrap());
        assert_eq!(store.list_environments().unwrap().len(), 1);
    }

    #[test]
    fn tab_persistence() {
        let store = open_store();

        let tab = HttpTab {
            id: "tab-1".into(),
            name: "获取用户".into(),
            method: "GET".into(),
            url: "/api/user".into(),
            request_mode: "rest".into(),
            body_mode: "none".into(),
            auth_type: "Bearer".into(),
            auth_value: "token123".into(),
            headers_text: "Content-Type=application/json".into(),
            cookies_text: "sid=abc".into(),
            body_text: String::new(),
            params_text: "page=1".into(),
            path_params_text: String::new(),
            pre_ops_text: "set token=abc".into(),
            post_ops_text: "extract id=$.data.id".into(),
            node_id: "node-1".into(),
            active_request_tab: 0,
            updated_at: String::new(),
        };

        store.save_tab(&tab).unwrap();
        let tabs = store.list_tabs().unwrap();
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0].name, "获取用户");
        assert_eq!(tabs[0].method, "GET");

        let mut tab2 = tab.clone();
        tab2.method = "POST".into();
        tab2.url = "/api/user/create".into();
        store.save_tab(&tab2).unwrap();
        let tabs = store.list_tabs().unwrap();
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0].method, "POST");

        assert!(store.delete_tab("tab-1").unwrap());
        assert_eq!(store.list_tabs().unwrap().len(), 0);
    }

    #[test]
    fn history_roundtrip() {
        let store = open_store();

        let id = store
            .insert_history(
                "tab-1",
                "GET",
                "/api/user",
                200,
                "200 OK",
                r#"{"code": 200}"#,
            )
            .unwrap();
        assert!(id > 0);

        let history = store.list_history("tab-1", 10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].status, 200);
        assert_eq!(history[0].method, "GET");

        for i in 0..5 {
            store
                .insert_history(
                    "tab-1",
                    "POST",
                    &format!("/api/item/{i}"),
                    201,
                    "201 Created",
                    "{}",
                )
                .unwrap();
        }
        let history = store.list_history("tab-1", 3).unwrap();
        assert_eq!(history.len(), 3);

        let cleared = store.clear_history("tab-1").unwrap();
        assert_eq!(cleared, 6);
        assert_eq!(store.list_history("tab-1", 10).unwrap().len(), 0);
    }

    #[test]
    fn scoped_variables() {
        let store = open_store();

        store
            .upsert_variable(VariableScope::Global, "", "API_VERSION", "v2")
            .unwrap();
        store
            .upsert_variable(VariableScope::Environment, "dev", "DB_HOST", "localhost")
            .unwrap();
        store
            .upsert_variable(VariableScope::Environment, "prod", "DB_HOST", "db.prod.com")
            .unwrap();

        let val = store
            .get_variable(VariableScope::Global, "", "API_VERSION")
            .unwrap();
        assert_eq!(val, Some("v2".into()));

        let val = store
            .get_variable(VariableScope::Environment, "dev", "DB_HOST")
            .unwrap();
        assert_eq!(val, Some("localhost".into()));

        let val = store
            .get_variable(VariableScope::Environment, "prod", "DB_HOST")
            .unwrap();
        assert_eq!(val, Some("db.prod.com".into()));

        let globals = store.list_variables(VariableScope::Global, "").unwrap();
        assert_eq!(globals.len(), 1);

        let dev_vars = store
            .list_variables(VariableScope::Environment, "dev")
            .unwrap();
        assert_eq!(dev_vars.len(), 1);

        store
            .upsert_variable(VariableScope::Global, "", "API_VERSION", "v3")
            .unwrap();
        let val = store
            .get_variable(VariableScope::Global, "", "API_VERSION")
            .unwrap();
        assert_eq!(val, Some("v3".into()));

        assert!(
            store
                .delete_variable(VariableScope::Global, "", "API_VERSION")
                .unwrap()
        );
        assert!(
            !store
                .delete_variable(VariableScope::Global, "", "API_VERSION")
                .unwrap()
        );
    }

    #[test]
    fn save_environments_full_replaces_all() {
        let store = open_store();

        let envs = vec![EnvironmentFull {
            env: Environment {
                id: "env-new".into(),
                name: "新环境".into(),
                base_url: "http://new.api.com".into(),
                sort_order: 0,
                created_at: String::new(),
                updated_at: String::new(),
            },
            variables: vec![EnvVariable {
                id: 0,
                environment_id: "env-new".into(),
                enabled: true,
                var_key: "KEY1".into(),
                var_value: "val1".into(),
                sort_order: 0,
            }],
            headers: vec![EnvHeader {
                id: 0,
                environment_id: "env-new".into(),
                enabled: true,
                header_key: "X-Custom".into(),
                header_value: "hello".into(),
                sort_order: 0,
            }],
        }];

        store.save_environments_full(&envs).unwrap();

        let loaded = store.list_environments().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].env.name, "新环境");
        assert_eq!(loaded[0].variables.len(), 1);
        assert_eq!(loaded[0].headers.len(), 1);
    }
}
