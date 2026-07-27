//! Shared Tokio runtime for the entire application.
//!
//! This module provides a globally shared Tokio runtime that can be used
//! by any plugin that needs to spawn async tasks (TCP servers, SSH connections, etc.).
//! The runtime runs in a dedicated background thread and persists for the
//! entire application lifetime.

use std::sync::OnceLock;

use tokio::runtime::Handle;

struct RuntimeHandle {
    _thread: std::thread::JoinHandle<()>,
    handle: Handle,
}

static RUNTIME: OnceLock<RuntimeHandle> = OnceLock::new();

/// Initialize the shared Tokio runtime. Safe to call multiple times;
/// subsequent calls return the existing runtime.
pub fn init() -> &'static Handle {
    &RUNTIME
        .get_or_init(|| {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .thread_name("qingqi-tokio")
                .build()
                .expect("Failed to create shared Tokio runtime");
            let handle = rt.handle().clone();
            let thread = std::thread::spawn(move || {
                rt.block_on(std::future::pending::<()>());
            });
            RuntimeHandle {
                _thread: thread,
                handle,
            }
        })
        .handle
}

/// Get the shared Tokio runtime handle. Initializes the runtime if not already done.
pub fn handle() -> &'static Handle {
    init()
}

/// Spawn a future on the shared runtime. Convenience function.
pub fn spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    handle().spawn(future)
}
