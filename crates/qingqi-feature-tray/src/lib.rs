pub mod manifest;
pub mod model;
pub mod plugin;
pub mod service;
pub mod settings;
pub mod settings_view;
pub mod view;

use std::sync::Arc;

use qingqi_plugin::{
    database::{DatabaseService, DatabaseSpec},
    plugin::Plugin,
};

use crate::plugin::TrayPlugin;

pub fn databases() -> Vec<DatabaseSpec> {
    vec![DatabaseSpec::app("plugin-dict", "plugin-dict.db")]
}

pub fn build(database: Arc<DatabaseService>) -> anyhow::Result<Box<dyn Plugin>> {
    Ok(Box::new(TrayPlugin::new(database)))
}
