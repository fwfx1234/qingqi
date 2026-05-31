pub mod plugin;

use qingqi_plugin::{database::DatabaseSpec, plugin::Plugin};

pub fn databases() -> Vec<DatabaseSpec> {
    Vec::new()
}

pub fn build() -> anyhow::Result<Box<dyn Plugin>> {
    Ok(Box::new(plugin::GpuiDemoPlugin::new()))
}
