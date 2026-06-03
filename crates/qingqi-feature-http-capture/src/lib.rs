pub mod certificate;
pub mod engine;
pub mod manifest;
pub mod mock_engine;
pub mod mock_model;
pub mod mock_store;
pub mod model;
pub mod plugin;
pub mod proxy_handler;
pub mod store;
pub mod view;

use std::sync::Arc;

use qingqi_plugin::{
    database::{DatabaseService, DatabaseSpec},
    events::AppEventBus,
    plugin::Plugin,
    storage::AppPaths,
};

pub fn databases() -> Vec<DatabaseSpec> {
    vec![
        DatabaseSpec::feature("http-capture", "capture", "capture.db"),
        DatabaseSpec::feature("http-capture", "mock", "mock.db"),
    ]
}

pub fn build(
    database: Arc<DatabaseService>,
    paths: AppPaths,
    events: AppEventBus,
) -> anyhow::Result<Box<dyn Plugin>> {
    Ok(Box::new(plugin::HttpCapturePlugin::new(
        database, paths, events,
    )?))
}
