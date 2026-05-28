use std::{fs, path::Path};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use time::{OffsetDateTime, macros::format_description};

use super::model::{DownloadTask, FileCategory, TaskStatus};

const SCHEMA_VERSION: i64 = 2;

pub struct DownloadStore {
    conn: Connection,
}

impl DownloadStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("无法创建下载管理器目录 {}", parent.display()))?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let store = Self { conn };
        store.ensure_schema()?;
        Ok(store)
    }

    fn ensure_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "
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
            ",
        )?;

        let version: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_info",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if version < SCHEMA_VERSION {
            // v1 -> v2: add settings table
            if version < 2 {
                self.conn.execute_batch(
                    "
                    CREATE TABLE IF NOT EXISTS download_manager_settings (
                        key         TEXT PRIMARY KEY,
                        value       TEXT NOT NULL DEFAULT '',
                        updated_at  INTEGER NOT NULL DEFAULT 0
                    );
                    ",
                )?;
            }

            self.conn.execute(
                "INSERT OR REPLACE INTO schema_info (version) VALUES (?1)",
                params![SCHEMA_VERSION],
            )?;
        }

        Ok(())
    }

    pub fn insert_task(&self, task: &DownloadTask) -> Result<()> {
        self.conn.execute(
            "INSERT INTO download_tasks
                 (id, url, file_name, save_path, file_size, downloaded,
                  status, category, error_msg, speed_bps, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
        self.conn.execute(
            "UPDATE download_tasks
                SET url = ?2, file_name = ?3, save_path = ?4, file_size = ?5,
                    downloaded = ?6, status = ?7, category = ?8, error_msg = ?9,
                    speed_bps = ?10, updated_at = ?11
              WHERE id = ?1",
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
        self.conn.execute(
            "UPDATE download_tasks
                SET downloaded = ?2, speed_bps = ?3, status = ?4, updated_at = ?5
              WHERE id = ?1",
            params![id, downloaded, speed_bps, status_to_db(status), now_label()],
        )?;
        Ok(())
    }

    pub fn update_status(&self, id: &str, status: TaskStatus, error_msg: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE download_tasks
                SET status = ?2, error_msg = ?3, speed_bps = 0.0, updated_at = ?4
              WHERE id = ?1",
            params![id, status_to_db(status), error_msg, now_label()],
        )?;
        Ok(())
    }

    pub fn get_task(&self, id: &str) -> Result<Option<DownloadTask>> {
        self.conn
            .query_row(
                "SELECT id, url, file_name, save_path, file_size, downloaded,
                        status, category, error_msg, speed_bps, created_at, updated_at
                   FROM download_tasks WHERE id = ?1",
                params![id],
                map_task,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list_tasks(&self, status_filter: Option<TaskStatus>) -> Result<Vec<DownloadTask>> {
        let sql = if let Some(_) = &status_filter {
            "SELECT id, url, file_name, save_path, file_size, downloaded,
                    status, category, error_msg, speed_bps, created_at, updated_at
               FROM download_tasks WHERE status = ?1
              ORDER BY created_at DESC"
        } else {
            "SELECT id, url, file_name, save_path, file_size, downloaded,
                    status, category, error_msg, speed_bps, created_at, updated_at
               FROM download_tasks
              ORDER BY created_at DESC"
        };

        let mut stmt = self.conn.prepare(sql)?;
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
        let mut stmt = self.conn.prepare(
            "SELECT id, url, file_name, save_path, file_size, downloaded,
                    status, category, error_msg, speed_bps, created_at, updated_at
               FROM download_tasks WHERE category = ?1
              ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![category_to_db(category)], map_task)?;
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }
        Ok(tasks)
    }

    pub fn list_active_tasks(&self) -> Result<Vec<DownloadTask>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, url, file_name, save_path, file_size, downloaded,
                    status, category, error_msg, speed_bps, created_at, updated_at
               FROM download_tasks
              WHERE status IN ('Downloading', 'Pending')
              ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], map_task)?;
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }
        Ok(tasks)
    }

    pub fn delete_task(&self, id: &str) -> Result<bool> {
        let affected = self
            .conn
            .execute("DELETE FROM download_tasks WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn clear_completed(&self) -> Result<usize> {
        let affected = self.conn.execute(
            "DELETE FROM download_tasks WHERE status IN ('Completed', 'Cancelled')",
            [],
        )?;
        Ok(affected)
    }

    pub fn clear_failed(&self) -> Result<usize> {
        let affected = self.conn.execute(
            "DELETE FROM download_tasks WHERE status IN ('Failed', 'Cancelled')",
            [],
        )?;
        Ok(affected)
    }

    // ── settings ──

    pub fn load_settings(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM download_manager_settings")?;
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
        for (key, value) in settings {
            self.conn.execute(
                "INSERT INTO download_manager_settings (key, value, updated_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![key, value, now],
            )?;
        }
        Ok(())
    }

    pub fn stats(&self) -> Result<DownloadStats> {
        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM download_tasks", [], |row| row.get(0))?;
        let completed: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM download_tasks WHERE status = 'Completed'",
            [],
            |row| row.get(0),
        )?;
        let active: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM download_tasks WHERE status IN ('Downloading', 'Pending')",
            [],
            |row| row.get(0),
        )?;
        let failed: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM download_tasks WHERE status = 'Failed'",
            [],
            |row| row.get(0),
        )?;
        let total_bytes: Option<i64> =
            self.conn
                .query_row("SELECT SUM(downloaded) FROM download_tasks", [], |row| {
                    row.get(0)
                })?;

        Ok(DownloadStats {
            total: total as usize,
            completed: completed as usize,
            active: active as usize,
            failed: failed as usize,
            total_downloaded: total_bytes.unwrap_or(0) as u64,
        })
    }

    pub fn task_counts(&self) -> Result<TaskCounts> {
        let mut counts = TaskCounts::default();
        let mut stmt = self.conn.prepare(
            "SELECT status, category, COUNT(*) FROM download_tasks GROUP BY status, category",
        )?;
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
}

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
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_db() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-download-store-{nanos}"));
        let _ = fs::create_dir_all(&dir);
        dir.join("test.db")
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
        let store = DownloadStore::open(&temp_db()).unwrap();
        let task = make_task("t1", "file.zip", TaskStatus::Pending);
        store.insert_task(&task).unwrap();

        let loaded = store.get_task("t1").unwrap().unwrap();
        assert_eq!(loaded.file_name, "file.zip");
        assert_eq!(loaded.status, TaskStatus::Pending);
    }

    #[test]
    fn update_progress() {
        let store = DownloadStore::open(&temp_db()).unwrap();
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
        let store = DownloadStore::open(&temp_db()).unwrap();
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
        let store = DownloadStore::open(&temp_db()).unwrap();
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
        let store = DownloadStore::open(&temp_db()).unwrap();
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
        let store = DownloadStore::open(&temp_db()).unwrap();
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
        let store = DownloadStore::open(&temp_db()).unwrap();
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
        let store = DownloadStore::open(&temp_db()).unwrap();
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
        let store = DownloadStore::open(&temp_db()).unwrap();
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
        let store = DownloadStore::open(&temp_db()).unwrap();
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
        let store = DownloadStore::open(&temp_db()).unwrap();
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
        assert_eq!(archives[0].file_name, "c.zip");

        let videos = store.list_tasks_by_category(FileCategory::Video).unwrap();
        assert!(videos.is_empty());
    }
}
