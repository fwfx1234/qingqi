//! SSH 远程管理插件 — 库入口

pub mod manifest;
pub mod model;
pub mod plugin;
pub mod store;
pub mod service;
pub mod connection;
pub mod protocol;
pub mod terminal;
pub mod transfer;
pub mod view;

use std::sync::Arc;

use anyhow::Result;
use qingqi_plugin::database::{DatabaseService, DatabaseSpec};

pub fn databases() -> Vec<DatabaseSpec> {
    vec![DatabaseSpec::feature("ssh", "profiles", "ssh_profiles.db")]
}

pub fn build(
    database: Arc<DatabaseService>,
    paths: Arc<qingqi_plugin::storage::AppPaths>,
) -> Result<Box<dyn qingqi_plugin::plugin::Plugin>> {
    let profile_db_path = database.path_for_key("ssh/profiles")?;
    let profile_store = Arc::new(store::ProfileStore::new(
        Arc::clone(&database),
        profile_db_path,
    ));
    profile_store.init()?;
    let migrated = profile_store.migrate_from_v2().unwrap_or(0);
    if migrated > 0 {
        tracing::info!("从旧版迁移了 {migrated} 个 Profile");
    }

    let service = Arc::new(service::SshService::new(
        Arc::clone(&database),
        profile_store,
        paths.feature_dir("ssh"),
    ));
    Ok(Box::new(plugin::SshPlugin { service }))
}
