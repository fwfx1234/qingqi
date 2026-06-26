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
    initial_section: usize,
    tray_manager_mode: bool,
    manifest: Manifest,
    title: Arc<str>,
}

impl SystemSettingsPlugin {
    pub fn new(
        theme_handle: ThemeHandleRef,
        app_paths: AppPaths,
        settings_store: Arc<Mutex<SettingsStore>>,
        app_index_handle: Option<AppIndexHandleRef>,
        shortcut_handle: Option<ShortcutHandleRef>,
    ) -> Self {
        Self::new_with_manifest(
            theme_handle,
            app_paths,
            settings_store,
            app_index_handle,
            shortcut_handle,
            crate::manifest::manifest(),
            "系统设置".into(),
            0,
            false,
        )
    }

    pub fn new_with_manifest(
        theme_handle: ThemeHandleRef,
        app_paths: AppPaths,
        settings_store: Arc<Mutex<SettingsStore>>,
        app_index_handle: Option<AppIndexHandleRef>,
        shortcut_handle: Option<ShortcutHandleRef>,
        manifest: Manifest,
        title: Arc<str>,
        initial_section: usize,
        tray_manager_mode: bool,
    ) -> Self {
        Self {
            theme_handle,
            settings_store,
            app_index_handle,
            shortcut_handle,
            app_paths,
            initial_section,
            tray_manager_mode,
            manifest,
            title,
        }
    }
}

impl Plugin for SystemSettingsPlugin {
    fn manifest(&self) -> Manifest {
        self.manifest.clone()
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        let theme_handle = Arc::clone(&self.theme_handle);
        let settings_store = Arc::clone(&self.settings_store);
        let app_index_handle = self.app_index_handle.clone();
        let shortcut_handle = self.shortcut_handle.clone();
        let app_paths = self.app_paths.clone();
        let initial_section = self.initial_section;
        let tray_manager_mode = self.tray_manager_mode;
        let title = Arc::clone(&self.title);

        let panel = cx.app.new(|_cx| {
            if tray_manager_mode {
                return SettingsView::tray_manager(
                    theme_handle,
                    settings_store,
                    app_index_handle,
                    shortcut_handle,
                    app_paths,
                );
            }
            SettingsView::new_with_initial_section(
                theme_handle,
                settings_store,
                app_index_handle,
                shortcut_handle,
                app_paths,
                initial_section,
            )
        });
        Ok(PluginView::Inline(Box::new(SystemSettingsView {
            panel,
            plugin_id: self.manifest.id.clone(),
            title,
        })))
    }

    fn close_idle(&mut self) {}
}

pub struct SystemSettingsView {
    panel: Entity<SettingsView>,
    plugin_id: PluginId,
    title: Arc<str>,
}

impl InlineView for SystemSettingsView {
    fn plugin_id(&self) -> PluginId {
        self.plugin_id.clone()
    }

    fn title(&self) -> Arc<str> {
        Arc::clone(&self.title)
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        self.panel.clone().into_any_element()
    }

    fn on_close(&mut self) {}
}
