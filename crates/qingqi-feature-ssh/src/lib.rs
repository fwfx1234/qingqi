//! SSH 远程管理插件 — 库入口

mod log_util;
pub mod connection;
pub mod manifest;
pub mod model;
pub mod plugin;
pub mod protocol;
pub mod service;
pub mod store;
pub mod terminal;
pub mod transfer;
pub mod view;

use std::sync::{Arc, OnceLock};

use anyhow::Result;
use qingqi_plugin::database::{DatabaseService, DatabaseSpec};

/// 全局 tokio runtime 句柄，由 main.rs 初始化
static TOKIO_RT: OnceLock<tokio::runtime::Handle> = OnceLock::new();

/// 由 main.rs 在启动时调用，存储 tokio runtime 句柄供 service 层使用
pub fn init_tokio_runtime(handle: tokio::runtime::Handle) {
    let _ = TOKIO_RT.set(handle);
}

/// 获取全局 tokio runtime 句柄
pub(crate) fn tokio_handle() -> &'static tokio::runtime::Handle {
    TOKIO_RT
        .get()
        .expect("tokio runtime 未初始化，main.rs 需要先调用 init_tokio_runtime")
}

pub fn databases() -> Vec<DatabaseSpec> {
    vec![DatabaseSpec::feature("ssh", "profiles", "ssh_profiles.db")]
}

pub fn build(
    database: Arc<DatabaseService>,
    paths: qingqi_plugin::storage::AppPaths,
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
