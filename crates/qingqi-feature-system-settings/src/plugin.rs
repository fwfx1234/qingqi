use std::sync::{Arc, Mutex};

use gpui::{AnyElement, App, AppContext, Entity, IntoElement, Window};

use crate::{settings_store::SettingsStore, view::SettingsView};
use qingqi_plugin::{
    host::{AppIndexHandleRef, ShortcutHandleRef, ThemeHandleRef},
    plugin::{InlineView, Manifest, Plugin, PluginCx, PluginId, PluginView},
    storage::AppPaths,
};

pub struct SystemSettingsPlugin {
    theme_handle: ThemeHandleRef,
    settings_store: Arc<Mutex<SettingsStore>>,
    app_index_handle: Option<AppIndexHandleRef>,
    shortcut_handle: Option<ShortcutHandleRef>,
    app_paths: AppPaths,
}

impl SystemSettingsPlugin {
    pub fn new(
        theme_handle: ThemeHandleRef,
        app_paths: AppPaths,
        settings_store: Arc<Mutex<SettingsStore>>,
        app_index_handle: Option<AppIndexHandleRef>,
        shortcut_handle: Option<ShortcutHandleRef>,
    ) -> Self {
        Self {
            theme_handle,
            settings_store,
            app_index_handle,
            shortcut_handle,
            app_paths,
        }
    }
}

impl Plugin for SystemSettingsPlugin {
    fn manifest(&self) -> Manifest {
        crate::manifest::manifest()
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        let theme_handle = Arc::clone(&self.theme_handle);
        let settings_store = Arc::clone(&self.settings_store);
        let app_index_handle = self.app_index_handle.clone();
        let shortcut_handle = self.shortcut_handle.clone();
        let app_paths = self.app_paths.clone();

        let panel = cx.app.new(|_cx| {
            SettingsView::new(
                theme_handle,
                settings_store,
                app_index_handle,
                shortcut_handle,
                app_paths,
            )
        });
        Ok(PluginView::Inline(Box::new(SystemSettingsView { panel })))
    }

    fn close_idle(&mut self) {}
}

pub struct SystemSettingsView {
    panel: Entity<SettingsView>,
}

impl InlineView for SystemSettingsView {
    fn plugin_id(&self) -> PluginId {
        crate::manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "系统设置".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        self.panel.clone().into_any_element()
    }

    fn on_close(&mut self) {}
}
