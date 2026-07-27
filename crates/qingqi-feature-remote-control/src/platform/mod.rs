// Platform abstraction layer
// On Windows, delegates to the Win32 implementations.
// On other platforms, uses stubs that return "not supported" errors.

use crate::protocol::responses::{AppInfo, ForegroundResponse, ProcessInfo};

pub struct ProcessActions;

impl ProcessActions {
    pub fn suspend(pid: u32) -> anyhow::Result<()> {
        platform_impl::suspend(pid)
    }

    pub fn resume(pid: u32) -> anyhow::Result<()> {
        platform_impl::resume(pid)
    }

    pub fn kill(pid: u32) -> anyhow::Result<()> {
        platform_impl::kill(pid)
    }
}

pub struct SystemController;

impl SystemController {
    pub fn shutdown(force: bool, delay_secs: u64) -> anyhow::Result<()> {
        platform_impl::shutdown(force, delay_secs)
    }

    pub fn sleep(hibernate: bool) -> anyhow::Result<()> {
        platform_impl::sleep(hibernate)
    }

    pub fn restart(force: bool) -> anyhow::Result<()> {
        platform_impl::restart(force)
    }

    pub fn logoff() -> anyhow::Result<()> {
        platform_impl::logoff()
    }

    pub fn lock() -> anyhow::Result<()> {
        platform_impl::lock()
    }
}

pub fn get_foreground_window_info() -> anyhow::Result<ForegroundResponse> {
    platform_impl::get_foreground_window_info()
}

pub fn launch_app(path: &str, args: &[String]) -> anyhow::Result<()> {
    platform_impl::launch_app(path, args)
}

pub fn search_installed_apps(query: &str) -> Vec<AppInfo> {
    platform_impl::search_installed_apps(query)
}

pub fn get_process_info(process: &sysinfo::Process) -> ProcessInfo {
    platform_impl::get_process_info(process)
}

pub fn set_priority(pid: u32, priority: &str) -> anyhow::Result<()> {
    platform_impl::set_priority(pid, priority)
}

// === Platform-specific implementations ===

#[cfg(target_os = "windows")]
mod platform_impl {
    pub use super::windows::*;

    use crate::protocol::responses::ProcessInfo;
    use sysinfo::Process;

    pub fn get_process_info(process: &Process) -> ProcessInfo {
        ProcessInfo {
            pid: process.pid().as_u32(),
            name: process.name().to_string_lossy().into_owned(),
            memory_bytes: process.memory(),
            cpu_usage: process.cpu_usage(),
            path: process.exe().map(|p| p.to_string_lossy().into_owned()),
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform_impl {
    pub use super::stub::*;

    use crate::protocol::responses::ProcessInfo;
    use sysinfo::Process;

    pub fn get_process_info(process: &Process) -> ProcessInfo {
        ProcessInfo {
            pid: process.pid().as_u32(),
            name: process.name().to_string_lossy().into_owned(),
            memory_bytes: process.memory(),
            cpu_usage: process.cpu_usage(),
            path: process.exe().map(|p| p.to_string_lossy().into_owned()),
        }
    }
}

#[cfg(target_os = "windows")]
mod windows;

#[cfg(not(target_os = "windows"))]
mod stub;
