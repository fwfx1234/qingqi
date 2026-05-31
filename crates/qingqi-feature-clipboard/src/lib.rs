pub mod data_source;
pub mod history_store;
pub mod manifest;
pub mod plugin;
pub mod service;
pub mod view;

use std::sync::{Arc, Mutex};

use qingqi_plugin::{
    clipboard::ClipboardContext,
    database::{DatabaseService, DatabaseSpec},
    plugin::Plugin,
    storage::AppPaths,
};

struct ClipboardContextAdapter {
    service: Arc<Mutex<service::ClipboardService>>,
}

impl ClipboardContext for ClipboardContextAdapter {
    fn latest_payload(&self) -> Option<qingqi_plugin::command::ClipboardPayload> {
        self.service
            .lock()
            .ok()
            .and_then(|service| service.latest_payload().ok().flatten())
    }
}

pub fn databases() -> Vec<DatabaseSpec> {
    Vec::new()
}

pub fn build_shared(
    database: Arc<DatabaseService>,
    paths: AppPaths,
) -> anyhow::Result<(Box<dyn Plugin>, Arc<dyn ClipboardContext>)> {
    let clipboard_db_path = paths.data_dir().join("clipboard.db");
    database.register_database(DatabaseSpec::path(
        "clipboard/history",
        clipboard_db_path.clone(),
    ))?;
    let service = Arc::new(Mutex::new(service::ClipboardService::new(
        database,
        clipboard_db_path,
    )));
    let plugin = Box::new(plugin::ClipboardPlugin::from_shared(Arc::clone(&service)));
    let context: Arc<dyn ClipboardContext> = Arc::new(ClipboardContextAdapter { service });
    Ok((plugin, context))
}
