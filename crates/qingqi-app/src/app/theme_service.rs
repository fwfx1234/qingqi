use std::{collections::HashMap, fs, path::PathBuf, sync::LazyLock};

use anyhow::Result;
use gpui::{App, WindowAppearance};
use qingqi_plugin::theme::ThemeMode;
use qingqi_ui::token::Token;

use qingqi_ui::theme_loader::{self, apply_custom_token};

static THEME_CACHE: LazyLock<std::sync::Mutex<HashMap<String, Token>>> =
    LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

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
            // also parse and cache
            let parsed = theme_loader::load_theme_file(content);
            if let Ok(mut cache) = THEME_CACHE.lock() {
                for (n, t) in parsed {
                    cache.insert(n, t);
                }
            }
        }
        Ok(())
    }

    pub fn init(&self, cx: &mut App) -> Result<()> {
        self.seed_builtin_themes()?;

        // load user themes from disk
        if let Ok(entries) = fs::read_dir(&self.themes_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        let parsed = theme_loader::load_theme_file(&content);
                        if let Ok(mut cache) = THEME_CACHE.lock() {
                            for (n, t) in parsed {
                                cache.insert(n, t);
                            }
                        }
                    }
                }
            }
        }

        let names = self.theme_names();
        tracing::info!(themes = ?names, "ThemeService initialized");
        Ok(())
    }

    pub fn theme_names(&self) -> Vec<String> {
        let names = if let Ok(cache) = THEME_CACHE.lock() {
            theme_loader::list_base_names(
                &cache
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<Vec<_>>(),
            )
        } else {
            Vec::new()
        };
        names
    }

    pub fn apply_theme(theme_name: &str, mode: ThemeMode, cx: &mut App) {
        let effective_dark = match mode {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::System => matches!(
                cx.window_appearance(),
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            ),
        };

        let variant_name = format!(
            "{} {}",
            theme_name,
            if effective_dark { "Dark" } else { "Light" }
        );

        let token = if let Ok(cache) = THEME_CACHE.lock() {
            cache
                .get(&variant_name)
                .or_else(|| cache.get(theme_name))
                .cloned()
        } else {
            None
        };

        if let Some(token) = token {
            apply_custom_token(token, cx);
        } else {
            // fallback to default color scheme
            qingqi_ui::token::install_tokens(cx, effective_dark);
        }
        cx.refresh_windows();
    }
}
