pub mod data_source;
pub mod manifest;
pub mod model;
pub mod plugin;
pub mod script_service;
pub mod service;
pub mod store;
pub mod variable_service;
pub mod view;

use std::sync::Arc;

use qingqi_plugin::{
    database::{DatabaseService, DatabaseSpec},
    plugin::Plugin,
    storage::AppPaths,
};

pub fn databases() -> Vec<DatabaseSpec> {
    vec![DatabaseSpec::app("api_debugger/main", "api_debugger.db")]
}

pub fn build(database: Arc<DatabaseService>, paths: AppPaths) -> anyhow::Result<Box<dyn Plugin>> {
    Ok(Box::new(plugin::ApiDebuggerPlugin::new(database, paths)))
}
