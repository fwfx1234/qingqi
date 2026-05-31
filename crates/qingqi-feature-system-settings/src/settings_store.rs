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
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        std::env::temp_dir()
            .join(format!("qingqi-settings-store-{nanos}"))
            .join(name)
    }

    #[test]
    fn defaults_to_300_seconds() {
        let store = SettingsStore::new(temp_path("settings.json"));
        assert_eq!(store.plugin_window_retention_seconds(), 300);
    }

    #[test]
    fn persists_and_reloads_retention() {
        let path = temp_path("settings.json");
        {
            let mut store = SettingsStore::new(path.clone());
            store
                .set_plugin_window_retention_seconds(120)
                .expect("set retention");
        }
        let store = SettingsStore::new(path.clone());
        assert_eq!(store.plugin_window_retention_seconds(), 120);

        let _ = std::fs::remove_dir_all(path.parent().expect("temp parent"));
    }

    #[test]
    fn clamps_retention_to_valid_range() {
        let path = temp_path("clamp.json");
        let mut store = SettingsStore::new(path.clone());
        let result = store
            .set_plugin_window_retention_seconds(0)
            .expect("set retention");
        assert_eq!(result, 1);
        assert_eq!(store.plugin_window_retention_seconds(), 1);

        let result = store
            .set_plugin_window_retention_seconds(9999)
            .expect("set retention");
        assert_eq!(result, 3600);
        assert_eq!(store.plugin_window_retention_seconds(), 3600);

        let _ = std::fs::remove_dir_all(path.parent().expect("temp parent"));
    }

    #[test]
    fn restore_default_sets_300() {
        let path = temp_path("restore.json");
        let mut store = SettingsStore::new(path.clone());
        store
            .set_plugin_window_retention_seconds(60)
            .expect("set retention");
        store.restore_default_retention().expect("restore default");
        assert_eq!(store.plugin_window_retention_seconds(), 300);

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
