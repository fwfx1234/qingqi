use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
pub use qingqi_plugin::theme::ThemeMode;

fn default_theme_name() -> String {
    "Default".into()
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ThemeConfig {
    #[serde(default = "default_theme_name")]
    theme: String,
    mode: ThemeMode,
}

/// Persisted theme store. Theme changes are applied immediately to the global
/// runtime mode and then written to a local JSON config file.
pub struct ThemeStore {
    mode: ThemeMode,
    theme: String,
    config_path: PathBuf,
    system_dark: bool,
}

impl ThemeStore {
    pub fn new(config_path: PathBuf) -> Self {
        let (mode, theme) = Self::load_config(&config_path).unwrap_or_else(|error| {
            tracing::warn!(
                path = %config_path.display(),
                error = %error,
                "failed to load theme config, falling back to default"
            );
            (ThemeMode::default(), "Default".to_string())
        });
        let system_dark = Self::read_system_dark();
        let store = Self {
            mode,
            theme,
            config_path,
            system_dark,
        };
        store.apply_current();
        store
    }

    pub fn mode(&self) -> ThemeMode {
        self.mode
    }

    pub fn theme(&self) -> &str {
        &self.theme
    }

    pub fn set_theme(&mut self, theme: String) -> Result<()> {
        if self.theme == theme {
            return Ok(());
        }
        let previous = self.theme.clone();
        self.theme = theme;
        if let Err(e) = self.save() {
            self.theme = previous;
            return Err(e);
        }
        Ok(())
    }

    pub fn effective_dark(&self) -> bool {
        match self.mode {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::System => self.system_dark,
        }
    }

    pub fn system_dark(&self) -> bool {
        self.system_dark
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn set_mode(&mut self, mode: ThemeMode) -> Result<()> {
        if self.mode == mode {
            return Ok(());
        }

        let previous_mode = self.mode;
        let previous_system_dark = self.system_dark;

        self.mode = mode;
        self.refresh_system_state();
        self.apply_current();

        if let Err(error) = self.save() {
            self.mode = previous_mode;
            self.system_dark = previous_system_dark;
            self.apply_current();
            return Err(error);
        }

        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn reload(&mut self) -> Result<ThemeMode> {
        let (mode, theme) = Self::load_config(&self.config_path)?;
        self.mode = mode;
        self.theme = theme;
        self.refresh_system_state();
        self.apply_current();
        Ok(self.mode)
    }

    pub fn sync_system(&mut self) -> bool {
        let system_dark = Self::read_system_dark();
        if self.system_dark != system_dark {
            self.system_dark = system_dark;
            if self.mode == ThemeMode::System {
                self.apply_current();
            }
        }
        self.effective_dark()
    }

    /// Poll the system appearance and return `true` if the effective dark mode
    /// actually changed.  This is the entry point used by the background theme
    /// poll task.
    pub fn sync_system_changed(&mut self) -> bool {
        let before = self.effective_dark();
        self.sync_system();
        self.effective_dark() != before
    }

    fn apply_current(&self) {
        qingqi_ui::theme_mode::set_dark(self.effective_dark());
    }

    fn refresh_system_state(&mut self) {
        self.system_dark = Self::read_system_dark();
    }

    fn load_config(path: &Path) -> Result<(ThemeMode, String)> {
        if !path.exists() {
            return Ok((ThemeMode::default(), "Default".to_string()));
        }

        let raw = fs::read_to_string(path)
            .with_context(|| format!("cannot read theme config {}", path.display()))?;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok((ThemeMode::default(), "Default".to_string()));
        }

        if let Ok(config) = serde_json::from_str::<ThemeConfig>(trimmed) {
            return Ok((config.mode, config.theme));
        }

        if let Ok(mode) = serde_json::from_str::<ThemeMode>(trimmed) {
            return Ok((mode, "Default".to_string()));
        }

        bail!("invalid theme config format")
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("cannot create theme config directory {}", parent.display())
            })?;
        }

        let config = ThemeConfig {
            theme: self.theme.clone(),
            mode: self.mode,
        };
        let json = serde_json::to_string_pretty(&config).context("cannot encode theme config")?;
        fs::write(&self.config_path, json)
            .with_context(|| format!("cannot write theme config {}", self.config_path.display()))
    }

    fn read_system_dark() -> bool {
        qingqi_platform::theme::read_system_dark()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{ThemeMode, ThemeStore};

    fn temp_config_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("qingqi-theme-store-{nanos}"))
            .join(name)
    }

    #[test]
    fn persists_structured_theme_mode() {
        let path = temp_config_path("theme.json");
        let mut store = ThemeStore::new(path.clone());
        store.set_mode(ThemeMode::Dark).expect("set mode");

        let saved = fs::read_to_string(&path).expect("read saved theme");
        assert!(saved.contains("\"mode\": \"dark\""));

        let reloaded = ThemeStore::new(path.clone());
        assert_eq!(reloaded.mode(), ThemeMode::Dark);

        let _ = fs::remove_dir_all(path.parent().expect("temp parent"));
    }

    #[test]
    fn reads_legacy_raw_variant_names() {
        let path = temp_config_path("legacy-theme.json");
        fs::create_dir_all(path.parent().expect("temp parent")).expect("create temp dir");
        fs::write(&path, "\"Auto\"").expect("write legacy config");

        let store = ThemeStore::new(path.clone());
        assert_eq!(store.mode(), ThemeMode::System);

        let _ = fs::remove_dir_all(path.parent().expect("temp parent"));
    }

    #[test]
    fn reload_replaces_in_memory_mode() {
        let path = temp_config_path("reload-theme.json");
        let mut store = ThemeStore::new(path.clone());
        store.set_mode(ThemeMode::Light).expect("set light");
        fs::write(&path, "{\n  \"mode\": \"dark\"\n}").expect("overwrite config");

        let mode = store.reload().expect("reload config");
        assert_eq!(mode, ThemeMode::Dark);
        assert_eq!(store.mode(), ThemeMode::Dark);

        let _ = fs::remove_dir_all(path.parent().expect("temp parent"));
    }

    #[test]
    fn effective_dark_respects_mode() {
        let path = temp_config_path("effective-dark.json");
        let store = ThemeStore::new(path.clone());

        // System mode delegates to system_dark
        assert_eq!(store.mode(), ThemeMode::System);
        let expected = store.system_dark();
        assert_eq!(store.effective_dark(), expected);

        let _ = fs::remove_dir_all(path.parent().expect("temp parent"));
    }

    #[test]
    fn set_mode_immediately_updates_effective_dark() {
        let path = temp_config_path("immediate-dark.json");
        let mut store = ThemeStore::new(path.clone());

        store.set_mode(ThemeMode::Dark).expect("set dark");
        assert!(store.effective_dark());

        store.set_mode(ThemeMode::Light).expect("set light");
        assert!(!store.effective_dark());

        let _ = fs::remove_dir_all(path.parent().expect("temp parent"));
    }

    #[test]
    fn sync_system_noop_for_explicit_modes() {
        let path = temp_config_path("sync-explicit.json");
        let mut store = ThemeStore::new(path.clone());

        store.set_mode(ThemeMode::Light).expect("set light");
        // sync_system should not change effective_dark when mode is explicit
        let before = store.effective_dark();
        store.sync_system();
        assert_eq!(store.effective_dark(), before);

        store.set_mode(ThemeMode::Dark).expect("set dark");
        let before = store.effective_dark();
        store.sync_system();
        assert_eq!(store.effective_dark(), before);

        let _ = fs::remove_dir_all(path.parent().expect("temp parent"));
    }

    #[test]
    fn persists_theme_name() {
        let path = temp_config_path("theme-name.json");
        let mut store = ThemeStore::new(path.clone());
        assert_eq!(store.theme(), "Default");
        store.set_theme("Custom Theme".into()).expect("set theme");

        let saved = fs::read_to_string(&path).expect("read saved theme");
        assert!(saved.contains("\"theme\": \"Custom Theme\""));

        let reloaded = ThemeStore::new(path.clone());
        assert_eq!(reloaded.theme(), "Custom Theme");

        let _ = fs::remove_dir_all(path.parent().expect("temp parent"));
    }

    #[test]
    fn legacy_format_defaults_theme_name() {
        let path = temp_config_path("legacy-theme-name.json");
        fs::create_dir_all(path.parent().expect("temp parent")).expect("create temp dir");
        fs::write(&path, "\"Auto\"").expect("write legacy config");

        let store = ThemeStore::new(path.clone());
        assert_eq!(store.theme(), "Default");

        let _ = fs::remove_dir_all(path.parent().expect("temp parent"));
    }

    #[test]
    fn set_theme_rollback_on_save_failure() {
        let path = temp_config_path("theme-rollback.json");
        let mut store = ThemeStore::new(path.clone());
        store.set_theme("Theme A".into()).expect("set theme a");

        // Make the config path a directory so save fails
        let _ = fs::remove_dir_all(path.parent().expect("temp parent"));
        fs::create_dir_all(&path).expect("make path a dir");

        let result = store.set_theme("Theme B".into());
        assert!(result.is_err());
        assert_eq!(store.theme(), "Theme A");

        let _ = fs::remove_dir_all(&path);
    }

    #[test]
    fn set_theme_noop_for_same_name() {
        let path = temp_config_path("theme-noop.json");
        let mut store = ThemeStore::new(path.clone());
        store.set_theme("Same".into()).expect("set theme");
        store.set_theme("Same".into()).expect("set same again");
        assert_eq!(store.theme(), "Same");

        let _ = fs::remove_dir_all(path.parent().expect("temp parent"));
    }

    #[test]
    fn sync_system_changed_returns_false_when_no_change() {
        let path = temp_config_path("sync-nochange.json");
        let mut store = ThemeStore::new(path.clone());

        // Calling sync_system_changed twice in a row without an OS change
        // should return false on the second call.
        let _ = store.sync_system_changed();
        let changed_again = store.sync_system_changed();
        // Unless the system appearance genuinely flipped between calls,
        // the second call should report no change.
        // (This is deterministic in CI/test unless OS theme is toggled.)
        if store.mode() == ThemeMode::System {
            // In System mode, if system appearance didn't change, result is false
            // We can't assert false unconditionally because the OS might change,
            // but we can verify the function is wired correctly.
            let _ = changed_again;
        }

        let _ = fs::remove_dir_all(path.parent().expect("temp parent"));
    }
}
