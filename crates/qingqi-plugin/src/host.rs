use std::sync::Arc;

use anyhow::Result;
use gpui::App;

use crate::{app::AppIndexSnapshot, shortcut::ShortcutView, theme::ThemeMode};

pub trait ThemeHandle {
    fn mode(&self) -> ThemeMode;
    fn config_path(&self) -> String;
    fn system_dark(&self) -> bool;
    fn set_mode(&self, mode: ThemeMode) -> Result<()>;
}

pub trait AppIndexHandle {
    fn snapshot(&self) -> AppIndexSnapshot;
    fn request_scan(&self) -> bool;
}

pub trait ShortcutHandle {
    fn views(&self) -> Vec<ShortcutView>;
    fn set_shortcut(
        &self,
        shortcut_id: &str,
        accelerator: &str,
        enabled: bool,
        cx: &mut App,
    ) -> Result<()>;
    fn restore_shortcut(&self, shortcut_id: &str, cx: &mut App) -> Result<()>;
}

pub type ThemeHandleRef = Arc<dyn ThemeHandle>;
pub type AppIndexHandleRef = Arc<dyn AppIndexHandle>;
pub type ShortcutHandleRef = Arc<dyn ShortcutHandle>;
