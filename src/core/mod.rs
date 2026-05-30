pub mod command;
pub mod command_usage;
pub mod database;
pub mod dict_store;
pub mod events;
pub mod icon;
pub mod job;
pub mod keymap;
pub mod page;
pub mod plugin;
pub mod plugin_spec;
pub mod registry;
pub mod shortcut;
pub mod storage;
pub mod view_model;

/// Lock a std::sync::Mutex with poison recovery.
///
/// If the mutex is poisoned (a prior holder panicked), the error is logged
/// and the inner guard is recovered via `PoisonError::into_inner()` so the
/// application can continue.  Panic-free is a hard design rule — runtime
/// errors must not crash the process.
pub fn lock_or_recover<'a, T>(mutex: &'a std::sync::Mutex<T>, name: &str) -> std::sync::MutexGuard<'a, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poison) => {
            tracing::error!(target = name, "mutex poisoned, attempting recovery");
            poison.into_inner()
        }
    }
}
