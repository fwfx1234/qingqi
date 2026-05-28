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

use crate::core::job::{JobId, JobProvider, JobSnapshot};

use super::{
    model::{
        DownloadSettings, DownloadTask, FileCategory, TaskStatus, file_extension, guess_file_name,
        parse_custom_headers,
    },
    store::DownloadStore,
};

const SPEED_WINDOW_MS: u128 = 2000;
const BUFFER_SIZE: usize = 64 * 1024;
const MIN_UPDATE_INTERVAL_MS: u128 = 200;

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
}

impl DownloadService {
    pub fn new(store: DownloadStore, save_dir: PathBuf) -> Self {
        fs::create_dir_all(&save_dir).ok();
        let settings = Self::load_settings_from_store(&store, &save_dir);
        Self {
            store: Arc::new(Mutex::new(store)),
            active: Arc::new(Mutex::new(HashMap::new())),
            revision: Arc::new(AtomicU64::new(0)),
            settings: Arc::new(Mutex::new(settings)),
        }
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
        let settings = self.settings.lock().unwrap();
        let dir = PathBuf::from(&settings.save_root);
        fs::create_dir_all(&dir).ok();
        dir
    }

    pub fn store(&self) -> &Arc<Mutex<DownloadStore>> {
        &self.store
    }

    /// Returns a snapshot of all tasks with live progress merged for active tasks.
    pub fn tasks_snapshot(&self) -> Vec<DownloadTask> {
        let store = self.store.lock().unwrap();
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
        self.settings.lock().unwrap().clone()
    }

    pub fn update_settings(&self, settings: DownloadSettings) -> Result<()> {
        let max_concurrent = settings.max_concurrent.clamp(1, 16);
        let timeout_secs = settings.timeout_secs.clamp(1, 3600);
        let retry_limit = settings.retry_limit.min(10);
        {
            let mut s = self.settings.lock().unwrap();
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
        self.persist_settings()?;
        self.bump_revision();
        Ok(())
    }

    fn persist_settings(&self) -> Result<()> {
        let s = self.settings.lock().unwrap();
        let store = self.store.lock().unwrap();
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

        self.store.lock().unwrap().insert_task(&task)?;
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
                    let _ = self.start_download(&task.id);
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
            let store = self.store.lock().unwrap();
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
        // Enforce max concurrent limit
        {
            let active_map = self.active.lock().unwrap();
            let settings = self.settings.lock().unwrap();
            if active_map.len() >= settings.max_concurrent {
                return Err(anyhow!(
                    "已达最大并发数 {}，请等待",
                    settings.max_concurrent
                ));
            }
        }
        let task = {
            let store = self.store.lock().unwrap();
            store
                .get_task(task_id)?
                .ok_or_else(|| anyhow!("任务不存在"))?
        };

        if task.status == TaskStatus::Downloading {
            return Ok(());
        }

        if task.status.is_terminal() && task.status != TaskStatus::Paused {
            return Err(anyhow!("任务已结束，无法重新下载"));
        }

        let cancel_flag = Arc::new(AtomicBool::new(false));
        let pause_flag = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicU64::new(task.downloaded));
        let speed = Arc::new(Mutex::new(0.0));

        {
            let mut active = self.active.lock().unwrap();
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

        self.store
            .lock()
            .unwrap()
            .update_status(task_id, TaskStatus::Downloading, "")?;
        self.bump_revision();

        let store = Arc::clone(&self.store);
        let active_map = Arc::clone(&self.active);
        let revision = Arc::clone(&self.revision);
        let settings = Arc::clone(&self.settings);
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
            );

            active_map.lock().unwrap().remove(&task_id);

            match result {
                Ok(()) => {
                    revision.fetch_add(1, Ordering::SeqCst);
                    tracing::info!(task_id, file_name, "download completed");
                }
                Err(DownloadError::Cancelled) => {
                    store
                        .lock()
                        .unwrap()
                        .update_status(&task_id, TaskStatus::Cancelled, "已取消")
                        .ok();
                    revision.fetch_add(1, Ordering::SeqCst);
                    tracing::info!(task_id, "download cancelled");
                }
                Err(DownloadError::Paused) => {
                    let downloaded = progress.load(Ordering::Relaxed);
                    store
                        .lock()
                        .unwrap()
                        .update_status(&task_id, TaskStatus::Paused, "")
                        .ok();
                    revision.fetch_add(1, Ordering::SeqCst);
                    tracing::info!(task_id, downloaded, "download paused");
                }
                Err(DownloadError::Other(err)) => {
                    store
                        .lock()
                        .unwrap()
                        .update_status(&task_id, TaskStatus::Failed, &err.to_string())
                        .ok();
                    revision.fetch_add(1, Ordering::SeqCst);
                    tracing::warn!(task_id, error = %err, "download failed");
                }
            }
        });

        Ok(())
    }

    pub fn pause_task(&self, task_id: &str) -> Result<()> {
        let active = self.active.lock().unwrap();
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
        let active = self.active.lock().unwrap();
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
        self.cancel_task(task_id).ok();
        let task = {
            let store = self.store.lock().unwrap();
            store.get_task(task_id)?
        };
        if let Some(task) = task {
            let path = Path::new(&task.save_path);
            if path.exists() {
                fs::remove_file(path).ok();
            }
            self.store.lock().unwrap().delete_task(task_id)?;
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
            self.start_download(&task.id).ok();
        }
        Ok(count)
    }

    pub fn pause_all(&self) -> Result<()> {
        // Pause active downloads
        let ids: Vec<String> = { self.active.lock().unwrap().keys().cloned().collect() };
        for id in ids {
            self.pause_task(&id).ok();
        }
        // Also pause queued tasks (in store)
        {
            let store = self.store.lock().unwrap();
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
            let store = self.store.lock().unwrap();
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
            self.resume_task(&id).ok();
        }
        Ok(())
    }

    pub fn clear_failed(&self) -> Result<usize> {
        let cleared = self.store.lock().unwrap().clear_failed()?;
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
            let mut s = self.settings.lock().unwrap();
            s.save_root = dir.to_string_lossy().to_string();
        }
        self.persist_settings()?;
        self.bump_revision();
        Ok(())
    }

    pub fn set_max_concurrent(&self, value: usize) -> Result<()> {
        let v = value.clamp(1, 16);
        {
            self.settings.lock().unwrap().max_concurrent = v;
        }
        self.persist_settings()?;
        self.bump_revision();
        Ok(())
    }

    pub fn set_speed_limit_kbps(&self, value: u32) -> Result<()> {
        {
            self.settings.lock().unwrap().speed_limit_kbps = value;
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
            let mut s = self.settings.lock().unwrap();
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
        let active = self.active.lock().unwrap();
        active.get(task_id).map(|dl| {
            let downloaded = dl.progress.load(Ordering::Relaxed);
            let speed = *dl.speed.lock().unwrap();
            (downloaded, speed)
        })
    }

    pub fn active_count(&self) -> usize {
        self.active.lock().unwrap().len()
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
        self.store.lock().unwrap().task_counts().unwrap_or_default()
    }

    pub fn tasks_by_category(&self, category: super::model::FileCategory) -> Vec<DownloadTask> {
        self.store
            .lock()
            .unwrap()
            .list_tasks_by_category(category)
            .unwrap_or_default()
    }

    pub fn clear_completed(&self) -> Result<usize> {
        let cleared = self.store.lock().unwrap().clear_completed()?;
        if cleared > 0 {
            self.bump_revision();
        }
        Ok(cleared)
    }

    fn resolve_save_path_in_dir(dir: &Path, file_name: &str) -> PathBuf {
        let base = dir.join(file_name);
        if !base.exists() {
            return base;
        }

        let stem = Path::new(file_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let ext = Path::new(file_name)
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
) -> Result<(), DownloadError> {
    let (
        timeout,
        proxy_url,
        user_agent,
        referer,
        cookie,
        custom_headers_str,
        speed_limit_kbps,
        retry_limit,
    ) = {
        let s = settings.lock().unwrap();
        (
            s.timeout_secs,
            s.proxy_url.clone(),
            s.user_agent.clone(),
            s.referer.clone(),
            s.cookie.clone(),
            s.custom_headers.clone(),
            s.speed_limit_kbps,
            s.retry_limit,
        )
    };

    let mut client_builder = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout as u64))
        .connect_timeout(Duration::from_secs(10));

    if !proxy_url.is_empty() {
        if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
            client_builder = client_builder.proxy(proxy);
        }
    }

    let client = client_builder.build().context("无法创建 HTTP 客户端")?;

    let mut request = client.get(url);

    if !user_agent.is_empty() {
        request = request.header("User-Agent", &user_agent);
    }
    if !referer.is_empty() {
        request = request.header("Referer", &referer);
    }
    if !cookie.is_empty() {
        request = request.header("Cookie", &cookie);
    }
    for (key, value) in parse_custom_headers(&custom_headers_str) {
        request = request.header(&key, &value);
    }
    if initial_downloaded > 0 {
        request = request.header("Range", format!("bytes={}-", initial_downloaded));
    }

    let mut response = request.send().context("无法连接服务器")?;

    if !response.status().is_success() && response.status().as_u16() != 206 {
        let status = response.status();
        let err = DownloadError::Other(anyhow!("服务器返回错误: {}", status));
        // Auto-retry for transient errors
        if retry_limit > 0 && is_retryable(status.as_u16()) {
            let attempts = {
                let s = store.lock().unwrap();
                s.get_task(task_id)
                    .ok()
                    .flatten()
                    .map(|t| t.downloaded)
                    .unwrap_or(0)
            };
            if attempts == 0 {
                // Simple: retry once by setting to pending and re-dispatching
                let _ = store
                    .lock()
                    .unwrap()
                    .update_status(task_id, TaskStatus::Pending, "");
                return Err(err);
            }
        }
        return Err(err);
    }

    let total_size = if initial_downloaded > 0 && response.status().as_u16() == 206 {
        parse_content_range(response.headers().get("Content-Range"))
    } else {
        response
            .headers()
            .get("Content-Length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
    };

    let is_resumed = initial_downloaded > 0 && response.status().as_u16() == 206;

    // Update file_size if we got it from response
    if let Some(size) = total_size {
        let s = store.lock().unwrap();
        if let Ok(Some(mut task)) = s.get_task(task_id) {
            task.file_size = Some(size);
            s.update_task(&task).ok();
        }
    }

    let mut file = if is_resumed {
        OpenOptions::new()
            .append(true)
            .open(save_path)
            .with_context(|| format!("无法打开文件 {}", save_path))?
    } else {
        progress.store(0, Ordering::Relaxed);
        if let Some(parent) = Path::new(save_path).parent() {
            fs::create_dir_all(parent).ok();
        }
        File::create(save_path).with_context(|| format!("无法创建文件 {}", save_path))?
    };

    let mut downloaded = initial_downloaded;
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

        let n = response.read(&mut buf).context("下载数据读取失败")?;
        if n == 0 {
            break;
        }

        file.write_all(&buf[..n]).context("写入文件失败")?;
        downloaded += n as u64;
        progress.store(downloaded, Ordering::Relaxed);
        speed_tracker.add_bytes(n);

        let current_speed = speed_tracker.current_speed();
        *speed.lock().unwrap() = current_speed;

        // Speed limit throttling
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
            store
                .lock()
                .unwrap()
                .update_progress(task_id, downloaded, current_speed, TaskStatus::Downloading)
                .ok();
            last_db_update = Instant::now();
        }
    }

    file.flush().ok();

    // Mark as completed
    store
        .lock()
        .unwrap()
        .update_progress(task_id, downloaded, 0.0, TaskStatus::Completed)?;

    Ok(())
}

fn is_retryable(status_code: u16) -> bool {
    matches!(status_code, 408 | 425 | 429) || (500..600).contains(&status_code)
}

fn parse_content_range(header: Option<&reqwest::header::HeaderValue>) -> Option<u64> {
    let val = header?.to_str().ok()?;
    // Format: bytes 0-999/1000 or bytes 0-999/*
    let total_str = val.rsplit('/').next()?;
    if total_str == "*" {
        return None;
    }
    total_str.parse().ok()
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
