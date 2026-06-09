pub mod manifest;
pub mod model;
pub mod plugin;
pub mod protocols;
pub mod runtime;
pub mod settings;
pub mod store;
pub mod terminal;
pub mod transfer;
pub mod view;

use std::sync::Arc;

use qingqi_plugin::{
    database::{DatabaseService, DatabaseSpec},
    plugin::Plugin,
    storage::AppPaths,
};

pub fn databases() -> Vec<DatabaseSpec> {
    vec![DatabaseSpec::feature(
        manifest::PLUGIN_ID,
        "profiles",
        "profiles.db",
    )]
}

pub fn build(database: Arc<DatabaseService>, paths: AppPaths) -> anyhow::Result<Box<dyn Plugin>> {
    Ok(Box::new(plugin::FtpSftpSshPlugin::new(database, paths)?))
}
