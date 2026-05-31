use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result};
use gpui::App;

use crate::{
    data_source::ClipboardDataSource,
    history_store::{
        self as history_store_mod, ClipboardConfig, ClipboardItemKind, ClipboardRecord,
    },
};
use qingqi_plugin::database::DatabaseService;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardFilter {
    All,
    Pinned,
    Text,
    Link,
    Code,
    Image,
    Files,
}

impl ClipboardFilter {
    pub fn kind(&self) -> Option<ClipboardItemKind> {
        match self {
            Self::All | Self::Pinned => None,
            Self::Text | Self::Link | Self::Code => Some(ClipboardItemKind::Text),
            Self::Image => Some(ClipboardItemKind::Image),
            Self::Files => Some(ClipboardItemKind::Files),
        }
    }

    pub fn pinned_only(&self) -> bool {
        matches!(self, Self::Pinned)
    }

    /// Returns the badge string to filter by, if applicable.
    pub fn badge_filter(&self) -> Option<&'static str> {
        match self {
            Self::Link => Some("链接"),
            Self::Code => Some("JSON"),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "全部",
            Self::Pinned => "置顶",
            Self::Text => "文本",
            Self::Link => "链接",
            Self::Code => "代码",
            Self::Image => "图片",
            Self::Files => "文件",
        }
    }
}

pub struct ClipboardService {
    database: Arc<DatabaseService>,
    image_dir: PathBuf,
    data_source: Mutex<Option<ClipboardDataSource>>,
    config: Arc<Mutex<ClipboardConfig>>,
    last_seen_text: Mutex<String>,
    last_seen_image_id: Mutex<u64>,
    last_seen_files: Mutex<String>,
    last_seen_change_count: Mutex<Option<i64>>,
    watch_started: bool,
}

enum ClipboardBackgroundRead {
    Unsupported,
    Unchanged,
    Snapshot(i64, qingqi_platform::clipboard::ClipboardSnapshot),
}

impl ClipboardService {
    pub fn new(database: Arc<DatabaseService>, db_path: PathBuf) -> Self {
        let image_dir = db_path
            .parent()
            .map(|dir| dir.join("clipboard-images"))
            .unwrap_or_else(|| PathBuf::from("clipboard-images"));
        Self::with_image_dir(database, image_dir)
    }

    pub fn with_image_dir(database: Arc<DatabaseService>, image_dir: PathBuf) -> Self {
        let opened = match ClipboardDataSource::open(Arc::clone(&database), "clipboard/history") {
            Ok(data_source) => Some(data_source),
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "clipboard data source unavailable; background service will retry"
                );
                None
            }
        };
        let config = opened
            .as_ref()
            .and_then(|data_source| data_source.load_config().ok())
            .unwrap_or_default();
        Self {
            database,
            image_dir,
            data_source: Mutex::new(opened),
            config: Arc::new(Mutex::new(config)),
            last_seen_text: Mutex::new(String::new()),
            last_seen_image_id: Mutex::new(0),
            last_seen_files: Mutex::new(String::new()),
            last_seen_change_count: Mutex::new(None),
            watch_started: false,
        }
    }

    pub fn start(&mut self) {}

    pub fn start_background(service: Arc<Mutex<Self>>, cx: &mut App) {
        {
            let Ok(mut service_guard) = service.lock() else {
                tracing::warn!("clipboard service lock poisoned while starting background worker");
                return;
            };
            if service_guard.watch_started {
                return;
            }
            service_guard.watch_started = true;
        }

        cx.spawn(async move |async_cx| {
            loop {
                async_cx
                    .background_executor()
                    .timer(Duration::from_millis(700))
                    .await;

                let service_for_snapshot = Arc::clone(&service);
                let snapshot = async_cx
                    .background_executor()
                    .spawn(async move {
                        let service = service_for_snapshot
                            .lock()
                            .map_err(|_| anyhow::anyhow!("clipboard service lock poisoned"))?;
                        service.read_background_snapshot()
                    })
                    .await;

                match snapshot {
                    Ok(ClipboardBackgroundRead::Unchanged) => {}
                    Ok(ClipboardBackgroundRead::Snapshot(change_count, snapshot))
                        if !snapshot.is_empty() =>
                    {
                        let service = Arc::clone(&service);
                        let _ = async_cx.update(move |_cx| {
                            if let Ok(service) = service.lock() {
                                match service.capture_snapshot(snapshot) {
                                    Ok(_) => {
                                        if let Err(error) =
                                            service.mark_change_count_seen(Some(change_count))
                                        {
                                            tracing::warn!(
                                                error = %error,
                                                "failed to mark clipboard change count"
                                            );
                                        }
                                    }
                                    Err(error) => {
                                        tracing::warn!(
                                            error = %error,
                                            "clipboard capture failed; will retry this change"
                                        );
                                    }
                                }
                            }
                        });
                    }
                    Ok(ClipboardBackgroundRead::Unsupported) => {
                        let service = Arc::clone(&service);
                        let _ = async_cx.update(move |cx| {
                            if let Ok(service) = service.lock() {
                                if let Err(error) = service.capture_current(cx) {
                                    tracing::warn!(
                                        error = %error,
                                        "clipboard fallback capture failed"
                                    );
                                }
                            }
                        });
                    }
                    Err(error) => {
                        tracing::warn!(error = %error, "clipboard background read failed");
                    }
                    Ok(ClipboardBackgroundRead::Snapshot(change_count, _)) => {
                        let service = Arc::clone(&service);
                        let _ = async_cx.update(move |_cx| {
                            if let Ok(service) = service.lock() {
                                if let Err(error) =
                                    service.mark_change_count_seen(Some(change_count))
                                {
                                    tracing::warn!(
                                        error = %error,
                                        "failed to mark empty clipboard change count"
                                    );
                                }
                            }
                        });
                    }
                }
            }
        })
        .detach();
    }

    pub fn capture_current(&self, cx: &App) -> Result<bool> {
        let change_count = qingqi_platform::clipboard::change_count();
        if self.has_seen_change_count(change_count)? == Some(true) {
            return Ok(false);
        }

        let snapshot = qingqi_platform::clipboard::read_snapshot(cx, self.last_seen_image_id());
        let captured = self.capture_snapshot(snapshot)?;
        self.mark_change_count_seen(change_count)?;
        Ok(captured)
    }

    fn read_background_snapshot(&self) -> Result<ClipboardBackgroundRead> {
        let Some(change_count) = qingqi_platform::clipboard::change_count() else {
            return Ok(ClipboardBackgroundRead::Unsupported);
        };
        if self.has_seen_change_count(Some(change_count))? == Some(true) {
            return Ok(ClipboardBackgroundRead::Unchanged);
        }
        Ok(ClipboardBackgroundRead::Snapshot(
            change_count,
            qingqi_platform::clipboard::read_background_snapshot(
                change_count,
                self.last_seen_image_id(),
            ),
        ))
    }

    fn has_seen_change_count(&self, change_count: Option<i64>) -> Result<Option<bool>> {
        let Some(change_count) = change_count else {
            return Ok(None);
        };
        let last = self
            .last_seen_change_count
            .lock()
            .map_err(|_| anyhow::anyhow!("clipboard change-count lock poisoned"))?;
        Ok(Some(*last == Some(change_count)))
    }

    fn mark_change_count_seen(&self, change_count: Option<i64>) -> Result<()> {
        let Some(change_count) = change_count else {
            return Ok(());
        };
        *self
            .last_seen_change_count
            .lock()
            .map_err(|_| anyhow::anyhow!("clipboard change-count lock poisoned"))? =
            Some(change_count);
        Ok(())
    }

    pub fn capture_snapshot(
        &self,
        snapshot: qingqi_platform::clipboard::ClipboardSnapshot,
    ) -> Result<bool> {
        let config = self.config();

        // Check for file clipboard first (native macOS file URLs)
        if config.capture_files {
            if let Some(paths) = snapshot.files.clone() {
                let signature = files_signature(&paths);
                if self
                    .last_seen_files
                    .lock()
                    .map(|last_seen| *last_seen == signature)
                    .unwrap_or(false)
                {
                    return Ok(false);
                }
                let captured = self.capture_files_with_config(&paths, &config)?;
                if let Ok(mut last_seen) = self.last_seen_files.lock() {
                    *last_seen = signature;
                }
                if captured {
                    return Ok(true);
                }
            }
        }

        if config.capture_text {
            if let Some(text) = snapshot.text {
                // On macOS, when files are copied the text representation can
                // contain paths. If we already captured via read_file_list()
                // above, skip the text path. Otherwise, check if this looks
                // like file paths and capture as files if enabled.
                if config.capture_files
                    && qingqi_platform::clipboard::text_looks_like_file_paths(&text)
                {
                    let paths: Vec<String> = text
                        .split(['\n', '\r'])
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty() && s.starts_with('/'))
                        .collect();
                    if !paths.is_empty() {
                        let signature = files_signature(&paths);
                        if self
                            .last_seen_files
                            .lock()
                            .map(|last_seen| *last_seen == signature)
                            .unwrap_or(false)
                        {
                            return Ok(false);
                        }
                        let captured = self.capture_files_with_config(&paths, &config)?;
                        if let Ok(mut last_seen) = self.last_seen_files.lock() {
                            *last_seen = signature;
                        }
                        if captured {
                            return Ok(true);
                        }
                    }
                }

                let seen_text = self
                    .last_seen_text
                    .lock()
                    .map(|last_seen| *last_seen == text)
                    .unwrap_or(false);
                if !seen_text {
                    let captured = self.capture_text_with_config(&text, &config)?;
                    if let Ok(mut last_seen) = self.last_seen_text.lock() {
                        *last_seen = text.clone();
                    }
                    if captured {
                        return Ok(true);
                    }
                }
            }
        }

        if config.capture_image {
            if let Some(image) = snapshot.image {
                if let Ok(last_seen) = self.last_seen_image_id.lock() {
                    if *last_seen == image.id {
                        return Ok(false);
                    }
                }
                let image_id = image.id;
                let captured = self.capture_image(image, &config)?;
                if captured {
                    if let Ok(mut last_seen) = self.last_seen_image_id.lock() {
                        *last_seen = image_id;
                    }
                }
                return Ok(captured);
            }
        }

        Ok(false)
    }

    pub fn last_seen_image_id(&self) -> Option<u64> {
        self.last_seen_image_id
            .lock()
            .ok()
            .and_then(|id| if *id == 0 { None } else { Some(*id) })
    }

    pub fn capture_text(&self, text: &str) -> Result<bool> {
        let config = self.config();
        self.capture_text_with_config(text, &config)
    }

    fn capture_text_with_config(&self, text: &str, config: &ClipboardConfig) -> Result<bool> {
        self.with_data_source(|data_source| data_source.add_text(text, config))
    }

    fn capture_files_with_config(
        &self,
        paths: &[String],
        config: &ClipboardConfig,
    ) -> Result<bool> {
        self.with_data_source(|data_source| data_source.add_files(paths, config))
    }

    fn capture_image(
        &self,
        image: qingqi_platform::clipboard::ClipboardImage,
        config: &ClipboardConfig,
    ) -> Result<bool> {
        fs::create_dir_all(&self.image_dir).with_context(|| {
            format!(
                "cannot create clipboard image directory {}",
                self.image_dir.display()
            )
        })?;
        let extension = qingqi_platform::clipboard::image_format_extension(image.format);
        let path = self
            .image_dir
            .join(format!("clipboard-{}.{}", image.id, extension));
        if !path.exists() {
            fs::write(&path, &image.bytes)
                .with_context(|| format!("cannot write clipboard image {}", path.display()))?;
        }

        let size_label = format_bytes(image.bytes.len());
        let format_label = qingqi_platform::clipboard::image_format_label(image.format);
        let preview = format!("图片剪贴板 · {format_label} · {size_label}");
        self.with_data_source(|data_source| {
            data_source.add_image(&path.to_string_lossy(), &preview, format_label, config)
        })
    }

    pub fn copy_record_to_clipboard(&self, record: &ClipboardRecord, cx: &mut App) -> Result<()> {
        match record.kind {
            ClipboardItemKind::Text => {
                qingqi_platform::clipboard::write_text(cx, record.content.clone());
            }
            ClipboardItemKind::Image => {
                let path = Path::new(&record.content);
                let format = qingqi_platform::clipboard::image_format_from_path(path)
                    .unwrap_or(gpui::ImageFormat::Png);
                let bytes = fs::read(path)
                    .with_context(|| format!("cannot read clipboard image {}", path.display()))?;
                qingqi_platform::clipboard::write_image(cx, format, bytes);
            }
            ClipboardItemKind::Files => {
                let paths = history_store_mod::parse_file_paths(&record.content);
                if paths.is_empty() {
                    // No parseable file paths — cannot restore file clipboard
                    anyhow::bail!("file record has no parseable paths");
                }
                // Try native file write first
                match qingqi_platform::clipboard::write_file_list(&paths) {
                    Ok(()) => {}
                    Err(_) => {
                        // Fallback: write text representation (honest about limitation)
                        let text = paths.join("\n");
                        qingqi_platform::clipboard::write_text(cx, text);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn search(
        &self,
        query: &str,
        filter: ClipboardFilter,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<ClipboardRecord>> {
        self.with_data_source(|data_source| data_source.search(query, filter, offset, limit))
    }

    pub fn search_all(&self, query: &str, filter: ClipboardFilter) -> Result<Vec<ClipboardRecord>> {
        self.with_data_source(|data_source| data_source.search_all(query, filter))
    }

    pub fn latest_record(&self) -> Result<Option<ClipboardRecord>> {
        self.with_data_source(|data_source| data_source.latest())
    }

    pub fn toggle_pin(&self, id: i64) -> Result<Option<bool>> {
        self.with_data_source(|data_source| data_source.toggle_pin(id))
    }

    pub fn delete(&self, id: i64) -> Result<bool> {
        self.with_data_source(|data_source| data_source.delete(id))
    }

    pub fn clear_all(&self) -> Result<usize> {
        self.with_data_source(|data_source| data_source.clear_all())
    }

    pub fn clear_unpinned(&self) -> Result<usize> {
        self.with_data_source(|data_source| data_source.clear_unpinned())
    }

    pub fn config(&self) -> ClipboardConfig {
        self.config
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default()
    }

    pub fn set_capture_text(&self, enabled: bool) -> Result<ClipboardConfig> {
        let mut config = self.config();
        config.capture_text = enabled;
        self.persist_config(config)
    }

    pub fn set_capture_image(&self, enabled: bool) -> Result<ClipboardConfig> {
        let mut config = self.config();
        config.capture_image = enabled;
        self.persist_config(config)
    }

    pub fn set_capture_files(&self, enabled: bool) -> Result<ClipboardConfig> {
        let mut config = self.config();
        config.capture_files = enabled;
        self.persist_config(config)
    }

    pub fn set_max_text_chars(&self, max_text_chars: usize) -> Result<ClipboardConfig> {
        let mut config = self.config();
        config.max_text_chars = max_text_chars;
        self.persist_config(config)
    }

    pub fn set_ignore_patterns(&self, ignore_patterns: Vec<String>) -> Result<ClipboardConfig> {
        let mut config = self.config();
        config.ignore_patterns = ignore_patterns;
        self.persist_config(config)
    }

    pub fn set_hotkey(&self, hotkey: String) -> Result<ClipboardConfig> {
        let mut config = self.config();
        config.hotkey = hotkey;
        self.persist_config(config)
    }

    pub fn parse_ignore_patterns(text: &str) -> Vec<String> {
        text.split(['|', '\n'])
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    }

    pub fn normalize_hotkey(text: &str) -> Option<String> {
        qingqi_plugin::shortcut::normalize_accelerator(text)
    }

    pub fn close(&mut self) {}

    /// Runs `f` against a lazily-opened, reused SQLite connection so each
    /// capture/search/mutation no longer pays for opening the database and
    /// re-running schema migrations every time.
    fn with_data_source<T>(&self, f: impl FnOnce(&ClipboardDataSource) -> Result<T>) -> Result<T> {
        let mut guard = self
            .data_source
            .lock()
            .map_err(|_| anyhow::anyhow!("clipboard data source lock poisoned"))?;
        if guard.is_none() {
            *guard = Some(
                ClipboardDataSource::open(Arc::clone(&self.database), "clipboard/history")
                    .context("cannot open clipboard data source")?,
            );
        }
        f(guard
            .as_ref()
            .expect("clipboard data source just initialized"))
    }

    fn persist_config(&self, config: ClipboardConfig) -> Result<ClipboardConfig> {
        self.with_data_source(|data_source| data_source.save_config(&config))?;
        if let Ok(mut current) = self.config.lock() {
            *current = config.clone();
        }
        Ok(config)
    }
}

impl Drop for ClipboardService {
    fn drop(&mut self) {
        self.close();
    }
}

fn format_bytes(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / MB)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / KB)
    } else {
        format!("{bytes} B")
    }
}

/// Create a dedup signature for a file list.
fn files_signature(paths: &[String]) -> String {
    format!("files:{}", paths.join("|"))
}

#[cfg(test)]
mod tests {
    use super::ClipboardService;
    use crate::data_source::ClipboardDataSource;
    use qingqi_plugin::{database::DatabaseService, storage::AppPaths};
    use std::{
        fs,
        path::PathBuf,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_db(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-clipboard-service-{nanos}"));
        let _ = fs::create_dir_all(&dir);
        dir.join(name)
    }

    #[test]
    fn parse_ignore_patterns_supports_pipe_and_newline() {
        assert_eq!(
            ClipboardService::parse_ignore_patterns(" secret | token\n\n demo "),
            vec!["secret", "token", "demo"]
        );
    }

    #[test]
    fn normalize_hotkey_matches_suishou_style() {
        assert_eq!(
            ClipboardService::normalize_hotkey("cmd + shift + v"),
            Some(String::from("Shift+Win+V"))
        );
        assert_eq!(
            ClipboardService::normalize_hotkey("ctrl+alt+space"),
            Some(String::from("Ctrl+Alt+Space"))
        );
        assert_eq!(ClipboardService::normalize_hotkey("ctrl+alt"), None);
    }

    #[test]
    fn config_mutators_persist_and_refresh_cache() {
        let path = temp_db("config-service.db");
        let database = Arc::new(DatabaseService::new(AppPaths::for_test(
            path.parent().unwrap().to_path_buf(),
        )));
        database
            .register_database(qingqi_plugin::database::DatabaseSpec::path(
                "clipboard/history",
                path.clone(),
            ))
            .unwrap();
        let service = ClipboardService::new(database, path.clone());

        let config = service
            .set_capture_text(false)
            .expect("capture_text should persist");
        assert!(!config.capture_text);

        let config = service
            .set_capture_image(false)
            .expect("capture_image should persist");
        assert!(!config.capture_image);

        let config = service
            .set_capture_files(false)
            .expect("capture_files should persist");
        assert!(!config.capture_files);

        let config = service
            .set_max_text_chars(512)
            .expect("max_text_chars should persist");
        assert_eq!(config.max_text_chars, 512);

        let config = service
            .set_ignore_patterns(vec![String::from("secret"), String::from("^token:")])
            .expect("ignore patterns should persist");
        assert_eq!(config.ignore_patterns.len(), 2);

        let config = service
            .set_hotkey(String::from("Ctrl+Alt+V"))
            .expect("hotkey should persist");
        assert_eq!(config.hotkey, "Ctrl+Alt+V");

        let cached = service.config();
        assert!(!cached.capture_text);
        assert!(!cached.capture_image);
        assert!(!cached.capture_files);
        assert_eq!(cached.max_text_chars, 512);
        assert_eq!(
            cached.ignore_patterns,
            vec![String::from("secret"), String::from("^token:")]
        );
        assert_eq!(cached.hotkey, "Ctrl+Alt+V");

        let database = Arc::new(DatabaseService::new(AppPaths::for_test(
            path.parent().unwrap().to_path_buf(),
        )));
        database
            .register_database(qingqi_plugin::database::DatabaseSpec::path(
                "clipboard/history",
                path,
            ))
            .unwrap();
        let data_source = ClipboardDataSource::open(database, "clipboard/history")
            .expect("data source should reopen");
        let loaded = data_source.load_config().expect("config should load");
        assert_eq!(loaded, cached);
    }

    #[test]
    fn files_signature_deduplicates_same_paths() {
        let a = vec![String::from("/tmp/a.txt"), String::from("/tmp/b.txt")];
        let b = vec![String::from("/tmp/a.txt"), String::from("/tmp/b.txt")];
        let c = vec![String::from("/tmp/c.txt")];
        assert_eq!(super::files_signature(&a), super::files_signature(&b));
        assert_ne!(super::files_signature(&a), super::files_signature(&c));
    }
}
