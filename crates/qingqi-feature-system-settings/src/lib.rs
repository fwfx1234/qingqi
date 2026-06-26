pub mod manifest;
pub mod plugin;
pub mod settings_store;
pub mod view;

use std::sync::{Arc, Mutex};

use qingqi_plugin::{
    database::DatabaseSpec,
    host::{AppIndexHandleRef, ShortcutHandleRef, ThemeHandleRef},
    plugin::Plugin,
    storage::AppPaths,
};

use crate::{plugin::SystemSettingsPlugin, settings_store::SettingsStore};

pub fn databases() -> Vec<DatabaseSpec> {
    Vec::new()
}

pub fn build(
    theme_handle: ThemeHandleRef,
    paths: AppPaths,
    app_index_handle: Option<AppIndexHandleRef>,
    shortcut_handle: Option<ShortcutHandleRef>,
) -> anyhow::Result<Box<dyn Plugin>> {
    let settings_store = Arc::new(Mutex::new(SettingsStore::new(
        paths.config("system_settings.json"),
    )));
    Ok(Box::new(SystemSettingsPlugin::new(
        theme_handle,
        paths,
        settings_store,
        app_index_handle,
        shortcut_handle,
    )))
}

pub fn build_tray_manager(
    theme_handle: ThemeHandleRef,
    paths: AppPaths,
    app_index_handle: Option<AppIndexHandleRef>,
    shortcut_handle: Option<ShortcutHandleRef>,
) -> anyhow::Result<Box<dyn Plugin>> {
    let settings_store = Arc::new(Mutex::new(SettingsStore::new(
        paths.config("system_settings.json"),
    )));
    Ok(Box::new(SystemSettingsPlugin::new_with_manifest(
        theme_handle,
        paths,
        settings_store,
        app_index_handle,
        shortcut_handle,
        crate::manifest::tray_manager_manifest(),
        "托盘管理".into(),
        2,
        true,
    )))
}
