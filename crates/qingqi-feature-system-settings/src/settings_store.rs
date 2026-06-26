use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use qingqi_platform::tray_settings::{NetworkSpeedDisplayMode, NetworkSpeedTextMode};
use serde::{Deserialize, Serialize};

const DEFAULT_PLUGIN_WINDOW_RETENTION_SECONDS: u64 = 300;
const MIN_RETENTION_SECONDS: u64 = 1;
const MAX_RETENTION_SECONDS: u64 = 3600;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SettingsData {
    plugin_window_retention_seconds: u64,
    #[serde(default = "default_network_speed_visible")]
    tray_network_speed_visible: bool,
    #[serde(default)]
    tray_network_speed_show_icon: bool,
    #[serde(default)]
    tray_network_speed_display_mode: NetworkSpeedDisplayMode,
    #[serde(default)]
    tray_network_speed_text_mode: NetworkSpeedTextMode,
    #[serde(default = "default_network_speed_update_interval_ms")]
    tray_network_speed_update_interval_ms: u64,
    #[serde(default = "default_popup_width")]
    tray_popup_width: u32,
    #[serde(default = "default_popup_height")]
    tray_popup_height: u32,
    #[serde(default = "default_network_speed_show_totals")]
    tray_network_speed_show_totals: bool,
    #[serde(default = "default_network_speed_show_interfaces")]
    tray_network_speed_show_interfaces: bool,
    #[serde(default = "default_network_speed_max_interfaces")]
    tray_network_speed_max_interfaces: u8,
}

impl Default for SettingsData {
    fn default() -> Self {
        Self {
            plugin_window_retention_seconds: DEFAULT_PLUGIN_WINDOW_RETENTION_SECONDS,
            tray_network_speed_visible: default_network_speed_visible(),
            tray_network_speed_show_icon: false,
            tray_network_speed_display_mode: NetworkSpeedDisplayMode::TextOnly,
            tray_network_speed_text_mode: NetworkSpeedTextMode::DownloadOnly,
            tray_network_speed_update_interval_ms: default_network_speed_update_interval_ms(),
            tray_popup_width: default_popup_width(),
            tray_popup_height: default_popup_height(),
            tray_network_speed_show_totals: default_network_speed_show_totals(),
            tray_network_speed_show_interfaces: default_network_speed_show_interfaces(),
            tray_network_speed_max_interfaces: default_network_speed_max_interfaces(),
        }
    }
}

fn default_network_speed_visible() -> bool {
    true
}

fn default_network_speed_update_interval_ms() -> u64 {
    1000
}

fn default_popup_width() -> u32 {
    340
}

fn default_popup_height() -> u32 {
    360
}

fn default_network_speed_show_totals() -> bool {
    true
}

fn default_network_speed_show_interfaces() -> bool {
    true
}

fn default_network_speed_max_interfaces() -> u8 {
    5
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

    pub fn tray_network_speed_visible(&self) -> bool {
        self.data.tray_network_speed_visible
    }

    pub fn tray_network_speed_show_icon(&self) -> bool {
        self.data.tray_network_speed_show_icon
    }

    pub fn tray_network_speed_display_mode(&self) -> NetworkSpeedDisplayMode {
        self.data.tray_network_speed_display_mode
    }

    pub fn tray_network_speed_text_mode(&self) -> NetworkSpeedTextMode {
        self.data.tray_network_speed_text_mode
    }

    pub fn tray_network_speed_update_interval_ms(&self) -> u64 {
        self.data.tray_network_speed_update_interval_ms
    }

    pub fn tray_popup_width(&self) -> u32 {
        self.data.tray_popup_width
    }

    pub fn tray_popup_height(&self) -> u32 {
        self.data.tray_popup_height
    }

    pub fn tray_network_speed_show_totals(&self) -> bool {
        self.data.tray_network_speed_show_totals
    }

    pub fn tray_network_speed_show_interfaces(&self) -> bool {
        self.data.tray_network_speed_show_interfaces
    }

    pub fn tray_network_speed_max_interfaces(&self) -> u8 {
        self.data.tray_network_speed_max_interfaces
    }

    pub fn set_tray_network_speed_visible(&mut self, visible: bool) -> Result<()> {
        self.data.tray_network_speed_visible = visible;
        self.save()?;
        self.save_tray_settings()
    }

    pub fn set_tray_network_speed_show_icon(&mut self, show_icon: bool) -> Result<()> {
        self.data.tray_network_speed_show_icon = show_icon;
        self.data.tray_network_speed_display_mode = if show_icon {
            NetworkSpeedDisplayMode::IconAndText
        } else {
            NetworkSpeedDisplayMode::TextOnly
        };
        self.save()?;
        self.save_tray_settings()
    }

    pub fn set_tray_network_speed_display_mode(
        &mut self,
        mode: NetworkSpeedDisplayMode,
    ) -> Result<()> {
        self.data.tray_network_speed_display_mode = mode;
        self.data.tray_network_speed_show_icon = matches!(
            mode,
            NetworkSpeedDisplayMode::IconOnly | NetworkSpeedDisplayMode::IconAndText
        );
        self.save()?;
        self.save_tray_settings()
    }

    pub fn set_tray_network_speed_text_mode(&mut self, mode: NetworkSpeedTextMode) -> Result<()> {
        self.data.tray_network_speed_text_mode = mode;
        self.save()?;
        self.save_tray_settings()
    }

    pub fn set_tray_network_speed_update_interval_ms(&mut self, interval_ms: u64) -> Result<()> {
        self.data.tray_network_speed_update_interval_ms = interval_ms.clamp(500, 5000);
        self.save()?;
        self.save_tray_settings()
    }

    pub fn set_tray_popup_width(&mut self, width: u32) -> Result<()> {
        self.data.tray_popup_width = width.clamp(280, 520);
        self.save()?;
        self.save_tray_settings()
    }

    pub fn set_tray_popup_height(&mut self, height: u32) -> Result<()> {
        self.data.tray_popup_height = height.clamp(240, 640);
        self.save()?;
        self.save_tray_settings()
    }

    pub fn set_tray_network_speed_show_totals(&mut self, show: bool) -> Result<()> {
        self.data.tray_network_speed_show_totals = show;
        self.save()?;
        self.save_tray_settings()
    }

    pub fn set_tray_network_speed_show_interfaces(&mut self, show: bool) -> Result<()> {
        self.data.tray_network_speed_show_interfaces = show;
        self.save()?;
        self.save_tray_settings()
    }

    pub fn set_tray_network_speed_max_interfaces(&mut self, max_interfaces: u8) -> Result<()> {
        self.data.tray_network_speed_max_interfaces = max_interfaces.clamp(0, 10);
        self.save()?;
        self.save_tray_settings()
    }

    fn save_tray_settings(&self) -> Result<()> {
        let mut store = qingqi_platform::tray_settings::TraySettingsStore::new(
            self.path
                .parent()
                .map(|parent| parent.join("tray.json"))
                .unwrap_or_else(|| PathBuf::from("tray.json")),
        );
        store.set_network_speed_visible(self.data.tray_network_speed_visible)?;
        store.set_network_speed_display_mode(self.data.tray_network_speed_display_mode)?;
        store.set_network_speed_text_mode(self.data.tray_network_speed_text_mode)?;
        store.set_network_speed_update_interval_ms(
            self.data.tray_network_speed_update_interval_ms,
        )?;
        store.set_popup_size(self.data.tray_popup_width, self.data.tray_popup_height)?;
        store.set_network_speed_show_totals(self.data.tray_network_speed_show_totals)?;
        store.set_network_speed_show_interfaces(self.data.tray_network_speed_show_interfaces)?;
        store.set_network_speed_max_interfaces(self.data.tray_network_speed_max_interfaces)
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
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!(
                "qingqi-settings-store-{}-{nanos}-{counter}",
                std::process::id()
            ))
            .join(name)
    }

    #[test]
    fn defaults_to_300_seconds() {
        let store = SettingsStore::new(temp_path("settings.json"));
        assert_eq!(store.plugin_window_retention_seconds(), 300);
        assert!(store.tray_network_speed_visible());
        assert!(!store.tray_network_speed_show_icon());
        assert_eq!(
            store.tray_network_speed_display_mode(),
            NetworkSpeedDisplayMode::TextOnly
        );
        assert_eq!(
            store.tray_network_speed_text_mode(),
            NetworkSpeedTextMode::DownloadOnly
        );
        assert_eq!(store.tray_network_speed_update_interval_ms(), 1000);
        assert_eq!(store.tray_popup_width(), 340);
        assert_eq!(store.tray_popup_height(), 360);
        assert!(store.tray_network_speed_show_totals());
        assert!(store.tray_network_speed_show_interfaces());
        assert_eq!(store.tray_network_speed_max_interfaces(), 5);
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

    #[test]
    fn persists_tray_settings() {
        let path = temp_path("settings.json");
        {
            let mut store = SettingsStore::new(path.clone());
            store
                .set_tray_network_speed_visible(false)
                .expect("visible");
            store
                .set_tray_network_speed_show_icon(true)
                .expect("show icon");
            store
                .set_tray_network_speed_text_mode(NetworkSpeedTextMode::Both)
                .expect("text mode");
            store
                .set_tray_network_speed_update_interval_ms(500)
                .expect("interval");
            store.set_tray_popup_width(420).expect("popup width");
            store.set_tray_popup_height(480).expect("popup height");
            store
                .set_tray_network_speed_show_totals(false)
                .expect("totals");
            store
                .set_tray_network_speed_show_interfaces(false)
                .expect("interfaces");
            store
                .set_tray_network_speed_max_interfaces(2)
                .expect("max interfaces");
        }
        let store = SettingsStore::new(path.clone());
        assert!(!store.tray_network_speed_visible());
        assert!(store.tray_network_speed_show_icon());
        assert_eq!(
            store.tray_network_speed_display_mode(),
            NetworkSpeedDisplayMode::IconAndText
        );
        assert_eq!(
            store.tray_network_speed_text_mode(),
            NetworkSpeedTextMode::Both
        );
        assert_eq!(store.tray_network_speed_update_interval_ms(), 500);
        assert_eq!(store.tray_popup_width(), 420);
        assert_eq!(store.tray_popup_height(), 480);
        assert!(!store.tray_network_speed_show_totals());
        assert!(!store.tray_network_speed_show_interfaces());
        assert_eq!(store.tray_network_speed_max_interfaces(), 2);

        let tray_settings = qingqi_platform::tray_settings::load_tray_settings(
            &path.parent().unwrap().join("tray.json"),
        )
        .expect("tray settings");
        assert!(!tray_settings.network_speed_visible);
        assert!(tray_settings.network_speed_show_icon);
        assert_eq!(
            tray_settings.network_speed_display_mode,
            NetworkSpeedDisplayMode::IconAndText
        );
        assert_eq!(
            tray_settings.network_speed_text_mode,
            NetworkSpeedTextMode::Both
        );
        assert_eq!(tray_settings.network_speed_update_interval_ms, 500);
        assert_eq!(tray_settings.popup_width, 420);
        assert_eq!(tray_settings.popup_height, 480);
        assert!(!tray_settings.network_speed_show_totals);
        assert!(!tray_settings.network_speed_show_interfaces);
        assert_eq!(tray_settings.network_speed_max_interfaces, 2);

        let _ = std::fs::remove_dir_all(path.parent().expect("temp parent"));
    }
}
