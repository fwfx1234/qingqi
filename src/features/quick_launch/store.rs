use std::{collections::HashMap, fs, path::Path};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use time::{OffsetDateTime, macros::format_description};

use crate::features::quick_launch::model::{
    ActionKind, FeedbackMode, QuickAction, QuickActionDraft, QuickRun, QuickRunDraft, RunStatus,
    ScriptSource, ScriptType,
};

pub struct QuickLaunchStore {
    conn: Connection,
}

impl QuickLaunchStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("无法创建快速启动目录 {}", parent.display()))?;
        }

        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let store = Self { conn };
        store.ensure_schema()?;
        Ok(store)
    }

    pub fn list_actions(&self, enabled: Option<bool>) -> Result<Vec<QuickAction>> {
        let sql = match enabled {
            Some(_) => {
                "SELECT * FROM quick_launch_actions WHERE enabled = ?1 ORDER BY sort_order ASC, id ASC"
            }
            None => "SELECT * FROM quick_launch_actions ORDER BY sort_order ASC, id ASC",
        };
        let mut stmt = self.conn.prepare(sql)?;
        let rows = match enabled {
            Some(enabled) => stmt.query_map(params![if enabled { 1 } else { 0 }], map_action)?,
            None => stmt.query_map([], map_action)?,
        };
        let mut actions = Vec::new();
        for row in rows {
            actions.push(row?);
        }
        Ok(actions)
    }

    pub fn get_action(&self, action_id: i64) -> Result<Option<QuickAction>> {
        self.conn
            .query_row(
                "SELECT * FROM quick_launch_actions WHERE id = ?1",
                params![action_id],
                map_action,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn create_action(&self, draft: &QuickActionDraft) -> Result<QuickAction> {
        let now = now_label();
        let sort_order = draft
            .sort_order
            .unwrap_or_else(|| self.next_sort_order().unwrap_or(0));

        self.conn.execute(
            "
            INSERT INTO quick_launch_actions
                (name, description, kind, script_type, script_source, script_body, interpreter,
                 path, url, args_json, cwd, env_json, keywords_json, prefixes_json, icon,
                 feedback_mode, timeout_sec, enabled, sort_order, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)
            ",
            params![
                draft.name.trim(),
                draft.description,
                draft.kind.as_str(),
                draft.script_type.as_str(),
                draft.script_source.as_str(),
                draft.script_body,
                draft.interpreter,
                draft.path,
                draft.url,
                serde_json::to_string(&draft.args)?,
                draft.cwd,
                serde_json::to_string(&draft.env)?,
                serde_json::to_string(&draft.keywords)?,
                serde_json::to_string(&draft.prefixes)?,
                draft.icon,
                draft.feedback_mode.as_str(),
                draft.timeout_sec,
                if draft.enabled { 1 } else { 0 },
                sort_order,
                now,
                now,
            ],
        )?;
        let action_id = self.conn.last_insert_rowid();
        self.get_action(action_id)?
            .context("新建动作后无法重新读取记录")
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn update_action(&self, action_id: i64, draft: &QuickActionDraft) -> Result<bool> {
        let updated = self.conn.execute(
            "
            UPDATE quick_launch_actions
            SET name = ?1,
                description = ?2,
                kind = ?3,
                script_type = ?4,
                script_source = ?5,
                script_body = ?6,
                interpreter = ?7,
                path = ?8,
                url = ?9,
                args_json = ?10,
                cwd = ?11,
                env_json = ?12,
                keywords_json = ?13,
                prefixes_json = ?14,
                icon = ?15,
                feedback_mode = ?16,
                timeout_sec = ?17,
                enabled = ?18,
                sort_order = COALESCE(?19, sort_order),
                updated_at = ?20
            WHERE id = ?21
            ",
            params![
                draft.name.trim(),
                draft.description,
                draft.kind.as_str(),
                draft.script_type.as_str(),
                draft.script_source.as_str(),
                draft.script_body,
                draft.interpreter,
                draft.path,
                draft.url,
                serde_json::to_string(&draft.args)?,
                draft.cwd,
                serde_json::to_string(&draft.env)?,
                serde_json::to_string(&draft.keywords)?,
                serde_json::to_string(&draft.prefixes)?,
                draft.icon,
                draft.feedback_mode.as_str(),
                draft.timeout_sec,
                if draft.enabled { 1 } else { 0 },
                draft.sort_order,
                now_label(),
                action_id,
            ],
        )?;
        Ok(updated > 0)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn delete_action(&self, action_id: i64) -> Result<bool> {
        let deleted = self.conn.execute(
            "DELETE FROM quick_launch_actions WHERE id = ?1",
            params![action_id],
        )?;
        Ok(deleted > 0)
    }

    pub fn seed_defaults(&self, defaults: &[QuickActionDraft]) -> Result<usize> {
        if self.count_actions()? > 0 {
            return Ok(0);
        }

        let mut inserted = 0;
        for draft in defaults {
            self.create_action(draft)?;
            inserted += 1;
        }
        Ok(inserted)
    }

    pub fn record_run(&self, draft: &QuickRunDraft) -> Result<QuickRun> {
        self.conn.execute(
            "
            INSERT INTO quick_launch_runs
                (action_id, status, exit_code, stdout, stderr, duration_ms,
                 started_at, finished_at, message)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ",
            params![
                draft.action_id,
                draft.status.as_str(),
                draft.exit_code,
                draft.stdout,
                draft.stderr,
                draft.duration_ms,
                draft.started_at,
                draft.finished_at,
                draft.message,
            ],
        )?;
        let run_id = self.conn.last_insert_rowid();
        self.get_run(run_id)?
            .context("写入运行记录后无法重新读取记录")
    }

    pub fn list_runs(&self, action_id: i64, limit: usize) -> Result<Vec<QuickRun>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM quick_launch_runs
            WHERE action_id = ?1
            ORDER BY id DESC
            LIMIT ?2
            ",
        )?;
        let rows = stmt.query_map(params![action_id, limit as i64], map_run)?;
        let mut runs = Vec::new();
        for row in rows {
            runs.push(row?);
        }
        Ok(runs)
    }

    pub fn latest_run_for_actions(&self, action_ids: &[i64]) -> Result<HashMap<i64, QuickRun>> {
        if action_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = action_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT * FROM quick_launch_runs WHERE id IN (\
             SELECT MAX(id) FROM quick_launch_runs \
             WHERE action_id IN ({}) GROUP BY action_id)",
            placeholders
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(action_ids.iter().copied()),
            map_run,
        )?;
        let mut result = HashMap::new();
        for row in rows {
            let run = row?;
            result.insert(run.action_id, run);
        }
        Ok(result)
    }

    fn get_run(&self, run_id: i64) -> Result<Option<QuickRun>> {
        self.conn
            .query_row(
                "SELECT * FROM quick_launch_runs WHERE id = ?1",
                params![run_id],
                map_run,
            )
            .optional()
            .map_err(Into::into)
    }

    fn count_actions(&self) -> Result<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM quick_launch_actions", [], |row| {
                row.get(0)
            })
            .map_err(Into::into)
    }

    fn next_sort_order(&self) -> Result<i64> {
        self.conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM quick_launch_actions",
                [],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    fn ensure_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS quick_launch_actions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                kind TEXT NOT NULL DEFAULT 'script',
                script_type TEXT NOT NULL DEFAULT 'shell',
                script_source TEXT NOT NULL DEFAULT 'inline',
                script_body TEXT NOT NULL DEFAULT '',
                interpreter TEXT NOT NULL DEFAULT '',
                path TEXT NOT NULL DEFAULT '',
                url TEXT NOT NULL DEFAULT '',
                args_json TEXT NOT NULL DEFAULT '[]',
                cwd TEXT NOT NULL DEFAULT '',
                env_json TEXT NOT NULL DEFAULT '{}',
                keywords_json TEXT NOT NULL DEFAULT '[]',
                prefixes_json TEXT NOT NULL DEFAULT '[]',
                icon TEXT NOT NULL DEFAULT '',
                feedback_mode TEXT NOT NULL DEFAULT 'notification',
                timeout_sec INTEGER NOT NULL DEFAULT 300,
                enabled INTEGER NOT NULL DEFAULT 1,
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_quick_launch_actions_enabled
                ON quick_launch_actions(enabled, sort_order, id);

            CREATE TABLE IF NOT EXISTS quick_launch_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                action_id INTEGER NOT NULL,
                status TEXT NOT NULL,
                exit_code INTEGER,
                stdout TEXT NOT NULL DEFAULT '',
                stderr TEXT NOT NULL DEFAULT '',
                duration_ms INTEGER NOT NULL DEFAULT 0,
                started_at TEXT NOT NULL,
                finished_at TEXT NOT NULL,
                message TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_quick_launch_runs_action
                ON quick_launch_runs(action_id, id DESC);
            ",
        )?;
        self.ensure_action_columns()?;
        Ok(())
    }

    fn ensure_action_columns(&self) -> Result<()> {
        let columns = self.action_columns()?;
        self.ensure_action_column(
            &columns,
            "script_type",
            "ALTER TABLE quick_launch_actions ADD COLUMN script_type TEXT NOT NULL DEFAULT 'shell'",
        )?;
        self.ensure_action_column(
            &columns,
            "script_source",
            "ALTER TABLE quick_launch_actions ADD COLUMN script_source TEXT NOT NULL DEFAULT 'inline'",
        )?;
        self.ensure_action_column(
            &columns,
            "interpreter",
            "ALTER TABLE quick_launch_actions ADD COLUMN interpreter TEXT NOT NULL DEFAULT ''",
        )?;
        self.ensure_action_column(
            &columns,
            "env_json",
            "ALTER TABLE quick_launch_actions ADD COLUMN env_json TEXT NOT NULL DEFAULT '{}'",
        )?;
        Ok(())
    }

    fn ensure_action_column(&self, columns: &[String], name: &str, sql: &str) -> Result<()> {
        if !columns.iter().any(|column| column == name) {
            self.conn.execute(sql, [])?;
        }
        Ok(())
    }

    fn action_columns(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("PRAGMA table_info(quick_launch_actions)")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        let mut columns = Vec::new();
        for row in rows {
            columns.push(row?);
        }
        Ok(columns)
    }
}

fn map_action(row: &rusqlite::Row<'_>) -> rusqlite::Result<QuickAction> {
    let args_json: String = row.get("args_json")?;
    let env_json: String = row.get("env_json")?;
    let keywords_json: String = row.get("keywords_json")?;
    let prefixes_json: String = row.get("prefixes_json")?;
    let kind: String = row.get("kind")?;
    let script_type: String = row.get("script_type")?;
    let script_source: String = row.get("script_source")?;
    let feedback_mode: String = row.get("feedback_mode")?;

    Ok(QuickAction {
        id: row.get("id")?,
        name: row.get("name")?,
        description: row.get("description")?,
        kind: ActionKind::from_db(&kind),
        script_type: ScriptType::from_db(&script_type),
        script_source: ScriptSource::from_db(&script_source),
        script_body: row.get("script_body")?,
        interpreter: row.get("interpreter")?,
        path: row.get("path")?,
        url: row.get("url")?,
        args: serde_json::from_str(&args_json).unwrap_or_default(),
        cwd: row.get("cwd")?,
        env: serde_json::from_str(&env_json).unwrap_or_default(),
        keywords: serde_json::from_str(&keywords_json).unwrap_or_default(),
        prefixes: serde_json::from_str(&prefixes_json).unwrap_or_default(),
        icon: row.get("icon")?,
        feedback_mode: FeedbackMode::from_db(&feedback_mode),
        timeout_sec: row.get("timeout_sec")?,
        enabled: row.get::<_, i64>("enabled")? != 0,
        sort_order: row.get("sort_order")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn map_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<QuickRun> {
    let status: String = row.get("status")?;
    Ok(QuickRun {
        id: row.get("id")?,
        action_id: row.get("action_id")?,
        status: RunStatus::from_db(&status),
        exit_code: row.get("exit_code")?,
        stdout: row.get("stdout")?,
        stderr: row.get("stderr")?,
        duration_ms: row.get("duration_ms")?,
        started_at: row.get("started_at")?,
        finished_at: row.get("finished_at")?,
        message: row.get("message")?,
    })
}

fn now_label() -> String {
    let fmt = format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .format(&fmt)
        .unwrap_or_else(|_| String::from("1970-01-01 00:00:00"))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn temp_db(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-quick-launch-{nanos}"));
        let _ = fs::create_dir_all(&dir);
        dir.join(name)
    }

    fn sample_draft(name: &str) -> QuickActionDraft {
        let mut draft = QuickActionDraft::script(name, "demo action", "echo hello");
        draft.keywords = vec![String::from("demo")];
        draft.prefixes = vec![String::from("ql")];
        draft.icon = String::from("测");
        draft
    }

    #[test]
    fn seed_defaults_only_once() {
        let path = temp_db("seed.db");
        let store = QuickLaunchStore::open(&path).expect("store should open");
        let defaults = vec![sample_draft("动作一"), sample_draft("动作二")];

        assert_eq!(store.seed_defaults(&defaults).expect("first seed"), 2);
        assert_eq!(store.seed_defaults(&defaults).expect("second seed"), 0);
        assert_eq!(store.list_actions(None).expect("list should work").len(), 2);
    }

    #[test]
    fn create_update_delete_action() {
        let path = temp_db("crud.db");
        let store = QuickLaunchStore::open(&path).expect("store should open");
        let created = store
            .create_action(&sample_draft("原始动作"))
            .expect("create should work");

        let mut updated = sample_draft("更新动作");
        updated.enabled = false;
        updated.kind = ActionKind::OpenUrl;
        updated.url = String::from("https://openai.com");
        updated.script_source = ScriptSource::Path;
        updated.interpreter = String::from("/usr/bin/env node");
        updated
            .env
            .insert(String::from("MODE"), String::from("prod"));
        assert!(
            store
                .update_action(created.id, &updated)
                .expect("update should work")
        );

        let loaded = store
            .get_action(created.id)
            .expect("get should work")
            .expect("action should exist");
        assert_eq!(loaded.name, "更新动作");
        assert!(!loaded.enabled);
        assert_eq!(loaded.kind, ActionKind::OpenUrl);
        assert_eq!(loaded.script_source, ScriptSource::Path);
        assert_eq!(loaded.interpreter, "/usr/bin/env node");
        assert_eq!(loaded.env.get("MODE"), Some(&String::from("prod")));

        assert!(store.delete_action(created.id).expect("delete should work"));
        assert!(
            store
                .get_action(created.id)
                .expect("get should work")
                .is_none()
        );
    }

    #[test]
    fn records_run_history() {
        let path = temp_db("runs.db");
        let store = QuickLaunchStore::open(&path).expect("store should open");
        let action = store
            .create_action(&sample_draft("运行动作"))
            .expect("action should create");

        let run = store
            .record_run(&QuickRunDraft {
                action_id: action.id,
                status: RunStatus::Success,
                exit_code: Some(0),
                stdout: String::from("ok"),
                stderr: String::new(),
                duration_ms: 12,
                started_at: String::from("2026-05-25 20:00:00"),
                finished_at: String::from("2026-05-25 20:00:01"),
                message: String::from("已执行"),
            })
            .expect("record should work");

        let runs = store.list_runs(action.id, 10).expect("list should work");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, run.id);
        assert_eq!(runs[0].stdout, "ok");
    }

    #[test]
    fn migrates_missing_action_columns() {
        let path = temp_db("migration.db");
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let conn = Connection::open(&path).expect("connection should open");
        conn.execute_batch(
            "
            CREATE TABLE quick_launch_actions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                kind TEXT NOT NULL DEFAULT 'script',
                script_body TEXT NOT NULL DEFAULT '',
                path TEXT NOT NULL DEFAULT '',
                url TEXT NOT NULL DEFAULT '',
                args_json TEXT NOT NULL DEFAULT '[]',
                cwd TEXT NOT NULL DEFAULT '',
                keywords_json TEXT NOT NULL DEFAULT '[]',
                prefixes_json TEXT NOT NULL DEFAULT '[]',
                icon TEXT NOT NULL DEFAULT '',
                feedback_mode TEXT NOT NULL DEFAULT 'notification',
                timeout_sec INTEGER NOT NULL DEFAULT 300,
                enabled INTEGER NOT NULL DEFAULT 1,
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            ",
        )
        .expect("legacy schema should create");
        conn.execute(
            "
            INSERT INTO quick_launch_actions
                (name, description, kind, script_body, path, url, args_json, cwd,
                 keywords_json, prefixes_json, icon, feedback_mode, timeout_sec,
                 enabled, sort_order, created_at, updated_at)
            VALUES (?1, '', 'script', 'echo legacy', '', '', '[]', '', '[]', '[]', '', 'notification', 300, 1, 0, '2026-01-01 00:00:00', '2026-01-01 00:00:00')
            ",
            params!["旧动作"],
        )
        .expect("legacy row should insert");
        drop(conn);

        let store = QuickLaunchStore::open(&path).expect("store should migrate");
        let action = store
            .list_actions(None)
            .expect("actions should load")
            .into_iter()
            .next()
            .expect("row should exist");
        assert_eq!(action.script_type, ScriptType::Shell);
        assert_eq!(action.script_source, ScriptSource::Inline);
        assert_eq!(action.interpreter, "");
        assert!(action.env.is_empty());
        assert_eq!(action.script_body, "echo legacy");
    }

    #[test]
    fn latest_run_for_actions_returns_most_recent_per_action() {
        let path = temp_db("latest_runs.db");
        let store = QuickLaunchStore::open(&path).expect("store should open");
        let a1 = store
            .create_action(&sample_draft("动作一"))
            .expect("action 1 should create");
        let a2 = store
            .create_action(&sample_draft("动作二"))
            .expect("action 2 should create");

        // Record two runs for action 1, one for action 2
        store
            .record_run(&QuickRunDraft {
                action_id: a1.id,
                status: RunStatus::Success,
                exit_code: Some(0),
                stdout: String::from("first"),
                stderr: String::new(),
                duration_ms: 10,
                started_at: String::from("2026-05-28 10:00:00"),
                finished_at: String::from("2026-05-28 10:00:01"),
                message: String::from("ok"),
            })
            .expect("first run should record");
        store
            .record_run(&QuickRunDraft {
                action_id: a1.id,
                status: RunStatus::Failed,
                exit_code: Some(1),
                stdout: String::new(),
                stderr: String::from("boom"),
                duration_ms: 20,
                started_at: String::from("2026-05-28 10:01:00"),
                finished_at: String::from("2026-05-28 10:01:01"),
                message: String::from("失败"),
            })
            .expect("second run should record");
        store
            .record_run(&QuickRunDraft {
                action_id: a2.id,
                status: RunStatus::Timeout,
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                duration_ms: 5000,
                started_at: String::from("2026-05-28 10:02:00"),
                finished_at: String::from("2026-05-28 10:02:05"),
                message: String::from("超时"),
            })
            .expect("third run should record");

        let latest = store
            .latest_run_for_actions(&[a1.id, a2.id])
            .expect("latest runs should load");

        assert_eq!(latest.len(), 2);
        assert_eq!(latest.get(&a1.id).unwrap().status, RunStatus::Failed);
        assert_eq!(latest.get(&a1.id).unwrap().stdout, "");
        assert_eq!(latest.get(&a1.id).unwrap().stderr, "boom");
        assert_eq!(latest.get(&a2.id).unwrap().status, RunStatus::Timeout);
        assert_eq!(latest.get(&a2.id).unwrap().duration_ms, 5000);
    }

    #[test]
    fn latest_run_for_actions_empty_input() {
        let path = temp_db("latest_empty.db");
        let store = QuickLaunchStore::open(&path).expect("store should open");
        let result = store
            .latest_run_for_actions(&[])
            .expect("empty input should work");
        assert!(result.is_empty());
    }

    #[test]
    fn latest_run_for_actions_skips_actions_without_runs() {
        let path = temp_db("latest_partial.db");
        let store = QuickLaunchStore::open(&path).expect("store should open");
        let a1 = store
            .create_action(&sample_draft("有运行记录"))
            .expect("action should create");
        let a2 = store
            .create_action(&sample_draft("无运行记录"))
            .expect("action should create");

        store
            .record_run(&QuickRunDraft {
                action_id: a1.id,
                status: RunStatus::Success,
                exit_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
                duration_ms: 5,
                started_at: String::from("2026-05-28 10:00:00"),
                finished_at: String::from("2026-05-28 10:00:01"),
                message: String::from("ok"),
            })
            .expect("run should record");

        let latest = store
            .latest_run_for_actions(&[a1.id, a2.id])
            .expect("latest runs should load");
        assert_eq!(latest.len(), 1);
        assert!(latest.contains_key(&a1.id));
        assert!(!latest.contains_key(&a2.id));
    }
}
