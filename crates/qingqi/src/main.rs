#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod features;

use anyhow::Result;

fn main() -> Result<()> {
    let mut host = qingqi_app::app::runtime::bootstrap()?;
    features::registry::register_builtin_plugins(&mut host)?;
    qingqi_app::app::runtime::run(host)
}
