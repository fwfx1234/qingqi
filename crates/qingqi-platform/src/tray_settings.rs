use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraySettings {
    pub network_speed_visible: bool,
    pub network_speed_show_icon: bool,
}

impl Default for TraySettings {
    fn default() -> Self {
        Self {
            network_speed_visible: true,
            network_speed_show_icon: false,
        }
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
    serde_json::from_str(trimmed).context("invalid tray settings JSON")
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
        let _ = std::fs::remove_dir_all(path.parent().expect("temp parent"));
    }
}
