pub mod manifest;
pub mod model;
pub mod parameters;
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
        "quick-launch",
        "actions",
        "actions.db",
    )]
}

pub fn build(database: Arc<DatabaseService>, paths: AppPaths) -> anyhow::Result<Box<dyn Plugin>> {
    Ok(Box::new(plugin::QuickLaunchPlugin::new(database, paths)?))
}
