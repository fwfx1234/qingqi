mod features;

use anyhow::Result;

fn main() -> Result<()> {
    let mut host = qingqi_app::app::runtime::bootstrap()?;
    let clipboard = features::registry::register_builtin_plugins(&mut host)?;
    qingqi_app::app::runtime::run(host, clipboard)
}
