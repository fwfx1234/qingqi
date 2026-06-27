use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const DEFAULT_PLUGIN_WINDOW_RETENTION_SECONDS: u64 = 300;
const MIN_RETENTION_SECONDS: u64 = 1;
const MAX_RETENTION_SECONDS: u64 = 3600;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SettingsData {
    plugin_window_retention_seconds: u64,
}

impl Default for SettingsData {
    fn default() -> Self {
        Self {
            plugin_window_retention_seconds: DEFAULT_PLUGIN_WINDOW_RETENTION_SECONDS,
        }
    }
}

pub struct SettingsStore {
    path: PathBuf,
    data: SettingsData,
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        let data = Self::load(&path).unwrap_or_else(|error| {
            tracing::warn!(
                path = %path.display(),
                error = %error,
                "failed to load system settings, using defaults"
            );
            SettingsData::default()
        });
        Self { path, data }
    }

    pub fn plugin_window_retention_seconds(&self) -> u64 {
        self.data.plugin_window_retention_seconds
    }

    pub fn set_plugin_window_retention_seconds(&mut self, seconds: u64) -> Result<u64> {
        let clamped = seconds.clamp(MIN_RETENTION_SECONDS, MAX_RETENTION_SECONDS);
        self.data.plugin_window_retention_seconds = clamped;
        self.save()?;
        Ok(clamped)
    }

    pub fn restore_default_retention(&mut self) -> Result<u64> {
        self.data.plugin_window_retention_seconds = DEFAULT_PLUGIN_WINDOW_RETENTION_SECONDS;
        self.save()?;
        Ok(DEFAULT_PLUGIN_WINDOW_RETENTION_SECONDS)
    }

    fn load(path: &Path) -> Result<SettingsData> {
        if !path.exists() {
            return Ok(SettingsData::default());
        }
        let raw = fs::read_to_string(path)
            .with_context(|| format!("cannot read settings {}", path.display()))?;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(SettingsData::default());
        }
        serde_json::from_str(trimmed).context("invalid settings JSON")
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("cannot create settings directory {}", parent.display())
            })?;
        }
        let json = serde_json::to_string_pretty(&self.data).context("cannot encode settings")?;
        fs::write(&self.path, json)
            .with_context(|| format!("cannot write settings {}", self.path.display()))
    }
}

pub fn retention_status_text(seconds: u64) -> String {
    if seconds < 60 {
        format!("内联插件退出后保留 {seconds} 秒")
    } else {
        let minutes = seconds / 60;
        let remain = seconds % 60;
        if remain > 0 {
            format!("内联插件退出后保留 {minutes} 分 {remain} 秒")
        } else {
            format!("内联插件退出后保留 {minutes} 分钟")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_path(name: &str) -> PathBuf {
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        std::env::temp_dir()
            .join(format!("qingqi-settings-test-{nanos}-{id}"))
            .join(name)
    }

    #[test]
    fn defaults_to_5_min_retention() {
        let store = SettingsStore::new(temp_path("settings.json"));
        assert_eq!(store.plugin_window_retention_seconds(), 300);
    }

    #[test]
    fn roundtrip_settings() {
        let path = temp_path("settings.json");
        {
            let mut store = SettingsStore::new(path.clone());
            store.set_plugin_window_retention_seconds(120).unwrap();
        }
        let store2 = SettingsStore::new(path.clone());
        assert_eq!(store2.plugin_window_retention_seconds(), 120);
        let _ = std::fs::remove_dir_all(path.parent().expect("temp parent"));
    }

    #[test]
    fn retention_status_text_formats_correctly() {
        assert_eq!(retention_status_text(30), "内联插件退出后保留 30 秒");
        assert_eq!(retention_status_text(60), "内联插件退出后保留 1 分钟");
        assert_eq!(retention_status_text(90), "内联插件退出后保留 1 分 30 秒");
        assert_eq!(retention_status_text(300), "内联插件退出后保留 5 分钟");
    }
}
