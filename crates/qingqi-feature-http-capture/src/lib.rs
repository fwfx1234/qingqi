pub mod breakpoint;
pub mod certificate;
pub mod composer;
pub mod engine;
pub mod har;
pub mod manifest;
pub mod mock_engine;
pub mod mock_enhanced;
pub mod mock_model;
pub mod mock_store;
pub mod model;
pub mod performance;
pub mod plugin;
pub mod proxy_handler;
pub mod rewrite;
pub mod request_diff;
pub mod session_tree;
pub mod store;
pub mod text_tools;
pub mod throttle;
pub mod view;
pub mod video_sniff;

use std::sync::Arc;

use qingqi_plugin::{
    database::{DatabaseService, DatabaseSpec},
    events::AppEventBus,
    plugin::Plugin,
    storage::AppPaths,
};

pub fn databases() -> Vec<DatabaseSpec> {
    vec![
        DatabaseSpec::feature("http-capture", "capture", "capture.db"),
        DatabaseSpec::feature("http-capture", "mock", "mock.db"),
    ]
}

pub fn build(
    database: Arc<DatabaseService>,
    paths: AppPaths,
    events: AppEventBus,
) -> anyhow::Result<Box<dyn Plugin>> {
    Ok(Box::new(plugin::HttpCapturePlugin::new(
        database, paths, events,
    )?))
}
