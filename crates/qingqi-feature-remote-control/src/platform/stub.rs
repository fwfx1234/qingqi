// Stub implementations for non-Windows platforms

use crate::protocol::responses::{AppInfo, ForegroundResponse};

pub fn suspend(_pid: u32) -> anyhow::Result<()> {
    anyhow::bail!("Process suspend is only supported on Windows")
}

pub fn resume(_pid: u32) -> anyhow::Result<()> {
    anyhow::bail!("Process resume is only supported on Windows")
}

pub fn kill(_pid: u32) -> anyhow::Result<()> {
    anyhow::bail!("Process kill is only supported on Windows")
}

pub fn shutdown(_force: bool, _delay_secs: u64) -> anyhow::Result<()> {
    anyhow::bail!("System shutdown is only supported on Windows")
}

pub fn sleep(_hibernate: bool) -> anyhow::Result<()> {
    anyhow::bail!("System sleep is only supported on Windows")
}

pub fn restart(_force: bool) -> anyhow::Result<()> {
    anyhow::bail!("System restart is only supported on Windows")
}

pub fn logoff() -> anyhow::Result<()> {
    anyhow::bail!("System logoff is only supported on Windows")
}

pub fn lock() -> anyhow::Result<()> {
    anyhow::bail!("Workstation lock is only supported on Windows")
}

pub fn get_foreground_window_info() -> anyhow::Result<ForegroundResponse> {
    anyhow::bail!("Foreground window query is only supported on Windows")
}

pub fn launch_app(_path: &str, _args: &[String]) -> anyhow::Result<()> {
    anyhow::bail!("App launch is only supported on Windows")
}

pub fn search_installed_apps(_query: &str) -> Vec<AppInfo> {
    Vec::new()
}

pub fn set_priority(_pid: u32, _priority: &str) -> anyhow::Result<()> {
    anyhow::bail!("Process priority adjustment is only supported on Windows")
}

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub hwnd: usize,
    pub title: String,
    pub pid: u32,
    pub exe_path: Option<String>,
    pub is_visible: bool,
    pub is_foreground: bool,
    pub is_fullscreen: bool,
    pub is_topmost: bool,
}

pub fn enum_windows() -> Vec<WindowInfo> {
    Vec::new()
}

pub fn focus_window(_hwnd: usize) -> anyhow::Result<()> {
    anyhow::bail!("Window management is only supported on Windows")
}

pub fn minimize_window(_hwnd: usize) -> anyhow::Result<()> {
    anyhow::bail!("Window minimize is only supported on Windows")
}

pub fn maximize_window(_hwnd: usize) -> anyhow::Result<()> {
    anyhow::bail!("Window maximize is only supported on Windows")
}

pub fn restore_window(_hwnd: usize) -> anyhow::Result<()> {
    anyhow::bail!("Window restore is only supported on Windows")
}

pub fn close_window(_hwnd: usize) -> anyhow::Result<()> {
    anyhow::bail!("Window close is only supported on Windows")
}

pub fn move_window(_hwnd: usize, _x: i32, _y: i32, _width: u32, _height: u32) -> anyhow::Result<()> {
    anyhow::bail!("Window move is only supported on Windows")
}

pub fn set_always_on_top(_hwnd: usize, _enable: bool) -> anyhow::Result<()> {
    anyhow::bail!("Window always-on-top is only supported on Windows")
}
