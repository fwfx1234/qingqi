pub mod custom_dir;
pub mod manifest;
pub mod platform;
pub mod plugin;
pub mod protocol;
pub mod qrcode_gen;
pub mod server;
pub mod service;
pub mod steam;
pub mod view;

use std::sync::Arc;

use qingqi_plugin::{database::{DatabaseService, DatabaseSpec}, plugin::Plugin, storage::AppPaths};

pub use manifest::PLUGIN_ID;

pub fn databases() -> Vec<DatabaseSpec> {
    vec![DatabaseSpec::feature(
        PLUGIN_ID,
        "data",
        "remote-control.db",
    )]
}

pub fn build(
    database: Arc<DatabaseService>,
    paths: AppPaths,
) -> anyhow::Result<Box<dyn Plugin>> {
    Ok(Box::new(plugin::RemoteControlPlugin::new(database, paths)?))
}
