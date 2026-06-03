pub mod app;
pub mod command;
pub mod database;
pub mod dict_store;
pub mod events;
pub mod host;
pub mod icon;
pub mod job;
pub mod log;
pub mod page;
pub mod plugin;
pub mod plugin_spec;
pub mod shortcut;
pub mod storage;
pub mod theme;

pub use log::log_and_return;

/// Lock a std::sync::Mutex with poison recovery.
pub fn lock_or_recover<'a, T>(
    mutex: &'a std::sync::Mutex<T>,
    name: &str,
) -> std::sync::MutexGuard<'a, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poison) => {
            tracing::error!(target = name, "mutex poisoned, attempting recovery");
            poison.into_inner()
        }
    }
}
