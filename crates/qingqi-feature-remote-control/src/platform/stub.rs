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
