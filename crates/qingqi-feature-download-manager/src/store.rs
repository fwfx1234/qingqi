use std::sync::Arc;

use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params};
use time::{OffsetDateTime, macros::format_description};

use qingqi_plugin::database::{DatabaseService, PooledConnection, SqlitePool};

use super::model::{DownloadTask, FileCategory, TaskStatus};

pub struct DownloadStore {
    pool: SqlitePool,
}

impl DownloadStore {
    pub fn open(database: Arc<DatabaseService>, key: &str) -> Result<Self> {
        let pool = database.pool(key)?;
        let store = Self { pool };
        store.ensure_schema()?;
        Ok(store)
    }

    fn ensure_schema(&self) -> Result<()> {
        let conn = self.connection()?;
        conn.execute_batch(SCHEMA)?;

        let version: i64 = conn.query_row(READ_SCHEMA_VERSION, [], |row| row.get(0))?;
        if version < SCHEMA_VERSION {
            if version < 2 {
                conn.execute_batch(SETTINGS_TABLE_MIGRATION)?;
            }
            conn.execute(UPSERT_SCHEMA_VERSION, params![SCHEMA_VERSION])?;
        }
        Ok(())
    }

    pub fn insert_task(&self, task: &DownloadTask) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            INSERT_TASK,
            params![
                task.id,
                task.url,
                task.file_name,
                task.save_path,
                task.file_size,
                task.downloaded,
                status_to_db(task.status),
                category_to_db(task.category),
                task.error_msg,
                task.speed_bps,
                task.created_at,
                task.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_task(&self, task: &DownloadTask) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            UPDATE_TASK,
            params![
                task.id,
                task.url,
                task.file_name,
                task.save_path,
                task.file_size,
                task.downloaded,
                status_to_db(task.status),
                category_to_db(task.category),
                task.error_msg,
                task.speed_bps,
                now_label(),
            ],
        )?;
        Ok(())
    }

    pub fn update_progress(
        &self,
        id: &str,
        downloaded: u64,
        speed_bps: f64,
        status: TaskStatus,
    ) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            UPDATE_PROGRESS,
            params![id, downloaded, speed_bps, status_to_db(status), now_label()],
        )?;
        Ok(())
    }

    pub fn update_status(&self, id: &str, status: TaskStatus, error_msg: &str) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            UPDATE_STATUS,
            params![id, status_to_db(status), error_msg, now_label()],
        )?;
        Ok(())
    }

    pub fn get_task(&self, id: &str) -> Result<Option<DownloadTask>> {
        let conn = self.connection()?;
        conn.query_row(GET_TASK, params![id], map_task)
            .optional()
            .map_err(Into::into)
    }

    pub fn list_tasks(&self, status_filter: Option<TaskStatus>) -> Result<Vec<DownloadTask>> {
        let conn = self.connection()?;
        let sql = if status_filter.is_some() {
            LIST_TASKS_BY_STATUS
        } else {
            LIST_TASKS_ALL
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = if let Some(status) = status_filter {
            stmt.query_map(params![status_to_db(status)], map_task)?
        } else {
            stmt.query_map([], map_task)?
        };

        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }
        Ok(tasks)
    }

    pub fn list_tasks_by_category(&self, category: FileCategory) -> Result<Vec<DownloadTask>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(LIST_TASKS_BY_CATEGORY)?;
        let rows = stmt.query_map(params![category_to_db(category)], map_task)?;
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }
        Ok(tasks)
    }

    pub fn list_active_tasks(&self) -> Result<Vec<DownloadTask>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(LIST_ACTIVE_TASKS)?;
        let rows = stmt.query_map([], map_task)?;
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }
        Ok(tasks)
    }

    pub fn delete_task(&self, id: &str) -> Result<bool> {
        let conn = self.connection()?;
        let affected = conn.execute(DELETE_TASK, params![id])?;
        Ok(affected > 0)
    }

    pub fn clear_completed(&self) -> Result<usize> {
        let conn = self.connection()?;
        let affected = conn.execute(CLEAR_COMPLETED, [])?;
        Ok(affected)
    }

    pub fn clear_failed(&self) -> Result<usize> {
        let conn = self.connection()?;
        let affected = conn.execute(CLEAR_FAILED, [])?;
        Ok(affected)
    }

    pub fn load_settings(&self) -> Result<Vec<(String, String)>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(LOAD_SETTINGS)?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut pairs = Vec::new();
        for row in rows {
            pairs.push(row?);
        }
        Ok(pairs)
    }

    pub fn save_settings(&self, settings: &[(&str, &str)]) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let conn = self.connection()?;
        for (key, value) in settings {
            conn.execute(UPSERT_SETTING, params![key, value, now])?;
        }
        Ok(())
    }

    pub fn stats(&self) -> Result<DownloadStats> {
        let conn = self.connection()?;
        let total: i64 = conn.query_row(COUNT_TOTAL, [], |row| row.get(0))?;
        let completed: i64 = conn.query_row(COUNT_COMPLETED, [], |row| row.get(0))?;
        let active: i64 = conn.query_row(COUNT_ACTIVE, [], |row| row.get(0))?;
        let failed: i64 = conn.query_row(COUNT_FAILED, [], |row| row.get(0))?;
        let total_bytes: Option<i64> = conn.query_row(SUM_DOWNLOADED, [], |row| row.get(0))?;

        Ok(DownloadStats {
            total: total as usize,
            completed: completed as usize,
            active: active as usize,
            failed: failed as usize,
            total_downloaded: total_bytes.unwrap_or(0) as u64,
        })
    }

    pub fn task_counts(&self) -> Result<TaskCounts> {
        let conn = self.connection()?;
        let mut counts = TaskCounts::default();
        let mut stmt = conn.prepare(COUNT_BY_STATUS_AND_CATEGORY)?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? as usize,
            ))
        })?;
        for row in rows {
            let (status, category, count) = row?;
            match status.as_str() {
                "Pending" => counts.pending += count,
                "Downloading" => counts.downloading += count,
                "Paused" => counts.paused += count,
                "Completed" => counts.completed += count,
                "Failed" => counts.failed += count,
                "Cancelled" => counts.cancelled += count,
                _ => {}
            }
            match category.as_str() {
                "Video" => counts.video += count,
                "Audio" => counts.audio += count,
                "Document" => counts.document += count,
                "Archive" => counts.archive += count,
                "Image" => counts.image += count,
                "Software" => counts.software += count,
                _ => counts.other += count,
            }
            counts.total += count;
        }
        Ok(counts)
    }

    fn connection(&self) -> Result<PooledConnection> {
        self.pool
            .get()
            .context("cannot get download manager pooled connection")
    }
}

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS schema_info (
    version INTEGER PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS download_tasks (
    id          TEXT PRIMARY KEY,
    url         TEXT NOT NULL DEFAULT '',
    file_name   TEXT NOT NULL DEFAULT '',
    save_path   TEXT NOT NULL DEFAULT '',
    file_size   INTEGER,
    downloaded  INTEGER NOT NULL DEFAULT 0,
    status      TEXT NOT NULL DEFAULT 'Pending',
    category    TEXT NOT NULL DEFAULT 'Other',
    error_msg   TEXT NOT NULL DEFAULT '',
    speed_bps   REAL NOT NULL DEFAULT 0.0,
    created_at  TEXT NOT NULL DEFAULT '',
    updated_at  TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_task_status
    ON download_tasks(status);
CREATE INDEX IF NOT EXISTS idx_task_created
    ON download_tasks(created_at DESC);

CREATE TABLE IF NOT EXISTS download_manager_settings (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL DEFAULT '',
    updated_at  INTEGER NOT NULL DEFAULT 0
);
";

pub const SETTINGS_TABLE_MIGRATION: &str = "
CREATE TABLE IF NOT EXISTS download_manager_settings (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL DEFAULT '',
    updated_at  INTEGER NOT NULL DEFAULT 0
);
";

pub const SCHEMA_VERSION: i64 = 2;
pub const READ_SCHEMA_VERSION: &str = "SELECT COALESCE(MAX(version), 0) FROM schema_info";
pub const UPSERT_SCHEMA_VERSION: &str = "INSERT OR REPLACE INTO schema_info (version) VALUES (?1)";

pub const INSERT_TASK: &str = "
INSERT INTO download_tasks
     (id, url, file_name, save_path, file_size, downloaded,
      status, category, error_msg, speed_bps, created_at, updated_at)
 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
";

pub const UPDATE_TASK: &str = "
UPDATE download_tasks
    SET url = ?2, file_name = ?3, save_path = ?4, file_size = ?5,
        downloaded = ?6, status = ?7, category = ?8, error_msg = ?9,
        speed_bps = ?10, updated_at = ?11
  WHERE id = ?1
";

pub const UPDATE_PROGRESS: &str = "
UPDATE download_tasks
    SET downloaded = ?2, speed_bps = ?3, status = ?4, updated_at = ?5
  WHERE id = ?1
";

pub const UPDATE_STATUS: &str = "
UPDATE download_tasks
    SET status = ?2, error_msg = ?3, speed_bps = 0.0, updated_at = ?4
  WHERE id = ?1
";

pub const GET_TASK: &str = "
SELECT id, url, file_name, save_path, file_size, downloaded,
       status, category, error_msg, speed_bps, created_at, updated_at
  FROM download_tasks WHERE id = ?1
";

pub const LIST_TASKS_ALL: &str = "
SELECT id, url, file_name, save_path, file_size, downloaded,
       status, category, error_msg, speed_bps, created_at, updated_at
  FROM download_tasks
 ORDER BY created_at DESC
";

pub const LIST_TASKS_BY_STATUS: &str = "
SELECT id, url, file_name, save_path, file_size, downloaded,
       status, category, error_msg, speed_bps, created_at, updated_at
  FROM download_tasks WHERE status = ?1
 ORDER BY created_at DESC
";

pub const LIST_TASKS_BY_CATEGORY: &str = "
SELECT id, url, file_name, save_path, file_size, downloaded,
       status, category, error_msg, speed_bps, created_at, updated_at
  FROM download_tasks WHERE category = ?1
 ORDER BY created_at DESC
";

pub const LIST_ACTIVE_TASKS: &str = "
SELECT id, url, file_name, save_path, file_size, downloaded,
       status, category, error_msg, speed_bps, created_at, updated_at
  FROM download_tasks
 WHERE status IN ('Downloading', 'Pending')
 ORDER BY created_at ASC
";

pub const DELETE_TASK: &str = "DELETE FROM download_tasks WHERE id = ?1";
pub const CLEAR_COMPLETED: &str =
    "DELETE FROM download_tasks WHERE status IN ('Completed', 'Cancelled')";
pub const CLEAR_FAILED: &str = "DELETE FROM download_tasks WHERE status IN ('Failed', 'Cancelled')";

pub const LOAD_SETTINGS: &str = "SELECT key, value FROM download_manager_settings";
pub const UPSERT_SETTING: &str = "
INSERT INTO download_manager_settings (key, value, updated_at)
VALUES (?1, ?2, ?3)
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
";

pub const COUNT_TOTAL: &str = "SELECT COUNT(*) FROM download_tasks";
pub const COUNT_COMPLETED: &str = "SELECT COUNT(*) FROM download_tasks WHERE status = 'Completed'";
pub const COUNT_ACTIVE: &str =
    "SELECT COUNT(*) FROM download_tasks WHERE status IN ('Downloading', 'Pending')";
pub const COUNT_FAILED: &str = "SELECT COUNT(*) FROM download_tasks WHERE status = 'Failed'";
pub const SUM_DOWNLOADED: &str = "SELECT SUM(downloaded) FROM download_tasks";
pub const COUNT_BY_STATUS_AND_CATEGORY: &str =
    "SELECT status, category, COUNT(*) FROM download_tasks GROUP BY status, category";

#[derive(Clone, Debug, Default)]
pub struct DownloadStats {
    pub total: usize,
    pub completed: usize,
    pub active: usize,
    pub failed: usize,
    pub total_downloaded: u64,
}

#[derive(Clone, Debug, Default)]
pub struct TaskCounts {
    pub total: usize,
    pub pending: usize,
    pub downloading: usize,
    pub paused: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub video: usize,
    pub audio: usize,
    pub document: usize,
    pub archive: usize,
    pub image: usize,
    pub software: usize,
    pub other: usize,
}

impl TaskCounts {
    pub fn active(&self) -> usize {
        self.pending + self.downloading
    }
}

fn map_task(row: &rusqlite::Row) -> std::result::Result<DownloadTask, rusqlite::Error> {
    Ok(DownloadTask {
        id: row.get(0)?,
        url: row.get(1)?,
        file_name: row.get(2)?,
        save_path: row.get(3)?,
        file_size: row.get::<_, Option<i64>>(4)?.map(|v| v as u64),
        downloaded: row.get::<_, i64>(5)? as u64,
        status: status_from_db(&row.get::<_, String>(6)?),
        category: category_from_db(&row.get::<_, String>(7)?),
        error_msg: row.get(8)?,
        speed_bps: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn status_to_db(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "Pending",
        TaskStatus::Downloading => "Downloading",
        TaskStatus::Paused => "Paused",
        TaskStatus::Completed => "Completed",
        TaskStatus::Failed => "Failed",
        TaskStatus::Cancelled => "Cancelled",
    }
}

fn status_from_db(s: &str) -> TaskStatus {
    match s {
        "Downloading" => TaskStatus::Downloading,
        "Paused" => TaskStatus::Paused,
        "Completed" => TaskStatus::Completed,
        "Failed" => TaskStatus::Failed,
        "Cancelled" => TaskStatus::Cancelled,
        _ => TaskStatus::Pending,
    }
}

fn category_to_db(cat: FileCategory) -> &'static str {
    match cat {
        FileCategory::Video => "Video",
        FileCategory::Audio => "Audio",
        FileCategory::Document => "Document",
        FileCategory::Archive => "Archive",
        FileCategory::Image => "Image",
        FileCategory::Software => "Software",
        FileCategory::Other => "Other",
    }
}

fn category_from_db(s: &str) -> FileCategory {
    match s {
        "Video" => FileCategory::Video,
        "Audio" => FileCategory::Audio,
        "Document" => FileCategory::Document,
        "Archive" => FileCategory::Archive,
        "Image" => FileCategory::Image,
        "Software" => FileCategory::Software,
        _ => FileCategory::Other,
    }
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
    use super::*;
    use qingqi_plugin::{database::DatabaseService, storage::AppPaths};
    use std::{
        fs,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_db() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let suffix = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("qingqi-download-store-{nanos}-{suffix}"));
        let _ = fs::create_dir_all(&dir);
        dir.join("test.db")
    }

    fn open_test_store() -> DownloadStore {
        let path = temp_db();
        let database = Arc::new(DatabaseService::new(AppPaths::for_test(
            path.parent().unwrap().to_path_buf(),
        )));
        database
            .register_database(qingqi_plugin::database::DatabaseSpec::path(
                qingqi_plugin::database::feature_database_key("download-manager", "tasks"),
                path,
            ))
            .unwrap();
        DownloadStore::open(
            database,
            &qingqi_plugin::database::feature_database_key("download-manager", "tasks"),
        )
        .unwrap()
    }

    fn make_task(id: &str, name: &str, status: TaskStatus) -> DownloadTask {
        DownloadTask {
            id: id.to_string(),
            url: format!("https://example.com/{name}"),
            file_name: name.to_string(),
            save_path: format!("/tmp/{name}"),
            file_size: Some(1024),
            downloaded: 0,
            status,
            category: FileCategory::Other,
            error_msg: String::new(),
            speed_bps: 0.0,
            created_at: "2025-01-01 00:00:00".into(),
            updated_at: "2025-01-01 00:00:00".into(),
        }
    }

    #[test]
    fn insert_and_get_task() {
        let store = open_test_store();
        let task = make_task("t1", "file.zip", TaskStatus::Pending);
        store.insert_task(&task).unwrap();

        let loaded = store.get_task("t1").unwrap().unwrap();
        assert_eq!(loaded.file_name, "file.zip");
        assert_eq!(loaded.status, TaskStatus::Pending);
    }

    #[test]
    fn update_progress() {
        let store = open_test_store();
        store
            .insert_task(&make_task("t1", "file.zip", TaskStatus::Downloading))
            .unwrap();

        store
            .update_progress("t1", 512, 1024.0, TaskStatus::Downloading)
            .unwrap();
        let loaded = store.get_task("t1").unwrap().unwrap();
        assert_eq!(loaded.downloaded, 512);
        assert!((loaded.speed_bps - 1024.0).abs() < 0.01);
    }

    #[test]
    fn list_with_filter() {
        let store = open_test_store();
        store
            .insert_task(&make_task("t1", "a.zip", TaskStatus::Pending))
            .unwrap();
        store
            .insert_task(&make_task("t2", "b.zip", TaskStatus::Completed))
            .unwrap();
        store
            .insert_task(&make_task("t3", "c.zip", TaskStatus::Downloading))
            .unwrap();

        let all = store.list_tasks(None).unwrap();
        assert_eq!(all.len(), 3);

        let active = store.list_active_tasks().unwrap();
        assert_eq!(active.len(), 2);
    }

    #[test]
    fn delete_and_clear() {
        let store = open_test_store();
        store
            .insert_task(&make_task("t1", "a.zip", TaskStatus::Completed))
            .unwrap();
        store
            .insert_task(&make_task("t2", "b.zip", TaskStatus::Cancelled))
            .unwrap();
        store
            .insert_task(&make_task("t3", "c.zip", TaskStatus::Pending))
            .unwrap();

        let cleared = store.clear_completed().unwrap();
        assert_eq!(cleared, 2);
        assert_eq!(store.list_tasks(None).unwrap().len(), 1);
    }

    #[test]
    fn stats_count() {
        let store = open_test_store();
        store
            .insert_task(&make_task("t1", "a.zip", TaskStatus::Completed))
            .unwrap();
        store
            .insert_task(&make_task("t2", "b.zip", TaskStatus::Downloading))
            .unwrap();
        store
            .insert_task(&make_task("t3", "c.zip", TaskStatus::Failed))
            .unwrap();

        let stats = store.stats().unwrap();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.completed, 1);
        assert_eq!(stats.active, 1);
        assert_eq!(stats.failed, 1);
    }

    #[test]
    fn clear_failed_removes_failed_and_cancelled() {
        let store = open_test_store();
        store
            .insert_task(&make_task("t1", "a.zip", TaskStatus::Failed))
            .unwrap();
        store
            .insert_task(&make_task("t2", "b.zip", TaskStatus::Cancelled))
            .unwrap();
        store
            .insert_task(&make_task("t3", "c.zip", TaskStatus::Completed))
            .unwrap();
        store
            .insert_task(&make_task("t4", "d.zip", TaskStatus::Pending))
            .unwrap();

        let cleared = store.clear_failed().unwrap();
        assert_eq!(cleared, 2);
        let remaining = store.list_tasks(None).unwrap();
        assert_eq!(remaining.len(), 2);
        let states: Vec<TaskStatus> = remaining.iter().map(|t| t.status).collect();
        assert!(states.contains(&TaskStatus::Completed));
        assert!(states.contains(&TaskStatus::Pending));
    }

    #[test]
    fn settings_save_and_load() {
        let store = open_test_store();
        store
            .save_settings(&[
                ("saveRoot", "/tmp/downloads"),
                ("maxConcurrent", "5"),
                ("proxyUrl", "http://127.0.0.1:7890"),
            ])
            .unwrap();

        let pairs = store.load_settings().unwrap();
        assert!(
            pairs
                .iter()
                .any(|(k, v)| k == "saveRoot" && v == "/tmp/downloads")
        );
        assert!(pairs.iter().any(|(k, v)| k == "maxConcurrent" && v == "5"));
        assert!(
            pairs
                .iter()
                .any(|(k, v)| k == "proxyUrl" && v == "http://127.0.0.1:7890")
        );
    }

    #[test]
    fn settings_overwrite() {
        let store = open_test_store();
        store.save_settings(&[("maxConcurrent", "3")]).unwrap();
        store.save_settings(&[("maxConcurrent", "8")]).unwrap();

        let pairs = store.load_settings().unwrap();
        let vals: Vec<_> = pairs
            .iter()
            .filter(|(k, _)| k == "maxConcurrent")
            .map(|(_, v)| v.as_str())
            .collect();
        assert_eq!(vals, vec!["8"]);
    }

    #[test]
    fn task_counts_breakdown() {
        let store = open_test_store();
        let mut t1 = make_task("t1", "a.zip", TaskStatus::Pending);
        t1.category = FileCategory::Archive;
        store.insert_task(&t1).unwrap();

        let mut t2 = make_task("t2", "b.mp4", TaskStatus::Downloading);
        t2.category = FileCategory::Video;
        store.insert_task(&t2).unwrap();

        let mut t3 = make_task("t3", "c.pdf", TaskStatus::Completed);
        t3.category = FileCategory::Document;
        store.insert_task(&t3).unwrap();

        let mut t4 = make_task("t4", "d.zip", TaskStatus::Paused);
        t4.category = FileCategory::Archive;
        store.insert_task(&t4).unwrap();

        let mut t5 = make_task("t5", "e.exe", TaskStatus::Failed);
        t5.category = FileCategory::Software;
        store.insert_task(&t5).unwrap();

        let counts = store.task_counts().unwrap();
        assert_eq!(counts.total, 5);
        assert_eq!(counts.pending, 1);
        assert_eq!(counts.downloading, 1);
        assert_eq!(counts.paused, 1);
        assert_eq!(counts.completed, 1);
        assert_eq!(counts.failed, 1);
        assert_eq!(counts.active(), 2);
        assert_eq!(counts.archive, 2);
        assert_eq!(counts.video, 1);
        assert_eq!(counts.document, 1);
        assert_eq!(counts.software, 1);
        assert_eq!(counts.other, 0);
    }

    #[test]
    fn list_tasks_by_category() {
        let store = open_test_store();
        let mut t1 = make_task("t1", "a.mp4", TaskStatus::Pending);
        t1.category = FileCategory::Video;
        store.insert_task(&t1).unwrap();

        let mut t2 = make_task("t2", "b.zip", TaskStatus::Completed);
        t2.category = FileCategory::Archive;
        store.insert_task(&t2).unwrap();

        let mut t3 = make_task("t3", "c.mp4", TaskStatus::Downloading);
        t3.category = FileCategory::Video;
        store.insert_task(&t3).unwrap();

        let all = store.list_tasks(None).unwrap();
        assert_eq!(all.len(), 3);

        let videos: Vec<_> = all
            .iter()
            .filter(|t| t.category == FileCategory::Video)
            .collect();
        assert_eq!(videos.len(), 2);

        let archives: Vec<_> = all
            .iter()
            .filter(|t| t.category == FileCategory::Archive)
            .collect();
        assert_eq!(archives.len(), 1);
    }

    #[test]
    fn list_tasks_by_category_sql() {
        let store = open_test_store();
        let mut t1 = make_task("t1", "a.pdf", TaskStatus::Completed);
        t1.category = FileCategory::Document;
        store.insert_task(&t1).unwrap();

        let mut t2 = make_task("t2", "b.pdf", TaskStatus::Pending);
        t2.category = FileCategory::Document;
        store.insert_task(&t2).unwrap();

        let mut t3 = make_task("t3", "c.zip", TaskStatus::Downloading);
        t3.category = FileCategory::Archive;
        store.insert_task(&t3).unwrap();

        let documents = store
            .list_tasks_by_category(FileCategory::Document)
            .unwrap();
        assert_eq!(documents.len(), 2);
        assert!(
            documents
                .iter()
                .all(|t| t.category == FileCategory::Document)
        );

        let archives = store.list_tasks_by_category(FileCategory::Archive).unwrap();
        assert_eq!(archives.len(), 1);
        assert_eq!(archives[0].category, FileCategory::Archive);
    }
}
