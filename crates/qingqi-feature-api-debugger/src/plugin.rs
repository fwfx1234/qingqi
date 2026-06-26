use std::sync::Arc;

use gpui::{AnyElement, App, AppContext, Entity, IntoElement, Window};

use crate::{manifest, service::ApiService, view};
use qingqi_plugin::{
    command::{Command, ContextKind, ContextMatcher},
    database::DatabaseService,
    plugin::{Plugin, PluginCx, PluginId, PluginView, WindowView},
    storage::AppPaths,
};

pub struct ApiDebuggerPlugin {
    database: Arc<DatabaseService>,
    paths: AppPaths,
    service: Option<Arc<ApiService>>,
}

impl ApiDebuggerPlugin {
    pub fn new(database: Arc<DatabaseService>, paths: AppPaths) -> Self {
        Self {
            database,
            paths,
            service: None,
        }
    }

    fn service(&mut self) -> Arc<ApiService> {
        if let Some(service) = &self.service {
            return Arc::clone(service);
        }
        let service = Arc::new(ApiService::new(
            Arc::clone(&self.database),
            self.paths.clone(),
        ));
        self.service = Some(Arc::clone(&service));
        service
    }
}

impl Default for ApiDebuggerPlugin {
    fn default() -> Self {
        let paths = AppPaths::resolve().expect("failed to resolve qingqi data path");
        let database = Arc::new(DatabaseService::new(paths.clone()));
        Self::new(database, paths)
    }
}

impl Plugin for ApiDebuggerPlugin {
    fn manifest(&self) -> qingqi_plugin::plugin::Manifest {
        manifest::manifest()
    }
    fn commands(&self, _query: &str) -> Vec<Command> {
        let manifest = self.manifest();
        vec![
            Command::plugin_open(
                manifest.id.as_ref(),
                manifest.name.as_ref(),
                manifest.description.as_ref(),
                manifest.keywords.iter().map(|s| s.as_ref()),
                manifest.command_prefixes.iter().map(|s| s.as_ref()),
                manifest.icon.as_str(),
            )
            .with_recommend_matchers([
                ContextMatcher::new(ContextKind::Url, 120),
                ContextMatcher::new(ContextKind::Url, 70),
            ]),
        ]
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        let service = self.service();
        Ok(PluginView::Window(Box::new(ApiDebuggerWindow {
            service,
            view: None,
        })))
    }

    fn close_idle(&mut self) {}
}

struct ApiDebuggerWindow {
    service: Arc<ApiService>,
    view: Option<Entity<view::ApiDebuggerView>>,
}

impl WindowView for ApiDebuggerWindow {
    fn plugin_id(&self) -> PluginId {
        manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "API 调试器".into()
    }

    fn render(&mut self, window: &mut Window, cx: &mut App) -> AnyElement {
        let view = self
            .view
            .get_or_insert_with(|| {
                let service = Arc::clone(&self.service);
                cx.new(|cx| view::ApiDebuggerView::new(service, window, cx))
            })
            .clone();
        view.into_any_element()
    }
}
