pub mod manifest;
pub mod plugin;

use qingqi_plugin::{database::DatabaseSpec, plugin::Plugin, storage::AppPaths};

pub fn databases() -> Vec<DatabaseSpec> {
    Vec::new()
}

pub fn build(paths: AppPaths) -> anyhow::Result<Box<dyn Plugin>> {
    Ok(Box::new(plugin::AntiPeepingPlugin::new(paths)))
}
