// Platform abstraction layer
// On Windows, delegates to the Win32 implementations.
// On other platforms, uses stubs that return "not supported" errors.

use crate::protocol::responses::{AppInfo, ForegroundResponse, ProcessInfo};

#[cfg(target_os = "windows")]
pub use windows::WindowInfo;

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

// === 窗口管理 ===

#[cfg(target_os = "windows")]
pub fn enum_windows() -> Vec<WindowInfo> {
    windows::enum_windows()
}

#[cfg(not(target_os = "windows"))]
pub fn enum_windows() -> Vec<WindowInfo> {
    stub::enum_windows()
}

#[cfg(target_os = "windows")]
pub fn focus_window(hwnd: usize) -> anyhow::Result<()> {
    windows::focus_window(hwnd)
}

#[cfg(not(target_os = "windows"))]
pub fn focus_window(hwnd: usize) -> anyhow::Result<()> {
    stub::focus_window(hwnd)
}

#[cfg(target_os = "windows")]
pub fn minimize_window(hwnd: usize) -> anyhow::Result<()> {
    windows::minimize_window(hwnd)
}

#[cfg(not(target_os = "windows"))]
pub fn minimize_window(hwnd: usize) -> anyhow::Result<()> {
    stub::minimize_window(hwnd)
}

#[cfg(target_os = "windows")]
pub fn maximize_window(hwnd: usize) -> anyhow::Result<()> {
    windows::maximize_window(hwnd)
}

#[cfg(not(target_os = "windows"))]
pub fn maximize_window(hwnd: usize) -> anyhow::Result<()> {
    stub::maximize_window(hwnd)
}

#[cfg(target_os = "windows")]
pub fn restore_window(hwnd: usize) -> anyhow::Result<()> {
    windows::restore_window(hwnd)
}

#[cfg(not(target_os = "windows"))]
pub fn restore_window(hwnd: usize) -> anyhow::Result<()> {
    stub::restore_window(hwnd)
}

#[cfg(target_os = "windows")]
pub fn close_window(hwnd: usize) -> anyhow::Result<()> {
    windows::close_window(hwnd)
}

#[cfg(not(target_os = "windows"))]
pub fn close_window(hwnd: usize) -> anyhow::Result<()> {
    stub::close_window(hwnd)
}

#[cfg(target_os = "windows")]
pub fn move_window(hwnd: usize, x: i32, y: i32, width: u32, height: u32) -> anyhow::Result<()> {
    windows::move_window(hwnd, x, y, width, height)
}

#[cfg(not(target_os = "windows"))]
pub fn move_window(hwnd: usize, x: i32, y: i32, width: u32, height: u32) -> anyhow::Result<()> {
    stub::move_window(hwnd, x, y, width, height)
}

#[cfg(target_os = "windows")]
pub fn set_always_on_top(hwnd: usize, enable: bool) -> anyhow::Result<()> {
    windows::set_always_on_top(hwnd, enable)
}

#[cfg(not(target_os = "windows"))]
pub fn set_always_on_top(hwnd: usize, enable: bool) -> anyhow::Result<()> {
    stub::set_always_on_top(hwnd, enable)
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
