use std::sync::Arc;

use crate::app::app_index::AppIndexService;
use qingqi_plugin::app::AppEntry;
use qingqi_plugin::command::Command;

const APP_SEARCH_LIMIT: usize = 5_000;

pub struct AppCatalog {
    service: Arc<AppIndexService>,
}

impl AppCatalog {
    pub fn new(service: Arc<AppIndexService>) -> Self {
        Self { service }
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<Command> {
        let limit = if limit == 0 { APP_SEARCH_LIMIT } else { limit };
        let apps = self.service.search_database(query.trim(), limit);
        apps.into_iter().map(app_command).collect()
    }

    pub fn launch(&self, path: &str) -> Result<(), String> {
        self.service.record_launch(path).unwrap_or_else(
            |error| tracing::warn!(error = %error, "app launch usage record failed"),
        );
        self.service.open_app(path)
    }

    pub fn start_background(&self) {
        self.service.request_probe_scan();
    }
}

fn app_command(app: AppEntry) -> Command {
    let path = app.path.clone();
    let mut keywords = vec![
        app.name.clone(),
        app.bundle_id.clone().unwrap_or_default(),
        path.clone(),
    ];
    keywords.extend(app.aliases.clone());
    Command::app_launch(
        path,
        app.name,
        app.bundle_id.unwrap_or_else(|| app.path.clone()),
        keywords,
        app.icon_path.unwrap_or_default(),
    )
}
