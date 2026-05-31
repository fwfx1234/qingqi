pub mod manifest;
pub mod model;
pub mod plugin;
pub mod store;
pub mod view;

use std::sync::Arc;

use qingqi_plugin::{
    database::{DatabaseService, DatabaseSpec},
    plugin::Plugin,
    storage::AppPaths,
};

pub fn databases() -> Vec<DatabaseSpec> {
    vec![DatabaseSpec::feature(
        "http-capture",
        "capture",
        "capture.db",
    )]
}

pub fn build(database: Arc<DatabaseService>, paths: AppPaths) -> anyhow::Result<Box<dyn Plugin>> {
    Ok(Box::new(plugin::HttpCapturePlugin::new(database, paths)?))
}
