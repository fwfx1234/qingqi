#![allow(dead_code)]

mod app;
mod core;
mod features;
mod platform;

use anyhow::Result;

fn main() -> Result<()> {
    app::runtime::run()
}
