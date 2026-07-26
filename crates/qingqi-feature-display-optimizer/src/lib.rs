pub mod manifest;
pub mod model;
pub mod plugin;
pub mod service;
pub mod store;
pub mod view;

use std::sync::Arc;

use qingqi_plugin::{database::DatabaseSpec, plugin::Plugin, storage::AppPaths};

pub fn databases() -> Vec<DatabaseSpec> {
    Vec::new()
}

pub fn build(paths: AppPaths) -> anyhow::Result<Box<dyn Plugin>> {
    let service = Arc::new(service::DisplayOptimizerService::new(
        paths.feature_dir(manifest::PLUGIN_ID),
    ));
    Ok(Box::new(plugin::DisplayOptimizerPlugin::new(service)))
}
