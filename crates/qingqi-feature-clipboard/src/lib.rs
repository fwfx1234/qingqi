pub mod data_source;
pub mod history_store;
pub mod manifest;
pub mod plugin;
pub mod service;
pub mod view;

use std::sync::{Arc, Mutex};

use qingqi_plugin::{
    database::{DatabaseService, DatabaseSpec},
    host::ShortcutHandleRef,
    plugin::Plugin,
};

pub fn databases() -> Vec<DatabaseSpec> {
    vec![DatabaseSpec::feature(
        "clipboard",
        "history",
        "clipboard.db",
    )]
}

pub fn build_shared(
    database: Arc<DatabaseService>,
    shortcut_handle: Option<ShortcutHandleRef>,
) -> anyhow::Result<Box<dyn Plugin>> {
    let clipboard_db_path = database.path_for_key("clipboard/history")?;
    let service = Arc::new(Mutex::new(service::ClipboardService::new(
        Arc::clone(&database),
        clipboard_db_path,
    )));
    let plugin = Box::new(
        plugin::ClipboardPlugin::from_shared(Arc::clone(&service))
            .with_shortcut_handle(shortcut_handle),
    );
    Ok(plugin)
}
