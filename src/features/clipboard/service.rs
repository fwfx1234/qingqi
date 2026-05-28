use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use gpui::App;

use crate::{
    features::clipboard::history_store::{
        self as history_store_mod, ClipboardConfig, ClipboardHistoryStore, ClipboardItemKind,
        ClipboardRecord,
    },
    platform,
};

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
    db_path: PathBuf,
    image_dir: PathBuf,
    config: Arc<Mutex<ClipboardConfig>>,
    last_seen_text: Mutex<String>,
    last_seen_image_id: Mutex<u64>,
    last_seen_files: Mutex<String>,
}

impl ClipboardService {
    pub fn new(db_path: PathBuf) -> Self {
        let image_dir = db_path
            .parent()
            .map(|dir| dir.join("clipboard-images"))
            .unwrap_or_else(|| PathBuf::from("clipboard-images"));
        Self::with_image_dir(db_path, image_dir)
    }

    pub fn with_image_dir(db_path: PathBuf, image_dir: PathBuf) -> Self {
        let config = ClipboardHistoryStore::open(&db_path)
            .and_then(|store| store.load_config())
            .unwrap_or_default();
        Self {
            db_path,
            image_dir,
            config: Arc::new(Mutex::new(config)),
            last_seen_text: Mutex::new(String::new()),
            last_seen_image_id: Mutex::new(0),
            last_seen_files: Mutex::new(String::new()),
        }
    }

    pub fn start(&mut self) {}

    pub fn capture_current(&self, cx: &App) -> Result<bool> {
        let config = self.config();

        // Check for file clipboard first (native macOS file URLs)
        if config.capture_files {
            if let Some(paths) = platform::clipboard::read_file_list() {
                let signature = files_signature(&paths);
                if let Ok(mut last_seen) = self.last_seen_files.lock() {
                    if *last_seen == signature {
                        return Ok(false);
                    }
                    *last_seen = signature;
                }
                if self.capture_files_with_config(&paths, &config)? {
                    return Ok(true);
                }
            }
        }

        if config.capture_text {
            if let Some(text) = platform::clipboard::read_text(cx) {
                // On macOS, when files are copied the text representation can
                // contain paths. If we already captured via read_file_list()
                // above, skip the text path. Otherwise, check if this looks
                // like file paths and capture as files if enabled.
                if config.capture_files && platform::clipboard::text_looks_like_file_paths(&text) {
                    let paths: Vec<String> = text
                        .split(['\n', '\r'])
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty() && s.starts_with('/'))
                        .collect();
                    if !paths.is_empty() {
                        let signature = files_signature(&paths);
                        if let Ok(mut last_seen) = self.last_seen_files.lock() {
                            if *last_seen == signature {
                                return Ok(false);
                            }
                            *last_seen = signature;
                        }
                        if self.capture_files_with_config(&paths, &config)? {
                            return Ok(true);
                        }
                    }
                }
                if let Ok(mut last_seen) = self.last_seen_text.lock() {
                    if *last_seen != text {
                        *last_seen = text.clone();
                        if self.capture_text_with_config(&text, &config)? {
                            return Ok(true);
                        }
                    }
                } else if self.capture_text_with_config(&text, &config)? {
                    return Ok(true);
                }
            }
        }

        if config.capture_image {
            if let Some(image) = platform::clipboard::read_image(cx) {
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

    pub fn capture_text(&self, text: &str) -> Result<bool> {
        let config = self.config();
        self.capture_text_with_config(text, &config)
    }

    fn capture_text_with_config(&self, text: &str, config: &ClipboardConfig) -> Result<bool> {
        let store = self.open_store()?;
        store.add_text(text, config)
    }

    fn capture_files_with_config(
        &self,
        paths: &[String],
        config: &ClipboardConfig,
    ) -> Result<bool> {
        let store = self.open_store()?;
        store.add_files(paths, config)
    }

    fn capture_image(
        &self,
        image: platform::clipboard::ClipboardImage,
        config: &ClipboardConfig,
    ) -> Result<bool> {
        fs::create_dir_all(&self.image_dir).with_context(|| {
            format!(
                "cannot create clipboard image directory {}",
                self.image_dir.display()
            )
        })?;
        let extension = platform::clipboard::image_format_extension(image.format);
        let path = self
            .image_dir
            .join(format!("clipboard-{}.{}", image.id, extension));
        if !path.exists() {
            fs::write(&path, &image.bytes)
                .with_context(|| format!("cannot write clipboard image {}", path.display()))?;
        }

        let size_label = format_bytes(image.bytes.len());
        let format_label = platform::clipboard::image_format_label(image.format);
        let preview = format!("图片剪贴板 · {format_label} · {size_label}");
        self.open_store()?
            .add_image(&path.to_string_lossy(), &preview, format_label, config)
    }

    pub fn copy_record_to_clipboard(&self, record: &ClipboardRecord, cx: &mut App) -> Result<()> {
        match record.kind {
            ClipboardItemKind::Text => {
                platform::clipboard::write_text(cx, record.content.clone());
            }
            ClipboardItemKind::Image => {
                let path = Path::new(&record.content);
                let format = platform::clipboard::image_format_from_path(path)
                    .unwrap_or(gpui::ImageFormat::Png);
                let bytes = fs::read(path)
                    .with_context(|| format!("cannot read clipboard image {}", path.display()))?;
                platform::clipboard::write_image(cx, format, bytes);
            }
            ClipboardItemKind::Files => {
                let paths = history_store_mod::parse_file_paths(&record.content);
                if paths.is_empty() {
                    // No parseable file paths — cannot restore file clipboard
                    anyhow::bail!("file record has no parseable paths");
                }
                // Try native file write first
                match platform::clipboard::write_file_list(&paths) {
                    Ok(()) => {}
                    Err(_) => {
                        // Fallback: write text representation (honest about limitation)
                        let text = paths.join("\n");
                        platform::clipboard::write_text(cx, text);
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
        self.open_store()?.search(query, filter, offset, limit)
    }

    pub fn search_all(&self, query: &str, filter: ClipboardFilter) -> Result<Vec<ClipboardRecord>> {
        self.open_store()?.search_all(query, filter)
    }

    pub fn latest_record(&self) -> Result<Option<ClipboardRecord>> {
        self.open_store()?.latest()
    }

    pub fn toggle_pin(&self, id: i64) -> Result<Option<bool>> {
        self.open_store()?.toggle_pin(id)
    }

    pub fn delete(&self, id: i64) -> Result<bool> {
        self.open_store()?.delete(id)
    }

    pub fn clear_all(&self) -> Result<usize> {
        self.open_store()?.clear_all()
    }

    pub fn clear_unpinned(&self) -> Result<usize> {
        self.open_store()?.clear_unpinned()
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
        crate::core::shortcut::normalize_accelerator(text)
    }

    pub fn close(&mut self) {}

    fn open_store(&self) -> Result<ClipboardHistoryStore> {
        ClipboardHistoryStore::open(&self.db_path)
    }

    fn persist_config(&self, config: ClipboardConfig) -> Result<ClipboardConfig> {
        self.open_store()?.save_config(&config)?;
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
    use crate::features::clipboard::history_store::ClipboardHistoryStore;
    use std::{
        fs,
        path::PathBuf,
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
        let service = ClipboardService::new(path.clone());

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

        let store = ClipboardHistoryStore::open(&path).expect("store should reopen");
        let loaded = store.load_config().expect("config should load");
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
