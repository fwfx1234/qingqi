#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod features;

use anyhow::Result;

fn main() -> Result<()> {
    if let Some(config) = qingqi_app::app::dock_agent::config_from_args(std::env::args_os())? {
        return qingqi_app::app::dock_agent::run(config);
    }
    let mut host = qingqi_app::app::runtime::bootstrap()?;
    features::registry::register_builtin_plugins(&mut host)?;
    qingqi_app::app::runtime::run(host)
}
