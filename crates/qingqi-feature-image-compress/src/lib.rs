pub mod manifest;
pub mod plugin;
pub mod service;
pub mod view;

use qingqi_plugin::{database::DatabaseSpec, plugin::Plugin, storage::AppPaths};

pub fn databases() -> Vec<DatabaseSpec> {
    Vec::new()
}

pub fn build(paths: AppPaths) -> anyhow::Result<Box<dyn Plugin>> {
    Ok(Box::new(plugin::runtime(paths)))
}
