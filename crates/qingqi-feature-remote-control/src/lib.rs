pub mod custom_dir;
pub mod manifest;
pub mod platform;
pub mod plugin;
pub mod protocol;
pub mod server;
pub mod service;
pub mod steam;
pub mod view;

use qingqi_plugin::{database::DatabaseSpec, plugin::Plugin, storage::AppPaths};

pub fn databases() -> Vec<DatabaseSpec> {
    vec![DatabaseSpec::feature(
        "remote-control",
        "tokens",
        "tokens.db",
    )]
}

pub fn build(
    paths: AppPaths,
) -> anyhow::Result<Box<dyn Plugin>> {
    Ok(Box::new(plugin::RemoteControlPlugin::new(paths)?))
}
