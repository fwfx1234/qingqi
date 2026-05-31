pub mod manifest;
pub mod plugin;
pub mod service;
pub mod view;

use qingqi_plugin::{database::DatabaseSpec, plugin::Plugin};

pub fn databases() -> Vec<DatabaseSpec> {
    Vec::new()
}

pub fn build() -> anyhow::Result<Box<dyn Plugin>> {
    Ok(Box::new(plugin::runtime()))
}
