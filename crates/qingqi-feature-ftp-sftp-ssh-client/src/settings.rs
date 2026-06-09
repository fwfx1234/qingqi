use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const DEFAULT_FONT_FAMILY: &str = "SF Mono";
const DEFAULT_FONT_SIZE: f32 = 13.0;
const DEFAULT_LINE_HEIGHT: f32 = 18.0;
const DEFAULT_SCROLLBACK_LINES: usize = 2500;
const DEFAULT_CURSOR_STYLE: CursorStyle = CursorStyle::Block;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CursorStyle {
    Block,
    Beam,
    Underline,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TerminalTheme {
    OneLight,
    OneDark,
    SolarizedLight,
    SolarizedDark,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerminalSettings {
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
    pub scrollback_lines: usize,
    pub cursor_style: CursorStyle,
    pub theme: TerminalTheme,
    pub blink_cursor: bool,
    pub word_separators: String,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            font_family: DEFAULT_FONT_FAMILY.to_string(),
            font_size: DEFAULT_FONT_SIZE,
            line_height: DEFAULT_LINE_HEIGHT,
            scrollback_lines: DEFAULT_SCROLLBACK_LINES,
            cursor_style: DEFAULT_CURSOR_STYLE,
            theme: TerminalTheme::OneLight,
            blink_cursor: false,
            word_separators: String::from(" (),<>[]{}'\"`"),
        }
    }
}

impl TerminalSettings {
    pub fn clamp(&mut self) {
        self.font_size = self.font_size.clamp(8.0, 32.0);
        self.line_height = self.line_height.clamp(12.0, 48.0);
        self.scrollback_lines = self.scrollback_lines.clamp(100, 100_000);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PluginSettings {
    pub terminal: TerminalSettings,
}

pub struct SettingsStore {
    path: PathBuf,
    data: PluginSettings,
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        let data = Self::load(&path).unwrap_or_else(|error| {
            tracing::warn!(
                path = %path.display(),
                error = %error,
                "failed to load SSH plugin settings, using defaults"
            );
            PluginSettings::default()
        });
        Self { path, data }
    }

    pub fn get(&self) -> &PluginSettings {
        &self.data
    }

    pub fn get_mut(&mut self) -> &mut PluginSettings {
        &mut self.data
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("cannot create settings directory {}", parent.display())
            })?;
        }
        let json = serde_json::to_string_pretty(&self.data).context("cannot encode settings")?;
        std::fs::write(&self.path, json)
            .with_context(|| format!("cannot write settings {}", self.path.display()))
    }

    fn load(path: &Path) -> Result<PluginSettings> {
        if !path.exists() {
            return Ok(PluginSettings::default());
        }
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read settings {}", path.display()))?;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(PluginSettings::default());
        }
        serde_json::from_str(trimmed).context("invalid settings JSON")
    }
}
