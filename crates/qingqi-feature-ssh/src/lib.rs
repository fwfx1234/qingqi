//! SSH 远程管理插件 — 库入口

pub mod connection;
pub mod download;
mod log_util;
pub mod manifest;
mod mappings;
pub mod model;
pub mod plugin;
pub mod protocol;
pub mod service;
pub mod shell_cwd;
pub mod store;
pub mod terminal;
pub mod transfer;
pub mod upload;
pub mod view;

use std::sync::{Arc, OnceLock};

use anyhow::Result;
use qingqi_plugin::database::{DatabaseService, DatabaseSpec};

/// 插件私有 tokio 运行时：在后台线程保持存活，不干扰 GPUI 主线程事件循环。
struct TokioRuntime {
    handle: tokio::runtime::Handle,
    _thread: std::thread::JoinHandle<()>,
}

static TOKIO_RT: OnceLock<TokioRuntime> = OnceLock::new();

fn ensure_tokio_runtime() -> &'static tokio::runtime::Handle {
    &TOKIO_RT
        .get_or_init(|| {
            let rt = tokio::runtime::Runtime::new().expect("创建 tokio 运行时失败");
            let handle = rt.handle().clone();
            let thread = std::thread::spawn(move || {
                rt.block_on(std::future::pending::<()>());
            });
            TokioRuntime {
                handle,
                _thread: thread,
            }
        })
        .handle
}

/// 获取插件 tokio runtime 句柄
pub(crate) fn tokio_handle() -> &'static tokio::runtime::Handle {
    ensure_tokio_runtime()
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
