use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const DEFAULT_NETWORK_SPEED_UPDATE_INTERVAL_MS: u64 = 1000;
const MIN_NETWORK_SPEED_UPDATE_INTERVAL_MS: u64 = 500;
const MAX_NETWORK_SPEED_UPDATE_INTERVAL_MS: u64 = 5000;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NetworkSpeedDisplayMode {
    TextOnly,
    IconOnly,
    IconAndText,
}

impl Default for NetworkSpeedDisplayMode {
    fn default() -> Self {
        Self::TextOnly
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NetworkSpeedTextMode {
    Both,
    DownloadOnly,
    UploadOnly,
    Dominant,
}

impl Default for NetworkSpeedTextMode {
    fn default() -> Self {
        Self::DownloadOnly
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraySettings {
    pub network_speed_visible: bool,
    #[serde(default)]
    pub network_speed_show_icon: bool,
    #[serde(default)]
    pub network_speed_display_mode: NetworkSpeedDisplayMode,
    #[serde(default)]
    pub network_speed_text_mode: NetworkSpeedTextMode,
    #[serde(default = "default_network_speed_update_interval_ms")]
    pub network_speed_update_interval_ms: u64,
    #[serde(default = "default_popup_width")]
    pub popup_width: u32,
    #[serde(default = "default_popup_height")]
    pub popup_height: u32,
    #[serde(default = "default_network_speed_show_totals")]
    pub network_speed_show_totals: bool,
    #[serde(default = "default_network_speed_show_interfaces")]
    pub network_speed_show_interfaces: bool,
    #[serde(default = "default_network_speed_max_interfaces")]
    pub network_speed_max_interfaces: u8,
}

impl Default for TraySettings {
    fn default() -> Self {
        Self {
            network_speed_visible: true,
            network_speed_show_icon: false,
            network_speed_display_mode: NetworkSpeedDisplayMode::TextOnly,
            network_speed_text_mode: NetworkSpeedTextMode::DownloadOnly,
            network_speed_update_interval_ms: DEFAULT_NETWORK_SPEED_UPDATE_INTERVAL_MS,
            popup_width: default_popup_width(),
            popup_height: default_popup_height(),
            network_speed_show_totals: default_network_speed_show_totals(),
            network_speed_show_interfaces: default_network_speed_show_interfaces(),
            network_speed_max_interfaces: default_network_speed_max_interfaces(),
        }
    }
}

fn default_network_speed_update_interval_ms() -> u64 {
    DEFAULT_NETWORK_SPEED_UPDATE_INTERVAL_MS
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

impl TraySettings {
    pub fn sanitized(mut self) -> Self {
        self.network_speed_update_interval_ms = self.network_speed_update_interval_ms.clamp(
            MIN_NETWORK_SPEED_UPDATE_INTERVAL_MS,
            MAX_NETWORK_SPEED_UPDATE_INTERVAL_MS,
        );
        self.popup_width = self.popup_width.clamp(280, 520);
        self.popup_height = self.popup_height.clamp(240, 640);
        self.network_speed_max_interfaces = self.network_speed_max_interfaces.clamp(0, 10);
        self
    }

    pub fn effective_network_speed_show_icon(&self) -> bool {
        matches!(
            self.network_speed_display_mode,
            NetworkSpeedDisplayMode::IconOnly | NetworkSpeedDisplayMode::IconAndText
        ) || self.network_speed_show_icon
    }

    pub fn effective_network_speed_show_text(&self) -> bool {
        !matches!(
            self.network_speed_display_mode,
            NetworkSpeedDisplayMode::IconOnly
        )
    }

    pub fn network_speed_update_interval(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.network_speed_update_interval_ms)
    }
}

pub struct TraySettingsStore {
    path: PathBuf,
    data: TraySettings,
}

impl TraySettingsStore {
    pub fn new(path: PathBuf) -> Self {
        let data = load_tray_settings(&path).unwrap_or_else(|error| {
            tracing::warn!(
                path = %path.display(),
                error = %error,
                "failed to load tray settings, using defaults"
            );
            TraySettings::default()
        });
        Self { path, data }
    }

    pub fn settings(&self) -> TraySettings {
        self.data.clone()
    }

    pub fn set_network_speed_visible(&mut self, visible: bool) -> Result<()> {
        self.data.network_speed_visible = visible;
        self.save()
    }

    pub fn set_network_speed_show_icon(&mut self, show_icon: bool) -> Result<()> {
        self.data.network_speed_show_icon = show_icon;
        self.data.network_speed_display_mode = if show_icon {
            NetworkSpeedDisplayMode::IconAndText
        } else {
            NetworkSpeedDisplayMode::TextOnly
        };
        self.save()
    }

    pub fn set_network_speed_display_mode(&mut self, mode: NetworkSpeedDisplayMode) -> Result<()> {
        self.data.network_speed_display_mode = mode;
        self.data.network_speed_show_icon = matches!(
            mode,
            NetworkSpeedDisplayMode::IconOnly | NetworkSpeedDisplayMode::IconAndText
        );
        self.save()
    }

    pub fn set_network_speed_text_mode(&mut self, mode: NetworkSpeedTextMode) -> Result<()> {
        self.data.network_speed_text_mode = mode;
        self.save()
    }

    pub fn set_network_speed_update_interval_ms(&mut self, interval_ms: u64) -> Result<()> {
        self.data.network_speed_update_interval_ms = interval_ms.clamp(
            MIN_NETWORK_SPEED_UPDATE_INTERVAL_MS,
            MAX_NETWORK_SPEED_UPDATE_INTERVAL_MS,
        );
        self.save()
    }

    pub fn set_popup_size(&mut self, width: u32, height: u32) -> Result<()> {
        self.data.popup_width = width.clamp(280, 520);
        self.data.popup_height = height.clamp(240, 640);
        self.save()
    }

    pub fn set_network_speed_show_totals(&mut self, show: bool) -> Result<()> {
        self.data.network_speed_show_totals = show;
        self.save()
    }

    pub fn set_network_speed_show_interfaces(&mut self, show: bool) -> Result<()> {
        self.data.network_speed_show_interfaces = show;
        self.save()
    }

    pub fn set_network_speed_max_interfaces(&mut self, max_interfaces: u8) -> Result<()> {
        self.data.network_speed_max_interfaces = max_interfaces.clamp(0, 10);
        self.save()
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("cannot create tray settings directory {}", parent.display())
            })?;
        }
        let json =
            serde_json::to_string_pretty(&self.data).context("cannot encode tray settings")?;
        fs::write(&self.path, json)
            .with_context(|| format!("cannot write tray settings {}", self.path.display()))
    }
}

pub fn load_tray_settings(path: &Path) -> Result<TraySettings> {
    if !path.exists() {
        return Ok(TraySettings::default());
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("cannot read tray settings {}", path.display()))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(TraySettings::default());
    }
    let settings: TraySettings =
        serde_json::from_str(trimmed).context("invalid tray settings JSON")?;
    Ok(settings.sanitized())
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
            .join(format!("qingqi-tray-settings-{nanos}"))
            .join(name)
    }

    #[test]
    fn defaults_to_text_only_network_speed() {
        let settings = TraySettingsStore::new(temp_path("tray.json")).settings();
        assert!(settings.network_speed_visible);
        assert!(!settings.network_speed_show_icon);
        assert_eq!(
            settings.network_speed_display_mode,
            NetworkSpeedDisplayMode::TextOnly
        );
        assert_eq!(
            settings.network_speed_text_mode,
            NetworkSpeedTextMode::DownloadOnly
        );
        assert!(settings.network_speed_show_totals);
        assert!(settings.network_speed_show_interfaces);
        assert_eq!(settings.network_speed_max_interfaces, 5);
    }

    #[test]
    fn persists_tray_settings() {
        let path = temp_path("tray.json");
        {
            let mut store = TraySettingsStore::new(path.clone());
            store.set_network_speed_visible(false).expect("visible");
            store.set_network_speed_show_icon(true).expect("show icon");
        }
        let settings = TraySettingsStore::new(path.clone()).settings();
        assert!(!settings.network_speed_visible);
        assert!(settings.network_speed_show_icon);
        assert_eq!(
            settings.network_speed_display_mode,
            NetworkSpeedDisplayMode::IconAndText
        );
        let _ = std::fs::remove_dir_all(path.parent().expect("temp parent"));
    }

    #[test]
    fn sanitizes_numeric_settings() {
        let path = temp_path("tray.json");
        std::fs::create_dir_all(path.parent().expect("temp parent")).expect("create temp");
        std::fs::write(
            &path,
            r#"{"network_speed_visible":true,"network_speed_show_icon":false,"network_speed_update_interval_ms":10,"popup_width":9999,"popup_height":1,"network_speed_max_interfaces":200}"#,
        )
        .expect("write settings");

        let settings = load_tray_settings(&path).expect("settings");
        assert_eq!(settings.network_speed_update_interval_ms, 500);
        assert_eq!(settings.popup_width, 520);
        assert_eq!(settings.popup_height, 240);
        assert_eq!(settings.network_speed_max_interfaces, 10);
        let _ = std::fs::remove_dir_all(path.parent().expect("temp parent"));
    }
}
