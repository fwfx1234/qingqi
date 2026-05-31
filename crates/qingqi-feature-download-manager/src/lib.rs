pub mod manifest;
pub mod model;
pub mod plugin;
pub mod service;
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
        "download-manager",
        "tasks",
        "tasks.db",
    )]
}

pub fn build(database: Arc<DatabaseService>, paths: AppPaths) -> anyhow::Result<Box<dyn Plugin>> {
    Ok(Box::new(plugin::DownloadManagerPlugin::new(
        database, paths,
    )?))
}
