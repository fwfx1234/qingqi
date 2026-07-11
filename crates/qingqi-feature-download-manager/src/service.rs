use qingqi_plugin::{lock_or_recover, log_error};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, ensure};
use time::{OffsetDateTime, macros::format_description};
use uuid::Uuid;

use qingqi_plugin::job::{JobId, JobProvider, JobSnapshot};

use super::{
    model::{
        DownloadSettings, DownloadTask, FileCategory, TaskStatus, file_extension, guess_file_name,
        parse_custom_headers, sanitize_file_name,
    },
    store::DownloadStore,
};

const SPEED_WINDOW_MS: u128 = 2000;
const BUFFER_SIZE: usize = 64 * 1024;
const MIN_UPDATE_INTERVAL_MS: u128 = 200;

type DownloadSleeper = Arc<dyn Fn(Duration) + Send + Sync>;

struct ActiveDownload {
    cancel_flag: Arc<AtomicBool>,
    pause_flag: Arc<AtomicBool>,
    progress: Arc<AtomicU64>,
    speed: Arc<Mutex<f64>>,
}

pub struct DownloadService {
    store: Arc<Mutex<DownloadStore>>,
    active: Arc<Mutex<HashMap<String, ActiveDownload>>>,
    revision: Arc<AtomicU64>,
    settings: Arc<Mutex<DownloadSettings>>,
    /// 复用的 HTTP 客户端，设置变更时重建以节省 TLS 握手和连接建立开销
    client: Arc<Mutex<reqwest::blocking::Client>>,
    /// 重试退避等待，可由测试注入以提高测试速度并避免真实长 sleep。
    sleeper: Arc<Mutex<DownloadSleeper>>,
}

impl DownloadService {
    pub fn new(store: DownloadStore, save_dir: PathBuf) -> Self {
        log_error!(fs::create_dir_all(&save_dir), error, "创建下载保存目录失败");
        let settings = Self::load_settings_from_store(&store, &save_dir);
        let client = Self::build_client(&settings);
        Self {
            store: Arc::new(Mutex::new(store)),
            active: Arc::new(Mutex::new(HashMap::new())),
            revision: Arc::new(AtomicU64::new(0)),
            settings: Arc::new(Mutex::new(settings)),
            client: Arc::new(Mutex::new(client)),
            sleeper: Arc::new(Mutex::new(
                Arc::new(|dur: Duration| thread::sleep(dur)) as DownloadSleeper
            )),
        }
    }

    /// 根据设置构建复用的 HTTP 客户端
    fn build_client(settings: &DownloadSettings) -> reqwest::blocking::Client {
        let mut builder = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(settings.timeout_secs as u64))
            .connect_timeout(Duration::from_secs(10));
        if !settings.proxy_url.is_empty() {
            if let Ok(proxy) = reqwest::Proxy::all(&settings.proxy_url) {
                builder = builder.proxy(proxy);
            }
        }
        builder.build().expect("构建 HTTP 客户端失败")
    }

    fn runtime(&self) -> DownloadRuntime {
        DownloadRuntime {
            store: Arc::clone(&self.store),
            active: Arc::clone(&self.active),
            revision: Arc::clone(&self.revision),
            settings: Arc::clone(&self.settings),
            client: Arc::clone(&self.client),
            sleeper: lock_or_recover(&self.sleeper, "download-sleeper").clone(),
        }
    }

    #[cfg(test)]
    pub fn set_sleeper(&self, sleeper: DownloadSleeper) {
        let mut s = lock_or_recover(&self.sleeper, "download-sleeper");
        *s = sleeper;
    }

    fn load_settings_from_store(store: &DownloadStore, save_dir: &Path) -> DownloadSettings {
        let mut settings = DownloadSettings::default();
        settings.save_root = save_dir.to_string_lossy().to_string();
        if let Ok(pairs) = store.load_settings() {
            for (key, value) in pairs {
                match key.as_str() {
                    "saveRoot" => settings.save_root = value,
                    "maxConcurrent" => {
                        if let Ok(v) = value.parse::<usize>() {
                            settings.max_concurrent = v.clamp(1, 16);
                        }
                    }
                    "speedLimitKbps" => {
                        if let Ok(v) = value.parse::<u32>() {
                            settings.speed_limit_kbps = v;
                        }
                    }
                    "timeoutSec" => {
                        if let Ok(v) = value.parse::<u32>() {
                            settings.timeout_secs = v.clamp(1, 3600);
                        }
                    }
                    "retryLimit" => {
                        if let Ok(v) = value.parse::<u32>() {
                            settings.retry_limit = v.min(10);
                        }
                    }
                    "proxyUrl" => settings.proxy_url = value,
                    "userAgent" => settings.user_agent = value,
                    "referer" => settings.referer = value,
                    "cookie" => settings.cookie = value,
                    "customHeaders" => settings.custom_headers = value,
                    _ => {}
                }
            }
        }
        settings
    }

    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::SeqCst)
    }

    fn bump_revision(&self) {
        self.revision.fetch_add(1, Ordering::SeqCst);
    }

    /// Returns the current effective save directory (may differ from initial if changed via settings).
    pub fn effective_save_dir(&self) -> PathBuf {
        let settings = lock_or_recover(&self.settings, "download-settings");
        let dir = PathBuf::from(&settings.save_root);
        log_error!(fs::create_dir_all(&dir), error, "创建下载目录失败");
        dir
    }

    pub fn store(&self) -> &Arc<Mutex<DownloadStore>> {
        &self.store
    }

    /// Returns a snapshot of all tasks with live progress merged for active tasks.
    pub fn tasks_snapshot(&self) -> Vec<DownloadTask> {
        let store = lock_or_recover(&self.store, "download-store");
        let mut tasks = store.list_tasks(None).unwrap_or_default();
        drop(store);
        for task in &mut tasks {
            if task.status == TaskStatus::Downloading {
                if let Some((downloaded, speed)) = self.get_progress(&task.id) {
                    task.downloaded = downloaded;
                    task.speed_bps = speed;
                }
            }
        }
        tasks
    }

    // ── settings ──

    pub fn settings_snapshot(&self) -> DownloadSettings {
        lock_or_recover(&self.settings, "download-settings").clone()
    }

    pub fn update_settings(&self, settings: DownloadSettings) -> Result<()> {
        let max_concurrent = settings.max_concurrent.clamp(1, 16);
        let timeout_secs = settings.timeout_secs.clamp(1, 3600);
        let retry_limit = settings.retry_limit.min(10);
        let proxy_changed;
        let timeout_changed;
        {
            let mut s = lock_or_recover(&self.settings, "download-settings");
            proxy_changed = s.proxy_url != settings.proxy_url;
            timeout_changed = s.timeout_secs != settings.timeout_secs;
            s.save_root = settings.save_root.clone();
            s.max_concurrent = max_concurrent;
            s.speed_limit_kbps = settings.speed_limit_kbps;
            s.timeout_secs = timeout_secs;
            s.retry_limit = retry_limit;
            s.proxy_url = settings.proxy_url.clone();
            s.user_agent = settings.user_agent.clone();
            s.referer = settings.referer.clone();
            s.cookie = settings.cookie.clone();
            s.custom_headers = settings.custom_headers.clone();
        }
        // 代理或超时变更时重建 HTTP 客户端
        if proxy_changed || timeout_changed {
            let settings_snapshot = self.settings_snapshot();
            if let Ok(mut client) = self.client.lock() {
                *client = Self::build_client(&settings_snapshot);
            }
        }
        self.persist_settings()?;
        self.bump_revision();
        Ok(())
    }

    fn persist_settings(&self) -> Result<()> {
        let s = lock_or_recover(&self.settings, "download-settings");
        let store = lock_or_recover(&self.store, "download-store");
        store.save_settings(&[
            ("saveRoot", s.save_root.as_str()),
            ("maxConcurrent", &s.max_concurrent.to_string()),
            ("speedLimitKbps", &s.speed_limit_kbps.to_string()),
            ("timeoutSec", &s.timeout_secs.to_string()),
            ("retryLimit", &s.retry_limit.to_string()),
            ("proxyUrl", s.proxy_url.as_str()),
            ("userAgent", s.user_agent.as_str()),
            ("referer", s.referer.as_str()),
            ("cookie", s.cookie.as_str()),
            ("customHeaders", s.custom_headers.as_str()),
        ])
    }

    // ── task management ──

    pub fn add_task(&self, url: &str) -> Result<DownloadTask> {
        let url = url.trim();
        ensure!(!url.is_empty(), "URL 不能为空");
        ensure!(
            url.starts_with("http://") || url.starts_with("https://"),
            "仅支持 HTTP/HTTPS 协议"
        );

        let id = Uuid::new_v4().to_string();
        let file_name = guess_file_name(url);
        let category = FileCategory::from_extension(file_extension(&file_name));
        let save_dir = self.effective_save_dir();
        let save_path = Self::resolve_save_path_in_dir(&save_dir, &file_name);

        let now = now_label();
        let task = DownloadTask {
            id: id.clone(),
            url: url.to_string(),
            file_name,
            save_path: save_path.to_string_lossy().to_string(),
            file_size: None,
            downloaded: 0,
            status: TaskStatus::Pending,
            category,
            error_msg: String::new(),
            speed_bps: 0.0,
            created_at: now.clone(),
            updated_at: now,
        };

        lock_or_recover(&self.store, "download-store").insert_task(&task)?;
        self.bump_revision();
        Ok(task)
    }

    pub fn add_urls_from_text(&self, text: &str) -> Result<Vec<DownloadTask>> {
        use super::model::extract_urls_from_text;

        let urls = extract_urls_from_text(text);
        ensure!(!urls.is_empty(), "未识别到 HTTP/HTTPS 链接");

        let mut tasks = Vec::new();
        for url in urls {
            match self.add_task(&url) {
                Ok(task) => {
                    log_error!(self.start_download(&task.id), warn, "启动下载失败");
                    tasks.push(task);
                }
                Err(e) => {
                    tracing::warn!(url, error = %e, "failed to add task from text");
                }
            }
        }
        ensure!(!tasks.is_empty(), "未能添加任何任务");
        self.bump_revision();
        Ok(tasks)
    }

    pub fn retry_task(&self, task_id: &str) -> Result<()> {
        let task = {
            let store = lock_or_recover(&self.store, "download-store");
            store
                .get_task(task_id)?
                .ok_or_else(|| anyhow!("任务不存在"))?
        };

        ensure!(
            task.status == TaskStatus::Failed || task.status == TaskStatus::Cancelled,
            "只能重试失败或已取消的任务"
        );

        // Reset to pending and restart
        self.store
            .lock()
            .unwrap()
            .update_status(task_id, TaskStatus::Pending, "")?;
        self.bump_revision();
        self.start_download(task_id)
    }

    pub fn start_download(&self, task_id: &str) -> Result<()> {
        self.runtime().start_download(task_id)
    }
}

#[derive(Clone)]
struct DownloadRuntime {
    store: Arc<Mutex<DownloadStore>>,
    active: Arc<Mutex<HashMap<String, ActiveDownload>>>,
    revision: Arc<AtomicU64>,
    settings: Arc<Mutex<DownloadSettings>>,
    client: Arc<Mutex<reqwest::blocking::Client>>,
    sleeper: DownloadSleeper,
}

impl DownloadRuntime {
    fn http_client(&self) -> reqwest::blocking::Client {
        lock_or_recover(&self.client, "download-client").clone()
    }

    fn schedule_pending_downloads(&self) {
        loop {
            let next_task = {
                let store = lock_or_recover(&self.store, "download-store");
                match store.list_tasks(Some(TaskStatus::Pending)) {
                    Ok(mut pending) => pending.pop(),
                    Err(error) => {
                        tracing::warn!(error = %error, "failed to list pending downloads");
                        return;
                    }
                }
            };

            let Some(task) = next_task else {
                return;
            };

            if let Err(error) = self.start_download(&task.id) {
                let message = error.to_string();
                if message.contains("已达最大并发数") {
                    return;
                }
                tracing::warn!(task_id = %task.id, error = %error, "failed to schedule pending download");
            }
        }
    }

    fn start_download(&self, task_id: &str) -> Result<()> {
        let task = {
            let store = lock_or_recover(&self.store, "download-store");
            store
                .get_task(task_id)?
                .ok_or_else(|| anyhow!("任务不存在"))?
        };

        if task.status == TaskStatus::Downloading
            && lock_or_recover(&self.active, "download-active").contains_key(task_id)
        {
            return Ok(());
        }

        if task.status.is_terminal() && task.status != TaskStatus::Paused {
            return Err(anyhow!("任务已结束，无法重新下载"));
        }

        let cancel_flag = Arc::new(AtomicBool::new(false));
        let pause_flag = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicU64::new(task.downloaded));
        let speed = Arc::new(Mutex::new(0.0));
        let max_concurrent = lock_or_recover(&self.settings, "download-settings").max_concurrent;

        {
            let mut active = lock_or_recover(&self.active, "download-active");
            if active.contains_key(task_id) {
                return Ok(());
            }
            if active.len() >= max_concurrent {
                return Err(anyhow!("已达最大并发数 {}，请等待", max_concurrent));
            }
            active.insert(
                task_id.to_string(),
                ActiveDownload {
                    cancel_flag: cancel_flag.clone(),
                    pause_flag: pause_flag.clone(),
                    progress: progress.clone(),
                    speed: speed.clone(),
                },
            );
        }

        if let Err(error) = lock_or_recover(&self.store, "download-store").update_status(
            task_id,
            TaskStatus::Downloading,
            "",
        ) {
            lock_or_recover(&self.active, "download-active").remove(task_id);
            return Err(error);
        }
        self.revision.fetch_add(1, Ordering::SeqCst);

        let store = Arc::clone(&self.store);
        let active_map = Arc::clone(&self.active);
        let revision = Arc::clone(&self.revision);
        let settings = Arc::clone(&self.settings);
        let client = self.http_client();
        let sleeper = self.sleeper.clone();
        let scheduler = self.clone();
        let task_id = task_id.to_string();
        let url = task.url.clone();
        let save_path = task.save_path.clone();
        let file_name = task.file_name.clone();
        let initial_downloaded = task.downloaded;

        thread::spawn(move || {
            let result = download_file(
                &task_id,
                &url,
                &save_path,
                initial_downloaded,
                &cancel_flag,
                &pause_flag,
                &progress,
                &speed,
                &store,
                &settings,
                &client,
                &sleeper,
            );

            lock_or_recover(&active_map, "download-active-map").remove(&task_id);

            match result {
                Ok(()) => {
                    revision.fetch_add(1, Ordering::SeqCst);
                    tracing::info!(task_id, file_name, "download completed");
                    scheduler.schedule_pending_downloads();
                }
                Err(DownloadError::Cancelled) => {
                    log_error!(
                        store.lock().unwrap().update_status(
                            &task_id,
                            TaskStatus::Cancelled,
                            "已取消"
                        ),
                        warn,
                        "更新下载状态失败"
                    );
                    revision.fetch_add(1, Ordering::SeqCst);
                    tracing::info!(task_id, "download cancelled");
                    scheduler.schedule_pending_downloads();
                }
                Err(DownloadError::Paused) => {
                    let downloaded = progress.load(Ordering::Relaxed);
                    log_error!(
                        store
                            .lock()
                            .unwrap()
                            .update_status(&task_id, TaskStatus::Paused, ""),
                        warn,
                        "更新下载状态失败"
                    );
                    revision.fetch_add(1, Ordering::SeqCst);
                    tracing::info!(task_id, downloaded, "download paused");
                    scheduler.schedule_pending_downloads();
                }
                Err(DownloadError::Other(err)) => {
                    log_error!(
                        store.lock().unwrap().update_status(
                            &task_id,
                            TaskStatus::Failed,
                            &err.to_string()
                        ),
                        warn,
                        "更新下载状态失败"
                    );
                    revision.fetch_add(1, Ordering::SeqCst);
                    tracing::warn!(task_id, error = %err, "download failed");
                    scheduler.schedule_pending_downloads();
                }
            }
        });

        Ok(())
    }
}

impl DownloadService {
    pub fn pause_task(&self, task_id: &str) -> Result<()> {
        let active = lock_or_recover(&self.active, "download-active");
        if let Some(dl) = active.get(task_id) {
            dl.pause_flag.store(true, Ordering::Relaxed);
            self.bump_revision();
            Ok(())
        } else {
            self.store
                .lock()
                .unwrap()
                .update_status(task_id, TaskStatus::Paused, "")?;
            self.bump_revision();
            Ok(())
        }
    }

    pub fn resume_task(&self, task_id: &str) -> Result<()> {
        self.start_download(task_id)
    }

    pub fn cancel_task(&self, task_id: &str) -> Result<()> {
        let active = lock_or_recover(&self.active, "download-active");
        if let Some(dl) = active.get(task_id) {
            dl.cancel_flag.store(true, Ordering::Relaxed);
            self.bump_revision();
            Ok(())
        } else {
            self.store
                .lock()
                .unwrap()
                .update_status(task_id, TaskStatus::Cancelled, "已取消")?;
            self.bump_revision();
            Ok(())
        }
    }

    pub fn delete_task(&self, task_id: &str) -> Result<()> {
        log_error!(self.cancel_task(task_id), warn, "取消下载任务失败");
        let task = {
            let store = lock_or_recover(&self.store, "download-store");
            store.get_task(task_id)?
        };
        if let Some(task) = task {
            let path = Path::new(&task.save_path);
            if path.exists() {
                log_error!(fs::remove_file(path), warn, "删除下载文件失败");
            }
            lock_or_recover(&self.store, "download-store").delete_task(task_id)?;
            self.bump_revision();
        }
        Ok(())
    }

    pub fn start_all_pending(&self) -> Result<usize> {
        let pending = {
            self.store
                .lock()
                .unwrap()
                .list_tasks(Some(TaskStatus::Pending))?
        };
        let count = pending.len();
        for task in pending {
            log_error!(self.start_download(&task.id), warn, "批量启动下载失败");
        }
        Ok(count)
    }

    pub fn pause_all(&self) -> Result<()> {
        // Pause active downloads
        let ids: Vec<String> = {
            lock_or_recover(&self.active, "download-active")
                .keys()
                .cloned()
                .collect()
        };
        for id in ids {
            log_error!(self.pause_task(&id), warn, "批量暂停下载失败");
        }
        // Also pause queued tasks (in store)
        {
            let store = lock_or_recover(&self.store, "download-store");
            let queued = store.list_tasks(Some(TaskStatus::Pending))?;
            for task in queued {
                store.update_status(&task.id, TaskStatus::Paused, "")?;
            }
        }
        self.bump_revision();
        Ok(())
    }

    pub fn resume_all(&self) -> Result<()> {
        let ids: Vec<String> = {
            let store = lock_or_recover(&self.store, "download-store");
            let tasks = store.list_tasks(None)?;
            tasks
                .iter()
                .filter(|t| {
                    matches!(
                        t.status,
                        TaskStatus::Paused | TaskStatus::Failed | TaskStatus::Cancelled
                    )
                })
                .map(|t| t.id.clone())
                .collect()
        };
        for id in ids {
            log_error!(self.resume_task(&id), warn, "批量恢复下载失败");
        }
        Ok(())
    }

    pub fn clear_failed(&self) -> Result<usize> {
        let cleared = lock_or_recover(&self.store, "download-store").clear_failed()?;
        if cleared > 0 {
            self.bump_revision();
        }
        Ok(cleared)
    }

    // ── runtime settings ──

    pub fn set_save_root(&self, path: &str) -> Result<()> {
        let dir = PathBuf::from(path);
        fs::create_dir_all(&dir).with_context(|| format!("无法创建下载目录: {}", dir.display()))?;
        {
            let mut s = lock_or_recover(&self.settings, "download-settings");
            s.save_root = dir.to_string_lossy().to_string();
        }
        self.persist_settings()?;
        self.bump_revision();
        Ok(())
    }

    pub fn set_max_concurrent(&self, value: usize) -> Result<()> {
        let v = value.clamp(1, 16);
        {
            lock_or_recover(&self.settings, "download-settings").max_concurrent = v;
        }
        self.persist_settings()?;
        self.bump_revision();
        Ok(())
    }

    pub fn set_speed_limit_kbps(&self, value: u32) -> Result<()> {
        {
            lock_or_recover(&self.settings, "download-settings").speed_limit_kbps = value;
        }
        self.persist_settings()?;
        self.bump_revision();
        Ok(())
    }

    pub fn set_network_options(
        &self,
        user_agent: &str,
        referer: &str,
        cookie: &str,
        custom_headers: &str,
        proxy_url: &str,
        timeout_secs: u32,
        retry_limit: u32,
    ) -> Result<()> {
        {
            let mut s = lock_or_recover(&self.settings, "download-settings");
            s.user_agent = user_agent.trim().to_string();
            s.referer = referer.trim().to_string();
            s.cookie = cookie.trim().to_string();
            s.custom_headers = custom_headers.trim().to_string();
            s.proxy_url = {
                let text = proxy_url.trim();
                if text.is_empty() {
                    String::new()
                } else if !text.contains("://") {
                    format!("http://{text}")
                } else {
                    text.to_string()
                }
            };
            s.timeout_secs = timeout_secs.clamp(1, 3600);
            s.retry_limit = retry_limit.min(10);
        }
        self.persist_settings()?;
        self.bump_revision();
        Ok(())
    }

    pub fn get_progress(&self, task_id: &str) -> Option<(u64, f64)> {
        let active = lock_or_recover(&self.active, "download-active");
        active.get(task_id).map(|dl| {
            let downloaded = dl.progress.load(Ordering::Relaxed);
            let speed = *lock_or_recover(&dl.speed, "download-speed");
            (downloaded, speed)
        })
    }

    pub fn active_count(&self) -> usize {
        lock_or_recover(&self.active, "download-active").len()
    }

    pub fn stats(&self) -> super::store::DownloadStats {
        self.store
            .lock()
            .unwrap()
            .stats()
            .unwrap_or(super::store::DownloadStats {
                total: 0,
                completed: 0,
                active: 0,
                failed: 0,
                total_downloaded: 0,
            })
    }

    pub fn task_counts(&self) -> super::store::TaskCounts {
        lock_or_recover(&self.store, "download-store")
            .task_counts()
            .unwrap_or_default()
    }

    pub fn tasks_by_category(&self, category: super::model::FileCategory) -> Vec<DownloadTask> {
        self.store
            .lock()
            .unwrap()
            .list_tasks_by_category(category)
            .unwrap_or_default()
    }

    pub fn clear_completed(&self) -> Result<usize> {
        let cleared = lock_or_recover(&self.store, "download-store").clear_completed()?;
        if cleared > 0 {
            self.bump_revision();
        }
        Ok(cleared)
    }

    fn resolve_save_path_in_dir(dir: &Path, file_name: &str) -> PathBuf {
        let safe = sanitize_file_name(file_name);
        let base = dir.join(&safe);
        if !base.exists() {
            return base;
        }

        let stem = Path::new(&safe)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let ext = Path::new(&safe)
            .extension()
            .and_then(|s| s.to_str())
            .map(|e| format!(".{e}"))
            .unwrap_or_default();

        for i in 1..10000 {
            let candidate = dir.join(format!("{stem} ({i}){ext}"));
            if !candidate.exists() {
                return candidate;
            }
        }

        dir.join(format!(
            "{stem}_{}{ext}",
            Uuid::new_v4()
                .to_string()
                .chars()
                .take(8)
                .collect::<String>()
        ))
    }
}

impl JobProvider for DownloadService {
    fn job_snapshots(&self) -> Vec<JobSnapshot> {
        let Ok(store) = self.store.lock() else {
            return Vec::new();
        };
        let mut tasks = store.list_tasks(None).unwrap_or_default();
        drop(store);

        for task in &mut tasks {
            if task.status == TaskStatus::Downloading {
                if let Some((downloaded, speed)) = self.get_progress(&task.id) {
                    task.downloaded = downloaded;
                    task.speed_bps = speed;
                }
            }
        }

        tasks
            .into_iter()
            .map(|task| JobSnapshot {
                id: JobId::new(task.id),
                source: super::manifest::PLUGIN_ID,
                title: task.file_name,
                status: task.status.into(),
                completed_units: task.downloaded,
                total_units: task.file_size,
                rate_per_second: task.speed_bps,
                message: task.error_msg,
            })
            .collect()
    }

    fn cancel_job(&self, id: &JobId) -> Result<()> {
        self.cancel_task(&id.0)
    }

    fn pause_job(&self, id: &JobId) -> Result<()> {
        self.pause_task(&id.0)
    }

    fn resume_job(&self, id: &JobId) -> Result<()> {
        self.resume_task(&id.0)
    }
}

enum DownloadError {
    Cancelled,
    Paused,
    Other(anyhow::Error),
}

impl From<anyhow::Error> for DownloadError {
    fn from(e: anyhow::Error) -> Self {
        DownloadError::Other(e)
    }
}

fn download_file(
    task_id: &str,
    url: &str,
    save_path: &str,
    initial_downloaded: u64,
    cancel_flag: &AtomicBool,
    pause_flag: &AtomicBool,
    progress: &AtomicU64,
    speed: &Mutex<f64>,
    store: &Arc<Mutex<DownloadStore>>,
    settings: &Arc<Mutex<DownloadSettings>>,
    client: &reqwest::blocking::Client,
    sleeper: &DownloadSleeper,
) -> Result<(), DownloadError> {
    let (user_agent, referer, cookie, custom_headers_str, speed_limit_kbps, retry_limit) = {
        let s = lock_or_recover(&settings, "download-settings");
        (
            s.user_agent.clone(),
            s.referer.clone(),
            s.cookie.clone(),
            s.custom_headers.clone(),
            s.speed_limit_kbps,
            s.retry_limit,
        )
    };

    let max_attempts = retry_limit as usize + 1;
    let mut last_error: Option<anyhow::Error> = None;

    'attempts: for attempt in 1..=max_attempts {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err(DownloadError::Cancelled);
        }
        if pause_flag.load(Ordering::Relaxed) {
            return Err(DownloadError::Paused);
        }

        let local_file_len = Path::new(save_path).metadata().map(|m| m.len()).ok();

        let effective_downloaded = if initial_downloaded > 0 {
            match local_file_len {
                Some(len) if len == initial_downloaded => initial_downloaded,
                _ => 0,
            }
        } else {
            0
        };

        let mut response = match try_request(
            client,
            url,
            effective_downloaded,
            &user_agent,
            &referer,
            &cookie,
            &custom_headers_str,
        ) {
            Ok(resp) => resp,
            Err(err) => {
                tracing::warn!(
                    task_id,
                    attempt,
                    max_attempts,
                    error = %err,
                    status = "",
                    "download request failed"
                );
                if attempt < max_attempts {
                    last_error = Some(err);
                    let delay = backoff_delay(attempt);
                    (sleeper)(delay);
                    continue;
                }
                return Err(DownloadError::Other(err.context("无法连接服务器")));
            }
        };

        let status_code = response.status().as_u16();
        let is_206 = status_code == 206;

        if !response.status().is_success() && !is_206 {
            if is_retryable(status_code) && attempt < max_attempts {
                tracing::warn!(
                    task_id,
                    attempt,
                    max_attempts,
                    status = status_code,
                    error = "",
                    "download got retryable HTTP status"
                );
                last_error = Some(anyhow!("服务器返回错误: {}", response.status()));
                let delay = backoff_delay(attempt);
                (sleeper)(delay);
                continue;
            }
            return Err(DownloadError::Other(anyhow!(
                "服务器返回错误: {}",
                response.status()
            )));
        }

        let parsed_range = if is_206 {
            parse_content_range(response.headers().get("Content-Range"))
        } else {
            None
        };

        if is_206 {
            match parsed_range {
                Some(ref cr) if cr.start == effective_downloaded => {}
                _ => {
                    let start_display = parsed_range
                        .as_ref()
                        .map(|cr| cr.start.to_string())
                        .unwrap_or_else(|| "未解析".to_string());
                    return Err(DownloadError::Other(anyhow!(
                        "服务器返回 206 但 Content-Range 起点 {} 不等于本地进度 {}",
                        start_display,
                        effective_downloaded
                    )));
                }
            }
        }

        let total_size = match parsed_range {
            Some(ref cr) => Some(cr.total),
            None => response
                .headers()
                .get("Content-Length")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok()),
        };

        let is_resumed = is_206 && effective_downloaded > 0;

        if let Some(size) = total_size {
            let s = lock_or_recover(&store, "download-store");
            if let Ok(Some(mut task)) = s.get_task(task_id) {
                task.file_size = Some(size);
                log_error!(s.update_task(&task), warn, "更新下载任务信息失败");
            }
        }

        let mut file = if is_resumed {
            let current_len = fs::metadata(save_path)
                .context("无法读取本地文件元数据")?
                .len();
            if current_len != effective_downloaded {
                return Err(DownloadError::Other(anyhow!(
                    "本地文件长度 {} 不等于续传起点 {}",
                    current_len,
                    effective_downloaded
                )));
            }
            OpenOptions::new()
                .append(true)
                .open(save_path)
                .with_context(|| format!("无法打开文件 {}", save_path))?
        } else {
            progress.store(0, Ordering::Relaxed);
            if let Some(parent) = Path::new(save_path).parent() {
                log_error!(fs::create_dir_all(parent), error, "创建下载子目录失败");
            }
            File::create(save_path).with_context(|| format!("无法创建文件 {}", save_path))?
        };

        let mut downloaded = if is_resumed { effective_downloaded } else { 0 };
        let response_start = downloaded;
        let mut speed_tracker = SpeedTracker::new();
        let mut last_db_update = Instant::now();

        let mut buf = vec![0u8; BUFFER_SIZE];
        loop {
            if cancel_flag.load(Ordering::Relaxed) {
                return Err(DownloadError::Cancelled);
            }

            if pause_flag.load(Ordering::Relaxed) {
                return Err(DownloadError::Paused);
            }

            let n = match response.read(&mut buf) {
                Ok(n) => n,
                Err(error) if attempt < max_attempts => {
                    let error = anyhow!(error).context("下载数据读取失败");
                    tracing::warn!(
                        task_id,
                        attempt,
                        max_attempts,
                        error = %error,
                        "download body read failed"
                    );
                    last_error = Some(error);
                    drop(file);
                    (sleeper)(backoff_delay(attempt));
                    continue 'attempts;
                }
                Err(error) => {
                    return Err(DownloadError::Other(
                        anyhow!(error).context("下载数据读取失败"),
                    ));
                }
            };
            if n == 0 {
                break;
            }

            file.write_all(&buf[..n]).context("写入文件失败")?;
            downloaded += n as u64;
            progress.store(downloaded, Ordering::Relaxed);
            speed_tracker.add_bytes(n);

            let current_speed = speed_tracker.current_speed();
            *lock_or_recover(&speed, "download-speed") = current_speed;

            if speed_limit_kbps > 0 {
                let expected_bytes_per_sec = speed_limit_kbps as f64 * 1024.0;
                let actual_speed = speed_tracker.current_speed();
                if actual_speed > expected_bytes_per_sec {
                    let delay = (actual_speed / expected_bytes_per_sec - 1.0) * 0.1;
                    if delay > 0.0 {
                        thread::sleep(Duration::from_secs_f64(delay.min(0.5)));
                    }
                }
            }

            if last_db_update.elapsed().as_millis() >= MIN_UPDATE_INTERVAL_MS {
                log_error!(
                    store.lock().unwrap().update_progress(
                        task_id,
                        downloaded,
                        current_speed,
                        TaskStatus::Downloading
                    ),
                    warn,
                    "更新下载进度失败"
                );
                last_db_update = Instant::now();
            }
        }

        file.flush().context("刷新下载文件失败")?;

        let received = downloaded.saturating_sub(response_start);
        let expected_received = parsed_range
            .as_ref()
            .map(|range| range.end - range.start + 1)
            .or_else(|| {
                response
                    .headers()
                    .get("Content-Length")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok())
            });
        let final_size_valid = parsed_range
            .as_ref()
            .is_none_or(|range| downloaded == range.total && range.end + 1 == range.total);
        if expected_received.is_some_and(|expected| received != expected) || !final_size_valid {
            let error = anyhow!(
                "下载响应不完整: 本次接收 {} 字节，声明 {:?} 字节，最终大小 {}",
                received,
                expected_received,
                downloaded
            );
            if attempt < max_attempts {
                last_error = Some(error);
                drop(file);
                (sleeper)(backoff_delay(attempt));
                continue 'attempts;
            }
            return Err(DownloadError::Other(error));
        }

        store
            .lock()
            .unwrap()
            .update_progress(task_id, downloaded, 0.0, TaskStatus::Completed)?;

        return Ok(());
    }

    Err(DownloadError::Other(
        last_error.unwrap_or_else(|| anyhow!("所有重试均失败")),
    ))
}

fn try_request(
    client: &reqwest::blocking::Client,
    url: &str,
    effective_downloaded: u64,
    user_agent: &str,
    referer: &str,
    cookie: &str,
    custom_headers_str: &str,
) -> anyhow::Result<reqwest::blocking::Response> {
    let mut request = client.get(url);
    if !user_agent.is_empty() {
        request = request.header("User-Agent", user_agent);
    }
    if !referer.is_empty() {
        request = request.header("Referer", referer);
    }
    if !cookie.is_empty() {
        request = request.header("Cookie", cookie);
    }
    for (key, value) in parse_custom_headers(custom_headers_str) {
        request = request.header(&key, &value);
    }
    if effective_downloaded > 0 {
        request = request.header("Range", format!("bytes={}-", effective_downloaded));
    }
    request.send().map_err(|e| anyhow!(e))
}

fn backoff_delay(attempt: usize) -> Duration {
    let exp = 2_u32.pow((attempt as u32).min(5));
    let millis = (200_u64 * exp as u64).min(8000);
    Duration::from_millis(millis)
}

fn is_retryable(status_code: u16) -> bool {
    matches!(status_code, 408 | 425 | 429) || (500..600).contains(&status_code)
}

#[derive(Debug, PartialEq)]
struct ContentRange {
    start: u64,
    end: u64,
    total: u64,
}

fn parse_content_range(header: Option<&reqwest::header::HeaderValue>) -> Option<ContentRange> {
    let val = header?.to_str().ok()?;
    let (unit, rest) = val.split_once(' ')?;
    if unit != "bytes" {
        return None;
    }
    let (range, total_str) = rest.split_once('/')?;
    let (start_str, end_str) = range.split_once('-')?;
    let start = start_str.parse::<u64>().ok()?;
    let end = end_str.parse::<u64>().ok()?;
    if total_str == "*" {
        return None;
    }
    let total = total_str.parse::<u64>().ok()?;
    if start > end || end >= total {
        return None;
    }
    Some(ContentRange { start, end, total })
}

struct SpeedTracker {
    samples: Vec<(Instant, usize)>,
    window: Duration,
}

impl SpeedTracker {
    fn new() -> Self {
        Self {
            samples: Vec::new(),
            window: Duration::from_millis(SPEED_WINDOW_MS as u64),
        }
    }

    fn add_bytes(&mut self, bytes: usize) {
        let now = Instant::now();
        self.samples.push((now, bytes));
        self.gc(now);
    }

    fn current_speed(&mut self) -> f64 {
        let now = Instant::now();
        self.gc(now);
        if self.samples.is_empty() {
            return 0.0;
        }
        let total: usize = self.samples.iter().map(|(_, b)| b).sum();
        let elapsed = now
            .duration_since(self.samples.first().unwrap().0)
            .as_secs_f64();
        if elapsed <= 0.0 {
            return 0.0;
        }
        total as f64 / elapsed
    }

    fn gc(&mut self, now: Instant) {
        let cutoff = now.checked_sub(self.window).unwrap_or(now);
        while let Some(first) = self.samples.first() {
            if first.0 < cutoff {
                self.samples.remove(0);
            } else {
                break;
            }
        }
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
    use qingqi_plugin::{
        database::{DatabaseService, DatabaseSpec, feature_database_key},
        storage::AppPaths,
    };
    use std::{
        env, fs,
        io::{ErrorKind, Read, Write},
        net::TcpListener,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
            mpsc,
        },
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_root(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let suffix = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!("{prefix}-{nanos}-{suffix}"));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn make_service() -> (DownloadService, PathBuf) {
        let root = temp_root("qingqi-download-service");
        let database = Arc::new(DatabaseService::new(AppPaths::for_test(root.clone())));
        let key = feature_database_key(super::super::manifest::PLUGIN_ID, "tasks");
        database
            .register_database(DatabaseSpec::path(key.clone(), root.join("tasks.db")))
            .unwrap();
        let store = DownloadStore::open(database, &key).unwrap();
        let service = DownloadService::new(store, root.join("downloads"));
        (service, root)
    }

    fn wait_until(timeout: Duration, mut predicate: impl FnMut() -> bool) -> bool {
        let started = Instant::now();
        while started.elapsed() < timeout {
            if predicate() {
                return true;
            }
            thread::sleep(Duration::from_millis(25));
        }
        predicate()
    }

    fn accept_with_timeout(listener: &TcpListener) -> Option<std::net::TcpStream> {
        let started = Instant::now();
        loop {
            match listener.accept() {
                Ok((stream, _)) => return Some(stream),
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    if started.elapsed() > Duration::from_secs(5) {
                        return None;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => return None,
            }
        }
    }

    fn spawn_two_response_server() -> (String, mpsc::Sender<()>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let (release_tx, release_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            for index in 0..2 {
                let Some(mut stream) = accept_with_timeout(&listener) else {
                    return;
                };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                let mut buffer = [0; 1024];
                let _ = stream.read(&mut buffer);
                if index == 0 {
                    let _ = release_rx.recv_timeout(Duration::from_secs(5));
                }
                let body = format!("download-{index}");
                let headers = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(headers.as_bytes());
                let _ = stream.write_all(body.as_bytes());
            }
        });
        (base_url, release_tx, handle)
    }

    fn spawn_record_response_server(
        status: u16,
        extra_headers: &str,
        body: &[u8],
    ) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let extra = extra_headers.to_string();
        let body = body.to_vec();
        let handle = thread::spawn(move || {
            let mut captured = String::new();
            if let Some(mut stream) = accept_with_timeout(&listener) {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let mut buffer = [0; 2048];
                let n = stream.read(&mut buffer).unwrap_or(0);
                captured = String::from_utf8_lossy(&buffer[..n]).to_string();
                let status_text = match status {
                    200 => "OK",
                    206 => "Partial Content",
                    _ => "Error",
                };
                let response = format!(
                    "HTTP/1.1 {} {}\r\n{}Content-Length: {}\r\nConnection: close\r\n\r\n",
                    status,
                    status_text,
                    extra,
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.write_all(&body);
            }
            captured
        });
        (base_url, handle)
    }

    fn task_status(service: &DownloadService, task_id: &str) -> Option<TaskStatus> {
        service
            .tasks_snapshot()
            .into_iter()
            .find(|task| task.id == task_id)
            .map(|task| task.status)
    }

    #[test]
    fn schedules_pending_download_after_slot_frees() {
        let (service, root) = make_service();
        service.set_max_concurrent(1).unwrap();
        let (base_url, release_first, server) = spawn_two_response_server();

        let first = service.add_task(&format!("{base_url}/first.bin")).unwrap();
        let second = service.add_task(&format!("{base_url}/second.bin")).unwrap();

        service.start_download(&first.id).unwrap();
        assert!(wait_until(Duration::from_secs(1), || service
            .active_count()
            == 1));

        let queued = service.start_download(&second.id).unwrap_err();
        assert!(queued.to_string().contains("已达最大并发数"));
        assert_eq!(task_status(&service, &second.id), Some(TaskStatus::Pending));

        release_first.send(()).unwrap();
        assert!(
            wait_until(Duration::from_secs(5), || {
                let tasks = service.tasks_snapshot();
                tasks
                    .iter()
                    .filter(|task| task.status == TaskStatus::Completed)
                    .count()
                    == 2
                    && service.active_count() == 0
            }),
            "pending download was not scheduled after the active slot freed"
        );

        server.join().unwrap();
        assert_eq!(
            task_status(&service, &first.id),
            Some(TaskStatus::Completed)
        );
        assert_eq!(
            task_status(&service, &second.id),
            Some(TaskStatus::Completed)
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parse_content_range_bytes_100_199_over_200() {
        let header = reqwest::header::HeaderValue::from_static("bytes 100-199/200");
        assert_eq!(
            parse_content_range(Some(&header)),
            Some(ContentRange {
                start: 100,
                end: 199,
                total: 200
            })
        );
    }

    #[test]
    fn parse_content_range_star_returns_none() {
        let header = reqwest::header::HeaderValue::from_static("bytes 0-999/*");
        assert_eq!(parse_content_range(Some(&header)), None);
    }

    #[test]
    fn parse_content_range_malformed_returns_none() {
        assert_eq!(
            parse_content_range(Some(&reqwest::header::HeaderValue::from_static("bogus"))),
            None
        );
        assert_eq!(
            parse_content_range(Some(&reqwest::header::HeaderValue::from_static(
                "bytes 100"
            ))),
            None
        );
        assert_eq!(
            parse_content_range(Some(&reqwest::header::HeaderValue::from_static(
                "bytes abc-def/ghi"
            ))),
            None
        );
        assert_eq!(parse_content_range(None), None);
        for invalid in ["bytes 100-99/200", "bytes 0-200/200", "bytes 0-0/0"] {
            let header = reqwest::header::HeaderValue::from_str(invalid).unwrap();
            assert_eq!(parse_content_range(Some(&header)), None, "{invalid}");
        }
    }

    #[test]
    fn resume_rejects_206_with_mismatched_start() {
        let (service, root) = make_service();
        service
            .set_network_options("", "", "", "", "", 30, 0)
            .unwrap();

        let (base_url, server) =
            spawn_record_response_server(206, "Content-Range: bytes 0-99/200\r\n", &[b'X'; 100]);

        let task = service.add_task(&format!("{base_url}/test.bin")).unwrap();

        service
            .store()
            .lock()
            .unwrap()
            .update_progress(&task.id, 100, 0.0, TaskStatus::Paused)
            .unwrap();
        let save_path = PathBuf::from(&task.save_path);
        if let Some(parent) = save_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        File::create(&save_path)
            .unwrap()
            .write_all(&[b'A'; 100])
            .unwrap();

        service.start_download(&task.id).unwrap();

        assert!(
            wait_until(Duration::from_secs(5), || {
                task_status(&service, &task.id) == Some(TaskStatus::Failed)
            }),
            "task should fail when 206 start mismatches local progress"
        );

        let task = service
            .tasks_snapshot()
            .into_iter()
            .find(|t| t.id == task.id)
            .unwrap();
        assert!(
            task.error_msg.contains("Content-Range"),
            "error was: {}",
            task.error_msg
        );

        let request = server.join().unwrap();
        assert!(
            request.contains("Range: bytes=100-"),
            "should have sent Range header"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resume_with_200_response_resets_progress_and_rewrites_file() {
        let (service, root) = make_service();
        service
            .set_network_options("", "", "", "", "", 30, 0)
            .unwrap();

        let new_body = b"fresh-content";
        let (base_url, server) = spawn_record_response_server(200, "", new_body);

        let task = service.add_task(&format!("{base_url}/test.bin")).unwrap();

        service
            .store()
            .lock()
            .unwrap()
            .update_progress(&task.id, 100, 0.0, TaskStatus::Paused)
            .unwrap();
        let save_path = PathBuf::from(&task.save_path);
        if let Some(parent) = save_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        File::create(&save_path)
            .unwrap()
            .write_all(&[b'A'; 100])
            .unwrap();

        service.start_download(&task.id).unwrap();

        assert!(
            wait_until(Duration::from_secs(5), || {
                task_status(&service, &task.id) == Some(TaskStatus::Completed)
            }),
            "task should complete"
        );

        let content = fs::read(&save_path).unwrap();
        assert_eq!(
            content,
            new_body.to_vec(),
            "file should contain only new response body"
        );

        let request = server.join().unwrap();
        assert!(
            request.contains("Range: bytes=100-"),
            "should have sent Range header"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_local_file_with_db_progress_does_not_append() {
        let (service, root) = make_service();
        service
            .set_network_options("", "", "", "", "", 30, 0)
            .unwrap();

        let body = b"full-content-from-start";
        let (base_url, server) = spawn_record_response_server(200, "", body);

        let task = service.add_task(&format!("{base_url}/test.bin")).unwrap();

        service
            .store()
            .lock()
            .unwrap()
            .update_progress(&task.id, 100, 0.0, TaskStatus::Paused)
            .unwrap();
        let save_path = PathBuf::from(&task.save_path);
        if save_path.exists() {
            fs::remove_file(&save_path).unwrap();
        }

        service.start_download(&task.id).unwrap();

        assert!(
            wait_until(Duration::from_secs(5), || {
                task_status(&service, &task.id) == Some(TaskStatus::Completed)
            }),
            "task should complete"
        );

        let content = fs::read(&save_path).unwrap();
        assert_eq!(content, body.to_vec());

        let request = server.join().unwrap();
        assert!(
            !request.contains("Range:"),
            "should NOT have sent Range header when local file missing"
        );

        let _ = fs::remove_dir_all(root);
    }

    fn make_service_with_sleeper(sleeper: DownloadSleeper) -> (DownloadService, PathBuf) {
        let (service, root) = make_service();
        service.set_sleeper(sleeper);
        (service, root)
    }

    fn spawn_sequence_server(
        status_codes: Vec<u16>,
    ) -> (String, Arc<AtomicU64>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let counter = Arc::new(AtomicU64::new(0));
        let counter_task = counter.clone();
        let handle = thread::spawn(move || {
            let mut index = 0;
            loop {
                let Some(mut stream) = accept_with_timeout(&listener) else {
                    return;
                };
                counter_task.fetch_add(1, Ordering::Relaxed);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let mut buffer = [0; 2048];
                let _ = stream.read(&mut buffer);
                let code = status_codes.get(index).copied().unwrap_or(200);
                index += 1;
                let status_text = match code {
                    200 => "OK",
                    404 => "Not Found",
                    500 => "Internal Server Error",
                    503 => "Service Unavailable",
                    _ => "Error",
                };
                let body = if code == 200 {
                    format!("success-body-{index}")
                } else {
                    "error".to_string()
                };
                let headers = format!(
                    "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    code,
                    status_text,
                    body.len()
                );
                let _ = stream.write_all(headers.as_bytes());
                let _ = stream.write_all(body.as_bytes());
            }
        });
        (base_url, counter, handle)
    }

    #[test]
    fn retry_succeeds_after_two_500s_with_three_requests() {
        let (service, root) = make_service_with_sleeper(Arc::new(|_dur| {
            // 测试中注入极短 sleep 以避免真实长等待
            thread::sleep(Duration::from_millis(1));
        }));
        service
            .set_network_options("", "", "", "", "", 30, 2)
            .unwrap();

        let (base_url, counter, _server) = spawn_sequence_server(vec![500, 500, 200]);

        let task = service.add_task(&format!("{base_url}/test.bin")).unwrap();
        service.start_download(&task.id).unwrap();

        assert!(
            wait_until(Duration::from_secs(5), || {
                task_status(&service, &task.id) == Some(TaskStatus::Completed)
            }),
            "task should eventually complete after retries"
        );
        assert_eq!(
            counter.load(Ordering::Relaxed),
            3,
            "should make exactly 3 attempts"
        );

        let content = fs::read_to_string(PathBuf::from(
            service
                .tasks_snapshot()
                .into_iter()
                .find(|t| t.id == task.id)
                .unwrap()
                .save_path,
        ))
        .unwrap();
        assert!(
            content.starts_with("success-body-"),
            "saved file content: {content}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn retry_limit_zero_single_request_only() {
        let (service, root) = make_service_with_sleeper(Arc::new(|_dur| {
            thread::sleep(Duration::from_millis(1));
        }));
        service
            .set_network_options("", "", "", "", "", 30, 0)
            .unwrap();

        let (base_url, counter, _server) = spawn_sequence_server(vec![500, 200]);

        let task = service.add_task(&format!("{base_url}/test.bin")).unwrap();
        service.start_download(&task.id).unwrap();

        assert!(
            wait_until(Duration::from_secs(5), || {
                task_status(&service, &task.id) == Some(TaskStatus::Failed)
            }),
            "task should fail without retrying"
        );
        assert_eq!(
            counter.load(Ordering::Relaxed),
            1,
            "should make only 1 request when retry_limit=0"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn no_retry_on_4xx_error() {
        let (service, root) = make_service_with_sleeper(Arc::new(|_dur| {
            thread::sleep(Duration::from_millis(1));
        }));
        service
            .set_network_options("", "", "", "", "", 30, 3)
            .unwrap();

        let (base_url, counter, _server) = spawn_sequence_server(vec![404, 200]);

        let task = service.add_task(&format!("{base_url}/test.bin")).unwrap();
        service.start_download(&task.id).unwrap();

        assert!(
            wait_until(Duration::from_secs(5), || {
                task_status(&service, &task.id) == Some(TaskStatus::Failed)
            }),
            "task should fail on 404 without retrying"
        );
        assert_eq!(
            counter.load(Ordering::Relaxed),
            1,
            "should make only 1 request on 404"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn retry_on_transport_error_then_succeed() {
        let (service, root) = make_service_with_sleeper(Arc::new(|_dur| {
            thread::sleep(Duration::from_millis(1));
        }));
        service
            .set_network_options("", "", "", "", "", 30, 3)
            .unwrap();

        // 模拟传输错误：第一次连接直接关闭，之后返回 200
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let counter = Arc::new(AtomicU64::new(0));
        let counter_task = counter.clone();
        let _server = thread::spawn(move || {
            let mut index = 0;
            loop {
                let Some(stream) = accept_with_timeout(&listener) else {
                    return;
                };
                counter_task.fetch_add(1, Ordering::Relaxed);
                if index == 0 {
                    // 模拟传输错误：不发送任何响应
                    drop(stream);
                } else {
                    let mut stream = stream;
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                    let mut buffer = [0; 1024];
                    let _ = stream.read(&mut buffer);
                    let body = b"recovered";
                    let headers = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(headers.as_bytes());
                    let _ = stream.write_all(body);
                }
                index += 1;
            }
        });

        let task = service.add_task(&format!("{base_url}/test.bin")).unwrap();
        service.start_download(&task.id).unwrap();

        assert!(
            wait_until(Duration::from_secs(5), || {
                task_status(&service, &task.id) == Some(TaskStatus::Completed)
            }),
            "task should recover after transport error"
        );
        assert!(
            counter.load(Ordering::Relaxed) >= 2,
            "should make at least 2 requests (1 failure + 1 success)"
        );

        let content = fs::read_to_string(PathBuf::from(
            service
                .tasks_snapshot()
                .into_iter()
                .find(|t| t.id == task.id)
                .unwrap()
                .save_path,
        ))
        .unwrap();
        assert_eq!(content, "recovered");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn retry_on_truncated_response_body_then_succeed() {
        let (service, root) = make_service_with_sleeper(Arc::new(|_| {}));
        service
            .set_network_options("", "", "", "", "", 30, 1)
            .unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let counter = Arc::new(AtomicU64::new(0));
        let counter_task = Arc::clone(&counter);
        let server = thread::spawn(move || {
            for index in 0..2 {
                let Some(mut stream) = accept_with_timeout(&listener) else {
                    return;
                };
                counter_task.fetch_add(1, Ordering::Relaxed);
                let mut request = [0; 1024];
                let _ = stream.read(&mut request);
                if index == 0 {
                    let _ = stream.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nshort",
                    );
                } else {
                    let _ = stream.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\nrecovered",
                    );
                }
            }
        });

        let task = service.add_task(&format!("{base_url}/test.bin")).unwrap();
        service.start_download(&task.id).unwrap();

        assert!(
            wait_until(Duration::from_secs(5), || {
                task_status(&service, &task.id) == Some(TaskStatus::Completed)
            }),
            "task should retry after a truncated response body"
        );
        assert_eq!(counter.load(Ordering::Relaxed), 2);
        assert_eq!(fs::read_to_string(&task.save_path).unwrap(), "recovered");
        server.join().unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cancel_during_backoff_prevents_further_requests() {
        let (service, root) = make_service_with_sleeper(Arc::new(|_dur| {
            thread::sleep(Duration::from_millis(50));
        }));
        // retry_limit=1: 最多 2 次尝试
        service
            .set_network_options("", "", "", "", "", 30, 1)
            .unwrap();

        // 始终返回 500，退避期间取消后不应再发起请求
        let (base_url, counter, _server) = spawn_sequence_server(vec![500, 500, 500]);

        let task = service.add_task(&format!("{base_url}/test.bin")).unwrap();
        service.start_download(&task.id).unwrap();

        // 等待第一次请求完成并确认处于 Downloading 状态
        assert!(
            wait_until(Duration::from_secs(3), || {
                counter.load(Ordering::Relaxed) >= 1
                    && task_status(&service, &task.id) == Some(TaskStatus::Downloading)
            }),
            "task should be in Downloading after first attempt"
        );

        // 取消任务
        service.cancel_task(&task.id).unwrap();

        // 确认状态变为 Cancelled
        assert!(
            wait_until(Duration::from_secs(5), || {
                task_status(&service, &task.id) == Some(TaskStatus::Cancelled)
            }),
            "task should be cancelled"
        );

        let request_count = counter.load(Ordering::Relaxed);
        // 第一次 500 后进入退避，取消后应停止，不应有第二次请求
        assert!(
            request_count <= 1,
            "cancel during backoff should not trigger more requests, got {request_count}"
        );

        let _ = fs::remove_dir_all(root);
    }
}
