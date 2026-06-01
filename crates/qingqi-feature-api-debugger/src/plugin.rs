use std::{sync::Arc, time::Duration};

use gpui::{AnyElement, App, AppContext, Entity, IntoElement, Task, Window};

use crate::{manifest, service::ApiService, view};
use qingqi_plugin::{
    command::{Command, ContextKind, ContextMatcher},
    database::DatabaseService,
    events::{AppEventBus, AppEventKind},
    plugin::{Plugin, PluginCx, PluginId, PluginView, WindowView},
    storage::AppPaths,
};

pub struct ApiDebuggerPlugin {
    database: Arc<DatabaseService>,
    paths: AppPaths,
    service: Option<Arc<ApiService>>,
    watcher_task: Option<Task<()>>,
}

impl ApiDebuggerPlugin {
    pub fn new(database: Arc<DatabaseService>, paths: AppPaths) -> Self {
        Self {
            database,
            paths,
            service: None,
            watcher_task: None,
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

    fn ensure_watcher(&mut self, service: Arc<ApiService>, events: AppEventBus, cx: &mut App) {
        if self.watcher_task.is_some() {
            return;
        }

        self.watcher_task = Some(cx.spawn(async move |async_cx| {
            let mut revision = service.revision();
            loop {
                async_cx
                    .background_executor()
                    .timer(Duration::from_millis(180))
                    .await;
                let next_revision = service.revision();
                if next_revision != revision {
                    revision = next_revision;
                    events.publish(manifest::PLUGIN_ID, AppEventKind::FeatureChanged);
                }
            }
        }));
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
        self.ensure_watcher(Arc::clone(&service), cx.events.clone(), cx.app);
        let view = cx.app.new(|cx| view::ApiDebuggerView::new(service, cx));
        Ok(PluginView::Window(Box::new(ApiDebuggerWindow { view })))
    }

    fn close_idle(&mut self) {
        // Stop the revision watcher when the window closes; the service is kept
        // so a later reopen reuses it and restarts the watcher.
        self.watcher_task = None;
    }
}

struct ApiDebuggerWindow {
    view: Entity<view::ApiDebuggerView>,
}

impl WindowView for ApiDebuggerWindow {
    fn plugin_id(&self) -> PluginId {
        manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "API 调试器".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> AnyElement {
        self.view.clone().into_any_element()
    }
}
