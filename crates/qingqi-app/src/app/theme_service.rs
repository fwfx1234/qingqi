use std::{fs, path::PathBuf};

use anyhow::Result;
use gpui::{App, WindowAppearance};
use gpui_component::theme::{Theme, ThemeMode as GpuiThemeMode, ThemeRegistry};

pub struct ThemeService {
    themes_dir: PathBuf,
}

impl ThemeService {
    pub fn new(themes_dir: PathBuf) -> Self {
        Self { themes_dir }
    }

    fn seed_builtin_themes(&self) -> Result<()> {
        fs::create_dir_all(&self.themes_dir)?;
        let builtins: &[(&str, &str)] = &[
            ("adventure", include_str!("themes/adventure.json")),
            ("alduin", include_str!("themes/alduin.json")),
            ("asciinema", include_str!("themes/asciinema.json")),
            ("ayu", include_str!("themes/ayu.json")),
            ("catppuccin", include_str!("themes/catppuccin.json")),
            ("everforest", include_str!("themes/everforest.json")),
            ("fahrenheit", include_str!("themes/fahrenheit.json")),
            ("flexoki", include_str!("themes/flexoki.json")),
            ("gruvbox", include_str!("themes/gruvbox.json")),
            ("harper", include_str!("themes/harper.json")),
            ("hybrid", include_str!("themes/hybrid.json")),
            ("jellybeans", include_str!("themes/jellybeans.json")),
            ("kibble", include_str!("themes/kibble.json")),
            ("macos-classic", include_str!("themes/macos-classic.json")),
            ("matrix", include_str!("themes/matrix.json")),
            ("mellifluous", include_str!("themes/mellifluous.json")),
            ("molokai", include_str!("themes/molokai.json")),
            ("solarized", include_str!("themes/solarized.json")),
            ("spaceduck", include_str!("themes/spaceduck.json")),
            ("tokyonight", include_str!("themes/tokyonight.json")),
            ("twilight", include_str!("themes/twilight.json")),
        ];
        for (name, content) in builtins {
            let path = self.themes_dir.join(format!("{name}.json"));
            if !path.exists() {
                fs::write(&path, *content)?;
            }
        }
        Ok(())
    }

    pub fn init(&self, cx: &mut App) -> Result<()> {
        self.seed_builtin_themes()?;

        ThemeRegistry::watch_dir(self.themes_dir.clone(), cx, |_cx| {})?;

        // 打印已注册的所有主题名，确认 macOS Classic 在内
        let registry = ThemeRegistry::global(cx);
        let names: Vec<_> = registry
            .themes()
            .values()
            .map(|c| c.name.as_ref())
            .collect();
        tracing::info!(themes = ?names, "ThemeRegistry initialized");

        Ok(())
    }

    /// 获取所有可用主题名（去除 Light/Dark 后缀，去重排序）
    pub fn theme_names(cx: &App) -> Vec<String> {
        let registry = ThemeRegistry::global(cx);
        let mut names: Vec<String> = registry
            .themes()
            .values()
            .map(|c| {
                let n = c.name.as_ref();
                // "XXX Light" -> "XXX", "XXX Dark" -> "XXX"
                n.strip_suffix(" Light")
                    .or_else(|| n.strip_suffix(" Dark"))
                    .unwrap_or(n)
                    .to_string()
            })
            .collect();
        names.sort();
        names.dedup();
        names
    }

    pub fn apply_theme(theme_name: &str, mode: qingqi_plugin::theme::ThemeMode, cx: &mut App) {
        let effective_dark = match mode {
            qingqi_plugin::theme::ThemeMode::Light => false,
            qingqi_plugin::theme::ThemeMode::Dark => true,
            qingqi_plugin::theme::ThemeMode::System => {
                matches!(
                    cx.window_appearance(),
                    WindowAppearance::Dark | WindowAppearance::VibrantDark
                )
            }
        };

        let target_mode = if effective_dark {
            GpuiThemeMode::Dark
        } else {
            GpuiThemeMode::Light
        };

        let registry = ThemeRegistry::global(cx);
        let themes = registry.themes();

        // 精确匹配：theme_name + " Light" 或 theme_name + " Dark"
        let variant_name = format!(
            "{} {}",
            theme_name,
            if effective_dark { "Dark" } else { "Light" }
        );

        let config = themes
            .values()
            .find(|c| c.name.as_ref() == variant_name)
            .or_else(|| {
                // fallback: 精确匹配 theme_name（某些主题可能就叫一个词不带后缀）
                themes.values().find(|c| c.name.as_ref() == theme_name)
            })
            .cloned()
            .or_else(|| {
                if effective_dark {
                    Some(registry.default_dark_theme().clone())
                } else {
                    Some(registry.default_light_theme().clone())
                }
            });

        let theme = Theme::global_mut(cx);
        if let Some(ref config) = config {
            tracing::info!(
                theme_name = %theme_name,
                variant = %config.name.as_ref(),
                mode = ?mode,
                effective_dark = effective_dark,
                "apply_theme: matched config"
            );
            theme.apply_config(config);
        } else {
            tracing::warn!(
                theme_name = %theme_name,
                mode = ?mode,
                effective_dark = effective_dark,
                "apply_theme: no theme matched, using existing config"
            );
        }

        Theme::change(target_mode, None, cx);
        cx.refresh_windows();
    }
}
